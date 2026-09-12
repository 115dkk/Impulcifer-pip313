#![forbid(unsafe_code)]

use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::{
    HostAdapter, ImpulciferService,
    update::{
        self, UpdateOptions,
        install_kind::{self, InstallKind},
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(windows)]
#[path = "support/local_feed.rs"]
mod local_feed;

fn golden() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/p20_updater.json"
    ))
    .unwrap()
}

#[derive(Default)]
struct HostState {
    calls: Vec<(String, String)>,
    fail_download: bool,
    fail_apply: bool,
    fail_open: bool,
}
struct Host(Arc<Mutex<HostState>>);
impl HostAdapter for Host {
    fn select_file(&self, _: &str) -> Option<String> {
        None
    }
    fn select_directory(&self) -> Option<String> {
        None
    }
    fn open_url(&self, _: &str) -> Result<(), String> {
        Ok(())
    }
    fn apply_title_theme(&self, _: &str) {}
    fn open_path(&self, path: &str) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        assert!(Path::new(path).is_file());
        state.calls.push(("open".into(), path.into()));
        if state.fail_open {
            Err("open failed".into())
        } else {
            Ok(())
        }
    }
    fn download_update(
        &self,
        version: &str,
        progress: &(dyn Fn(f64, &str) + Sync),
    ) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        state.calls.push(("download".into(), version.into()));
        if state.fail_download {
            return Err("download failed".into());
        }
        progress(-1.0, "Downloading: 0%");
        progress(2.0, "Downloading: 100%");
        Ok(())
    }
    fn apply_staged_update(&self) -> Result<(), String> {
        let mut state = self.0.lock().unwrap();
        state.calls.push(("apply".into(), String::new()));
        if state.fail_apply {
            Err("apply failed".into())
        } else {
            Ok(())
        }
    }
}

fn options(root: &Path, kind: InstallKind) -> UpdateOptions {
    // Explicitly override every production endpoint and process probe.
    UpdateOptions {
        install_kind: kind,
        platform: "windows".into(),
        current_version: "2.0.0".into(),
        latest_endpoint: "http://127.0.0.1:0/latest".into(),
        releases_endpoint: "http://127.0.0.1:0/releases".into(),
        releases_url: "http://127.0.0.1:0/feed".into(),
        timeout: Duration::from_secs(2),
        download_root: root.join("downloads"),
        appimage: None,
        velopack_root: None,
    }
}
fn service(root: &Path, options: UpdateOptions, host: Arc<Mutex<HostState>>) -> ImpulciferService {
    ImpulciferService::with_dependencies(
        Box::new(Host(host)),
        root.join("settings.json"),
        impulcifer_audio_io::default_backend(),
        JobRegistry::new(),
        root.to_owned(),
    )
    .with_update_options(options)
}
fn finish(service: &ImpulciferService, request: Value) -> Value {
    let started = service.call("start_update", vec![request]);
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(started["data"]["job"]["kind"], "update");
    assert_eq!(started["data"]["job"]["cancellable"], false);
    let id = started["data"]["job"]["job_id"].clone();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let polled = service.call("poll_job", vec![id.clone()]);
        if matches!(
            polled["data"]["job"]["status"].as_str(),
            Some("succeeded" | "failed")
        ) {
            return polled["data"].clone();
        }
        assert!(Instant::now() < deadline, "update job did not finish");
        std::thread::yield_now();
    }
}

/// A bounded local server whose thread is always joined, including on failure.
struct Server {
    base: String,
    requests: Arc<Mutex<Vec<String>>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(responses: Vec<(u16, Vec<u8>, Duration)>) -> Self {
        Self::new_chunked(
            responses
                .into_iter()
                .map(|(status, body, delay)| (status, vec![body], delay))
                .collect(),
        )
    }

    fn new_chunked(responses: Vec<(u16, Vec<Vec<u8>>, Duration)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let thread = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut handlers = Vec::new();
            for (status, chunks, delay) in responses {
                let stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "mock server was not called");
                            std::thread::yield_now();
                        }
                        Err(e) => panic!("{e}"),
                    }
                };
                let captured = captured.clone();
                handlers.push(std::thread::spawn(move || {
                    let mut stream = stream;
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut request = Vec::new();
                    let mut buffer = [0; 4096];
                    while !request.ends_with(b"\r\n\r\n") {
                        let count = stream.read(&mut buffer).unwrap();
                        assert!(count > 0, "client closed before request headers");
                        request.extend_from_slice(&buffer[..count]);
                    }
                    captured
                        .lock()
                        .unwrap()
                        .push(String::from_utf8(request).unwrap());
                    let body_len: usize = chunks.iter().map(Vec::len).sum();
                    let header = format!(
                        "HTTP/1.1 {status} Test\r\nContent-Length: {body_len}\r\nConnection: close\r\n\r\n"
                    );
                    if stream.write_all(header.as_bytes()).is_ok() {
                        let mut complete = true;
                        for chunk in chunks {
                            std::thread::sleep(delay);
                            if stream.write_all(&chunk).is_err() {
                                complete = false;
                                break;
                            }
                        }
                        if complete {
                            // Content-Length terminates the response. Let the client
                            // close first; do not half-close under its buffered reader.
                            let _ = stream.read_to_end(&mut Vec::new());
                        }
                    }
                }));
            }
            for handler in handlers {
                handler.join().unwrap();
            }
        });
        Self {
            base,
            requests,
            thread: Some(thread),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let result = self.thread.take().unwrap().join();
        if !std::thread::panicking() {
            result.unwrap();
        }
    }
}

#[test]
fn golden_asset_selection_and_version_compare_match_python() {
    let gold = golden();
    let deliberate_3x_divergences = [("3.0.0-alpha.0", "v3.0.0-rc1", true)];
    assert_eq!(gold["releases"].as_array().unwrap().len(), 30);
    for case in gold["releases"].as_array().unwrap() {
        let current = case["current"].as_str().unwrap();
        let latest = case["release"]["tag_name"].as_str().unwrap();
        assert_eq!(
            update::check::normalize_version(current),
            case["normalized_current"]
        );
        assert_eq!(
            update::check::normalize_version(latest),
            case["normalized_latest"]
        );
        let expected = deliberate_3x_divergences
            .iter()
            .find(|(divergent_current, divergent_latest, _)| {
                current == *divergent_current && latest == *divergent_latest
            })
            .map(|(_, _, expected)| *expected)
            .unwrap_or_else(|| case["newer"].as_bool().unwrap());
        assert_eq!(
            update::check::is_newer_version(current, latest),
            expected,
            "{case}"
        );
        for platform in ["windows", "darwin", "linux", "freebsd"] {
            assert_eq!(
                update::check::download_url(&case["release"], platform),
                case["selected"][platform]
            );
        }
    }
}

#[test]
fn prerelease_detection_follows_the_pep440_pre_segment() {
    for (version, expected) in [
        ("3.0.0-alpha.0", true),
        ("v3.0.0-rc1", true),
        ("3.0.0a1", true),
        ("3.0.0", false),
        ("2.13.3", false),
        ("2.3.1.post1", false),
        ("3.0.0.dev0", false),
        ("garbage", false),
    ] {
        assert_eq!(update::check::is_prerelease(version), expected, "{version}");
    }
}

#[cfg(windows)]
#[test]
fn velopack_source_follows_the_feed_url_and_the_release_channel() {
    use update::velopack::{SourceKind, source_kind};
    let repo = "https://github.com/115dkk/Impulcifer-pip313".to_owned();
    assert_eq!(
        source_kind(update::RELEASES_URL, "3.0.0-alpha.0"),
        SourceKind::Github {
            repo: repo.clone(),
            prerelease: true
        }
    );
    assert_eq!(
        source_kind(update::RELEASES_URL, "3.0.0"),
        SourceKind::Github {
            repo,
            prerelease: false
        }
    );
    assert_eq!(
        source_kind("http://127.0.0.1:8000", "3.0.0-alpha.0"),
        SourceKind::Http("http://127.0.0.1:8000".into())
    );
    assert_eq!(
        source_kind("https://github.com/", "3.0.0"),
        SourceKind::Http("https://github.com/".into())
    );
}

#[test]
fn prerelease_ordering_compares_equal_normalized_bases() {
    for (current, latest, expected) in [
        ("3.0.0-alpha.0", "v3.0.0", true),
        ("3.0.0-alpha.0", "v3.0.0-rc1", true),
        ("3.0.0-rc1", "3.0.0-alpha.0", false),
        ("3.0.0", "3.0.0-rc1", false),
        ("3.0.0-rc1", "3.0.0", true),
        ("2.3.1", "vv2.4.0-20241129123456", true),
    ] {
        assert_eq!(
            update::check::is_newer_version(current, latest),
            expected,
            "{current} -> {latest}"
        );
    }
}

#[test]
fn golden_install_kind_matches_python() {
    let root = tempfile::tempdir().unwrap();
    for (index, case) in golden()["layouts"].as_array().unwrap().iter().enumerate() {
        let directory = root.path().join(index.to_string());
        std::fs::create_dir_all(directory.join("current")).unwrap();
        if case["update_exe"] == true {
            std::fs::write(directory.join("Update.exe"), b"").unwrap();
        }
        assert_eq!(
            install_kind::detect("windows", &directory.join("current/app.exe"), None).as_str(),
            case["expected"]
        );
    }
    assert_eq!(
        install_kind::detect(
            "linux",
            Path::new("/usr/bin/app"),
            Some(Path::new("/tmp/App.AppImage"))
        ),
        InstallKind::Tauri
    );
    assert_eq!(
        install_kind::detect(
            "darwin",
            Path::new("/Applications/Impulcifer.app/Contents/MacOS/Impulcifer"),
            None
        ),
        InstallKind::Tauri
    );
    for executable in [
        "/x/Contents/MacOS/app",
        "/x.app/Contents/Other/app",
        "/usr/bin/app",
    ] {
        assert_eq!(
            install_kind::detect("darwin", Path::new(executable), None),
            InstallKind::Dev
        );
    }
}

#[test]
fn check_for_updates_reads_github_latest_from_mock_server() {
    let root = tempfile::tempdir().unwrap();
    let release = json!({"tag_name":"v3.1.0", "body":"notes", "html_url":"https://example.invalid/release", "assets":[{"name":"App-Setup.exe","browser_download_url":"https://example.invalid/Setup.exe"}]});
    let server = Server::new(vec![(
        200,
        serde_json::to_vec(&release).unwrap(),
        Duration::ZERO,
    )]);
    let mut opts = options(root.path(), InstallKind::Dev);
    opts.latest_endpoint = format!("{}/latest", server.base);
    let svc = service(root.path(), opts, Arc::default());
    let response = svc.call("check_for_updates", vec![]);
    assert_eq!(
        response,
        json!({"ok":true,"data":update::check::release_payload(&release,"2.0.0","windows")})
    );
    assert_eq!(response["data"].as_object().unwrap().len(), 6);
    assert_eq!(response["data"]["update_available"], true);
    let requests = server.requests.lock().unwrap();
    assert!(requests[0].starts_with("GET /latest "));
    assert!(
        requests[0]
            .to_lowercase()
            .contains("user-agent: impulcifer/2.0.0")
    );
}

fn release_list() -> Vec<Value> {
    let setup = |n: u32| json!([{"name":format!("Impulcifer-{n}-Setup.exe"),"browser_download_url":format!("https://example.invalid/{n}/Setup.exe")}]);
    vec![
        // A draft is invisible to installs even when it is the newest version.
        json!({"tag_name":"v3.0.0-alpha.9","draft":true,"assets":setup(9)}),
        // The rolling updater feed carries no version and must never be chosen.
        json!({"tag_name":"updater-3x-pre","assets":[{"name":"latest.json","browser_download_url":"https://example.invalid/latest.json"}]}),
        json!({"tag_name":"v3.0.0-alpha.1","body":"alpha one","html_url":"https://example.invalid/alpha1","assets":setup(1)}),
        json!({"tag_name":"v3.0.0-alpha.0","body":"alpha zero","assets":setup(0)}),
        // A 2.x stable published after the alphas is older by version.
        json!({"tag_name":"v2.14.1","body":"two","assets":setup(2)}),
    ]
}

fn prerelease_check(current: &str, releases: Value) -> (Value, Server) {
    let root = tempfile::tempdir().unwrap();
    let server = Server::new(vec![(
        200,
        serde_json::to_vec(&releases).unwrap(),
        Duration::ZERO,
    )]);
    let mut opts = options(root.path(), InstallKind::Dev);
    opts.current_version = current.into();
    opts.releases_endpoint = format!("{}/releases?per_page=30", server.base);
    let response = service(root.path(), opts, Arc::default()).call("check_for_updates", vec![]);
    (response, server)
}

#[test]
fn prerelease_install_reads_the_release_list_and_takes_the_newest_version() {
    let (response, server) = prerelease_check("3.0.0-alpha.0", json!(release_list()));
    assert_eq!(response["ok"], true, "{response}");
    let data = &response["data"];
    assert_eq!(data["update_available"], true);
    assert_eq!(data["latest_version"], "3.0.0-alpha.1");
    assert_eq!(data["download_url"], "https://example.invalid/1/Setup.exe");
    assert_eq!(data["release_notes"], "alpha one");
    assert_eq!(data["release_url"], "https://example.invalid/alpha1");
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0].starts_with("GET /releases?per_page=30 "),
        "{}",
        requests[0]
    );
    assert!(
        requests[0]
            .to_lowercase()
            .contains("user-agent: impulcifer/3.0.0-alpha.0")
    );
}

#[test]
fn prerelease_install_is_offered_the_following_stable() {
    let mut releases = release_list();
    releases.push(json!({"tag_name":"v3.0.0","body":"stable","assets":[{"name":"Impulcifer-Setup.exe","browser_download_url":"https://example.invalid/3/Setup.exe"}]}));
    let (response, _server) = prerelease_check("3.0.0-alpha.1", json!(releases));
    assert_eq!(response["data"]["update_available"], true, "{response}");
    assert_eq!(response["data"]["latest_version"], "3.0.0");
    assert_eq!(
        response["data"]["download_url"],
        "https://example.invalid/3/Setup.exe"
    );
}

#[test]
fn prerelease_install_on_the_newest_release_is_up_to_date() {
    let (response, _server) = prerelease_check("3.0.0-alpha.1", json!(release_list()));
    assert_eq!(response["data"]["update_available"], false, "{response}");
    assert_eq!(response["data"]["latest_version"], "3.0.0-alpha.1");
    assert!(response["data"]["download_url"].is_null());
    assert!(response["data"]["release_notes"].is_null());
}

#[test]
fn release_list_failures_are_retryable() {
    for releases in [
        json!({"message":"not a list"}),
        json!([]),
        json!([{"tag_name":"updater-3x-pre","assets":[]}]),
    ] {
        let (response, _server) = prerelease_check("3.0.0-alpha.1", releases);
        assert_eq!(
            response["error"]["code"], "UPDATE_CHECK_FAILED",
            "{response}"
        );
        assert_eq!(response["error"]["retryable"], true);
    }
    assert_eq!(
        update::check::display_version("v3.0.0-alpha.1"),
        "3.0.0-alpha.1"
    );
    assert_eq!(update::check::display_version("v3.0.0"), "3.0.0");
    assert_eq!(update::check::display_version("v2.3.1.post1"), "2.3.1");
    assert!(update::check::select_release(&[]).is_none());
}

/// Evidence against the live GitHub API: an alpha.0 install must be offered the
/// newest published 3.x prerelease, which `/releases/latest` (2.x stable) never
/// names. Needs the network; run with `--ignored`.
#[test]
#[ignore = "reads the live GitHub releases API"]
fn live_prerelease_check_sees_the_published_3x_prerelease() {
    let opts = UpdateOptions {
        current_version: "3.0.0-alpha.0".into(),
        ..UpdateOptions::default()
    };
    let payload = update::check::check(&opts).unwrap();
    let latest = payload["latest_version"].as_str().unwrap();
    assert!(latest.starts_with("3."), "{payload}");
    assert_eq!(payload["update_available"], true, "{payload}");
}

#[test]
fn check_failures_and_timeout_are_retryable() {
    for (status, body, delay) in [
        (503, b"failure".to_vec(), Duration::ZERO),
        (200, b"not json".to_vec(), Duration::ZERO),
        (200, b"[]".to_vec(), Duration::ZERO),
        (200, b"{}".to_vec(), Duration::from_secs(2)),
    ] {
        let root = tempfile::tempdir().unwrap();
        let server = Server::new(vec![(status, body, delay)]);
        let mut opts = options(root.path(), InstallKind::Dev);
        opts.latest_endpoint = server.base.clone();
        opts.timeout = if delay.is_zero() {
            Duration::from_secs(2)
        } else {
            Duration::from_millis(500)
        };
        let response = service(root.path(), opts, Arc::default()).call("check_for_updates", vec![]);
        assert_eq!(
            response["error"]["code"], "UPDATE_CHECK_FAILED",
            "{response}"
        );
        assert_eq!(response["error"]["retryable"], true);
        assert_eq!(response["error"]["details"], json!({}));
    }
}

#[test]
fn no_update_hides_download_and_notes() {
    for release in [
        json!({"tag_name":"v1.0.0","assets":[{"name":"Setup.exe","browser_download_url":"x"}],"body":"notes"}),
        json!({"tag_name":"v3.0.0","assets":[],"body":"notes"}),
        json!({}),
    ] {
        let value = update::check::release_payload(&release, "2.0.0", "windows");
        assert_eq!(value["update_available"], false);
        assert!(value["release_notes"].is_null());
        assert!(value["download_url"].is_null());
    }
}

#[test]
fn start_update_validation_matches_python() {
    let root = tempfile::tempdir().unwrap();
    let svc = service(
        root.path(),
        options(root.path(), InstallKind::Tauri),
        Arc::default(),
    );
    for case in golden()["validations"].as_array().unwrap() {
        let validated = update::validate(&case["request"]);
        if case["envelope"]["ok"] == false {
            assert_eq!(validated.err().unwrap(), case["envelope"]);
            assert_eq!(
                svc.call("start_update", vec![case["request"].clone()]),
                case["envelope"]
            );
        } else {
            let request = validated.ok().unwrap();
            assert_eq!(request.latest_version, case["normalized"]["latest_version"]);
            assert_eq!(request.download_url, case["normalized"]["download_url"]);
            assert_eq!(
                finish(&svc, case["request"].clone())["job"]["status"],
                "succeeded"
            );
        }
    }
}

fn legacy_run(
    sums_status: u16,
    sums: Vec<u8>,
    fail_open: bool,
) -> (Value, Vec<(String, String)>, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let server = Server::new(vec![
        (200, b"installer".to_vec(), Duration::ZERO),
        (sums_status, sums, Duration::ZERO),
    ]);
    let state = Arc::new(Mutex::new(HostState {
        fail_open,
        ..Default::default()
    }));
    let svc = service(
        root.path(),
        options(root.path(), InstallKind::Dev),
        state.clone(),
    );
    let result = finish(
        &svc,
        json!({"latest_version":"3.1.0", "download_url":format!("{}/Setup.exe",server.base)}),
    );
    let requests = server.requests.lock().unwrap();
    assert!(requests[0].starts_with("GET /Setup.exe "));
    assert!(
        requests
            .get(1)
            .is_some_and(|r| r.starts_with("GET /SHA256SUMS.txt ")),
        "{result}"
    );
    let calls = state.lock().unwrap().calls.clone();
    (result, calls, root)
}

#[test]
fn legacy_executor_downloads_verifies_and_opens() {
    let sums = format!("{:x} *Setup.exe\n", Sha256::digest(b"installer"));
    let (result, calls, _root) = legacy_run(200, sums.into_bytes(), false);
    assert_eq!(result["job"]["status"], "succeeded");
    assert_eq!(result["job"]["result"], golden()["results"]["legacy"]);
    assert_eq!(calls.len(), 1);
    assert_eq!(std::fs::read(&calls[0].1).unwrap(), b"installer");
    let progress: Vec<_> = result["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "progress")
        .map(|e| e["payload"].clone())
        .collect();
    assert_eq!(
        progress,
        vec![
            json!({"progress":0.1,"message":"update_downloading"}),
            json!({"progress":1.0,"message":"Downloading: 100%"}),
            json!({"progress":0.9,"message":"update_opening_installer"})
        ]
    );
}

#[test]
fn slow_installer_body_uses_download_timeout_policy() {
    let root = tempfile::tempdir().unwrap();
    let chunks = vec![b"inst".to_vec(), b"all".to_vec(), b"er".to_vec()];
    let sums = format!("{:x} *Setup.exe\n", Sha256::digest(b"installer"));
    let server = Server::new_chunked(vec![
        (200, chunks, Duration::from_secs(1)),
        (200, vec![sums.into_bytes()], Duration::ZERO),
    ]);
    let state = Arc::new(Mutex::new(HostState::default()));
    let mut opts = options(root.path(), InstallKind::Dev);
    opts.timeout = Duration::from_secs(1);
    let svc = service(root.path(), opts, state.clone());
    let result = finish(
        &svc,
        json!({"latest_version":"3.1.0", "download_url":format!("{}/Setup.exe",server.base)}),
    );
    assert_eq!(result["job"]["status"], "succeeded", "{result}");
    let calls = &state.lock().unwrap().calls;
    assert_eq!(calls.len(), 1);
    assert_eq!(std::fs::read(&calls[0].1).unwrap(), b"installer");
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /Setup.exe "));
    assert!(requests[1].starts_with("GET /SHA256SUMS.txt "));
}

#[test]
fn checksum_failures_discard_download_and_never_open() {
    for (status, sums) in [
        (200, format!("{}  Setup.exe\n", "0".repeat(64))),
        (
            200,
            format!("{:x}  other.exe\n", Sha256::digest(b"installer")),
        ),
        (403, "forbidden".into()),
        (500, "error".into()),
        (200, "malformed".into()),
    ] {
        let (result, calls, root) = legacy_run(status, sums.into_bytes(), false);
        assert_eq!(result["job"]["error"]["code"], "UPDATE_FAILED");
        assert_eq!(result["job"]["error"]["retryable"], true);
        assert!(calls.is_empty());
        assert_eq!(
            std::fs::read_dir(root.path().join("downloads"))
                .unwrap()
                .count(),
            0
        );
    }
}

#[test]
fn checksum_404_preserves_legacy_compatibility_and_open_failure_is_reported() {
    let (result, calls, _root) = legacy_run(404, vec![], false);
    assert_eq!(result["job"]["status"], "succeeded");
    assert_eq!(calls.len(), 1);
    let (result, _, root) = legacy_run(404, vec![], true);
    assert_eq!(result["job"]["error"]["message"], "open failed");
    assert_eq!(
        std::fs::read_dir(root.path().join("downloads"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn apply_pending_update_without_stage_is_invalid_request() {
    let root = tempfile::tempdir().unwrap();
    let svc = service(
        root.path(),
        options(root.path(), InstallKind::Dev),
        Arc::default(),
    );
    assert_eq!(
        svc.call("apply_pending_update", vec![]),
        json!({"ok":false,"error":{"code":"INVALID_REQUEST","message":"No staged update to apply.","details":{},"retryable":false}})
    );
    let failed = finish(&svc, json!({"latest_version":"3.1.0"}));
    assert_eq!(failed["job"]["error"]["code"], "UPDATE_FAILED");
    assert_eq!(
        svc.call("apply_pending_update", vec![])["error"]["code"],
        "INVALID_REQUEST"
    );
}

#[test]
fn tauri_kind_delegates_to_host() {
    let root = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(HostState::default()));
    let svc = service(
        root.path(),
        options(root.path(), InstallKind::Tauri),
        state.clone(),
    );
    assert_eq!(
        svc.call("bootstrap", vec![])["data"]["install_kind"],
        "tauri"
    );
    assert_eq!(
        svc.call("get_system_info", vec![])["data"]["install_kind"],
        "tauri"
    );
    let result = finish(&svc, json!({"latest_version":"3.1.0"}));
    assert_eq!(result["job"]["result"], golden()["results"]["restart"]);
    for event in result["events"].as_array().unwrap() {
        if event["type"] == "progress" {
            assert!((0.0..=1.0).contains(&event["payload"]["progress"].as_f64().unwrap()));
        }
    }
    assert_eq!(
        svc.call("apply_pending_update", vec![]),
        json!({"ok":true,"data":{"restarting":true}})
    );
    assert_eq!(
        state.lock().unwrap().calls,
        vec![
            ("download".into(), "3.1.0".into()),
            ("apply".into(), "".into())
        ]
    );
    assert_eq!(
        svc.call("apply_pending_update", vec![])["error"]["code"],
        "INVALID_REQUEST"
    );
}

#[test]
fn failed_download_preserves_previous_stage_and_apply_failure_consumes_it() {
    let root = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(HostState::default()));
    let svc = service(
        root.path(),
        options(root.path(), InstallKind::Tauri),
        state.clone(),
    );
    finish(&svc, json!({"latest_version":"3.1.0"}));
    state.lock().unwrap().fail_download = true;
    assert_eq!(
        finish(&svc, json!({"latest_version":"3.2.0"}))["job"]["error"]["code"],
        "UPDATE_FAILED"
    );
    state.lock().unwrap().fail_apply = true;
    let response = svc.call("apply_pending_update", vec![]);
    assert_eq!(response["error"]["code"], "UPDATE_FAILED");
    assert_eq!(response["error"]["retryable"], true);
    assert_eq!(
        svc.call("apply_pending_update", vec![])["error"]["code"],
        "INVALID_REQUEST"
    );
}

#[test]
fn updates_are_non_cancellable_and_busy_jobs_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let jobs = JobRegistry::new();
    let svc = ImpulciferService::with_dependencies(
        Box::new(Host(Arc::default())),
        root.path().join("settings.json"),
        impulcifer_audio_io::default_backend(),
        jobs.clone(),
        root.path().to_owned(),
    )
    .with_update_options(options(root.path(), InstallKind::Tauri));
    let (send, receive) = std::sync::mpsc::channel();
    let job = jobs
        .start(impulcifer_types::job::JobKind::Update, false, move |_| {
            receive.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(json!({}))
        })
        .unwrap();
    assert_eq!(
        svc.call("cancel_job", vec![json!(job.job_id)])["error"]["code"],
        "JOB_NOT_CANCELLABLE"
    );
    assert_eq!(
        svc.call("start_update", vec![json!({"latest_version":"3.1.0"})])["error"]["code"],
        "JOB_BUSY"
    );
    send.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !jobs.poll(&job.job_id, 0).unwrap().job.status.is_terminal() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn latest_successful_update_replaces_stage() {
    let root = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(HostState::default()));
    let svc = service(
        root.path(),
        options(root.path(), InstallKind::Tauri),
        state.clone(),
    );
    finish(&svc, json!({"latest_version":"3.1.0"}));
    finish(&svc, json!({"latest_version":"3.2.0"}));
    assert_eq!(svc.call("apply_pending_update", vec![])["ok"], true);
    assert_eq!(
        svc.call("apply_pending_update", vec![])["error"]["code"],
        "INVALID_REQUEST"
    );
    assert_eq!(
        state.lock().unwrap().calls,
        vec![
            ("download".into(), "3.1.0".into()),
            ("download".into(), "3.2.0".into()),
            ("apply".into(), "".into())
        ]
    );
}

#[test]
fn legacy_appimage_replaces_running_file_and_falls_back_when_missing() {
    for missing in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let current = if missing {
            root.path().join("missing/App.AppImage")
        } else {
            root.path().join("App.AppImage")
        };
        if !missing {
            std::fs::write(&current, b"old").unwrap();
        }
        let server = Server::new(vec![
            (200, b"new image".to_vec(), Duration::ZERO),
            (404, vec![], Duration::ZERO),
        ]);
        let mut opts = options(root.path(), InstallKind::Dev);
        opts.platform = "linux".into();
        opts.appimage = Some(current.clone());
        let state = Arc::new(Mutex::new(HostState::default()));
        let svc = service(root.path(), opts, state.clone());
        let result = finish(
            &svc,
            json!({"latest_version":"3.1.0", "download_url":format!("{}/App.AppImage",server.base)}),
        );
        assert_eq!(result["job"]["status"], "succeeded", "{result}");
        let calls = &state.lock().unwrap().calls;
        assert_eq!(calls.len(), 1);
        assert_eq!(std::fs::read(&calls[0].1).unwrap(), b"new image");
        if !missing {
            assert_eq!(Path::new(&calls[0].1), current);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&calls[0].1).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }
}

#[test]
fn installer_http_failure_never_opens_or_stages() {
    let root = tempfile::tempdir().unwrap();
    let server = Server::new(vec![(500, b"error".to_vec(), Duration::ZERO)]);
    let state = Arc::new(Mutex::new(HostState::default()));
    let svc = service(
        root.path(),
        options(root.path(), InstallKind::Dev),
        state.clone(),
    );
    let result = finish(
        &svc,
        json!({"latest_version":"3.1.0", "download_url":format!("{}/Setup.exe", server.base)}),
    );
    assert_eq!(result["job"]["error"]["code"], "UPDATE_FAILED");
    assert!(state.lock().unwrap().calls.is_empty());
    assert_eq!(
        std::fs::read_dir(root.path().join("downloads"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        svc.call("apply_pending_update", vec![])["error"]["code"],
        "INVALID_REQUEST"
    );
}

#[cfg(windows)]
#[test]
fn velopack_executor_downloads_from_local_feed_and_stages_apply() {
    use local_feed::{LocalFeed, TemporaryRoot};
    use sha1::{Digest as _, Sha1};
    use zip::{ZipWriter, write::SimpleFileOptions};

    let temp = TemporaryRoot::new();
    let install_root = temp.0.join("Impulcifer");
    let current = install_root.join("current");
    let packages = install_root.join("packages");
    std::fs::create_dir_all(&current).unwrap();
    std::fs::create_dir(&packages).unwrap();
    std::fs::write(
        install_root.join("Update.exe"),
        b"P20b inert locator marker; never execute",
    )
    .unwrap();
    std::fs::write(
        current.join("sq.version"),
        r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://schemas.microsoft.com/packaging/2010/07/nuspec.xsd"><metadata>
<id>Impulcifer</id><version>2.13.3</version><title>Impulcifer</title>
<mainExe>impulcifer-app.exe</mainExe><os>win</os><channel>win</channel>
</metadata></package>"#,
    )
    .unwrap();

    let version = "3.0.0-alpha.0";
    let filename = format!("Impulcifer-{version}-full.nupkg");
    let source = temp.0.join(&filename);
    let mut package = Vec::new();
    {
        let mut archive = ZipWriter::new(std::io::Cursor::new(&mut package));
        archive
            .start_file("Impulcifer.nuspec", SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(
                format!(
                    r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://schemas.microsoft.com/packaging/2010/07/nuspec.xsd"><metadata>
<id>Impulcifer</id><version>{version}</version><title>Impulcifer</title>
<mainExe>impulcifer-app.exe</mainExe><os>win</os><channel>win</channel>
</metadata></package>"#
                )
                .as_bytes(),
            )
            .unwrap();
        archive
            .start_file("lib/app/impulcifer-app.exe", SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"synthetic application marker").unwrap();
        archive.finish().unwrap();
    }
    std::fs::write(&source, &package).unwrap();
    let feed_path = temp.0.join("releases.win.json");
    let feed = json!({"Assets":[{
        "PackageId":"Impulcifer",
        "Version":version,
        "Type":"Full",
        "FileName":filename,
        "SHA1":Sha1::digest(&package).iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        "SHA256":format!("{:x}", Sha256::digest(&package)),
        "Size":package.len(),
        "NotesMarkdown":"",
        "NotesHtml":""
    }]});
    std::fs::write(&feed_path, serde_json::to_vec(&feed).unwrap()).unwrap();
    let log_path = temp.0.join("local-feed.log");
    let mut server = LocalFeed::new(
        vec![
            ("releases.win.json".into(), feed_path),
            (filename.clone(), source),
        ],
        &log_path,
    );

    let mut opts = options(&temp.0, InstallKind::Velopack);
    opts.platform = "windows".into();
    opts.current_version = "2.13.3".into();
    opts.releases_url = format!("http://{}", server.address);
    opts.velopack_root = Some(install_root.clone());
    let stage_manager = update::velopack::manager(&opts).unwrap();
    let svc = service(&temp.0, opts, Arc::default());
    let result = finish(&svc, json!({"latest_version":version}));
    assert_eq!(result["job"]["status"], "succeeded", "{result}");
    assert_eq!(result["job"]["result"], update::restart_result());
    let messages: Vec<_> = result["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["type"] == "progress")
        .filter_map(|event| event["payload"]["message"].as_str())
        .collect();
    let downloading = messages
        .iter()
        .position(|message| *message == "update_downloading")
        .unwrap();
    let percent = messages
        .iter()
        .position(|message| message.starts_with("Downloading: ") && message.ends_with('%'))
        .unwrap();
    let installing = messages
        .iter()
        .position(|message| *message == "update_installing")
        .unwrap();
    assert!(
        downloading < percent && percent < installing,
        "{messages:?}"
    );
    let staged = packages.join(&filename);
    assert_eq!(std::fs::read(&staged).unwrap(), package);
    assert!(staged.is_file());
    let pending = stage_manager.get_update_pending_restart().unwrap();
    assert_eq!(pending.FileName, filename);
    assert_eq!(pending.Version, version);
    server.finish().unwrap();

    let apply = svc.call("apply_pending_update", vec![]);
    assert_eq!(apply["error"]["code"], "UPDATE_FAILED", "{apply}");
    let message = apply["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("IO error") || message.contains("not a valid Win32 application"),
        "{message}"
    );
    assert!(staged.is_file());
    assert_eq!(
        svc.call("apply_pending_update", vec![])["error"]["code"],
        "INVALID_REQUEST"
    );
}

#[cfg(windows)]
#[test]
fn velopack_kind_selected_on_windows_with_update_exe() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("current")).unwrap();
    std::fs::write(root.path().join("Update.exe"), b"fake").unwrap();
    assert_eq!(
        install_kind::detect("windows", &root.path().join("current/app.exe"), None),
        InstallKind::Velopack
    );
}

#[cfg(windows)]
#[test]
#[ignore = "requires installed Velopack build and IMPULCIFER_LIVE_UPDATE_TEST=1"]
fn velopack_check_against_live_feed() {
    assert_eq!(
        std::env::var("IMPULCIFER_LIVE_UPDATE_TEST").as_deref(),
        Ok("1")
    );
    let manager = update::velopack::manager(&UpdateOptions::default()).unwrap();
    manager.check_for_updates().unwrap();
}
