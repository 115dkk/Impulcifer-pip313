#![forbid(unsafe_code)]

use std::path::Path;
use std::process::Command;

#[test]
fn bridge_script_contract() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("node")
        .arg(app.join("tests/bridge-contract.cjs"))
        .arg(app.join("src/bridge.js"))
        .output()
        .expect("bridge contract requires the existing Node.js runtime (no npm packages)");
    assert!(
        output.status.success(),
        "bridge contract failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn startup_observer_contract() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("node")
        .arg(app.join("tests/observer-contract.cjs"))
        .arg(app.join("src/smoke-observer.js"))
        .output()
        .expect("observer contract requires the existing Node.js runtime");
    assert!(
        output.status.success(),
        "observer contract failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn in_app_driver_failure_contract() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("node")
        .arg(app.join("tests/driver-contract.cjs"))
        .arg(app.join("../../tests/app_smoke/driver.js"))
        .output()
        .expect("driver contract requires Node.js");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn smoke_report_contract() {
    let source = include_str!("../src/smoke.rs");
    let gate = source
        .find("std::env::var(\"IMPULCIFER_APP_SMOKE\").as_deref() != Ok(\"1\")")
        .unwrap();
    let return_none = source[gate..].find("return Ok(None)").unwrap() + gate;
    let read_driver = source
        .find("std::env::var_os(\"IMPULCIFER_APP_SMOKE_DRIVER\")")
        .unwrap();
    assert!(
        return_none < read_driver,
        "production must not read a driver or sink"
    );
    let main = include_str!("../src/main.rs");
    assert_eq!(main.matches("#[tauri::command]").count(), 1);
    assert!(main.contains("smoke::dispatch(smoke.as_deref()"));
    // Behavioral forwarding, concurrent NDJSON, invalid args and sink failure
    // checks live beside the actual helper in smoke::tests::smoke_report_contract.
}

#[test]
fn updater_plugin_registered_with_public_key() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let updater = &config["plugins"]["updater"];
    let public = updater["pubkey"].as_str().unwrap();
    assert!(public.len() > 80);
    assert!(!public.contains("secret"));
    assert_eq!(
        updater["endpoints"],
        serde_json::json!([
            "https://github.com/115dkk/Impulcifer-pip313/releases/latest/download/latest.json"
        ])
    );
    let source = include_str!("../src/main.rs").replace("\r\n", "\n");
    assert!(source.contains("#[cfg(not(windows))]\n    let builder = builder.plugin("));
    assert!(source.contains("tauri_plugin_updater::Builder::new()"));
    assert!(source.contains(".endpoints("));
    assert!(source.contains("#[cfg(windows)]\n    velopack::VelopackApp::build().run()"));
    let permissions: serde_json::Value =
        serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
    assert!(
        permissions["permissions"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("updater:default"))
    );
}

/// Local M3 integration check, like recording.hardware: requires Windows,
/// installed WebView2, Python 3.14 and an already built release app. The
/// harness drives the frozen UI from an injected script (in-app path).
#[cfg(windows)]
fn run_smoke(hardware: bool) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let mut command = Command::new("py");
    command
        .arg("-3.14")
        .arg(root.join("tests/app_smoke/smoke.py"))
        .arg("--exe")
        .arg(root.join("target/release/impulcifer-app.exe"));
    if hardware {
        command.arg("--hardware");
    }
    let status = command
        .env("PYTHONIOENCODING", "utf-8")
        .current_dir(&root)
        .status()
        .expect("launch Python 3.14 app smoke harness");
    assert!(status.success(), "in-app smoke failed: {status}");
}

/// BRIR, recovery, settings and the updates envelope through the real UI.
#[cfg(windows)]
#[test]
#[ignore = "local M3: release app + WebView2 + Python 3.14; see docs/rust/APP-SMOKE.md"]
fn app_smoke_windows() {
    run_smoke(false);
}

/// The same plus the CABLE-A headphone recording of docs/rust/HARDWARE.md.
#[cfg(windows)]
#[test]
#[ignore = "local M3 hardware: CABLE-A virtual cable pair; see docs/rust/APP-SMOKE.md"]
fn app_smoke_hardware_windows() {
    run_smoke(true);
}
