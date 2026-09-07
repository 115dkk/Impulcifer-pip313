#![forbid(unsafe_code)]
mod brir_support;
use brir_support::*;
use impulcifer_service::brir::{
    BrirError, BrirEvents, discovery::discover, estimator::open_estimator, inputs::load_inputs,
};
use impulcifer_types::config::ProcessingConfig;
use serde_json::Value;
#[derive(Default)]
struct Events(Vec<String>);
impl BrirEvents for Events {
    fn step(&mut self, key: &str, _: Value) -> Result<(), BrirError> {
        self.0.push(key.into());
        Ok(())
    }
    fn log(&mut self, _: &str, key: &str, _: Value) {
        self.0.push(key.into());
    }
    fn check_cancelled(&self) -> Result<(), BrirError> {
        Ok(())
    }
}
#[test]
fn estimator_alias_generate_sidecar_and_auto_match() {
    let temp = Temp::new();
    let config = ProcessingConfig::default();
    let dir = discover(&temp.0, &config).unwrap();
    let generated = open_estimator(&dir, Some("generate:6.15s@48000")).unwrap();
    let alias = open_estimator(&dir, Some("default")).unwrap();
    assert_eq!(generated.test_signal, alias.test_signal);
    impulcifer_io::write_wav(
        &temp.0.join("test.wav"),
        48000,
        std::slice::from_ref(&generated.test_signal),
        32,
    )
    .unwrap();
    let dir = discover(&temp.0, &config).unwrap();
    let automatic = open_estimator(&dir, None).unwrap();
    assert_eq!(automatic.test_signal, generated.test_signal);
    assert!(matches!(
        open_estimator(&dir, Some("test.pkl")),
        Err(BrirError::Unsupported(_))
    ));
}
#[test]
fn room_and_eq_inputs_use_real_measurements_and_write_responses() {
    let temp = Temp::demo();
    let source = temp.0.join("FL,FR.wav");
    std::fs::copy(&source, temp.0.join("room-FL,FR.wav")).unwrap();
    let recording = impulcifer_io::read_wav(&source).unwrap();
    let mono: Vec<_> = recording.tracks[0]
        .iter()
        .zip(&recording.tracks[1])
        .map(|(l, r)| (l + r) * 0.5)
        .collect();
    impulcifer_io::write_wav(&temp.0.join("room.wav"), recording.sample_rate, &[mono], 32).unwrap();
    for name in [
        "room-target.csv",
        "room-mic-calibration.csv",
        "eq.csv",
        "eq-left.csv",
        "eq-right.csv",
    ] {
        std::fs::write(
            temp.0.join(name),
            "frequency,raw\n10.0,0.0\n100.0,1.0\n1000.0,0.0\n10000.0,-1.0\n24000.0,0.0\n",
        )
        .unwrap();
    }
    std::fs::write(temp.0.join("eq.txt"), "Preamp: -30 dB").unwrap();
    let config = ProcessingConfig::default();
    let dir = discover(&temp.0, &config).unwrap();
    assert_eq!(
        dir.eq.common.as_ref().unwrap().file_name().unwrap(),
        "eq.csv"
    );
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    assert!(!inputs.room.as_ref().unwrap().frs.0.is_empty());
    assert!(inputs.headphone.is_some());
    assert!(inputs.eq_left.is_some());
    assert!(inputs.eq_right.is_some());
    for name in ["room-responses.wav", "headphone-responses.wav"] {
        let wav = impulcifer_io::read_wav(&temp.0.join(name)).unwrap();
        assert_eq!(wav.tracks.len(), 32);
        assert_eq!(wav.sample_rate, 48000);
    }
    assert!(events.0.iter().any(|k| k == "cli_eq_plain_gain_curve"));
}
#[test]
fn eqapo_input_is_parsed_and_logged() {
    // core/pipeline_stages.py:199-291 through stages::eqapo (P12): a preamp and a
    // peaking filter at 1 kHz become an eq curve whose error is -gain.
    let temp = Temp::demo();
    std::fs::write(
        temp.0.join("eq.txt"),
        "Preamp: -3 dB\nFilter: ON PK Fc 1000 Hz Gain -6 dB Q 1.0\nFilter: ON XX Fc 1 Hz\n",
    )
    .unwrap();
    let config = ProcessingConfig {
        do_room_correction: false,
        do_headphone_compensation: false,
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let estimator = impulcifer_dsp::estimator::SweepEstimator::new(5.0, 48000).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    let left = inputs.eq_left.expect("eq.txt becomes eq_left");
    // No channel scopes: 2.x applies the single curve to both ears (select_eq_pair).
    assert_eq!(inputs.eq_right.as_ref(), Some(&left));
    let at_1k = left.frequency.iter().position(|f| *f >= 1000.0).unwrap();
    assert!(
        (left.raw[at_1k] + 9.0).abs() < 0.5,
        "raw at 1 kHz {}",
        left.raw[at_1k]
    );
    assert!((left.error[at_1k] - 9.0).abs() < 0.5, "error = -gain");
    assert!(events.0.contains(&"cli_eqapo_detected".to_string()));
    assert!(events.0.contains(&"cli_eqapo_preamp".to_string()));
    assert!(
        events.0.contains(&"cli_eqapo_bypassed_line".to_string()),
        "unknown XX filter is bypassed"
    );
    assert!(!events.0.contains(&"cli_eqapo_channel_split".to_string()));
}
