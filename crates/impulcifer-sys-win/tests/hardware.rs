#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
mod windows_hardware {
    use impulcifer_sys_win::WasapiBackend;
    use impulcifer_types::audio::{AudioBackend, CancelToken, Direction, ShareMode, StreamSpec};

    fn default_output(backend: &WasapiBackend) -> impulcifer_types::audio::Endpoint {
        backend
            .enumerate()
            .unwrap()
            .into_iter()
            .find(|endpoint| endpoint.is_default_output)
            .expect("default WASAPI output endpoint")
    }

    #[test]
    #[ignore = "requires Windows audio hardware"]
    fn enumerate_lists_default_endpoints() {
        let endpoints = WasapiBackend::new().enumerate().unwrap();
        assert!(endpoints.iter().any(|endpoint| endpoint.is_default_output));
        assert!(endpoints.iter().any(|endpoint| endpoint.is_default_input));
        assert!(
            endpoints
                .iter()
                .all(|endpoint| endpoint.host_api == "Windows WASAPI")
        );
    }

    #[test]
    #[ignore = "requires Windows audio hardware"]
    fn probe_matrix_runs_without_panic() {
        let backend = WasapiBackend::new();
        for endpoint in backend.enumerate().unwrap() {
            let (direction, maximum) = if endpoint.max_output_channels > 0 {
                (Direction::Output, endpoint.max_output_channels)
            } else {
                (Direction::Input, endpoint.max_input_channels)
            };
            for rate in [44_100, 48_000, 96_000] {
                for requested_channels in [2, 8, 16] {
                    let channels = requested_channels.min(maximum);
                    if channels == 0 {
                        continue;
                    }
                    for mode in [ShareMode::Exclusive, ShareMode::SharedAutoConvert] {
                        let _ = backend.probe(
                            &endpoint,
                            direction,
                            StreamSpec {
                                sample_rate: rate,
                                channels,
                            },
                            mode,
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "plays 200 ms of silence on the default output"]
    fn open_and_play_silence_exclusive_or_shared() {
        let backend = WasapiBackend::new();
        let endpoint = default_output(&backend);
        let spec = StreamSpec {
            sample_rate: endpoint.default_samplerate as u32,
            channels: endpoint.max_output_channels,
        };
        let (mut session, mode) = match backend.open_output(&endpoint, spec, ShareMode::Exclusive) {
            Ok(session) => (session, ShareMode::Exclusive),
            Err(_) => (
                backend
                    .open_output(&endpoint, spec, ShareMode::SharedAutoConvert)
                    .unwrap(),
                ShareMode::SharedAutoConvert,
            ),
        };
        let frames = usize::try_from(spec.sample_rate / 5).unwrap();
        let silence = vec![0.0; frames * usize::from(spec.channels)];
        let report = session
            .play_to_completion(&silence, &CancelToken::new())
            .unwrap();
        assert_eq!(report.mode, mode);
        assert_eq!(report.frames_submitted, frames as u64);
        assert_eq!(report.frames_drained, frames as u64);
        assert!(!report.cancelled);
    }
}
