#![forbid(unsafe_code)]
mod brir_support;
#[path = "support/room_measurement.rs"]
mod builder;
use brir_support::*;
use impulcifer_io::read_wav;
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_types::{job::JobStatus, layouts::layout};
use serde_json::{Value, json};

fn inspect(request: Value) -> Value {
    let temp = Temp::new();
    service(&temp, JobRegistry::default()).call("inspect_room_measurements", vec![request])
}
fn rew() -> std::path::PathBuf {
    root().join("crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat")
}

#[test]
fn inspect_room_measurements_reports_the_rew_fixture_with_its_automatic_map() {
    let result = inspect(json!({"path":rew(),"map":null}));
    assert_eq!(result["ok"], true, "{result}");
    let d = &result["data"];
    assert_eq!(d["format"], "rew");
    assert_eq!(d["purpose"], "room");
    assert_eq!(d["map"], "FL.left=1,FL.right=2");
    assert_eq!(d["auto"], true);
    assert_eq!(
        d["mapping"],
        json!([{"slot":"FL","side":"left","id":"1"},{"slot":"FL","side":"right","id":"2"}])
    );
    assert_eq!(d["measurements"].as_array().unwrap().len(), 3);
    assert_eq!(d["measurements"][2]["skipped"], "no_impulse_response");
    assert_eq!(d["measurements"][2]["samples"], 0);
    for field in ["title", "microphone"] {
        assert!(d[field].is_null());
    }
    for field in ["positions", "speakers"] {
        assert_eq!(d[field], json!([]));
    }
    for m in d["measurements"].as_array().unwrap() {
        assert!(m["speaker"].is_null());
        assert!(m["position"].is_null());
    }
    assert_eq!(d["mic_calibration"], false);
}
#[test]
fn inspect_room_measurements_applies_a_map_and_rejects_bad_requests() {
    let result = inspect(json!({"path":rew(),"map":"FL=2"}));
    assert_eq!(result["ok"], true);
    assert_eq!(result["data"]["map"], "FL=2");
    assert_eq!(result["data"]["auto"], false);
    assert_eq!(
        result["data"]["mapping"],
        json!([{"slot":"FL","side":"left","id":"2"},{"slot":"FL","side":"right","id":"2"}])
    );
    for request in [
        json!({"path":rew(),"map":"XX=1"}),
        json!({"path":rew(),"map":"FL=3"}),
        json!([]),
        json!({"path":rew(),"map":42}),
        json!({}),
    ] {
        assert_eq!(inspect(request)["error"]["code"], "INVALID_REQUEST");
    }
    let temp = Temp::new();
    assert_eq!(
        inspect(json!({"path":temp.0.join("missing")}))["error"]["code"],
        "FILE_NOT_FOUND"
    );
    std::fs::write(temp.0.join("invalid"), b"invalid").unwrap();
    assert_eq!(
        inspect(json!({"path":temp.0.join("invalid")}))["error"]["code"],
        "INVALID_REQUEST"
    );
}
#[test]
fn inspect_measurements_supports_all_purposes_and_rejects_invalid_purpose() {
    let default = inspect(json!({"path":rew()}));
    assert_eq!(default["data"]["purpose"], "room");
    let brir = inspect(json!({"path":rew(),"purpose":"brir"}));
    assert_eq!(brir["data"]["purpose"], "brir");
    assert_eq!(brir["data"]["map"], "FL.left=1,FL.right=2");
    let headphones = inspect(json!({"path":rew(),"purpose":"headphones"}));
    assert_eq!(headphones["data"]["purpose"], "headphones");
    assert_eq!(headphones["data"]["map"], "left=1,right=2");
    for purpose in [json!("ROOM"), json!(42), json!([])] {
        let result = inspect(json!({"path":rew(),"purpose":purpose}));
        assert_eq!(result["error"]["code"], "INVALID_REQUEST");
        assert_eq!(
            result["error"]["message"],
            "purpose must be room, brir or headphones."
        );
    }
}

#[test]
fn inspect_room_measurements_reports_a_dirac_project() {
    let temp = Temp::new();
    let path = temp.0.join("test.liveproject");
    std::fs::write(&path, builder::dirac_project(44100)).unwrap();
    let result = inspect(json!({"path":path}));
    assert_eq!(result["ok"], true, "{result}");
    let d = &result["data"];
    assert_eq!(d["format"], "dirac");
    assert_eq!(d["title"], "Test room");
    assert_eq!(d["sample_rate"], 44100);
    assert_eq!(d["microphone"], "Test microphone");
    assert_eq!(d["mic_calibration"], true);
    assert_eq!(d["positions"], json!([1, 2]));
    assert_eq!(
        d["speakers"],
        json!([{"index":1,"name":"Left","subwoofer":false,"slot":"FL"},{"index":2,"name":"Right","subwoofer":false,"slot":"FR"},{"index":3,"name":"Subwoofer","subwoofer":true,"slot":"LFE"}])
    );
    assert_eq!(d["map"], "FL=1@1,FR=2@1,LFE=3@1");
    assert_eq!(d["auto"], true);
    let ids: Vec<_> = d["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["1@1", "2@1", "3@1", "1@2", "2@2", "3@2"]);

    let brir = inspect(json!({"path":path,"purpose":"brir"}));
    assert_eq!(brir["ok"], true, "{brir}");
    assert_eq!(brir["data"]["speakers"][2]["slot"], "LFE");
    assert!(
        brir["data"]["map"]
            .as_str()
            .unwrap()
            .ends_with("LFE.left=3@1,LFE.right=3@2")
    );
}

#[test]
fn brir_measurement_file_reads_subwoofers_as_lfe() {
    fn run_case(include_lfe: bool) -> (Temp, impulcifer_jobs::registry::PollResult) {
        let temp = Temp::demo();
        let path = temp.0.join("ears.liveproject");
        std::fs::write(&path, builder::dirac_project(48_000)).unwrap();
        let map = if include_lfe {
            "FL.left=1@1,FL.right=1@2,LFE.left=3@1,LFE.right=3@2"
        } else {
            "FL.left=1@1,FL.right=1@2"
        };
        let jobs = JobRegistry::new();
        let result = service(&temp, jobs.clone()).call(
            "start_brir",
            vec![json!({
                "dir_path":temp.0,
                "brir_measurement_file":path,
                "brir_measurement_map":map,
                "layout_files":"9.1",
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
        (temp, poll)
    }

    let (measured, poll) = run_case(true);
    let format = layout("9.1").unwrap();
    let lfe = format
        .channels
        .iter()
        .position(|channel| channel.slot == "LFE")
        .unwrap();
    let wav = read_wav(&measured.0.join(format.file_name)).unwrap();
    for track in [&wav.tracks[2 * lfe], &wav.tracks[2 * lfe + 1]] {
        assert!(track.iter().any(|sample| *sample != 0.0));
    }
    let lfe_loaded = poll
        .events
        .iter()
        .find(|event| event.payload["key"] == "cli_brir_measurement_file_lfe")
        .unwrap();
    assert_eq!(
        lfe_loaded.payload["message"],
        "Measured subwoofer channels: LFE"
    );
    let lfe_filled = poll
        .events
        .iter()
        .find(|event| event.payload["key"] == "cli_info_lfe_filled")
        .unwrap();
    assert!(
        lfe_filled.payload["message"]
            .as_str()
            .unwrap()
            .contains("the measured subwoofer")
    );
    let readme = std::fs::read_to_string(measured.0.join("README.md")).unwrap();
    assert!(
        readme.contains("The LFE tracks use the measured subwoofer."),
        "{readme}"
    );
    let measured_hesuvi = std::fs::read(measured.0.join("hesuvi.wav")).unwrap();

    let (without, _) = run_case(false);
    assert_eq!(
        measured_hesuvi,
        std::fs::read(without.0.join("hesuvi.wav")).unwrap()
    );
}
