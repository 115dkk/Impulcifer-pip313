#![forbid(unsafe_code)]
mod brir_support;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_types::job::{JobEventKind, JobStatus};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[test]
fn start_brir_rejects_missing_dir() {
    let temp = Temp::new();
    let service = service(&temp, JobRegistry::new());
    let result = service.call(
        "start_brir",
        vec![json!({"dir_path":temp.0.join("missing")})],
    );
    assert_eq!(result["error"]["code"], "FILE_NOT_FOUND");
}
#[test]
fn start_brir_emits_progress_with_cli_keys_and_total_eleven() {
    let temp = Temp::demo();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let result = service.call("start_brir", vec![json!({"dir_path":temp.0})]);
    assert_eq!(result["ok"], true, "{result}");
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    let progress: Vec<_> = poll
        .events
        .iter()
        .filter(|e| e.kind == JobEventKind::Progress)
        .collect();
    let keys: Vec<_> = progress
        .iter()
        .map(|e| e.payload["key"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        [
            "cli_creating_estimator",
            "cli_running_room_correction",
            "cli_running_headphone_compensation",
            "cli_creating_equalization",
            "cli_creating_target",
            "cli_opening_measurements",
            "cli_cropping_responses",
            "cli_equalizing",
            "cli_normalizing_gain",
            "cli_plotting_results",
            "cli_writing_brirs"
        ]
    );
    for (i, e) in progress.iter().enumerate() {
        let previous = poll.events.iter().find(|p| p.seq + 1 == e.seq).unwrap();
        assert_eq!(previous.kind, JobEventKind::Log);
        assert_eq!(previous.payload["level"], "PROGRESS");
        assert_eq!(previous.payload["key"], e.payload["key"]);
        assert_eq!(previous.payload["message"], e.payload["message"]);
        assert_eq!(
            e.payload["progress"],
            json!((((i + 1) as f64 / 11.0) * 100.0).trunc() / 100.0)
        );
    }
}

#[test]
fn external_brir_without_folder_recordings_runs_without_sweep_fallback_warning() {
    let temp = Temp::new();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let external = root().join("crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat");
    let result = service.call(
        "start_brir",
        vec![json!({
            "dir_path":temp.0,
            "brir_measurement_file":external,
            "brir_measurement_map":"FL.left=1,FL.right=2",
            "do_room_correction":false,
            "do_headphone_compensation":false,
            "do_equalization":false
        })],
    );
    assert_eq!(result["ok"], true, "{result}");
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    assert!(temp.0.join("hesuvi.wav").is_file());
    assert!(
        !poll
            .events
            .iter()
            .any(|event| { event.payload["key"] == "cli_warning_sweep_detect_fallback" })
    );
}

#[test]
fn start_brir_forwards_recording_file_fields_and_names_validation_fields() {
    let temp = Temp::new();
    let recording = temp.0.join("arbitrary.wav");
    std::fs::copy(root().join("data/demo/FL,FR.wav"), &recording).unwrap();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let missing_sweep = temp.0.join("missing-sweep.wav");
    let result = service.call(
        "start_brir",
        vec![json!({
            "dir_path":temp.0,
            "brir_recording_file":recording,
            "brir_recording_speakers":"FL,FR",
            "brir_recording_sweep":missing_sweep,
            "do_room_correction":false,
            "do_headphone_compensation":false,
            "do_equalization":false
        })],
    );
    assert_eq!(result["ok"], true, "{result}");
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(poll.job.status, JobStatus::Failed);
    let error = poll.job.error.unwrap();
    assert_eq!(error["code"], "FILE_NOT_FOUND");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("missing-sweep.wav")
    );

    let cases = [
        (
            json!({"dir_path":temp.0,"brir_recording_file":recording}),
            "brir_recording_speakers",
        ),
        (
            json!({"dir_path":temp.0,"brir_recording_speakers":"FL"}),
            "brir_recording_speakers",
        ),
        (
            json!({"dir_path":temp.0,"brir_recording_sweep":"sweep.wav"}),
            "brir_recording_sweep",
        ),
        (
            json!({"dir_path":temp.0,"brir_recording_file":recording,"brir_recording_speakers":"FL","brir_measurement_file":"project.mdat"}),
            "brir_recording_file",
        ),
        (
            json!({"dir_path":temp.0,"brir_recording_file":recording,"brir_recording_speakers":"FL,unknown"}),
            "brir_recording_speakers",
        ),
    ];
    for (request, field) in cases {
        let result = service.call("start_brir", vec![request]);
        assert_eq!(result["error"]["code"], "INVALID_REQUEST", "{result}");
        assert_eq!(result["error"]["details"]["field"], field, "{result}");
    }
}

#[test]
fn start_brir_cancel_mid_run_reports_cancelled() {
    let temp = Temp::demo();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let result = service.call("start_brir", vec![json!({"dir_path":temp.0})]);
    let id = result["data"]["job"]["job_id"].as_str().unwrap();
    let start = Instant::now();
    loop {
        let p = jobs.poll(id, 0).unwrap();
        if p.events
            .iter()
            .any(|e| e.payload["key"] == "cli_running_headphone_compensation")
        {
            break;
        }
        assert!(!p.job.status.is_terminal());
        assert!(start.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(service.call("cancel_job", vec![json!(id)])["ok"], true);
    assert_eq!(wait(&jobs, id).job.status, JobStatus::Cancelled);
    assert!(!temp.0.join("hesuvi.wav").exists());
}
#[test]
fn start_brir_output_missing_when_write_fails() {
    // A directory at the final filename is a deterministic cross-platform write failure.
    // Unlike the packet's claim, Python propagates this as INTERNAL_ERROR, not OUTPUT_MISSING.
    let temp = Temp::demo();
    std::fs::create_dir(temp.0.join("hesuvi.wav")).unwrap();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let result = service.call("start_brir", vec![json!({"dir_path":temp.0})]);
    let p = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(p.job.status, JobStatus::Failed);
    assert_eq!(p.job.error.unwrap()["code"], "INTERNAL_ERROR");
    let error =
        impulcifer_service::brir::run::ensure_output(&temp.0.join("absent.wav")).unwrap_err();
    assert_eq!(error.error["code"], "OUTPUT_MISSING");
}
#[cfg(unix)]
#[test]
fn start_brir_readonly_directory_reports_internal_error() {
    use std::os::unix::fs::PermissionsExt;
    let temp = Temp::demo();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let original = std::fs::metadata(&temp.0).unwrap().permissions();
    struct Restore(std::path::PathBuf, std::fs::Permissions);
    impl Drop for Restore {
        fn drop(&mut self) {
            std::fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }
    let _restore = Restore(temp.0.clone(), original);
    std::fs::set_permissions(&temp.0, std::fs::Permissions::from_mode(0o555)).unwrap();
    // Elevated users may bypass mode bits. Never claim permission denial in that case.
    let probe = temp.0.join("permission-probe");
    if std::fs::File::create(&probe).is_ok() {
        println!("SKIP readonly directory: this account bypasses mode bits");
        return;
    }
    let result = service.call("start_brir", vec![json!({"dir_path":temp.0})]);
    assert_eq!(result["ok"], true, "{result}");
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(poll.job.status, JobStatus::Failed);
    assert_eq!(poll.job.error.unwrap()["code"], "INTERNAL_ERROR");
    assert!(!temp.0.join("hesuvi.wav").exists());
}
#[test]
fn detect_sweep_reports_demo_parameters() {
    let temp = Temp::demo();
    let service = service(&temp, JobRegistry::new());
    let result = service.call("detect_sweep", vec![json!(temp.0)]);
    let oracle: Value =
        serde_json::from_slice(&std::fs::read(golden("p11_detection.json")).unwrap()).unwrap();
    assert_eq!(result["ok"], true);
    for key in [
        "fs",
        "n_segments",
        "speakers",
        "confidence",
        "generate_spec",
        "source_files",
    ] {
        assert_eq!(result["data"][key], oracle[key], "{key}");
    }
    assert_eq!(result["data"]["found"], true);
    assert_eq!(result["data"]["is_default"], true);
}
#[test]
fn detect_sweep_checks_a_test_signal_file_against_the_recordings() {
    let temp = Temp::demo();
    let service = service(&temp, JobRegistry::new());
    let check = |check: Value| {
        let result = service.call("detect_sweep", vec![json!(temp.0), check]);
        assert_eq!(result["ok"], true, "{result}");
        result["data"].clone()
    };
    let files = |data: &Value| -> Vec<String> {
        data["test_signal"]["mismatches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| format!("{} {}", m["file"].as_str().unwrap(), m["duration_seconds"]))
            .collect()
    };
    // Without the second argument the answer is the one 2.x gives.
    let plain = service.call("detect_sweep", vec![json!(temp.0)]);
    assert!(plain["data"].get("test_signal").is_none());
    // The sweep the demo was recorded with fits every recording.
    let default = root().join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav");
    let data = check(json!({"test_signal":default,"headphone_file":""}));
    assert_eq!(data["found"], true);
    assert_eq!(data["test_signal"]["readable"], true);
    assert_eq!(data["test_signal"]["fs"], 48000);
    assert_eq!(data["test_signal"]["duration_seconds"], 6.15);
    assert!(files(&data).is_empty(), "{data}");
    // A 12.3 s signal fits none of them: the speaker recordings and the
    // headphone recording the job would read.
    let long = temp.0.join("long-sweep.wav");
    impulcifer_io::write_wav(&long, 48000, &[vec![0.0; 590540]], 16).unwrap();
    let data = check(json!({"test_signal":long,"headphone_file":""}));
    assert_eq!(data["test_signal"]["duration_seconds"], 12.3);
    assert_eq!(
        files(&data),
        [
            "BL,SL.wav 6.15",
            "FC.wav 6.15",
            "FL,FR.wav 6.15",
            "SR,BR.wav 6.15",
            "headphones.wav 6.15"
        ]
    );
    // The headphone recording is left out when the job reads none, the
    // speaker recordings when the BRIR measurement comes from a file.
    let data = check(json!({"test_signal":long}));
    assert_eq!(files(&data).len(), 4);
    let data = check(json!({"test_signal":long,"speakers":false,"headphone_file":null}));
    assert!(files(&data).is_empty());
    // A named headphone recording replaces the folder's own.
    let other = temp.0.join("other.wav");
    impulcifer_io::write_wav(
        &other,
        48000,
        &vec![vec![0.0; 6 * 48000 + 2 * 590540]; 2],
        16,
    )
    .unwrap();
    let data = check(json!({"test_signal":long,"speakers":false,"headphone_file":other}));
    assert!(files(&data).is_empty());
    // A file that is no WAV gets no verdict; the job explains it.
    let text = temp.0.join("notes.txt");
    std::fs::write(&text, "not a sweep").unwrap();
    let data = check(json!({"test_signal":text,"headphone_file":""}));
    assert_eq!(data["test_signal"]["readable"], false);
    assert!(files(&data).is_empty());
    for invalid in [
        json!("long-sweep.wav"),
        json!({"test_signal":""}),
        json!({"test_signal":long,"speakers":"yes"}),
        json!({"test_signal":long,"headphone_file":1}),
    ] {
        let result = service.call("detect_sweep", vec![json!(temp.0), invalid]);
        assert_eq!(result["error"]["code"], "INVALID_REQUEST", "{result}");
    }
}
#[test]
fn start_brir_warns_of_a_test_signal_the_recordings_were_not_made_with() {
    let temp = Temp::demo();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let run = |test_signal: &str| {
        let result = service.call(
            "start_brir",
            vec![json!({
                "dir_path":temp.0,
                "test_signal":test_signal,
                "do_room_correction":false,
                "do_equalization":false
            })],
        );
        assert_eq!(result["ok"], true, "{result}");
        wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap())
    };
    let warnings = |poll: &impulcifer_jobs::registry::PollResult| -> Vec<String> {
        poll.events
            .iter()
            .filter(|e| e.payload["key"] == "cli_warning_test_signal_mismatch")
            .map(|e| {
                assert_eq!(e.payload["level"], "WARNING");
                e.payload["message"].as_str().unwrap().to_owned()
            })
            .collect()
    };
    // The demo was recorded with the default sweep; a 12.3 s signal fits no
    // recording. The job says so for every file before it reads them, and
    // then stops at the headphone recording with an error about the signal.
    let poll = run("generate:12.3s@48000");
    let lines = warnings(&poll);
    assert_eq!(lines.len(), 5, "{lines:?}");
    for (line, file) in lines.iter().zip([
        "BL,SL.wav",
        "FC.wav",
        "FL,FR.wav",
        "SR,BR.wav",
        "headphones.wav",
    ]) {
        assert!(
            line.starts_with(file) && line.contains("6.15 s") && line.contains("12.30 s"),
            "{line}"
        );
    }
    assert_eq!(poll.job.status, JobStatus::Failed);
    let error = poll.job.error.unwrap();
    assert_eq!(error["code"], "INVALID_REQUEST");
    assert_eq!(
        error["message"],
        "The test signal does not match the headphone recording."
    );
    assert!(!temp.0.join("headphone-responses.wav").exists());
    // The signal the recordings were made with draws no warning.
    let poll = run("default");
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    assert!(warnings(&poll).is_empty());
}
#[test]
fn recording_file_test_signal_warnings_follow_the_selected_speaker_source() {
    let temp = Temp::new();
    let recording = temp.0.join("outside-name.wav");
    let demo = root().join("data/demo/FL,FR.wav");
    std::fs::copy(&demo, &recording).unwrap();
    std::fs::copy(demo, temp.0.join("FC.wav")).unwrap();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let run = |sweep: Option<&str>| {
        let mut request = json!({
            "dir_path":temp.0,
            "test_signal":"generate:12.3s@48000",
            "brir_recording_file":recording,
            "brir_recording_speakers":"FL,FR",
            "do_room_correction":false,
            "do_headphone_compensation":false,
            "do_equalization":false
        });
        if let Some(sweep) = sweep {
            request["brir_recording_sweep"] = json!(sweep);
        }
        let result = service.call("start_brir", vec![request]);
        assert_eq!(result["ok"], true, "{result}");
        wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap())
    };
    let warnings = |poll: &impulcifer_jobs::registry::PollResult| -> Vec<String> {
        poll.events
            .iter()
            .filter(|event| event.payload["key"] == "cli_warning_test_signal_mismatch")
            .map(|event| event.payload["message"].as_str().unwrap().to_owned())
            .collect()
    };
    let without_sweep = run(None);
    assert_eq!(without_sweep.job.status, JobStatus::Failed);
    let error = without_sweep.job.error.as_ref().unwrap();
    assert_eq!(error["code"], "INVALID_REQUEST");
    assert_eq!(
        error["message"],
        "The test signal does not match the recording file."
    );
    assert_eq!(
        error["details"],
        json!({"path": recording, "read": 1, "listed": 2})
    );
    let lines = warnings(&without_sweep);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("outside-name.wav"), "{}", lines[0]);
    assert!(!lines[0].contains("FC.wav"), "{}", lines[0]);

    let with_sweep = run(Some("missing-sweep.wav"));
    assert!(warnings(&with_sweep).is_empty());
}

#[test]
fn generate_sweep_set_writes_named_files_and_sidecar() {
    let temp = Temp::new();
    let service = service(&temp, JobRegistry::new());
    let result = service.call("generate_sweep_set", vec![json!(temp.0)]);
    assert_eq!(result["ok"], true, "{result}");
    let files = result["data"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 5);
    assert_eq!(result["data"]["play_path"], files[0]);
    let e = impulcifer_dsp::estimator::SweepEstimator::new(5.0, 48000).unwrap();
    for (i, file) in files.iter().enumerate() {
        let path = std::path::Path::new(file.as_str().unwrap());
        let info = impulcifer_io::sweep_files::parse_sweep_file_name(
            path.file_name().unwrap().to_str().unwrap(),
        )
        .unwrap();
        let names: Vec<_> = info.speakers.iter().map(String::as_str).collect();
        let expected =
            impulcifer_io::pcm32_round_trip(&e.sweep_sequence(&names, &info.layout).unwrap());
        let wav = impulcifer_io::read_wav(path).unwrap();
        assert_eq!(wav.sample_rate, 48000);
        assert_eq!(wav.tracks.len(), if i == 4 { 8 } else { 2 });
        assert_eq!(wav.tracks, expected);
    }
    let wav_count = std::fs::read_dir(&temp.0)
        .unwrap()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "wav")
        })
        .count();
    assert_eq!(
        wav_count, 6,
        "five listed playback files plus the explicit P11 sidecar"
    );
    let sidecar = impulcifer_io::read_wav(&temp.0.join("test.wav")).unwrap();
    assert_eq!(
        sidecar.tracks,
        impulcifer_io::pcm32_round_trip(&[e.test_signal])
    );
}
#[test]
fn brir_validation_preserves_seconds_and_rejects_cli_strings() {
    let temp = Temp::new();
    let service = service(&temp, JobRegistry::new());
    for request in [
        json!({"dir_path":temp.0,"decay":"FL:300"}),
        json!({"dir_path":temp.0,"decay":false}),
        json!({"dir_path":temp.0,"plot":1}),
        json!({"dir_path":temp.0,"unexpected":true}),
    ] {
        assert_eq!(
            service.call("start_brir", vec![request])["error"]["code"],
            "INVALID_REQUEST"
        );
    }
}
