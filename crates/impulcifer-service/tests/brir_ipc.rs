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
