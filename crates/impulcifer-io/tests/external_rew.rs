#![forbid(unsafe_code)]
use std::{fs, path::PathBuf};

use impulcifer_io::external::{Skip, read_external};

#[test]
fn rew_fixture_reads_two_impulse_responses_and_skips_the_frequency_response_only_one() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let file = read_external(&root.join("tests/fixtures/rew_two_irs_and_fr.mdat")).unwrap();
    assert_eq!(file.sample_rate, 48_000);
    assert_eq!(
        file.measurements
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
    assert_eq!(
        file.measurements
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>(),
        ["FL left", "FL 오른쪽 🎧", "FR only"]
    );
    assert_eq!(file.measurements[0].start_time, -0.01);
    assert_eq!(file.measurements[1].start_time, -0.0098);
    let json: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("tests/fixtures/rew_two_irs_and_fr.json")).unwrap(),
    )
    .unwrap();
    let arrays = json
        .as_array()
        .or_else(|| {
            json.get("measurements")
                .and_then(serde_json::Value::as_array)
        })
        .unwrap();
    for (measurement, expected) in file.measurements[..2].iter().zip(arrays) {
        assert_eq!(measurement.samples.len(), 8192);
        let expected = expected
            .get("samples")
            .and_then(serde_json::Value::as_array)
            .unwrap();
        assert_eq!(expected.len(), 4800);
        for (actual, expected) in measurement.samples.iter().zip(expected) {
            let expected = expected.as_f64().unwrap() as f32;
            assert_eq!(*actual, f64::from(expected));
        }
        assert!(
            measurement.samples[4800..]
                .iter()
                .all(|sample| *sample == 0.0)
        );
    }
    assert!(file.measurements[2].samples.is_empty());
    assert_eq!(file.measurements[2].skipped, Some(Skip::NoImpulseResponse));
}
