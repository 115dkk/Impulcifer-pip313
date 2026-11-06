#![forbid(unsafe_code)]

use impulcifer_io::{read_wav, write_wav};
use impulcifer_jobs::registry::{JobRegistry, PollResult};
use impulcifer_service::{
    ImpulciferService, NoopHost,
    recording::{devices, naming, progress, request, run, sweep},
};
use impulcifer_types::{audio::*, job::JobKind};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "impulcifer-p16-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens")
}
/// The 2.x recording golden, with the layout list 3.x accepts: ADR 0007 adds
/// the immersive formats to the "Supported:" list of an unknown layout.
fn golden() -> Value {
    let text = std::fs::read_to_string(fixtures().join("p16_recording.json")).unwrap();
    let text = text.replace(
        "Supported: mono, stereo, 5.1, 7.1, 7.1.4, 7.1.6.",
        &format!(
            "Supported: {}.",
            impulcifer_types::constants::SWEEP_TRACK_LAYOUTS.join(", ")
        ),
    );
    serde_json::from_str(&text).unwrap()
}
fn map_strings(value: &Value, f: &impl Fn(&str) -> String) -> Value {
    match value {
        Value::String(s) => json!(f(s)),
        Value::Array(a) => Value::Array(a.iter().map(|v| map_strings(v, f)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), map_strings(v, f)))
                .collect(),
        ),
        _ => value.clone(),
    }
}
fn service(temp: &Temp, backend: Box<dyn AudioBackend>) -> ImpulciferService {
    ImpulciferService::with_dependencies(
        Box::new(NoopHost),
        temp.0.join("settings.json"),
        backend,
        JobRegistry::new(),
        temp.0.clone(),
    )
}
#[test]
fn golden_recording_validation_matches_python() {
    let temp = Temp::new();
    for (channels, name) in [
        (1, "mono.wav"),
        (2, "stereo.wav"),
        (3, "three.wav"),
        (2, "sweep-seg-FL,FR-stereo-1s-test.wav"),
    ] {
        write_wav(&temp.0.join(name), 8000, &vec![vec![0.0; 32]; channels], 32).unwrap();
    }
    std::fs::write(temp.0.join("broken.wav"), b"broken").unwrap();
    let expand = |s: &str| s.replace("$ROOT", &temp.0.to_string_lossy());
    let normalize = |s: &str| {
        s.replace(temp.0.to_str().unwrap(), "$ROOT")
            .replace('\\', "/")
    };
    let g = golden();
    for case in g["validation"].as_array().unwrap() {
        let actual = match request::validate(&map_strings(&case["request"], &expand)) {
            Ok(v) => {
                let mut data = serde_json::to_value(v).unwrap();
                data.as_object_mut().unwrap().remove("share");
                data.as_object_mut().unwrap().remove("channel_order");
                data.as_object_mut().unwrap().remove("expected_azimuths");
                data.as_object_mut()
                    .unwrap()
                    .remove("reference_measurements");
                json!({"ok":true,"data":data})
            }
            Err(e) => e,
        };
        assert_eq!(
            map_strings(&actual, &normalize),
            case["response"],
            "{}",
            case["name"]
        );
    }
    for case in g["sweep_validation"].as_array().unwrap() {
        let mode = serde_json::from_value(case["mode"].clone()).unwrap();
        let actual = match request::validate_sweep(&case["sweep"], mode) {
            Ok(v) => json!({"ok":true,"data":{"spec":v}}),
            Err(e) => e,
        };
        assert_eq!(actual, case["response"], "{}", case["name"]);
    }
    let svc = service(&temp, Box::new(FakeBackend::default()));
    for case in g["paths"].as_array().unwrap() {
        let args = map_strings(&case["args"], &expand)
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(
            map_strings(&svc.call("resolve_recording_paths", args), &normalize),
            case["response"]
        );
    }
    for case in g["naming"].as_array().unwrap() {
        assert_eq!(
            naming::derive_record_filename(case["input"].as_str().unwrap()),
            case["output"]
        );
    }
    for case in g["speaker_names"].as_array().unwrap() {
        let names: Vec<String> = serde_json::from_value(case["input"].clone()).unwrap();
        let actual = match naming::record_filename_for_speakers(&names) {
            Ok(n) => json!({"ok":true,"name":n}),
            Err(e) => json!({"ok":false,"message":e}),
        };
        assert_eq!(actual, case["response"]);
    }
}
#[test]
fn share_mode_request_validation() {
    let missing_dir = |share: Value| request::validate(&json!({"share_mode":share})).unwrap_err();
    assert_eq!(
        request::validate(&json!({"record_dir":"missing","sweep":{}}))
            .unwrap()
            .share,
        SharePreference::Auto
    );
    assert_eq!(
        request::validate(&json!({"record_dir":"missing","sweep":{},"share_mode":null}))
            .unwrap()
            .share,
        SharePreference::Auto
    );
    for (name, expected) in [
        ("auto", SharePreference::Auto),
        ("exclusive", SharePreference::Exclusive),
        ("shared", SharePreference::Shared),
    ] {
        assert_eq!(
            request::validate(&json!({"record_dir":"missing","sweep":{},"share_mode":name}))
                .unwrap()
                .share,
            expected
        );
    }
    for invalid_share in [json!("EXCLUSIVE"), json!("both"), json!(1), json!(true)] {
        let error = missing_dir(invalid_share.clone());
        assert_eq!(
            error["error"]["message"],
            "share_mode must be auto, exclusive or shared."
        );
        assert_eq!(
            error["error"]["details"],
            json!({"share_mode":invalid_share})
        );
    }
}

#[test]
fn channel_order_request_validation() {
    use impulcifer_service::recording::channel_order::ChannelOrder;

    assert_eq!(
        request::validate(&json!({"record_dir":"missing","sweep":{}}))
            .unwrap()
            .channel_order,
        ChannelOrder::Auto
    );
    assert_eq!(
        request::validate(&json!({"record_dir":"missing","sweep":{},"channel_order":null}))
            .unwrap()
            .channel_order,
        ChannelOrder::Auto
    );
    for (raw, expected) in [
        ("auto", ChannelOrder::Auto),
        ("standard", ChannelOrder::Standard),
        ("hdmi", ChannelOrder::Hdmi),
    ] {
        assert_eq!(
            request::validate(&json!({"record_dir":"missing","sweep":{},"channel_order":raw}))
                .unwrap()
                .channel_order,
            expected
        );
    }
    for raw in [json!("HDMI"), json!("both"), json!(1), json!(true)] {
        let error = request::validate(
            &json!({"record_dir":"missing","sweep":{},"channel_order":raw.clone()}),
        )
        .unwrap_err();
        assert_eq!(
            error["error"]["message"],
            "channel_order must be auto, standard or hdmi."
        );
        assert_eq!(error["error"]["details"], json!({"channel_order":raw}));
    }
}

#[test]
fn expected_azimuths_validate_against_the_sweep() {
    let valid = request::validate(&json!({
        "record_dir":"missing",
        "sweep":{"speakers":["FL","FR"]},
        "expected_azimuths":{"fl":30,"FR":-30.5}
    }))
    .unwrap();
    assert_eq!(
        valid.expected_azimuths,
        Some([(String::from("FL"), 30.0), (String::from("FR"), -30.5)].into())
    );
    for expected in [json!([]), json!({"FL":"30"}), json!({"FL":181})] {
        let error = request::validate(&json!({
            "record_dir":"missing","sweep":{"speakers":["FL"]},
            "expected_azimuths":expected
        }))
        .unwrap_err();
        assert_eq!(error["error"]["code"], "INVALID_REQUEST");
    }
    let outside = request::validate(&json!({
        "record_dir":"missing","sweep":{"speakers":["FL"]},
        "expected_azimuths":{"FR":-30}
    }))
    .unwrap_err();
    assert_eq!(outside["error"]["code"], "INVALID_REQUEST");
    let headphones = request::validate(&json!({
        "mode":"headphones","record_dir":"missing","sweep":{},
        "expected_azimuths":"ignored"
    }))
    .unwrap();
    assert!(headphones.expected_azimuths.is_none());
}

#[test]
fn reference_measurements_validate_shape_limit_and_speakers() {
    let valid = request::validate(&json!({
        "record_dir":"missing",
        "sweep":{"speakers":["FR"]},
        "reference_measurements":[{
            "speaker":"fl","itd_us":250,"lf_db":-20.5,"lf_ild_db":-6.5,"azimuth":30
        }]
    }))
    .unwrap();
    let reference = &valid.reference_measurements.unwrap()[0];
    assert_eq!(reference.speaker, "FL");
    assert_eq!(reference.itd_us, Some(250.0));
    assert_eq!(reference.lf_db, Some(-20.5));
    assert_eq!(reference.lf_ild_db, Some(-6.5));
    assert_eq!(reference.azimuth, Some(30.0));

    let invalid = [
        json!({}),
        json!([{"speaker":"FL","itd_us":"250","lf_db":null,"azimuth":null}]),
        json!([{"speaker":"FL","itd_us":null,"lf_db":null,"lf_ild_db":"-6","azimuth":null}]),
        json!([{"speaker":"NOPE","itd_us":null,"lf_db":null,"azimuth":null}]),
        json!([{"speaker":"FL","itd_us":null,"lf_db":null,"azimuth":181}]),
        Value::Array(
            (0..65)
                .map(|_| json!({"speaker":"FL","itd_us":null,"lf_db":null,"azimuth":null}))
                .collect(),
        ),
    ];
    for reference_measurements in invalid {
        let error = request::validate(&json!({
            "record_dir":"missing","sweep":{"speakers":["FL"]},
            "reference_measurements":reference_measurements
        }))
        .unwrap_err();
        assert_eq!(error["error"]["code"], "INVALID_REQUEST");
    }
    let headphones = request::validate(&json!({
        "mode":"headphones","record_dir":"missing","sweep":{},
        "reference_measurements":[{
            "speaker":"FC","itd_us":null,"lf_db":null,"lf_ild_db":-7.0,"azimuth":0
        }]
    }))
    .unwrap();
    assert_eq!(
        headphones.reference_measurements.unwrap()[0].lf_ild_db,
        Some(-7.0)
    );
}

#[test]
fn golden_sweep_playback_matches_python() {
    for case in golden()["playback"].as_array().unwrap() {
        let spec: sweep::SweepSpec = serde_json::from_value(case["input"].clone()).unwrap();
        let p = sweep::build_sweep_playback(&spec).unwrap();
        assert_eq!(json!(p.spec), case["spec"]);
        assert_eq!(json!([p.tracks.len(), p.tracks[0].len()]), case["shape"]);
        assert_eq!(json!(p.segments), case["segments"]);
        assert_eq!(p.display_name, case["display_name"]);
        assert_eq!(p.record_filename, case["record_filename"]);
        let bytes: Vec<_> = p
            .tracks
            .iter()
            .flatten()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        let hash = sha256(&bytes);
        if hash != case["sha256"].as_str().unwrap() {
            // The packet explicitly permits P08's one-LSB fallback for libm.
            // Reconstruct every silent and active sample, not just a sparse probe.
            let raw = std::fs::read(fixtures().join(case["signal"].as_str().unwrap())).unwrap();
            let signal: Vec<_> = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| i32::from_le_bytes(*b) as f64 / 2147483648.0)
                .collect();
            assert_eq!(signal.len(), p.estimator.test_signal.len());
            let mut estimator = p.estimator.clone();
            estimator.test_signal = signal;
            let names: Vec<_> = p.spec.speakers.iter().map(String::as_str).collect();
            let reference = estimator.sweep_sequence(&names, &p.spec.tracks).unwrap();
            let reference_bytes: Vec<_> = reference
                .iter()
                .flatten()
                .flat_map(|x| x.to_le_bytes())
                .collect();
            assert_eq!(sha256(&reference_bytes), case["sha256"].as_str().unwrap());
            let mut differences = 0;
            let mut max: f64 = 0.0;
            for (a, b) in p.tracks.iter().flatten().zip(reference.iter().flatten()) {
                max = max.max((a - b).abs());
                if a != b {
                    differences += 1;
                }
            }
            eprintln!(
                "P16 {}: SHA differs; {} samples differ, max {} PCM32 LSB (limit 1)",
                p.display_name,
                differences,
                max * 2147483648.0
            );
            assert!(max <= 1.0 / 2147483648.0);
        }
    }
}
#[test]
fn golden_segments_and_progress_match_python() {
    for case in golden()["progress"].as_array().unwrap() {
        let duration = case["duration"].as_f64().unwrap();
        let segments = progress::infer_sweep_segments(case["file"].as_str().unwrap(), duration);
        assert_eq!(
            segments,
            serde_json::from_value::<Vec<progress::SweepSegment>>(case["segments"].clone())
                .unwrap()
        );
        for row in case["events"].as_array().unwrap() {
            assert_eq!(
                progress::event_for_elapsed(row["elapsed"].as_f64().unwrap(), duration, &segments),
                serde_json::from_value::<progress::RecorderProgressEvent>(row["event"].clone())
                    .unwrap()
            );
        }
    }
}
#[test]
fn golden_analyze_recording_matches_python() {
    for case in golden()["analysis"].as_array().unwrap() {
        let actual =
            run::analyze_recording(&fixtures().join(case["file"].as_str().unwrap())).unwrap();
        for key in ["sample_rate", "channels", "duration", "active_channels"] {
            assert_eq!(actual[key], case["summary"][key]);
        }
        assert!(
            (actual["peak_db"].as_f64().unwrap() - case["summary"]["peak_db"].as_f64().unwrap())
                .abs()
                < 1e-12
        );
    }
    let temp = Temp::new();
    assert!(run::analyze_recording(&temp.0.join("absent.wav")).is_none());
    write_wav(&temp.0.join("empty.wav"), 8000, &[vec![]], 32).unwrap();
    assert!(run::analyze_recording(&temp.0.join("empty.wav")).is_none());
}
fn endpoint(input: bool, name: &str, id: &str) -> Endpoint {
    Endpoint {
        id: id.into(),
        name: name.into(),
        host_api: "Windows WASAPI".into(),
        max_input_channels: if input { 8 } else { 0 },
        max_output_channels: if input { 0 } else { 8 },
        default_samplerate: 48000.0,
        is_default_input: input,
        is_default_output: !input,
    }
}
#[derive(Default)]
struct Shared {
    playback: Option<(Vec<f32>, u16)>,
    opens: Vec<(Direction, StreamSpec, ShareMode)>,
}
#[derive(Default)]
struct FakeBackend {
    shared: Arc<Mutex<Shared>>,
    fail: bool,
    fallback: bool,
    /// Capture fails once playback has started (both streams already open).
    fail_read: bool,
    capture_sample: Option<f32>,
    delay: Duration,
}
impl AudioBackend for FakeBackend {
    fn name(&self) -> &'static str {
        "fake"
    }
    fn selectable_share_modes(&self) -> &'static [ShareMode] {
        &[ShareMode::Exclusive, ShareMode::SharedAutoConvert]
    }
    fn enumerate(&self) -> Result<Vec<Endpoint>, AudioError> {
        Ok(vec![
            endpoint(true, "Microphone", "mic-id"),
            endpoint(false, "Speakers", "out-id"),
            Endpoint {
                is_default_output: false,
                ..endpoint(false, "Speakers Extra", "out-extra")
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
        panic!("policy opens streams; no independent probe")
    }
    fn open_output(
        &self,
        ep: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn OutputSession>, AudioError> {
        assert_eq!(ep.id, "out-id");
        self.shared
            .lock()
            .unwrap()
            .opens
            .push((Direction::Output, spec, mode));
        if self.fail {
            return Err(AudioError::Backend("injected playback failure".into()));
        }
        if self.fallback && mode == ShareMode::Exclusive {
            return Err(AudioError::UnsupportedFormat("exclusive rejected".into()));
        }
        Ok(Box::new(FakeOutput {
            shared: self.shared.clone(),
            channels: spec.channels,
            mode,
            delay: self.delay,
        }))
    }
    fn open_input(
        &self,
        ep: &Endpoint,
        spec: StreamSpec,
        mode: ShareMode,
    ) -> Result<Box<dyn InputSession>, AudioError> {
        assert_eq!(ep.id, "mic-id");
        self.shared
            .lock()
            .unwrap()
            .opens
            .push((Direction::Input, spec, mode));
        if self.fallback && mode == ShareMode::Exclusive {
            return Err(AudioError::UnsupportedFormat("exclusive rejected".into()));
        }
        Ok(Box::new(FakeInput {
            shared: self.shared.clone(),
            channels: spec.channels,
            cursor: 0,
            fail_read: self.fail_read,
            capture_sample: self.capture_sample,
        }))
    }
}
struct FakeOutput {
    shared: Arc<Mutex<Shared>>,
    channels: u16,
    mode: ShareMode,
    delay: Duration,
}
impl OutputSession for FakeOutput {
    fn play_to_completion(
        &mut self,
        data: &[f32],
        cancel: &CancelToken,
    ) -> Result<PlaybackReport, AudioError> {
        self.shared.lock().unwrap().playback = Some((data.to_vec(), self.channels));
        let deadline = Instant::now() + self.delay;
        while Instant::now() < deadline {
            if cancel.is_cancelled() {
                return Err(AudioError::Cancelled);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let frames = (data.len() / usize::from(self.channels)) as u64;
        Ok(PlaybackReport {
            frames_submitted: frames,
            frames_drained: frames,
            mode: self.mode,
            underruns: 0,
            cancelled: false,
        })
    }
}
struct FakeInput {
    shared: Arc<Mutex<Shared>>,
    channels: u16,
    cursor: usize,
    fail_read: bool,
    capture_sample: Option<f32>,
}
impl InputSession for FakeInput {
    fn start(&mut self) -> Result<(), AudioError> {
        Ok(())
    }
    fn stop(&mut self) -> Result<(), AudioError> {
        Ok(())
    }
    fn read_into(
        &mut self,
        dst: &mut [f32],
        cancel: &CancelToken,
    ) -> Result<CaptureRead, AudioError> {
        if cancel.is_cancelled() {
            return Err(AudioError::Cancelled);
        }
        let state = self.shared.lock().unwrap();
        let Some((playback, channels)) = &state.playback else {
            return Ok(CaptureRead {
                frames: 0,
                discontinuity: false,
                silent: false,
            });
        };
        if self.fail_read {
            return Err(AudioError::Backend("injected capture failure".into()));
        }
        let frames = dst.len() / usize::from(self.channels);
        for (i, frame) in dst.chunks_exact_mut(usize::from(self.channels)).enumerate() {
            for (ch, sample) in frame.iter_mut().enumerate() {
                *sample = self.capture_sample.unwrap_or_else(|| {
                    (self.cursor + i)
                        .checked_sub(1000)
                        .and_then(|i| {
                            playback.get(i * usize::from(*channels) + ch % usize::from(*channels))
                        })
                        .copied()
                        .unwrap_or(0.0)
                });
            }
        }
        self.cursor += frames;
        Ok(CaptureRead {
            frames,
            discontinuity: false,
            silent: false,
        })
    }
}
fn wait(registry: &JobRegistry, id: &str) -> PollResult {
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let p = registry.poll(id, 0).unwrap();
        if p.job.status.is_terminal() {
            return p;
        }
        assert!(Instant::now() < deadline, "recording timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn start(v: request::ValidatedRecording, backend: impl AudioBackend + 'static) -> PollResult {
    let registry = JobRegistry::new();
    let job = registry
        .start(JobKind::Recording, true, move |ctx| {
            run::run_recording(&v, &backend, ctx)
        })
        .unwrap();
    wait(&registry, &job.job_id)
}
#[test]
fn recording_job_with_fake_backend_emits_lifecycle_and_writes_pcm32() {
    let temp = Temp::new();
    let spec = json!({"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR"});
    let validated = request::validate(
        &json!({"record_dir":temp.0,"sweep":spec,"channels":4,"force_channels":true}),
    )
    .unwrap();
    let backend = FakeBackend {
        fallback: true,
        ..Default::default()
    };
    let shared = backend.shared.clone();
    let p = start(validated, backend);
    assert_eq!(json!(p.job.status), "succeeded", "{:?}", p.job.error);
    assert!(p.job.cancellable);
    assert!(
        p.events
            .iter()
            .all(|e| matches!(json!(e.kind).as_str(), Some("status" | "progress" | "log")))
    );
    let events: Vec<_> = p
        .events
        .iter()
        .filter(|e| e.payload["phase"].is_string())
        .collect();
    let phases: Vec<_> = events
        .iter()
        .map(|e| e.payload["phase"].as_str().unwrap())
        .collect();
    assert_eq!(&phases[..2], ["loading", "devices"]);
    assert!(phases.contains(&"recording"));
    assert_eq!(&phases[phases.len() - 2..], ["saving", "complete"]);
    for e in events {
        assert_eq!(e.payload.as_object().unwrap().len(), 10);
    }
    let result = p.job.result.unwrap();
    assert_eq!(result.as_object().unwrap().len(), 9);
    assert_eq!(result["record_dir"], json!(temp.0));
    assert!(result["measurements"].is_array());
    assert!(result["checks"].is_array());
    assert_eq!(result["mode"], "speakers");
    assert!(
        result["sidecar_path"]
            .as_str()
            .unwrap()
            .ends_with("test.wav")
    );
    let wav = read_wav(Path::new(result["record_path"].as_str().unwrap())).unwrap();
    let state = shared.lock().unwrap();
    let (played, channels) = state.playback.as_ref().unwrap();
    assert_eq!(wav.tracks.len(), 4);
    assert_eq!(wav.tracks[0].len(), played.len() / usize::from(*channels));
    assert_eq!(wav.sample_rate, 8000);
    // PA05 opens the capture and render clients on two threads at once, so the
    // recorded open order is a race; the input-before-output guarantee is about
    // `start` and is covered by session_starts_input_before_output.
    assert!(state.opens.iter().any(|x| x.0 == Direction::Input));
    assert!(state.opens.iter().any(|x| x.0 == Direction::Output));
    assert!(
        state
            .opens
            .iter()
            .any(|x| x.2 == ShareMode::SharedAutoConvert)
    );
    for (ch, t) in wav.tracks.iter().enumerate() {
        for (i, &x) in t.iter().enumerate() {
            let expected = i.checked_sub(1000).map_or(0.0, |i| {
                played[i * usize::from(*channels) + ch % usize::from(*channels)] as f64
            });
            assert_eq!(
                x,
                impulcifer_io::wav::pcm32_round_trip(&[vec![expected]])[0][0]
            );
        }
    }
    let bytes = std::fs::read(Path::new(result["record_path"].as_str().unwrap())).unwrap();
    assert_eq!(u16::from_le_bytes(bytes[34..36].try_into().unwrap()), 32);
    eprintln!("fake recording phases: {}", phases.join(", "));
}
#[test]
fn hdmi_channel_order_moves_fc_to_device_channel_four() {
    for (requested, expected_order, active_channel) in [
        ("hdmi", "hdmi", 3usize),
        ("standard", "standard", 2usize),
        ("auto", "standard", 2usize),
    ] {
        let temp = Temp::new();
        let backend = FakeBackend::default();
        let shared = backend.shared.clone();
        let validated = request::validate(&json!({
            "record_dir":temp.0,
            "sweep":{"mode":"custom","fs":8000,"duration":1,"speakers":"FC","tracks":"7.1"},
            "channel_order":requested
        }))
        .unwrap();
        let poll = start(validated, backend);
        assert_eq!(json!(poll.job.status), "succeeded", "{:?}", poll.job.error);
        let result = poll.job.result.unwrap();
        assert_eq!(result["channel_order"], expected_order);
        let state = shared.lock().unwrap();
        let (playback, channels) = state.playback.as_ref().unwrap();
        assert_eq!(*channels, 8);
        let energy = |channel: usize| {
            playback
                .chunks_exact(usize::from(*channels))
                .map(|frame| frame[channel].abs())
                .sum::<f32>()
        };
        assert!(energy(active_channel) > 0.0, "{requested}");
        assert_eq!(energy(if active_channel == 2 { 3 } else { 2 }), 0.0);
        drop(state);
        let log = poll
            .events
            .iter()
            .find(|event| {
                event.payload["key"]
                    == if expected_order == "hdmi" {
                        "recording_channel_order_hdmi"
                    } else {
                        "recording_channel_order_standard"
                    }
            })
            .expect("channel order log");
        if expected_order == "hdmi" {
            assert!(
                log.payload["channels"]
                    .as_str()
                    .unwrap()
                    .contains("3 LFE, 4 FC")
            );
        }
        assert_eq!(log.payload["auto"], requested == "auto");
    }
}

#[test]
fn short_output_records_the_channels_it_has() {
    for order in ["standard", "hdmi"] {
        let temp = Temp::new();
        let backend = FakeBackend::default();
        let shared = backend.shared.clone();
        let validated = request::validate(&json!({
            "record_dir":temp.0,
            "sweep":{"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR","tracks":"7.1.4"},
            "channel_order":order,
            "confirm_warnings":true
        }))
        .unwrap();
        let poll = start(validated, backend);
        assert_eq!(json!(poll.job.status), "succeeded", "{:?}", poll.job.error);
        let state = shared.lock().unwrap();
        assert_eq!(state.playback.as_ref().unwrap().1, 8);
        drop(state);
        let log = poll
            .events
            .iter()
            .find(|event| event.payload["key"] == "recording_output_channels_short")
            .expect("short output log");
        assert_eq!(log.payload["missing"], "TFL, TFR, TBL, TBR");
        assert_eq!(log.payload["first"], 9);
        assert_eq!(log.payload["last"], 12);
        let result = poll.job.result.unwrap();
        assert_eq!(result["output_channels"]["available"], 8);
        assert_eq!(result["output_channels"]["required"], 12);
        assert_eq!(
            result["output_channels"]["missing"],
            json!(["TFL", "TFR", "TBL", "TBR"])
        );
    }
}

#[test]
fn short_output_without_swept_extra_channels_is_quiet() {
    let temp = Temp::new();
    let validated = request::validate(&json!({
        "record_dir":temp.0,
        "sweep":{"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR","tracks":"7.1.4"},
        "channel_order":"standard"
    }))
    .unwrap();
    let poll = start(validated, FakeBackend::default());
    assert_eq!(json!(poll.job.status), "succeeded", "{:?}", poll.job.error);
    assert!(
        poll.events
            .iter()
            .all(|event| event.payload["key"] != "recording_output_channels_short")
    );
    let result = poll.job.result.unwrap();
    assert!(result.get("output_channels").is_none());
    assert!(result.get("warnings").is_none());
}

#[test]
fn short_output_asks_for_confirmation_first() {
    let temp = Temp::new();
    let svc = service(&temp, Box::new(FakeBackend::default()));
    let request = json!({
        "record_dir":temp.0.join("out"),
        "sweep":{"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR","tracks":"7.1.4"},
        "channel_order":"standard"
    });
    let warning = svc.call("start_recording", vec![request.clone()]);
    assert_eq!(warning["error"]["code"], "CONFIRMATION_REQUIRED");
    assert_eq!(
        warning["error"]["details"]["warning"],
        "output_channels_short"
    );
    assert_eq!(warning["error"]["details"]["available"], 8);
    assert_eq!(warning["error"]["details"]["required"], 12);
    assert_eq!(warning["error"]["details"]["first"], 9);
    assert_eq!(warning["error"]["details"]["last"], 12);
    assert_eq!(warning["error"]["details"]["missing"], "TFL, TFR, TBL, TBR");
    assert_eq!(warning["error"]["details"]["layout"], "7.1.4");
    let mut confirmed = request;
    confirmed["confirm_warnings"] = json!(true);
    let started = svc.call("start_recording", vec![confirmed]);
    assert_eq!(started["ok"], true, "{started}");
    let id = started["data"]["job"]["job_id"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let poll = svc.call("poll_job", vec![json!(id)]);
        if poll["data"]["job"]["status"] != "running" {
            assert_eq!(poll["data"]["job"]["status"], "succeeded", "{poll}");
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }

    let quiet = json!({
        "record_dir":temp.0.join("quiet"),
        "sweep":{"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR","tracks":"7.1.4"},
        "channel_order":"standard"
    });
    assert_eq!(svc.call("start_recording", vec![quiet])["ok"], true);
}

#[test]
fn silent_input_is_reported_as_one_error_check() {
    let expected_key = if cfg!(target_os = "macos") {
        "record_check_input_silent_macos"
    } else {
        "record_check_input_silent"
    };
    for mode in ["speakers", "headphones"] {
        let temp = Temp::new();
        let validated = request::validate(&json!({
            "mode":mode,
            "record_dir":temp.0,
            "sweep":{"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR"}
        }))
        .unwrap();
        let poll = start(
            validated,
            FakeBackend {
                capture_sample: Some(0.0),
                ..Default::default()
            },
        );
        assert_eq!(json!(poll.job.status), "succeeded", "{:?}", poll.job.error);
        let result = poll.job.result.unwrap();
        assert_eq!(
            result["checks"],
            json!([{"key":expected_key,"speaker":"","severity":"error","values":{}}])
        );
        if mode == "speakers" {
            assert!(result["measurements"].is_array());
        }
        assert!(poll.events.iter().any(|event| {
            event.payload["key"] == expected_key
                && event.payload["message"] == "The input recorded only digital silence."
        }));
    }

    let temp = Temp::new();
    let validated = request::validate(&json!({
        "record_dir":temp.0,
        "sweep":{"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR"}
    }))
    .unwrap();
    let poll = start(
        validated,
        FakeBackend {
            capture_sample: Some(0.25),
            ..Default::default()
        },
    );
    let result = poll.job.result.unwrap();
    assert!(
        result["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["key"] != expected_key)
    );
}

#[test]
fn recording_append_pads_and_stacks_like_python() {
    for (old, new) in [(3, 5), (5, 3)] {
        let temp = Temp::new();
        let path = temp.0.join("FL.wav");
        write_wav(&path, 44100, &[vec![0.5; old]], 32).unwrap();
        run::save_recording(&path, 8000, vec![vec![-0.25; new]; 2], true).unwrap();
        let wav = read_wav(&path).unwrap();
        assert_eq!(wav.sample_rate, 8000);
        assert_eq!(wav.tracks.len(), 3);
        let mut a = vec![0.5; old];
        a.resize(old.max(new), 0.0);
        let mut b = vec![-0.25; new];
        b.resize(old.max(new), 0.0);
        assert_eq!(wav.tracks, vec![a, b.clone(), b]);
    }
}
#[test]
fn fixed_share_mode_refusal_reports_the_mode() {
    let temp = Temp::new();
    let spec = json!({"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR"});
    let exclusive =
        request::validate(&json!({"record_dir":temp.0,"sweep":spec,"share_mode":"exclusive"}))
            .unwrap();
    let failed = start(
        exclusive,
        FakeBackend {
            fallback: true,
            ..Default::default()
        },
    );
    assert_eq!(json!(failed.job.status), "failed");
    let error = failed.job.error.unwrap();
    assert_eq!(error["code"], "DEVICE_ERROR");
    assert_eq!(error["details"]["kind"], "share_mode_refused");
    assert_eq!(error["details"]["share_mode"], "exclusive");
    assert!(failed.events.iter().any(|event| {
        event.payload["phase"] == "error"
            && event.payload["message"].as_str().is_some_and(|message| {
                message.starts_with("The device did not open in exclusive mode")
            })
    }));

    let automatic = request::validate(
        &json!({"record_dir":temp.0.join("auto"),"sweep":spec,"share_mode":"auto"}),
    )
    .unwrap();
    let succeeded = start(
        automatic,
        FakeBackend {
            fallback: true,
            ..Default::default()
        },
    );
    assert_eq!(json!(succeeded.job.status), "succeeded");
    assert_eq!(
        succeeded.job.result.as_ref().unwrap()["share"]["output"],
        "shared_auto_convert"
    );
    let log_index = succeeded
        .events
        .iter()
        .position(|event| event.payload["key"] == "recording_share_mode_opened")
        .unwrap();
    let saving_index = succeeded
        .events
        .iter()
        .position(|event| event.payload["phase"] == "saving")
        .unwrap();
    assert!(log_index < saving_index);
}

#[test]
fn fixed_share_mode_keeps_post_open_failures_as_device_errors() {
    // Codex #206: a failure after both streams opened (here a capture read that
    // fails once playback is running) is an ordinary device error, not a
    // share-mode refusal, even when the mode is fixed.
    let temp = Temp::new();
    let spec = json!({"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR"});
    let exclusive =
        request::validate(&json!({"record_dir":temp.0,"sweep":spec,"share_mode":"exclusive"}))
            .unwrap();
    let failed = start(
        exclusive,
        FakeBackend {
            fail_read: true,
            ..Default::default()
        },
    );
    assert_eq!(json!(failed.job.status), "failed");
    let error = failed.job.error.unwrap();
    assert_eq!(error["code"], "DEVICE_ERROR");
    assert!(error["details"].get("kind").is_none(), "{error}");
    let message = error["message"].as_str().unwrap();
    assert!(!message.starts_with("The device did not open"), "{message}");
    assert!(message.contains("injected capture failure"), "{message}");
    assert!(failed.events.iter().any(|event| {
        event.payload["phase"] == "error" && event.payload["message"] == json!(message)
    }));
}

#[test]
fn recording_device_error_is_retryable_device_error() {
    let temp = Temp::new();
    let play = temp.0.join("play.wav");
    write_wav(&play, 8000, &vec![vec![0.1; 2000]; 2], 32).unwrap();
    let v = request::validate(&json!({"record_dir":temp.0.join("out"),"play_path":play})).unwrap();
    let p = start(
        v,
        FakeBackend {
            fail: true,
            ..Default::default()
        },
    );
    let error = p.job.error.unwrap();
    assert_eq!(error["code"], "DEVICE_ERROR");
    assert_eq!(error["retryable"], true);
    assert!(p.events.iter().any(|e| e.payload["phase"] == "error"));
}
#[test]
fn recording_output_missing_when_write_fails() {
    let temp = Temp::new();
    let play = temp.0.join("play.wav");
    let out = temp.0.join("out");
    write_wav(&play, 8000, &vec![vec![0.1; 2000]; 2], 32).unwrap();
    std::fs::create_dir_all(out.join("play.wav")).unwrap();
    let v = request::validate(&json!({"record_dir":out,"play_path":play})).unwrap();
    let path = v.record_path.clone();
    let p = start(v, FakeBackend::default());
    let error = p.job.error.unwrap();
    assert_eq!(error["code"], "OUTPUT_MISSING");
    assert_eq!(error["details"]["record_path"], path);
}
#[test]
fn list_audio_devices_shape_and_unknown_host_api() {
    let backend = FakeBackend::default();
    let data = devices::list_audio_devices(&backend, None).unwrap();
    assert_eq!(data["host_apis"], json!(["Windows WASAPI"]));
    assert_eq!(data["default_input_index"], 0);
    assert_eq!(data["default_output_index"], 1);
    assert_eq!(data["devices"][0].as_object().unwrap().len(), 6);
    #[cfg(not(target_os = "macos"))]
    assert!(
        data["devices"]
            .as_array()
            .unwrap()
            .iter()
            .all(|device| device["hdmi"] == false)
    );
    let error = devices::list_audio_devices(&backend, Some("ASIO")).unwrap_err();
    assert_eq!(
        error["error"],
        json!({"code":"INVALID_REQUEST","message":"Unknown host API.","details":{"host_api":"ASIO"},"retryable":false})
    );
}
#[test]
fn resolve_devices_matches_2x_name_rules() {
    let backend = FakeBackend::default();
    for (input, output, api) in [
        (None, None, None),
        (Some("micro"), Some("Speakers"), None),
        (
            Some("Microphone WASAPI"),
            Some("Speakers Windows WASAPI"),
            None,
        ),
        (Some("Microphone"), Some("Speakers"), Some("Windows WASAPI")),
    ] {
        let (i, o) = devices::resolve_devices(&backend, input, output, api, 2).unwrap();
        assert_eq!(i.id, "mic-id");
        assert_eq!(o.id, "out-id");
    }
    let e = devices::resolve_devices(&backend, None, Some("Speak"), Some("Windows WASAPI"), 2)
        .unwrap_err();
    // 3.x deliberately fixes the trailing-space typo in the 2.x message.
    assert_eq!(
        e.message,
        "No device found with name \"Speak\" and host API \"WASAPI\"."
    );
    let (_, output) =
        devices::resolve_devices(&backend, None, Some("Speakers"), Some("Windows WASAPI"), 9)
            .unwrap();
    assert_eq!(output.max_output_channels, 8);
    assert_eq!(
        devices::resolve_devices(&backend, None, None, Some("ASIO"), 2)
            .unwrap_err()
            .code,
        impulcifer_types::ipc::ErrorCode::InvalidRequest
    );
}
#[test]
#[ignore = "requires the documented Windows CABLE-A output/input pair"]
fn recording_virtual_cable_end_to_end() {
    let temp = Temp::new();
    let backend = impulcifer_audio_io::default_backend();
    let eps = backend.enumerate().unwrap();
    let input = eps
        .iter()
        .find(|e| e.name.contains("CABLE-A Output") && e.max_input_channels >= 2)
        .expect("CABLE-A capture endpoint")
        .name
        .clone();
    let output = eps
        .iter()
        .find(|e| e.name.contains("CABLE-A Input") && e.max_output_channels >= 2)
        .expect("CABLE-A playback endpoint")
        .name
        .clone();
    let v=request::validate(&json!({"record_dir":temp.0,"sweep":{"mode":"default"},"input_device":input,"output_device":output})).unwrap();
    let registry = JobRegistry::new();
    let job = registry
        .start(JobKind::Recording, true, move |ctx| {
            run::run_recording(&v, &*backend, ctx)
        })
        .unwrap();
    let p = wait(&registry, &job.job_id);
    assert_eq!(json!(p.job.status), "succeeded", "{:?}", p.job.error);
    let wav = read_wav(&temp.0.join("FL,FR.wav")).unwrap();
    let expected = sweep::build_sweep_playback(&sweep::SweepSpec::default()).unwrap();
    assert_eq!(wav.tracks[0].len(), expected.tracks[0].len());
    assert_eq!(wav.tracks.len(), 2);
    let svc = service(&temp, impulcifer_audio_io::default_backend());
    let detection = svc.call("detect_sweep", vec![json!(temp.0)]);
    assert_eq!(detection["data"]["found"], true);
    assert_eq!(detection["data"]["is_default"], true);
    eprintln!(
        "CABLE-A frames={}, summary={}, detection={}",
        wav.tracks[0].len(),
        p.job.result.unwrap()["summary"],
        detection
    );
}

// Same dependency-free SHA-256 helper as impulcifer-io test_support.
fn sha256(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = bytes.to_vec();
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&((bytes.len() as u64) * 8).to_be_bytes());
    for chunk in data.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in chunk.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let x = w[i - 15];
            let y = w[i - 2];
            let s0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let s1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut z] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let t1 = z
                .wrapping_add(s1)
                .wrapping_add((e & f) ^ (!e & g))
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
            z = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (value, add) in h.iter_mut().zip([a, b, c, d, e, f, g, z]) {
            *value = value.wrapping_add(add);
        }
    }
    h.iter().map(|n| format!("{n:08x}")).collect()
}
#[test]
fn recording_sha256_known_answers() {
    assert_eq!(
        sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn recording_job_can_be_cancelled_and_writes_nothing() {
    for append in [false, true] {
        let temp = Temp::new();
        let backend = FakeBackend {
            delay: Duration::from_millis(1500),
            ..Default::default()
        };
        let svc = service(&temp, Box::new(backend));
        let play = temp.0.join("mono.wav");
        write_wav(&play, 8000, &[vec![0.25; 8000]], 32).unwrap();
        let record_dir = temp.0.join("out");
        let record_path = record_dir.join("headphones.wav");
        let original = b"existing recording".to_vec();
        if append {
            std::fs::create_dir_all(&record_dir).unwrap();
            std::fs::write(&record_path, &original).unwrap();
        }
        let payload = json!({"record_dir":record_dir,"play_path":play,"mode":"headphones","confirm_warnings":true,"append":append});
        let started = svc.call("start_recording", vec![payload]);
        assert_eq!(started["ok"], true, "{started}");
        assert_eq!(started["data"]["job"]["cancellable"], true);
        let id = started["data"]["job"]["job_id"].clone();
        let deadline = Instant::now() + Duration::from_secs(10);
        while svc.call("poll_job", vec![id.clone()])["data"]["events"]
            .as_array()
            .unwrap()
            .iter()
            .all(|event| event["payload"]["phase"] != "recording")
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(svc.call("cancel_job", vec![id.clone()])["ok"], true);
        let poll = loop {
            let poll = svc.call("poll_job", vec![id.clone()]);
            if matches!(
                poll["data"]["job"]["status"].as_str(),
                Some("succeeded" | "failed" | "cancelled")
            ) {
                break poll;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(poll["data"]["job"]["status"], "cancelled", "{poll}");
        assert!(
            poll["data"]["events"]
                .as_array()
                .unwrap()
                .iter()
                .any(|event| event["payload"]["phase"] == "cancelled")
        );
        if append {
            assert_eq!(std::fs::read(record_path).unwrap(), original);
        } else {
            assert!(!record_path.exists());
        }
        let boot = svc.call("bootstrap", vec![]);
        assert_eq!(boot["data"]["capabilities"]["recording_cancel"], true);
    }
}

#[test]
fn headphone_recordings_never_replace_an_earlier_one() {
    let temp = Temp::new();
    let svc = service(&temp, Box::new(FakeBackend::default()));
    let dir = temp.0.join("session");
    let preview = || {
        let response = svc.call(
            "resolve_recording_paths",
            vec![json!(dir), Value::Null, json!("headphones")],
        );
        PathBuf::from(response["data"]["record_path"].as_str().unwrap())
    };
    // A folder that does not exist yet, then an empty one: the original name.
    assert_eq!(preview(), dir.join("headphones.wav"));
    std::fs::create_dir(&dir).unwrap();
    assert_eq!(preview(), dir.join("headphones.wav"));
    let earlier = [
        ("headphones.wav", b"first take".as_slice()),
        ("headphones_n.wav", b"second take".as_slice()),
    ];
    for (name, bytes) in earlier {
        assert_eq!(preview(), dir.join(name));
        std::fs::write(dir.join(name), bytes).unwrap();
    }
    // The third take adds one more n, in the preview and in the job.
    let third = dir.join("headphones_nn.wav");
    assert_eq!(preview(), third);
    let sweep = json!({"mode":"custom","fs":8000,"duration":1});
    let v =
        request::validate(&json!({"record_dir":dir,"mode":"headphones","sweep":sweep})).unwrap();
    assert_eq!(PathBuf::from(&v.record_path), third);
    let p = start(v, FakeBackend::default());
    assert_eq!(json!(p.job.status), "succeeded", "{:?}", p.job.error);
    assert_eq!(
        p.job.result.unwrap()["record_path"],
        json!(third.to_string_lossy())
    );
    assert_eq!(read_wav(&third).unwrap().tracks.len(), 2);
    for (name, bytes) in earlier {
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), bytes, "{name}");
    }
    assert_eq!(preview(), dir.join("headphones_nnn.wav"));
}

/// A recording started through the service, polled until it has finished.
fn record_through(svc: &ImpulciferService, request: Value) -> Value {
    let started = svc.call("start_recording", vec![request]);
    assert_eq!(started["ok"], true, "{started}");
    let id = started["data"]["job"]["job_id"].clone();
    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let poll = svc.call("poll_job", vec![id.clone()]);
        if matches!(
            poll["data"]["job"]["status"].as_str(),
            Some("succeeded" | "failed" | "cancelled")
        ) {
            assert_eq!(poll["data"]["job"]["status"], "succeeded", "{poll}");
            return poll["data"].clone();
        }
        assert!(Instant::now() < deadline, "recording timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_session_records_into_a_subfolder_instead_of_replacing_an_earlier_recording() {
    let temp = Temp::new();
    let svc = service(&temp, Box::new(FakeBackend::default()));
    let dir = temp.0.join("measurement");
    std::fs::create_dir(&dir).unwrap();
    for name in ["FL,FR.wav", "headphones.wav"] {
        std::fs::write(dir.join(name), b"earlier").unwrap();
    }
    let sweep = json!({"mode":"custom","fs":8000,"duration":1,"speakers":"FL,FR"});
    let preview = |svc: &ImpulciferService, mode: &str| {
        let response = svc.call(
            "resolve_recording_paths",
            vec![json!(dir), Value::Null, json!(mode), sweep.clone()],
        );
        PathBuf::from(response["data"]["record_path"].as_str().unwrap())
    };
    let notices = |data: &Value| -> Vec<Value> {
        data["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["payload"]["key"] == "recording_session_folder")
            .map(|event| event["payload"].clone())
            .collect()
    };
    let sub = dir.join("n");
    assert_eq!(preview(&svc, "speakers"), sub.join("FL,FR.wav"));
    // Until a speaker recording moves the session, headphones follow the
    // folder's own names.
    assert_eq!(preview(&svc, "headphones"), dir.join("headphones_n.wav"));

    let first = record_through(&svc, json!({"record_dir":dir,"sweep":sweep}));
    assert_eq!(
        first["job"]["result"]["record_path"],
        json!(sub.join("FL,FR.wav"))
    );
    assert_eq!(first["job"]["result"]["record_dir"], json!(dir));
    let notice = notices(&first);
    assert_eq!(notice.len(), 1, "{first}");
    assert_eq!(notice[0]["folder"], "n");
    assert_eq!(notice[0]["level"], "info");

    // Recording the step again replaces the session's own take, quietly.
    let again = record_through(&svc, json!({"record_dir":dir,"sweep":sweep}));
    assert_eq!(
        again["job"]["result"]["record_path"],
        json!(sub.join("FL,FR.wav"))
    );
    assert!(notices(&again).is_empty(), "{again}");

    // The session's headphones go with its speakers.
    assert_eq!(preview(&svc, "headphones"), sub.join("headphones.wav"));
    let headphones = record_through(
        &svc,
        json!({"record_dir":dir,"mode":"headphones","sweep":{"mode":"custom","fs":8000,"duration":1}}),
    );
    assert_eq!(
        headphones["job"]["result"]["record_path"],
        json!(sub.join("headphones.wav"))
    );
    assert_eq!(read_wav(&sub.join("FL,FR.wav")).unwrap().tracks.len(), 2);
    for name in ["FL,FR.wav", "headphones.wav"] {
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), b"earlier", "{name}");
    }

    // A new service is a new run, and n now holds the last one.
    let next = service(&temp, Box::new(FakeBackend::default()));
    assert_eq!(preview(&next, "speakers"), dir.join("nn").join("FL,FR.wav"));
}

#[test]
fn recording_generated_headphones_have_no_sidecar_and_speaker_mono_stays_mono() {
    for headphones in [false, true] {
        let temp = Temp::new();
        let backend = FakeBackend::default();
        let shared = backend.shared.clone();
        let v=request::validate(&json!({"record_dir":temp.0,"mode":if headphones {"headphones"} else {"speakers"},"sweep":{"mode":"custom","fs":8000,"duration":1,"speakers":"FR","tracks":"mono"}})).unwrap();
        let p = start(v, backend);
        assert_eq!(json!(p.job.status), "succeeded", "{:?}", p.job.error);
        let result = p.job.result.unwrap();
        if headphones {
            assert!(result["sidecar_path"].is_null());
            assert!(
                result["record_path"]
                    .as_str()
                    .unwrap()
                    .ends_with("headphones.wav")
            );
            assert_eq!(shared.lock().unwrap().playback.as_ref().unwrap().1, 2);
        } else {
            // Validation uses the requested FR name; the mono playback itself
            // forces FL, exactly like the Python service/build split.
            assert!(result["record_path"].as_str().unwrap().ends_with("FR.wav"));
            assert!(
                result["sweep"]
                    .as_str()
                    .unwrap()
                    .contains("sweep-seg-FL-mono")
            );
            assert!(temp.0.join("test.wav").is_file());
            assert_eq!(shared.lock().unwrap().playback.as_ref().unwrap().1, 1);
        }
    }
}
