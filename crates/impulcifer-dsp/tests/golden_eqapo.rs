#![forbid(unsafe_code)]
//! Python core/eqapo.py:168-941 corpus; p12_*.
use impulcifer_dsp::stages::eqapo::{EqApoLoader, looks_like_eqapo_config, parse_eqapo_config};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Python exporter save (core/eqapo.py:565-590); p12_manifest.
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens")
}
/// Python exporter JSON encoding (core/eqapo.py:565-590); p12_*.
fn fixture(name: &str) -> Value {
    serde_json::from_slice(&fs::read(root().join(name)).unwrap()).unwrap()
}
/// Python array serialization (core/eqapo.py:579-590); p12_*.
fn array(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}
/// Python file inputs (core/eqapo.py:593-600,860-874); p12_files_*.
struct Loader<'a>(&'a Value);
impl EqApoLoader for Loader<'_> {
    /// Python _read_include_file (593-600); p12_files_include_*.
    fn read_text(&mut self, path: &Path) -> Result<String, String> {
        self.0["texts"][path.to_string_lossy().replace('\\', "/")]
            .as_str()
            .map(str::to_owned)
            .ok_or("missing text".into())
    }
    /// Python soundfile.read (860-874); p12_files_convolution_*.
    fn read_wav(&mut self, path: &Path) -> Result<(u32, Vec<Vec<f64>>), String> {
        let wav = &self.0["wavs"][path.to_string_lossy().replace('\\', "/")];
        Ok((
            wav["fs"].as_u64().ok_or("missing WAV")? as u32,
            wav["tracks"]
                .as_array()
                .ok_or("missing tracks")?
                .iter()
                .map(array)
                .collect(),
        ))
    }
}
/// Python parse_eqapo_config (565-590); every p12_* numeric array and exact report.
fn compare(name: &str) {
    let v = fixture(name);
    let frequency = array(&v["frequency"]);
    let actual = parse_eqapo_config(
        v["text"].as_str().unwrap(),
        v["fs"].as_u64().unwrap() as u32,
        &frequency,
        v["base_dir"].as_str().map(Path::new),
        &mut Loader(&v),
    )
    .unwrap();
    let expected = &v["expected"];
    let mut max = 0.0_f64;
    for (field, actual) in [("left_db", &actual.left_db), ("right_db", &actual.right_db)] {
        let expected = array(&expected[field]);
        assert_eq!(actual.len(), expected.len());
        for (i, (a, b)) in actual.iter().zip(expected).enumerate() {
            let error = (a - b).abs();
            max = max.max(error);
            assert!(
                error <= 1e-9 + 1e-11 * b.abs(),
                "{name} {field}[{i}]: actual={a:.17e} expected={b:.17e} error={error:.17e}"
            );
        }
    }
    assert_eq!(
        actual.applied_left,
        expected["applied_left"].as_u64().unwrap() as usize,
        "{name}"
    );
    assert_eq!(
        actual.applied_right,
        expected["applied_right"].as_u64().unwrap() as usize,
        "{name}"
    );
    assert_eq!(
        actual.preamp_left,
        expected["preamp_left"].as_f64().unwrap(),
        "{name}"
    );
    assert_eq!(
        actual.preamp_right,
        expected["preamp_right"].as_f64().unwrap(),
        "{name}"
    );
    assert_eq!(
        actual.channel_split(),
        expected["channel_split"].as_bool().unwrap(),
        "{name}"
    );
    assert_eq!(
        serde_json::to_value(&actual.applied).unwrap(),
        expected["applied"],
        "{name}"
    );
    assert_eq!(
        serde_json::to_value(&actual.bypassed).unwrap(),
        expected["bypassed"],
        "{name}"
    );
    assert_eq!(
        serde_json::to_value(&actual.skipped).unwrap(),
        expected["skipped"],
        "{name}"
    );
    println!("{} | {max:.17e} | exact", v["name"].as_str().unwrap());
}
/// Python parse_eqapo_config (565-590); p12_biquad_*, p12_condition_*, p12_reports.
#[test]
fn golden_eqapo_corpus_matches_python() {
    for name in fixture("p12_manifest.json")["files"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        if !name.starts_with("p12_files_") && name != "p12_detection.json" {
            compare(name);
        }
    }
}
/// Python looks_like_eqapo_config (168-189); p12_detection.
#[test]
fn golden_looks_like_eqapo_matches_python() {
    let cases = fixture("p12_detection.json");
    for case in cases.as_array().unwrap() {
        assert_eq!(
            looks_like_eqapo_config(case["text"].as_str().unwrap()),
            case["expected"].as_bool().unwrap(),
            "{case}"
        );
    }
    println!(
        "detection | 0 | exact ({} cases)",
        cases.as_array().unwrap().len()
    );
}
/// Python Include/Convolution handlers (827-940); p12_files_*.
/// Isolate environment expansion in a child test process instead of mutating a
/// multithreaded test runner's environment.
#[test]
fn golden_eqapo_include_and_convolution_match_python() {
    if std::env::var("P12_EQAPO_IR").as_deref() != Ok("mono.wav") {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "golden_eqapo_include_and_convolution_match_python",
                "--nocapture",
            ])
            .env("P12_EQAPO_IR", "mono.wav")
            .env("P12_EQAPO_CHILD", "environment.txt")
            .output()
            .unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    for name in fixture("p12_manifest.json")["files"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        if name.starts_with("p12_files_") {
            compare(name);
        }
    }
}
