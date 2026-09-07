#![forbid(unsafe_code)]
//! Python core/eqapo.py:168-941 boundary properties; p12_*.
use impulcifer_dsp::stages::{
    eq_files::read_eq_settings,
    eqapo::{EqApoLoader, NoFilesLoader, parse_eqapo_config},
};
use std::path::Path;

/// Python include files (593-600); p12_files_include_cycle.
struct Recursive;
impl EqApoLoader for Recursive {
    /// Python _read_include_file (593-600); p12_files_include_cycle.
    fn read_text(&mut self, _: &Path) -> Result<String, String> {
        Ok("Preamp: -1\nInclude: sub/../cycle.txt".into())
    }
    /// Python missing WAV (860-868); p12_files_missing.
    fn read_wav(&mut self, _: &Path) -> Result<(u32, Vec<Vec<f64>>), String> {
        Err("missing".into())
    }
}
/// Python _handle_include (907-940); p12_files_include_cycle.
#[test]
fn eqapo_rejects_recursive_include() {
    let result = parse_eqapo_config(
        "Include: cycle.txt",
        48000,
        &[100.0],
        Some(Path::new("/eqapo")),
        &mut Recursive,
    )
    .unwrap();
    assert_eq!(result.left_db, [-1.0]);
    assert_eq!(result.bypassed.len(), 1);
    assert_eq!(result.bypassed[0].reason, "include_not_found");
    assert_eq!(result.bypassed[0].line_number, 2);
}
/// Python missing-file/scoping ordering (827-940); p12_files_missing.
#[test]
fn eqapo_reports_unreadable_files_like_python() {
    let result = parse_eqapo_config(
        "Channel: C\nInclude: missing\nConvolution: missing",
        48000,
        &[100.0],
        Some(Path::new("/eqapo")),
        &mut NoFilesLoader,
    )
    .unwrap();
    assert_eq!(
        result
            .bypassed
            .iter()
            .map(|r| r.reason.as_str())
            .collect::<Vec<_>>(),
        ["include_not_found", "convolution_not_found"]
    );
}
/// Python channel_split/Preamp (163-165,785-806); p12_scopes.
#[test]
fn eqapo_channel_split_false_when_scopes_symmetric() {
    for scope in ["ALL", "L R", "1 2", "l,r", ""] {
        let result = parse_eqapo_config(
            &format!("Channel: {scope}\nPreamp: -3"),
            48000,
            &[10.0, 1000.0],
            None,
            &mut NoFilesLoader,
        )
        .unwrap();
        assert!(!result.channel_split());
        assert_eq!(result.left_db, [-3.0, -3.0]);
        assert_eq!(result.applied_left, 0);
        assert!(result.applied.is_empty());
    }
    let mut result = parse_eqapo_config("", 48000, &[10.0], None, &mut NoFilesLoader).unwrap();
    result.left_db[0] = f64::NAN;
    result.right_db[0] = f64::NAN;
    assert!(result.channel_split());
}
/// Python _read_eq_settings (pipeline_stages.py:199-291); p12_scopes/p12_reports.
#[test]
fn eq_files_routes_eqapo_text_to_parser() {
    let text = "Channel: L\nPreamp: -3\nFilter: ON PK Fc 1000 Hz Gain 2 dB Q 1\nChannel: R\nPreamp: -1\nFilter: ON None\nDelay: 1";
    let (left, right, report) =
        read_eq_settings("eq", text, 48000, &[10.0, 1000.0], None, &mut NoFilesLoader).unwrap();
    let right = right.unwrap();
    let report = report.unwrap();
    assert_eq!(left.name, "eq");
    assert_eq!(right.name, "eq (right)");
    assert_eq!(left.error, left.raw.iter().map(|v| -v).collect::<Vec<_>>());
    assert_eq!(
        right.error,
        right.raw.iter().map(|v| -v).collect::<Vec<_>>()
    );
    assert_eq!(report.preamp_left, -3.0);
    assert_eq!(report.preamp_right, -1.0);
    assert_eq!((report.applied_left, report.applied_right), (1, 0));
    assert_eq!(report.applied, ["PK Fc 1000 Hz Gain 2 dB Q 1 [L]"]);
    assert_eq!(report.bypassed[0].text, "Delay: 1");
    assert_eq!(report.skipped_count, 1);
    assert!(report.channel_split);
    let (_, right, report) = read_eq_settings(
        "eq",
        "Preamp: -2",
        48000,
        &[10.0, 1000.0],
        None,
        &mut NoFilesLoader,
    )
    .unwrap();
    assert!(right.is_none());
    assert!(!report.unwrap().channel_split);
    let (left, right, report) = read_eq_settings(
        "csv",
        "frequency,raw,error\n20.0,2.0,3.0\n20000.0,4.0,5.0",
        48000,
        &[10.0, 1000.0],
        None,
        &mut NoFilesLoader,
    )
    .unwrap();
    assert_eq!(left.error, [3.0, 5.0]);
    assert!(right.is_none());
    assert!(report.is_none());
    let (left, _, report) = read_eq_settings(
        "csv",
        "20.0,-3.0\n20000.0,-2.0",
        48000,
        &[10.0, 1000.0],
        None,
        &mut NoFilesLoader,
    )
    .unwrap();
    assert_eq!(left.error, [3.0, 2.0]);
    assert!(report.is_none());
}
/// Python math exceptions (218-303); p12_arithmetic boundary (nonfinite JSON avoided).
#[test]
fn eqapo_invalid_scalar_arithmetic_returns_error() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/migration/goldens/p12_arithmetic.json");
    let cases: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    for case in cases.as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let fs = case["fs"].as_u64().unwrap() as u32;
        assert!(
            parse_eqapo_config(text, fs, &[100.0], None, &mut NoFilesLoader).is_err(),
            "{case}"
        );
    }
    // A malformed filter in an empty scope must not evaluate invalid arithmetic.
    assert!(
        parse_eqapo_config(
            "Channel: C\nFilter: ON PK Fc 1000 Hz Gain 20000 dB Q 1",
            48000,
            &[100.0],
            None,
            &mut NoFilesLoader
        )
        .is_ok()
    );
}
/// Python np.interp rightmost duplicate and str.splitlines (464-473,618); p12_graphic_duplicates/p12_line_endings.
#[test]
fn eqapo_duplicate_nodes_and_line_numbers_match_python() {
    let r = parse_eqapo_config(
        "GraphicEQ: 100 1; 20 -2; 100 4; 1000 3\r\nDelay: 1\rDelay: 2\u{85}Delay: 3",
        48000,
        &[10.0, 20.0, 100.0, 1000.0],
        None,
        &mut NoFilesLoader,
    )
    .unwrap();
    assert_eq!(r.left_db, [-2.0, -2.0, 4.0, 3.0]);
    assert_eq!(
        r.bypassed.iter().map(|r| r.line_number).collect::<Vec<_>>(),
        [2, 3, 4]
    );
}
