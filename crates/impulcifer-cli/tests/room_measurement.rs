#![forbid(unsafe_code)]
use impulcifer_cli::{Parsed, parse};
use impulcifer_types::config::ProcessingConfig;

#[test]
fn cli_room_measurement_options_parse() {
    let args = [
        "impulcifer",
        "--dir_path=data/demo",
        "--room_measurement_file",
        "room.mdat",
        "--room_measurement_map",
        "FL.left=1,FL.right=2",
        "--brir_measurement_file",
        "brir.mdat",
        "--brir_measurement_map",
        "FL.left=1,FL.right=2",
        "--headphone_measurement_file",
        "headphones.mdat",
        "--headphone_measurement_map",
        "left=1,right=2",
    ]
    .map(str::to_owned)
    .to_vec();
    let Parsed::Kwargs(kwargs) = parse(&args).unwrap() else {
        panic!("expected config")
    };
    let config = ProcessingConfig::from_kwargs(&kwargs).unwrap();
    assert_eq!(config.room_measurement_file.as_deref(), Some("room.mdat"));
    assert_eq!(
        config.room_measurement_map.as_deref(),
        Some("FL.left=1,FL.right=2")
    );
    assert_eq!(config.brir_measurement_file.as_deref(), Some("brir.mdat"));
    assert_eq!(
        config.brir_measurement_map.as_deref(),
        Some("FL.left=1,FL.right=2")
    );
    assert_eq!(
        config.headphone_measurement_file.as_deref(),
        Some("headphones.mdat")
    );
    assert_eq!(
        config.headphone_measurement_map.as_deref(),
        Some("left=1,right=2")
    );
    assert!(ProcessingConfig::default().room_measurement_file.is_none());
    assert!(ProcessingConfig::default().room_measurement_map.is_none());
}

#[test]
fn cli_recording_file_options_parse() {
    let args = [
        "impulcifer",
        "--dir_path=data/demo",
        "--brir_recording_file",
        "recording.wav",
        "--brir_recording_speakers",
        "FL, FC, X, FR, LFE",
        "--brir_recording_sweep",
        "sweep.wav",
    ]
    .map(str::to_owned)
    .to_vec();
    let Parsed::Kwargs(kwargs) = parse(&args).unwrap() else {
        panic!("expected config")
    };
    let config = ProcessingConfig::from_kwargs(&kwargs).unwrap();
    assert_eq!(config.brir_recording_file.as_deref(), Some("recording.wav"));
    assert_eq!(
        config.brir_recording_speakers.as_deref(),
        Some("FL, FC, X, FR, LFE")
    );
    assert_eq!(config.brir_recording_sweep.as_deref(), Some("sweep.wav"));
    config.validate_room_options().unwrap();
    let defaults = ProcessingConfig::default();
    assert!(defaults.brir_recording_file.is_none());
    assert!(defaults.brir_recording_speakers.is_none());
    assert!(defaults.brir_recording_sweep.is_none());
}
