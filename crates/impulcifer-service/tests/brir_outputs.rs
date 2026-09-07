#![forbid(unsafe_code)]
mod brir_support;
use brir_support::*;
use impulcifer_dsp::{
    estimator::SweepEstimator,
    hrir::{Hrir, SpeakerIrs, compact_tracks},
    ir::ImpulseResponse,
    pipeline::PipelineOutputs,
    stages::readme::readme_data,
};
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::brir::{Catalog, outputs::write_outputs, run::run_brir};
use impulcifer_types::{
    config::ProcessingConfig,
    constants::{
        HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER, SPEAKER_NAMES, TRUEHD_11CH_ORDER,
        TRUEHD_13CH_ORDER,
    },
    job::{JobKind, JobStatus},
};
use serde_json::json;

fn outputs(compact: bool) -> (PipelineOutputs, SweepEstimator) {
    let estimator = SweepEstimator::new(0.1, 8000).unwrap();
    let hrir = Hrir {
        fs: 8000,
        speakers: SPEAKER_NAMES
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let ir = |gain: f64| {
                    let mut data = vec![0.0; 1024];
                    data[8] = gain;
                    data[20] = gain * 0.01;
                    ImpulseResponse {
                        data,
                        fs: 8000,
                        recording: None,
                    }
                };
                SpeakerIrs {
                    speaker: (*s).into(),
                    left: Some(ir((i + 1) as f64 / 32.0)),
                    right: Some(ir((i + 1) as f64 / 64.0)),
                }
            })
            .collect(),
    };
    let tracks = |order: &[&str]| {
        let full = hrir.stack_tracks(order, !compact).unwrap();
        if compact {
            compact_tracks(&full, order).0
        } else {
            full
        }
    };
    let mut jd = hrir.subset(&["FL", "FR"]);
    jd.normalize(Some(-0.1), None).unwrap();
    let output = PipelineOutputs {
        applied_gain_db: 0.0,
        readme: readme_data(&hrir, 8000, 0.0),
        hrir_tracks: tracks(&HEXADECAGONAL_TRACK_ORDER),
        hesuvi_tracks: tracks(&HESUVI_TRACK_ORDER),
        responses_tracks: hrir
            .stack_tracks(&HEXADECAGONAL_TRACK_ORDER, false)
            .unwrap(),
        truehd: [
            ("11ch", TRUEHD_11CH_ORDER.as_slice()),
            ("13ch", TRUEHD_13CH_ORDER.as_slice()),
        ]
        .iter()
        .map(|(label, order)| {
            let names: Vec<_> = order
                .iter()
                .flat_map(|s| [format!("{s}-left"), format!("{s}-right")])
                .collect();
            let tracks = hrir
                .stack_tracks(&names.iter().map(String::as_str).collect::<Vec<_>>(), false)
                .unwrap();
            (
                format!("truehd_{label}_{}ch.wav", order.len()),
                names,
                tracks,
            )
        })
        .collect(),
        jamesdsp: Some(
            jd.stack_tracks(&["FL-left", "FL-right", "FR-left", "FR-right"], false)
                .unwrap(),
        ),
        hangloose: SPEAKER_NAMES
            .iter()
            .map(|s| {
                (
                    (*s).into(),
                    hrir.stack_tracks(&[&format!("{s}-left"), &format!("{s}-right")], false)
                        .unwrap(),
                )
            })
            .collect(),
        hrir,
    };
    (output, estimator)
}
#[test]
fn output_variants_preserve_pcm32_tracks_names_and_write_order() {
    for compact in [false, true] {
        let temp = Temp::new();
        let (output, e) = outputs(compact);
        let config = ProcessingConfig {
            remove_silent_channels: compact,
            ..Default::default()
        };
        let mut catalog = Catalog::english();
        catalog.readme_date = Some("2026-09-07 12:00:00".into());
        let written = write_outputs(&temp.0, &output, &config, &catalog, &e).unwrap();
        assert_eq!(
            &written.files[..4],
            &[
                temp.0.join("responses.wav"),
                temp.0.join("README.md"),
                temp.0.join("hrir.wav"),
                temp.0.join("hesuvi.wav")
            ]
        );
        assert_eq!(written.files.len(), 22);
        for (filename, expected) in [
            ("responses.wav", &output.responses_tracks),
            ("hrir.wav", &output.hrir_tracks),
            ("hesuvi.wav", &output.hesuvi_tracks),
            ("jamesdsp.wav", output.jamesdsp.as_ref().unwrap()),
        ] {
            let wav = impulcifer_io::read_wav(&temp.0.join(filename)).unwrap();
            assert_eq!(wav.sample_rate, 8000);
            assert_eq!(wav.tracks, impulcifer_io::pcm32_round_trip(expected));
        }
        for (filename, _, expected) in &output.truehd {
            assert_eq!(
                impulcifer_io::read_wav(&temp.0.join(filename))
                    .unwrap()
                    .tracks,
                impulcifer_io::pcm32_round_trip(expected)
            );
        }
        for (s, expected) in &output.hangloose {
            assert_eq!(
                impulcifer_io::read_wav(&temp.0.join("Hangloose").join(format!("{s}.wav")))
                    .unwrap()
                    .tracks,
                impulcifer_io::pcm32_round_trip(expected)
            );
        }
        for (file, order) in [
            ("hrir.wav", HEXADECAGONAL_TRACK_ORDER.as_slice()),
            ("hesuvi.wav", HESUVI_TRACK_ORDER.as_slice()),
        ] {
            let wav = impulcifer_io::read_wav(&temp.0.join(file)).unwrap();
            let names = impulcifer_io::brir_layout::read_track_names(
                &temp.0.join(file),
                order,
                wav.tracks.len(),
            )
            .unwrap();
            if compact {
                assert_eq!(
                    names.unwrap(),
                    compact_tracks(&output.hrir.stack_tracks(order, false).unwrap(), order).1
                );
            } else {
                assert!(names.is_none());
            }
        }
    }
}
#[test]
fn output_readonly_file_failure_preserves_existing_bytes() {
    let temp = Temp::new();
    let (output, e) = outputs(false);
    let path = temp.0.join("responses.wav");
    std::fs::write(&path, b"existing").unwrap();
    let original = std::fs::metadata(&path).unwrap().permissions();
    let mut readonly = original.clone();
    readonly.set_readonly(true);
    std::fs::set_permissions(&path, readonly).unwrap();
    let result = write_outputs(
        &temp.0,
        &output,
        &ProcessingConfig::default(),
        &Catalog::english(),
        &e,
    );
    std::fs::set_permissions(&path, original).unwrap();
    assert!(result.is_err());
    assert_eq!(std::fs::read(path).unwrap(), b"existing");
    assert!(!temp.0.join("README.md").exists());
}
#[test]
fn demo_readme_korean_bytes_match_python() {
    let temp = Temp::demo();
    let jobs = JobRegistry::new();
    let config = ProcessingConfig {
        dir_path: Some(temp.0.to_string_lossy().into_owned()),
        ..Default::default()
    };
    let mut catalog = Catalog::english();
    catalog.strings.extend(
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(include_str!(
            "../../../i18n/locales/ko.json"
        ))
        .unwrap(),
    );
    catalog.readme_date = Some("2026-09-07 12:00:00".into());
    let job = jobs
        .start(JobKind::Brir, true, move |ctx| {
            run_brir(&config, &catalog, ctx).map(|r| json!({"output_path":r.output_path}))
        })
        .unwrap();
    let p = wait(&jobs, &job.job_id);
    assert_eq!(p.job.status, JobStatus::Succeeded, "{:?}", p.job.error);
    let expected = std::fs::read_to_string(golden("p11_default_ko.txt")).unwrap();
    let actual = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
    let diffs = readme_differences(&actual, &expected, 0.1, 2.0);
    assert!(diffs.is_empty(), "Korean README parity failed: {diffs:#?}");
}
