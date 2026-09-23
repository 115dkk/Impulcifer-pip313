#![forbid(unsafe_code)]
//! Custom EQ slots: `inspect_eq` reports what each slot resolves to and which
//! file feeds which ear; `start_brir` reads a chosen file in place.
mod brir_support;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_types::job::JobStatus;
use serde_json::{Value, json};

const SPLIT_APO: &str = "Preamp: -6 dB\nChannel: L\nFilter 1: ON PK Fc 1000 Hz Gain 6.0 dB Q 1.00\nChannel: R\nFilter 1: ON PK Fc 1000 Hz Gain -4.0 dB Q 1.00\nFilter 2: ON XYZ Fc 50 Hz\n";

fn inspect(service: &impulcifer_service::ImpulciferService, request: Value) -> Value {
    let response = service.call("inspect_eq", vec![request]);
    assert_eq!(response["ok"], true, "{response}");
    response["data"].clone()
}

/// Value of a preview curve at the grid point nearest to `f`.
fn at(data: &Value, ear: &str, f: f64) -> f64 {
    let grid = data["curves"]["frequency"].as_array().unwrap();
    let index = grid
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1.as_f64().unwrap() - f)
                .abs()
                .total_cmp(&(b.1.as_f64().unwrap() - f).abs())
        })
        .unwrap()
        .0;
    data["curves"][ear][index].as_f64().unwrap()
}

#[test]
fn inspect_eq_reports_a_channel_split_equalizer_apo_config_from_the_folder() {
    let temp = Temp::new();
    let service = service(&temp, JobRegistry::new());
    std::fs::write(temp.0.join("eq.txt"), SPLIT_APO).unwrap();
    let data = inspect(&service, json!({"dir_path":temp.0}));
    assert_eq!(data["blocked"], false);
    let both = &data["slots"][0];
    assert_eq!(both["slot"], "both");
    assert_eq!(both["source"], "folder");
    assert_eq!(both["name"], "eq.txt");
    assert_eq!(both["format"], "eqapo");
    assert_eq!(both["channels"], "split");
    assert_eq!(both["eqapo"]["bypassed"], 1, "{both}");
    assert_eq!(both["eqapo"]["preamp_db"], json!([-6.0, -6.0]));
    for slot in [&data["slots"][1], &data["slots"][2]] {
        assert_eq!(slot["source"], "folder");
        assert!(slot["path"].is_null());
    }
    assert_eq!(
        data["ears"]["left"],
        json!({"slot":"both","channel":"left"})
    );
    assert_eq!(
        data["ears"]["right"],
        json!({"slot":"both","channel":"right"})
    );
    // Preamp included: +6 - 6 = 0 dB on the left, -4 - 6 = -10 dB on the right at 1 kHz.
    assert!((at(&data, "left", 1000.0) - 0.0).abs() < 0.2, "{data}");
    assert!((at(&data, "right", 1000.0) + 10.0).abs() < 0.2, "{data}");
    assert!((at(&data, "left", 100.0) + 6.0).abs() < 0.2, "{data}");
}

#[test]
fn inspect_eq_prefers_the_csv_and_names_the_ignored_txt() {
    let temp = Temp::new();
    let service = service(&temp, JobRegistry::new());
    std::fs::write(
        temp.0.join("eq.csv"),
        "frequency,raw\n20.0,3.0\n20000.0,3.0\n",
    )
    .unwrap();
    std::fs::write(temp.0.join("eq.txt"), SPLIT_APO).unwrap();
    let data = inspect(&service, json!({"dir_path":temp.0}));
    assert_eq!(data["slots"][0]["name"], "eq.csv");
    assert_eq!(data["slots"][0]["format"], "csv", "{data}");
    assert_eq!(data["slots"][0]["channels"], "both");
    assert_eq!(data["slots"][0]["ignored"], json!(["eq.txt"]));
    assert_eq!(
        data["ears"]["left"],
        json!({"slot":"both","channel":"both"})
    );
    assert!((at(&data, "left", 1000.0) - 3.0).abs() < 0.05, "{data}");
    assert!((at(&data, "right", 1000.0) - 3.0).abs() < 0.05, "{data}");
}

#[test]
fn inspect_eq_applies_chosen_files_and_switched_off_slots() {
    let temp = Temp::new();
    let elsewhere = Temp::new();
    let service = service(&temp, JobRegistry::new());
    std::fs::write(
        temp.0.join("eq.csv"),
        "frequency,raw\n20.0,3.0\n20000.0,3.0\n",
    )
    .unwrap();
    std::fs::write(
        temp.0.join("eq-right.csv"),
        "frequency,raw\n20.0,-2.0\n20000.0,-2.0\n",
    )
    .unwrap();
    let chosen = elsewhere.0.join("my apo.txt");
    std::fs::write(&chosen, SPLIT_APO).unwrap();
    let data = inspect(
        &service,
        json!({"dir_path":temp.0,"eq_file":false,"eq_left_file":chosen}),
    );
    assert_eq!(data["slots"][0]["source"], "off");
    assert!(data["slots"][0]["name"].is_null());
    assert_eq!(data["slots"][1]["source"], "file");
    assert_eq!(data["slots"][1]["name"], "my apo.txt");
    assert_eq!(data["slots"][2]["source"], "folder");
    assert_eq!(data["slots"][2]["name"], "eq-right.csv");
    // eq-left takes the left channel of a split config; eq-right is a plain curve.
    assert_eq!(
        data["ears"]["left"],
        json!({"slot":"left","channel":"left"})
    );
    assert_eq!(
        data["ears"]["right"],
        json!({"slot":"right","channel":"both"})
    );
    assert!((at(&data, "left", 1000.0) - 0.0).abs() < 0.2, "{data}");
    assert!((at(&data, "right", 1000.0) + 2.0).abs() < 0.05, "{data}");
    // Every slot off: nothing feeds either ear.
    let off = inspect(
        &service,
        json!({"dir_path":temp.0,"eq_file":false,"eq_left_file":false,"eq_right_file":false}),
    );
    assert!(off["ears"]["left"].is_null() && off["ears"]["right"].is_null());
    assert!(off["curves"]["left"].is_null() && off["curves"]["right"].is_null());
}

#[test]
fn inspect_eq_reports_unreadable_files_per_slot_and_rejects_bad_requests() {
    let temp = Temp::new();
    let service = service(&temp, JobRegistry::new());
    std::fs::write(temp.0.join("eq-left.csv"), "not,a\ncurve,at all\n").unwrap();
    let data = inspect(&service, json!({"dir_path":temp.0}));
    assert_eq!(data["slots"][1]["name"], "eq-left.csv");
    assert!(data["slots"][1]["error"].is_string(), "{data}");
    assert_eq!(data["blocked"], true);
    assert!(data["ears"]["left"].is_null() && data["curves"]["left"].is_null());
    for (request, code) in [
        (json!({"dir_path":temp.0.join("missing")}), "FILE_NOT_FOUND"),
        (
            json!({"dir_path":temp.0,"eq_file":"nope.txt"}),
            "FILE_NOT_FOUND",
        ),
        (json!({"dir_path":temp.0,"eq_file":true}), "INVALID_REQUEST"),
        (json!({"dir_path":temp.0,"eq_file":5}), "INVALID_REQUEST"),
    ] {
        assert_eq!(
            service.call("inspect_eq", vec![request])["error"]["code"],
            code
        );
    }
}

#[test]
fn start_brir_reads_a_chosen_eq_file_in_place_without_copying_it() {
    let temp = Temp::demo();
    let elsewhere = Temp::new();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let chosen = elsewhere.0.join("preset.txt");
    std::fs::write(&chosen, SPLIT_APO).unwrap();
    let before: Vec<_> = std::fs::read_dir(&temp.0)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    let result = service.call(
        "start_brir",
        vec![json!({"dir_path":temp.0,"eq_file":chosen})],
    );
    assert_eq!(result["ok"], true, "{result}");
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    let logged: Vec<_> = poll
        .events
        .iter()
        .filter_map(|e| e.payload["key"].as_str())
        .collect();
    assert!(logged.contains(&"cli_eqapo_detected"), "{logged:?}");
    assert!(logged.contains(&"cli_eqapo_channel_split"), "{logged:?}");
    for name in ["eq.csv", "eq.txt", "eq-left.csv", "eq-right.csv"] {
        assert!(
            !temp.0.join(name).exists() || before.iter().any(|b| b == name),
            "{name} was written into the measurement folder"
        );
    }
    assert!(chosen.is_file());
}
