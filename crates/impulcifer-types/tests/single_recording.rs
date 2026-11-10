#![forbid(unsafe_code)]

use impulcifer_types::config::{
    ConfigError, EXTENSION_FIELD_NAMES, ProcessingConfig, parse_recording_speakers,
};

#[test]
fn recording_speaker_list_parses_and_rejects() {
    assert_eq!(
        parse_recording_speakers(" FL, FC, X, LFE, X, FR, LFE2 ").unwrap(),
        ["FL", "FC", "X", "LFE", "X", "FR", "LFE2"]
    );
    for (value, text) in [
        ("FL,,FR", "entry 2"),
        ("FL,fl", "fl"),
        ("FL,FR,FR", "FR"),
        ("FC,FR", "FL"),
    ] {
        let error = parse_recording_speakers(value).unwrap_err();
        assert!(error.contains(text), "{value}: {error}");
    }
    assert_eq!(EXTENSION_FIELD_NAMES.len(), 43);
    for field in [
        "brir_recording_file",
        "brir_recording_speakers",
        "brir_recording_sweep",
    ] {
        assert!(EXTENSION_FIELD_NAMES.contains(&field));
    }

    let valid = ProcessingConfig {
        brir_recording_file: Some("recording.wav".into()),
        brir_recording_speakers: Some("FL,FR".into()),
        brir_recording_sweep: Some("sweep.wav".into()),
        ..Default::default()
    };
    valid.validate_room_options().unwrap();
    for (config, field) in [
        (
            ProcessingConfig {
                brir_recording_file: Some("recording.wav".into()),
                brir_measurement_file: Some("project.mdat".into()),
                brir_recording_speakers: Some("FL".into()),
                ..Default::default()
            },
            "brir_recording_file",
        ),
        (
            ProcessingConfig {
                brir_recording_file: Some("recording.wav".into()),
                ..Default::default()
            },
            "brir_recording_speakers",
        ),
        (
            ProcessingConfig {
                brir_recording_speakers: Some("FL".into()),
                ..Default::default()
            },
            "brir_recording_speakers",
        ),
        (
            ProcessingConfig {
                brir_recording_sweep: Some("sweep.wav".into()),
                ..Default::default()
            },
            "brir_recording_sweep",
        ),
    ] {
        let ConfigError::Invalid { field: actual, .. } =
            config.validate_room_options().unwrap_err();
        assert_eq!(actual, field);
    }
    ProcessingConfig {
        brir_recording_file: Some(" ".into()),
        brir_recording_speakers: Some(" ".into()),
        brir_recording_sweep: Some(" ".into()),
        ..Default::default()
    }
    .validate_room_options()
    .unwrap();
}
