#![forbid(unsafe_code)]
mod brir_support;
#[path = "support/room_measurement.rs"]
mod builder;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
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
        json!([{"index":1,"name":"Left","subwoofer":false,"slot":"FL"},{"index":2,"name":"Right","subwoofer":false,"slot":"FR"},{"index":3,"name":"Subwoofer","subwoofer":true,"slot":null}])
    );
    assert_eq!(d["map"], "FL=1@1,FR=2@1");
    assert_eq!(d["auto"], true);
    let ids: Vec<_> = d["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["1@1", "2@1", "3@1", "1@2", "2@2", "3@2"]);
}
