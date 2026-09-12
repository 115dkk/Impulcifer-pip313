#![forbid(unsafe_code)]

use impulcifer_audio_io::null_backend::NullBackend;
use impulcifer_types::audio::{AudioBackend, Direction, Endpoint, ShareMode, StreamSpec};

fn endpoint() -> Endpoint {
    Endpoint {
        id: "none".into(),
        name: "none".into(),
        host_api: "none".into(),
        max_input_channels: 0,
        max_output_channels: 0,
        default_samplerate: 48000.0,
        is_default_input: false,
        is_default_output: false,
    }
}

#[test]
fn null_backend_lists_nothing_and_refuses_to_open() {
    let backend = NullBackend;
    let endpoint = endpoint();
    let spec = StreamSpec {
        sample_rate: 48000,
        channels: 2,
    };
    assert_eq!(backend.name(), "none");
    assert!(backend.enumerate().unwrap().is_empty());
    assert!(backend.selectable_share_modes().is_empty());
    for error in [
        backend
            .probe(&endpoint, Direction::Input, spec, ShareMode::Exclusive)
            .unwrap_err(),
        backend
            .open_output(&endpoint, spec, ShareMode::Exclusive)
            .err()
            .unwrap(),
        backend
            .open_input(&endpoint, spec, ShareMode::Exclusive)
            .err()
            .unwrap(),
    ] {
        assert_eq!(
            error.to_string(),
            "backend error: this build has no audio backend"
        );
    }
}
