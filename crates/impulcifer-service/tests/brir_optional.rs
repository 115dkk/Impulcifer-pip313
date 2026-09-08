#![forbid(unsafe_code)]
mod brir_support;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_types::job::JobStatus;
use serde_json::json;

#[test]
fn optional_dsp_stages_and_png_plots_complete_with_only_unsupported_plot_warnings() {
    let temp = Temp::demo();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let result=service.call("start_brir",vec![json!({"dir_path":temp.0,"do_headphone_compensation":false,"microphone_deviation_correction":true,"decay":{"FL":0.3,"FR":0.3},"channel_balance":1.0,"fs":44100,"plot":true,"mic_deviation_debug_plots":true,"interactive_plots":true,"jamesdsp":true,"hangloose":true,"output_truehd_layouts":true,"remove_silent_channels":true})]);
    assert_eq!(result["ok"], true, "{result}");
    let p = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(p.job.status, JobStatus::Succeeded, "{:?}", p.job.error);
    for key in [
        "cli_correcting_deviation",
        "cli_adjusting_decay",
        "cli_correcting_balance",
        "cli_resampling",
        "cli_generating_jamesdsp",
        "cli_generating_hangloose",
        "cli_generating_truehd",
    ] {
        assert!(p.events.iter().any(|e| e.payload["key"] == key), "{key}");
    }
    assert_eq!(
        p.events
            .iter()
            .filter(|e| e.payload["key"] == "cli_plots_not_available_yet")
            .count(),
        2
    );
    assert!(temp.0.join("plots/results.png").is_file());
    for speaker in ["FL", "FR", "FC", "BL", "BR", "SL", "SR"] {
        for side in ["left", "right"] {
            for stage in ["pre", "post", "room"] {
                assert!(
                    temp.0
                        .join(format!("plots/{stage}/{speaker}-{side}.png"))
                        .is_file()
                );
            }
        }
        assert!(
            temp.0
                .join(format!(
                    "plots/interaural_overlay/{speaker}_interaural_overlay.png"
                ))
                .is_file()
        );
    }
    assert!(!temp.0.join("plots/headphones.png").exists());
    assert!(!temp.0.join("interactive_plots").exists());
    let wav = impulcifer_io::read_wav(&temp.0.join("hesuvi.wav")).unwrap();
    assert_eq!(wav.sample_rate, 44100);
    assert_eq!(wav.tracks.len(), 14);
    assert!(temp.0.join("jamesdsp.wav").is_file());
    assert!(temp.0.join("Hangloose/FL.wav").is_file());
    for (key, level) in [
        ("cli_info_parallel_decay", "INFO"),
        ("cli_warning_truehd_11ch_fail", "WARNING"),
        ("cli_warning_truehd_13ch_fail", "WARNING"),
        ("cli_success_jamesdsp", "SUCCESS"),
        ("cli_success_hangloose_file", "INFO"),
        ("cli_success_hangloose", "SUCCESS"),
    ] {
        assert!(
            p.events
                .iter()
                .any(|e| e.payload["key"] == key && e.payload["level"] == level),
            "{key}"
        );
    }
    let jd = impulcifer_io::read_wav(&temp.0.join("jamesdsp.wav")).unwrap();
    assert_eq!(jd.sample_rate, 44100);
    assert_eq!(jd.tracks.len(), 4);
    assert_ne!(
        jd.tracks[0], wav.tracks[0],
        "JamesDSP normalizes its subset independently"
    );
    let split = impulcifer_io::read_wav(&temp.0.join("Hangloose/FL.wav")).unwrap();
    assert_eq!(split.tracks, wav.tracks[..2]);
    // Demo has seven speakers: neither TrueHD minimum is met. This is NOT a TrueHD synthesis test.
    assert!(!temp.0.join("truehd_11ch_7ch.wav").exists());
}
#[test]
fn truehd_layouts_succeed_end_to_end_from_measurements() {
    use impulcifer_types::constants::{
        HEXADECAGONAL_TRACK_ORDER, TRUEHD_11CH_ORDER, TRUEHD_13CH_ORDER,
    };
    let temp = Temp::demo();
    // Reuse an actual measured two-ear response as synthetic height measurements.
    // The job still performs ingestion, estimation, every default DSP stage and output.
    for speaker in ["TFL", "TFR", "TSL", "TSR", "TBL", "TBR"] {
        std::fs::copy(temp.0.join("FC.wav"), temp.0.join(format!("{speaker}.wav"))).unwrap();
    }
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let result = service.call(
        "start_brir",
        vec![json!({"dir_path":temp.0,"output_truehd_layouts":true})],
    );
    assert_eq!(result["ok"], true, "{result}");
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    let hrir = impulcifer_io::read_wav(&temp.0.join("hrir.wav")).unwrap();
    for (label, order) in [
        ("11ch", TRUEHD_11CH_ORDER.as_slice()),
        ("13ch", TRUEHD_13CH_ORDER.as_slice()),
    ] {
        let wav =
            impulcifer_io::read_wav(&temp.0.join(format!("truehd_{label}_{}ch.wav", order.len())))
                .unwrap();
        assert_eq!(wav.sample_rate, hrir.sample_rate);
        assert_eq!(wav.tracks.len(), order.len() * 2);
        for (i, speaker) in order.iter().enumerate() {
            for (side, name) in ["left", "right"].iter().enumerate() {
                let index = HEXADECAGONAL_TRACK_ORDER
                    .iter()
                    .position(|n| *n == format!("{speaker}-{name}"))
                    .unwrap();
                assert_eq!(wav.tracks[2 * i + side], hrir.tracks[index]);
                assert!(wav.tracks[2 * i + side].iter().any(|v| *v != 0.0));
            }
        }
        assert!(poll.events.iter().any(|e| e.payload["key"]
            == format!("cli_success_truehd_{label}")
            && e.payload["level"] == "SUCCESS"));
    }
}
#[test]
fn mic_deviation_is_skipped_with_headphone_compensation() {
    let temp = Temp::demo();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let result = service.call(
        "start_brir",
        vec![json!({"dir_path":temp.0,"microphone_deviation_correction":true})],
    );
    let p = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(p.job.status, JobStatus::Succeeded, "{:?}", p.job.error);
    assert!(
        p.events
            .iter()
            .any(|e| e.payload["key"] == "cli_mic_deviation_skipped_hpcomp")
    );
    assert!(
        !p.events
            .iter()
            .any(|e| e.payload["key"] == "cli_correcting_deviation")
    );
}
