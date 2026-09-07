#![forbid(unsafe_code)]

use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::{HostAdapter, ImpulciferService};
use impulcifer_types::audio::*;
use impulcifer_types::job::JobKind;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct TempRoot(PathBuf);
impl TempRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "impulcifer-p04-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        // Seed a deterministic language, but retain the actual first-run flag.
        std::fs::write(
            path.join("settings.json"),
            r#"{"language":"en","unrelated":{"keep":42}}"#,
        )
        .unwrap();
        Self(path)
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct HostState {
    calls: Vec<(String, String)>,
    fail: bool,
    panic: bool,
    cancelled: bool,
}
struct FakeHost(Arc<Mutex<HostState>>);
impl FakeHost {
    fn call(&self, method: &str, argument: &str) -> bool {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        state.calls.push((method.into(), argument.into()));
        assert!(!state.panic, "host panic");
        state.fail
    }
}
impl HostAdapter for FakeHost {
    fn select_file(&self, kind: &str) -> Option<String> {
        self.call("select_file", kind);
        if self.0.lock().unwrap().cancelled {
            None
        } else {
            Some("selected.wav".into())
        }
    }
    fn select_directory(&self) -> Option<String> {
        self.call("select_directory", "");
        if self.0.lock().unwrap().cancelled {
            None
        } else {
            Some("selected".into())
        }
    }
    fn open_path(&self, path: &str) -> Result<(), String> {
        if self.call("open_path", path) {
            Err("host failed".into())
        } else {
            Ok(())
        }
    }
    fn open_url(&self, url: &str) -> Result<(), String> {
        if self.call("open_url", url) {
            Err("host failed".into())
        } else {
            Ok(())
        }
    }
    fn apply_title_theme(&self, theme: &str) {
        self.call("theme", theme);
    }
}
struct FakeBackend {
    fail: bool,
    panic: bool,
    empty: bool,
}
impl AudioBackend for FakeBackend {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        assert!(!self.panic, "backend panic");
        if self.fail {
            return Err(AudioError::Backend("enumeration failed".into()));
        }
        if self.empty {
            return Ok(vec![]);
        }
        Ok(vec![
            Endpoint {
                id: "mic-id".into(),
                name: "Mic".into(),
                host_api: "WASAPI".into(),
                max_input_channels: 2,
                max_output_channels: 0,
                default_samplerate: 48000.0,
                is_default_input: true,
                is_default_output: false,
            },
            Endpoint {
                id: "speaker-id".into(),
                name: "Speaker".into(),
                host_api: "WASAPI".into(),
                max_input_channels: 0,
                max_output_channels: 8,
                default_samplerate: 48000.0,
                is_default_input: false,
                is_default_output: true,
            },
            Endpoint {
                id: "other-id".into(),
                name: "Other".into(),
                host_api: "Other API".into(),
                max_input_channels: 1,
                max_output_channels: 1,
                default_samplerate: 44100.0,
                is_default_input: false,
                is_default_output: false,
            },
        ])
    }
    fn probe(
        &self,
        _: &Endpoint,
        _: Direction,
        _: StreamSpec,
        _: ShareMode,
    ) -> Result<ProbeResult, AudioError> {
        panic!("unexpected probe")
    }
    fn open_output(
        &self,
        _: &Endpoint,
        _: StreamSpec,
        _: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError> {
        panic!("unexpected output")
    }
    fn open_input(
        &self,
        _: &Endpoint,
        _: StreamSpec,
        _: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        panic!("unexpected input")
    }
}
struct Fixture {
    service: ImpulciferService,
    root: TempRoot,
    host: Arc<Mutex<HostState>>,
    jobs: JobRegistry,
}
impl Fixture {
    fn new() -> Self {
        Self::backend(FakeBackend {
            fail: false,
            panic: false,
            empty: false,
        })
    }
    fn backend(backend: FakeBackend) -> Self {
        let root = TempRoot::new();
        let host = Arc::new(Mutex::new(HostState::default()));
        let jobs = JobRegistry::new();
        let service = ImpulciferService::with_dependencies(
            Box::new(FakeHost(host.clone())),
            root.0.join("settings.json"),
            Box::new(backend),
            jobs.clone(),
            root.0.clone(),
        );
        Self {
            service,
            root,
            host,
            jobs,
        }
    }
    fn call(&self, method: &str, args: Vec<Value>) -> Value {
        self.service.call(method, args)
    }
    fn saved(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.root.0.join("settings.json")).unwrap()).unwrap()
    }
}
fn keys(value: &Value, expected: &[&str]) {
    let mut actual: Vec<_> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut expected = expected.to_vec();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
fn data(response: Value) -> Value {
    keys(&response, &["ok", "data"]);
    assert_eq!(response["ok"], true);
    response["data"].clone()
}
fn failure(response: Value, code: &str) -> Value {
    keys(&response, &["ok", "error"]);
    assert_eq!(response["ok"], false);
    keys(
        &response["error"],
        &["code", "message", "details", "retryable"],
    );
    assert_eq!(response["error"]["code"], code);
    assert!(response["error"]["message"].is_string());
    assert!(response["error"]["details"].is_object());
    assert!(response["error"]["retryable"].is_boolean());
    response["error"].clone()
}
fn ui(value: &Value) {
    keys(
        value,
        &[
            "language",
            "theme",
            "skin",
            "frontend",
            "first_run",
            "languages",
            "strings",
        ],
    );
    assert_eq!(value["languages"].as_array().unwrap().len(), 9);
    for language in value["languages"].as_array().unwrap() {
        keys(language, &["code", "name"]);
    }
    assert!(value["strings"].as_object().unwrap().len() > 264);
}
fn snapshot(value: &Value) {
    keys(
        value,
        &["job_id", "kind", "status", "cancellable", "result", "error"],
    );
}
fn start_blocked(fixture: &Fixture, cancellable: bool) -> (String, mpsc::Sender<()>) {
    let (send, receive) = mpsc::channel();
    let job = fixture
        .jobs
        .start(JobKind::Recording, cancellable, move |context| {
            receive.recv_timeout(Duration::from_secs(10)).unwrap();
            context.check_cancelled()?;
            Ok(json!({"record_path":"out.wav"}))
        })
        .unwrap();
    (job.job_id, send)
}
fn finish(fixture: &Fixture, id: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !fixture.jobs.poll(id, 0).unwrap().job.status.is_terminal() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn ipc_bootstrap_shape() {
    let f = Fixture::new();
    let boot = data(f.call("bootstrap", vec![]));
    keys(
        &boot,
        &[
            "version",
            "platform",
            "brir_defaults",
            "sweep",
            "capabilities",
            "active_job",
            "ui",
            "webview_backend",
        ],
    );
    assert_eq!(boot["version"], env!("CARGO_PKG_VERSION"));
    assert!(boot["active_job"].is_null());
    assert_eq!(
        boot["capabilities"],
        json!({"recording":true,"brir":true,"output_recovery":true,"recording_cancel":false,"brir_cancel":true,"output_recovery_cancel":false})
    );
    let mut defaults =
        serde_json::to_value(impulcifer_types::config::ProcessingConfig::default()).unwrap();
    defaults.as_object_mut().unwrap().remove("dir_path");
    assert_eq!(boot["brir_defaults"], defaults);
    assert_eq!(
        boot["sweep"],
        json!({"layouts":["mono","stereo","5.1","7.1","7.1.4","7.1.6"],"default_fs":48000,"default_duration":5.0,"speaker_names":impulcifer_types::constants::SPEAKER_NAMES})
    );
    ui(&boot["ui"]);
    let (id, send) = start_blocked(&f, false);
    let boot = data(f.call("bootstrap", vec![]));
    snapshot(&boot["active_job"]);
    assert_eq!(boot["active_job"]["job_id"], id);
    send.send(()).unwrap();
    finish(&f, &id);
}
#[test]
fn ipc_get_ui_settings_shape() {
    let f = Fixture::new();
    let out = data(f.call("get_ui_settings", vec![]));
    ui(&out);
    assert_eq!(out["language"], "en");
    assert_eq!(out["theme"], "dark");
    assert_eq!(out["skin"], "stable");
    assert_eq!(out["frontend"], "webview");
    assert_eq!(out["first_run"], true);
    assert_eq!(f.saved()["unrelated"], json!({"keep":42}));
}
#[test]
fn ipc_set_language_shape() {
    let f = Fixture::new();
    for code in ["en", "ko", "fr", "de", "es", "ja", "zh_CN", "zh_TW", "ru"] {
        let out = data(f.call("set_language", vec![json!(code)]));
        ui(&out);
        assert_eq!(out["language"], code);
        assert_eq!(out["first_run"], false);
        let english: Value =
            serde_json::from_str(include_str!("../../../i18n/locales/en.json")).unwrap();
        let translated: Value = serde_json::from_slice(
            &std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../../i18n/locales/{code}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        let mut expected = english.as_object().unwrap().clone();
        expected.extend(translated.as_object().unwrap().clone());
        // The catalogue equals en + <language>, plus the 3.x-only overlay keys
        // the service adds itself (settings.rs EXTRA_STRINGS).
        let strings = out["strings"].as_object().unwrap();
        for (key, value) in &expected {
            assert_eq!(strings.get(key), Some(value), "{key}");
        }
        let extra: Vec<&String> = strings
            .keys()
            .filter(|key| !expected.contains_key(*key))
            .collect();
        assert_eq!(extra, vec!["cli_plots_not_available_yet"]);
    }
    assert_eq!(f.saved()["language_selected"], true);
    let error = failure(
        f.call("set_language", vec![json!("ko-KR")]),
        "INVALID_REQUEST",
    );
    assert_eq!(error["details"], json!({"language":"ko-KR"}));
}
#[test]
fn ipc_set_theme_shape() {
    let f = Fixture::new();
    for theme in ["dark", "light", "system"] {
        assert_eq!(
            data(f.call("set_theme", vec![json!(theme)])),
            json!({"theme":theme})
        );
    }
    assert_eq!(
        f.host.lock().unwrap().calls.last().unwrap(),
        &("theme".into(), "system".into())
    );
    assert_eq!(f.saved()["theme"], "system");
    assert_eq!(
        failure(f.call("set_theme", vec![json!("sepia")]), "INVALID_REQUEST")["details"],
        json!({"theme":"sepia"})
    );
}
#[test]
fn ipc_set_skin_shape() {
    let f = Fixture::new();
    for skin in ["stable", "studio"] {
        assert_eq!(
            data(f.call("set_skin", vec![json!(skin)])),
            json!({"skin":skin})
        );
    }
    assert_eq!(f.saved()["skin"], "studio");
    failure(f.call("set_skin", vec![json!("other")]), "INVALID_REQUEST");
}
#[test]
fn ipc_set_frontend_shape() {
    let f = Fixture::new();
    for frontend in ["webview", "ctk"] {
        assert_eq!(
            data(f.call("set_frontend", vec![json!(frontend)])),
            json!({"frontend":frontend})
        );
    }
    assert_eq!(f.saved()["frontend"], "ctk");
    failure(
        f.call("set_frontend", vec![json!("tauri")]),
        "INVALID_REQUEST",
    );
}
#[test]
fn ipc_get_system_info_shape() {
    let f = Fixture::new();
    let info = data(f.call("get_system_info", vec![]));
    keys(
        &info,
        &[
            "version",
            "install_kind",
            "python_version",
            "os",
            "cpu_count",
            "gil_enabled",
            "optimal_workers",
        ],
    );
    assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(info["install_kind"], "dev");
    assert!(
        info["python_version"]
            .as_str()
            .unwrap()
            .starts_with("rustc ")
    );
    assert!(info["gil_enabled"].is_null());
    assert!(!info["os"].as_str().unwrap().is_empty());
    assert_eq!(info["cpu_count"], info["optimal_workers"]);
}
#[test]
fn ipc_list_audio_devices_shape() {
    let f = Fixture::new();
    let all = data(f.call("list_audio_devices", vec![]));
    assert_eq!(
        all,
        json!({"host_apis":["WASAPI","Other API"],"devices":[
        {"index":0,"name":"Mic","host_api":"WASAPI","max_input_channels":2,"max_output_channels":0},
        {"index":1,"name":"Speaker","host_api":"WASAPI","max_input_channels":0,"max_output_channels":8},
        {"index":2,"name":"Other","host_api":"Other API","max_input_channels":1,"max_output_channels":1}
    ],"default_input_index":0,"default_output_index":1})
    );
    assert_eq!(data(f.call("list_audio_devices", vec![Value::Null])), all);
    assert_eq!(data(f.call("list_audio_devices", vec![json!("")])), all);
    let filtered = data(f.call("list_audio_devices", vec![json!("Other API")]));
    assert_eq!(filtered["devices"], json!([all["devices"][2]]));
    assert_eq!(filtered["default_input_index"], 0);
    assert_eq!(
        failure(
            f.call("list_audio_devices", vec![json!("absent")]),
            "INVALID_REQUEST"
        )["details"],
        json!({"host_api":"absent"})
    );
}
#[test]
fn ipc_poll_job_shape() {
    let f = Fixture::new();
    let (id, send) = start_blocked(&f, true);
    let out = data(f.call("poll_job", vec![json!(id)]));
    keys(&out, &["job", "events", "next_seq"]);
    snapshot(&out["job"]);
    assert_eq!(out["next_seq"], 1);
    let event = &out["events"][0];
    keys(event, &["seq", "timestamp_ms", "type", "payload"]);
    assert_eq!(event["seq"], 1);
    assert_eq!(event["type"], "status");
    assert_eq!(event["payload"], json!({"status":"running"}));
    assert!(event["timestamp_ms"].as_u64().unwrap() > 0);
    assert_eq!(
        data(f.call("poll_job", vec![json!(id), json!(1)]))["events"],
        json!([])
    );
    send.send(()).unwrap();
    finish(&f, &id);
    assert_eq!(
        data(f.call("poll_job", vec![json!(id)]))["job"]["result"],
        json!({"record_path":"out.wav"})
    );
    assert_eq!(
        failure(f.call("poll_job", vec![json!("missing")]), "JOB_NOT_FOUND")["details"],
        json!({"job_id":"missing"})
    );
}
#[test]
fn ipc_cancel_job_shape() {
    let f = Fixture::new();
    let (id, send) = start_blocked(&f, true);
    let out = data(f.call("cancel_job", vec![json!(id)]));
    keys(&out, &["job"]);
    snapshot(&out["job"]);
    assert_eq!(out["job"]["status"], "cancel_requested");
    send.send(()).unwrap();
    finish(&f, &id);
    assert_eq!(
        data(f.call("cancel_job", vec![json!(id)]))["job"]["status"],
        "cancelled"
    );
    let (id, send) = start_blocked(&f, false);
    let error = failure(f.call("cancel_job", vec![json!(id)]), "JOB_NOT_CANCELLABLE");
    assert_eq!(
        error["message"],
        "recording jobs cannot be cancelled safely."
    );
    assert_eq!(error["details"], json!({"job_id":id}));
    send.send(()).unwrap();
    finish(&f, &id);
    failure(
        f.call("cancel_job", vec![json!("missing")]),
        "JOB_NOT_FOUND",
    );
}
#[test]
fn ipc_resolve_recording_paths_shape() {
    let f = Fixture::new();
    let out = data(f.call(
        "resolve_recording_paths",
        vec![json!("out"), json!("sweep-seg-FL,FR-stereo-test.wav")],
    ));
    assert_eq!(
        out,
        json!({"record_path":Path::new("out").join("FL,FR.wav").to_string_lossy()})
    );
}
#[test]
fn resolve_recording_paths_matches_python_examples() {
    let f = Fixture::new();
    // All six cases come from test_application_service.py:673-681,992-1000.
    let cases = [
        (
            vec![json!("out"), json!("sweep-seg-FL,FR-stereo-test.wav")],
            Some("FL,FR.wav"),
        ),
        (
            vec![json!("out"), Value::Null, json!("headphones")],
            Some("headphones.wav"),
        ),
        (vec![json!("")], None),
        (vec![json!("out")], None),
        (
            vec![
                json!("out"),
                Value::Null,
                json!("speakers"),
                json!({"mode":"default","speakers":"BL,BR"}),
            ],
            Some("BL,BR.wav"),
        ),
        (
            vec![
                json!("out"),
                Value::Null,
                json!("speakers"),
                json!({"mode":"custom","speakers":["TFL"],"tracks":"7.1"}),
            ],
            None,
        ),
    ];
    for (args, expected) in cases {
        let response = f.call("resolve_recording_paths", args);
        if let Some(file) = expected {
            assert_eq!(
                data(response),
                json!({"record_path":Path::new("out").join(file).to_string_lossy()})
            );
        } else {
            failure(response, "INVALID_REQUEST");
        }
    }
}
#[test]
fn ipc_open_path_shape() {
    let f = Fixture::new();
    let expected = json!({"path":f.root.0.to_string_lossy()});
    assert_eq!(data(f.call("open_path", vec![])), expected);
    assert_eq!(data(f.call("open_path", vec![Value::Null])), expected);
    assert_eq!(
        data(f.call(
            "open_path",
            vec![json!(format!("  {}  ", f.root.0.display()))]
        )),
        expected
    );
    let file = f.root.0.join("settings.json");
    assert_eq!(
        failure(
            f.call("open_path", vec![json!(file.to_string_lossy())]),
            "FILE_NOT_FOUND"
        )["message"],
        "Folder does not exist."
    );
    failure(
        f.call("open_path", vec![json!(f.root.0.join("missing"))]),
        "FILE_NOT_FOUND",
    );
}
#[test]
fn ipc_open_url_shape() {
    let f = Fixture::new();
    for (name, url) in [
        (
            "original_repo",
            "https://github.com/jaakkopasanen/Impulcifer",
        ),
        ("fork_repo", "https://github.com/115dkk/Impulcifer-pip313"),
        (
            "report_bug",
            "https://github.com/115dkk/Impulcifer-pip313/issues/new",
        ),
        (
            "license",
            "https://github.com/115dkk/Impulcifer-pip313/blob/master/LICENSE",
        ),
    ] {
        assert_eq!(
            data(f.call("open_url", vec![json!(name)])),
            json!({"url":url})
        );
    }
    for name in [
        "https://github.com/115dkk/Impulcifer-pip313",
        "file:///etc/passwd",
        "javascript:alert(1)",
        "",
        "Fork_repo",
    ] {
        assert_eq!(
            failure(f.call("open_url", vec![json!(name)]), "INVALID_REQUEST")["message"],
            "Unknown project link."
        );
    }
    assert_eq!(f.host.lock().unwrap().calls.len(), 4);
}
#[test]
fn ipc_select_file_shape() {
    let f = Fixture::new();
    for (args, expected_kind) in [
        (vec![], "audio"),
        (vec![Value::Null], "audio"),
        (vec![json!("unknown")], "audio"),
        (vec![json!("text")], "text"),
        (vec![json!("wav")], "wav"),
    ] {
        assert_eq!(
            data(f.call("select_file", args)),
            json!({"path":"selected.wav"})
        );
        assert_eq!(
            f.host.lock().unwrap().calls.last().unwrap().1,
            expected_kind
        );
    }
    f.host.lock().unwrap().cancelled = true;
    assert_eq!(data(f.call("select_file", vec![])), json!({"path":null}));
}
#[test]
fn ipc_select_directory_shape() {
    let f = Fixture::new();
    assert_eq!(
        data(f.call("select_directory", vec![])),
        json!({"path":"selected"})
    );
    f.host.lock().unwrap().cancelled = true;
    assert_eq!(
        data(f.call("select_directory", vec![])),
        json!({"path":null})
    );
}
#[test]
fn argument_types_defaults_null_and_unknown_methods() {
    let f = Fixture::new();
    failure(f.call("does_not_exist", vec![]), "INVALID_REQUEST");
    for method in [
        "bootstrap",
        "get_ui_settings",
        "get_system_info",
        "select_directory",
    ] {
        failure(f.call(method, vec![Value::Null]), "INVALID_REQUEST");
    }
    for method in [
        "set_language",
        "set_theme",
        "set_skin",
        "set_frontend",
        "poll_job",
        "cancel_job",
        "open_url",
    ] {
        for args in [
            vec![],
            vec![Value::Null],
            vec![json!(true)],
            vec![json!({})],
            vec![json!([])],
            vec![json!(1)],
        ] {
            failure(f.call(method, args), "INVALID_REQUEST");
        }
    }
    for after in [
        Value::Null,
        json!(true),
        json!(-1),
        json!(1.0),
        json!("0"),
        json!({}),
    ] {
        assert_eq!(
            failure(
                f.call("poll_job", vec![json!("id"), after]),
                "INVALID_REQUEST"
            )["message"],
            "after_seq must be a non-negative integer."
        );
    }
    for method in ["list_audio_devices", "select_file", "open_path"] {
        for value in [json!(false), json!(42), json!([]), json!({})] {
            failure(f.call(method, vec![value]), "INVALID_REQUEST");
        }
    }
    // Python _optional_string coerces the preview directory to text.
    assert_eq!(
        data(f.call("resolve_recording_paths", vec![json!(9), json!("x.wav")]))["record_path"],
        Path::new("9").join("x.wav").to_string_lossy().as_ref()
    );
    failure(
        f.call(
            "resolve_recording_paths",
            vec![json!("out"), json!("x.wav"), Value::Null],
        ),
        "INVALID_REQUEST",
    );
}
#[test]
fn invalid_sweeps_and_naming_edges() {
    let f = Fixture::new();
    for sweep in [
        json!(true),
        json!({"extra":1}),
        json!({"mode":null}),
        json!({"mode":"bad"}),
        json!({"speakers":null}),
        json!({"speakers":[]}),
        json!({"speakers":"fl,FL"}),
        json!({"speakers":["LFE"]}),
        json!({"tracks":null}),
        json!({"tracks":"bad"}),
        json!({"speakers":"FL,FR,FC"}),
        json!({"speakers":"WL","tracks":"7.1.6"}),
        json!({"speakers":"TFL","tracks":"7.1"}),
        json!({"mode":"custom","fs":true}),
        json!({"mode":"custom","fs":8000.5}),
        json!({"mode":"custom","fs":999}),
        json!({"mode":"custom","duration":false}),
        json!({"mode":"custom","duration":0}),
        json!({"mode":"custom","duration":61}),
    ] {
        failure(
            f.call(
                "resolve_recording_paths",
                vec![json!("out"), json!("x.wav"), json!("speakers"), sweep],
            ),
            "INVALID_REQUEST",
        );
    }
    for (play, file) in [
        ("prefix-sweep-seg-tfl,tfr-7.1.4.wav", "TFL,TFR.wav"),
        ("custom.wav", "custom.wav"),
        (".hidden", ".hidden.wav"),
        ("..hidden", "..hidden.wav"),
        ("a.b.c", "a.b.wav"),
        ("folder/", ".wav"),
        ("sweep-seg-FL,FL-stereo.wav", "FL,FL.wav"),
        ("sweep-seg-XX-stereo.wav", "sweep-seg-XX-stereo.wav"),
    ] {
        assert_eq!(
            data(f.call("resolve_recording_paths", vec![json!("out"), json!(play)])),
            json!({"record_path":Path::new("out").join(file).to_string_lossy()})
        );
    }
    for sweep in [
        json!({"mode":"custom","fs":8000.0,"duration":0.1,"speakers":" fl, fr "}),
        json!({"mode":"default","fs":false,"duration":null}),
        json!({"mode":"custom","fs":384000,"duration":60}),
    ] {
        assert_eq!(
            data(f.call(
                "resolve_recording_paths",
                vec![json!("out"), Value::Null, json!("speakers"), sweep]
            ))["record_path"],
            json!(Path::new("out").join("FL,FR.wav"))
        );
    }
    // Headphone previews ignore even invalid sweep contents; unknown string
    // recording modes take the speaker path, matching the actual helper.
    data(f.call(
        "resolve_recording_paths",
        vec![json!("out"), Value::Null, json!("headphones"), json!(false)],
    ));
    data(f.call(
        "resolve_recording_paths",
        vec![json!("out"), json!("x.wav"), json!("unvalidated-mode")],
    ));
    data(f.call(
        "resolve_recording_paths",
        vec![
            json!("out"),
            json!("x.wav"),
            json!("speakers"),
            json!({"mode":"file","fs":false}),
        ],
    ));
}
#[test]
fn settings_persist_preserve_other_keys_and_reload_python_preferences() {
    let f = Fixture::new();
    std::fs::write(f.root.0.join("settings.json"), r#"{"language":"zh-tw","theme":null,"skin":"studio","frontend":"ctk","language_selected":true,"other":17}"#).unwrap();
    let ui = data(f.call("get_ui_settings", vec![]));
    assert_eq!(ui["language"], "zh_TW");
    assert_eq!(ui["theme"], Value::Null);
    assert_eq!(ui["first_run"], false);
    let mut external = f.saved();
    external["external"] = json!("preserved");
    std::fs::write(
        f.root.0.join("settings.json"),
        serde_json::to_vec(&external).unwrap(),
    )
    .unwrap();
    data(f.call("set_theme", vec![json!("light")]));
    assert_eq!(f.saved()["external"], "preserved");
    assert_eq!(f.saved()["other"], 17);
    let reloaded = ImpulciferService::with_dependencies(
        Box::new(impulcifer_service::NoopHost),
        f.root.0.join("settings.json"),
        Box::new(FakeBackend {
            fail: false,
            panic: false,
            empty: true,
        }),
        JobRegistry::new(),
        f.root.0.clone(),
    );
    assert_eq!(
        data(reloaded.call("get_ui_settings", vec![]))["theme"],
        "light"
    );
}
#[test]
fn missing_or_invalid_settings_and_write_failures_keep_python_first_run_semantics() {
    for contents in [
        None,
        Some("not json"),
        Some("{}"),
        Some(r#"{"language":"invalid"}"#),
    ] {
        let f = Fixture::new();
        if let Some(contents) = contents {
            std::fs::write(f.root.0.join("settings.json"), contents).unwrap();
        } else {
            std::fs::remove_file(f.root.0.join("settings.json")).unwrap();
        }
        let out = data(f.call("get_ui_settings", vec![]));
        assert_eq!(out["first_run"], true);
        assert!(f.saved()["language"].is_string());
        data(f.call("set_language", vec![json!("en")]));
        assert_eq!(f.saved()["language_selected"], true);
    }
    let f = Fixture::new();
    let impossible = f.root.0.join("settings.json/child.json");
    let service = ImpulciferService::with_dependencies(
        Box::new(impulcifer_service::NoopHost),
        impossible,
        Box::new(FakeBackend {
            fail: false,
            panic: false,
            empty: true,
        }),
        JobRegistry::new(),
        f.root.0.clone(),
    );
    assert_eq!(
        data(service.call("set_theme", vec![json!("light")])),
        json!({"theme":"light"})
    );
    assert_eq!(
        data(service.call("get_ui_settings", vec![]))["theme"],
        "light"
    );
}
#[test]
fn host_failures_and_panics_do_not_escape_boundary() {
    let f = Fixture::new();
    f.host.lock().unwrap().fail = true;
    for (method, args) in [
        ("open_url", vec![json!("fork_repo")]),
        ("open_path", vec![]),
    ] {
        assert_eq!(
            failure(f.call(method, args), "INTERNAL_ERROR")["message"],
            "host failed"
        );
    }
    f.host.lock().unwrap().panic = true;
    for (method, args) in [
        ("open_url", vec![json!("fork_repo")]),
        ("open_path", vec![]),
        ("select_file", vec![]),
        ("select_directory", vec![]),
        ("set_theme", vec![json!("dark")]),
    ] {
        assert_eq!(
            failure(f.call(method, args), "INTERNAL_ERROR")["message"],
            "host panic"
        );
    }
    // A poisoned host mutex must not poison service settings or later calls.
    assert_eq!(data(f.call("get_ui_settings", vec![]))["theme"], "dark");
}
#[test]
fn backend_errors_panics_and_empty_enumeration() {
    let f = Fixture::backend(FakeBackend {
        fail: true,
        panic: false,
        empty: false,
    });
    let error = failure(f.call("list_audio_devices", vec![]), "DEVICE_ERROR");
    assert_eq!(error["retryable"], true);
    let f = Fixture::backend(FakeBackend {
        fail: false,
        panic: true,
        empty: false,
    });
    assert_eq!(
        failure(f.call("list_audio_devices", vec![]), "INTERNAL_ERROR")["message"],
        "backend panic"
    );
    let f = Fixture::backend(FakeBackend {
        fail: false,
        panic: false,
        empty: true,
    });
    assert_eq!(
        data(f.call("list_audio_devices", vec![])),
        json!({"host_apis":[],"devices":[],"default_input_index":-1,"default_output_index":-1})
    );
}
#[test]
fn deferred_methods_keep_not_implemented_envelopes() {
    let f = Fixture::new();
    for method in [
        "start_output_recovery",
        "check_for_updates",
        "start_update",
        "apply_pending_update",
    ] {
        assert_eq!(
            failure(f.call(method, vec![]), "INTERNAL_ERROR")["message"],
            format!("{method} not implemented")
        );
    }
}
