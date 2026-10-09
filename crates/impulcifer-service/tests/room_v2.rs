#![forbid(unsafe_code)]
mod brir_support;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::brir::{
    BrirError, BrirEvents, Catalog, discovery::discover, estimator::open_estimator,
    inputs::load_inputs, run::run_brir,
};
use impulcifer_types::{
    config::ProcessingConfig,
    job::{JobKind, JobStatus},
};
use serde_json::{Value, json};
#[derive(Default)]
struct Events(Vec<(String, String, Value)>);
impl BrirEvents for Events {
    fn step(&mut self, _: &str, _: Value) -> Result<(), BrirError> {
        Ok(())
    }
    fn log(&mut self, level: &str, key: &str, args: Value) {
        self.0.push((level.into(), key.into(), args));
    }
    fn check_cancelled(&self) -> Result<(), BrirError> {
        Ok(())
    }
}
#[test]
fn config_room_range_defaults_and_validation() {
    let temp = Temp::new();
    let service = service(&temp, JobRegistry::new());
    let boot = service.call("bootstrap", vec![]);
    let defaults = &boot["data"]["brir_defaults"];
    assert_eq!(defaults["room_range"], "schroeder");
    assert_eq!(defaults["room_volume"], Value::Null);
    assert_eq!(defaults["schroeder_freq"], Value::Null);
    assert_eq!(defaults["room_max_boost"], 12.0);
    assert_eq!(defaults["room_mode"], "eq");
    assert_eq!(defaults["room_tuning_delay"], "10");
    assert_eq!(defaults["room_tuning_phase_limit"], "full");
    assert_eq!(defaults["room_tuning_max_boost"], 6.0);
    assert_eq!(defaults["room_tuning_curtain"], 300.0);
    assert_eq!(defaults["room_tuning_level_match"], true);
    let numeric = ProcessingConfig::from_kwargs(
        json!({"room_tuning_delay": 10, "room_tuning_phase_limit": 300})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(numeric.room_tuning_delay, "10");
    assert_eq!(numeric.room_tuning_phase_limit, "300");
    for (field, value) in [
        ("room_range", json!("bad")),
        ("room_volume", json!(0)),
        ("room_volume", json!(10001)),
        ("schroeder_freq", json!(49)),
        ("schroeder_freq", json!(1001)),
        ("room_max_boost", json!(-1)),
        ("room_max_boost", json!(25)),
        ("room_max_boost", json!(null)),
        ("room_volume", json!("bad")),
        ("room_mode", json!("bad")),
        ("room_mode", json!(3)),
        ("room_tuning_delay", json!(1.9)),
        ("room_tuning_delay", json!(20.1)),
        ("room_tuning_delay", json!(null)),
        ("room_tuning_delay", json!("NaN")),
        ("room_tuning_phase_limit", json!(299)),
        ("room_tuning_phase_limit", json!(20001)),
        ("room_tuning_phase_limit", json!("NaN")),
        ("room_tuning_max_boost", json!(-1)),
        ("room_tuning_max_boost", json!(13)),
        ("room_tuning_curtain", json!(99)),
        ("room_tuning_curtain", json!(5001)),
        ("room_tuning_level_match", json!("false")),
    ] {
        let mut request = json!({"dir_path":temp.0});
        request[field] = value;
        let result = service.call("start_brir", vec![request]);
        assert_eq!(result["ok"], false, "{result}");
        assert_eq!(result["error"]["code"], "INVALID_REQUEST");
        assert_eq!(result["error"]["details"]["field"], field, "{result}");
    }
}
fn run_demo(range: &str) -> (Temp, Vec<String>) {
    let temp = Temp::demo();
    let config = ProcessingConfig {
        dir_path: Some(temp.0.to_string_lossy().into_owned()),
        room_range: range.into(),
        ..Default::default()
    };
    let jobs = JobRegistry::new();
    let job = jobs
        .start(JobKind::Brir, true, move |ctx| {
            let out = run_brir(&config, &Catalog::english(), ctx)?;
            Ok(json!({"output_path":out.output_path}))
        })
        .unwrap();
    let poll = wait(&jobs, &job.job_id);
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    let keys = poll
        .events
        .iter()
        .filter_map(|e| {
            e.payload
                .get("key")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    (temp, keys)
}
#[test]
fn demo_default_room_range_runs_schroeder() {
    let (new, keys) = run_demo(&ProcessingConfig::default().room_range);
    assert!(keys.iter().any(|k| k == "cli_room_range"));
    assert!(keys.iter().any(|k| k.starts_with("cli_room_schroeder")));
    let wav = impulcifer_io::read_wav(&new.0.join("hesuvi.wav")).unwrap();
    assert!(wav.tracks.iter().flatten().all(|x| x.is_finite()));
    let (legacy, keys) = run_demo("legacy");
    assert!(!keys.iter().any(|k| k.starts_with("cli_room_")));
    assert!(keys.iter().any(|k| k == "cli_running_room_correction"));
    assert_ne!(
        std::fs::read(new.0.join("hesuvi.wav")).unwrap(),
        std::fs::read(legacy.0.join("hesuvi.wav")).unwrap()
    );
    let config = ProcessingConfig::default();
    let dir = discover(&new.0, &config).unwrap();
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
    println!(
        "DEMO DIAGNOSTICS {:#?}",
        inputs.room.unwrap().diagnostics.unwrap()
    );
    for (level, key, args) in events.0 {
        if key.starts_with("cli_room_") {
            println!("DEMO LOG {level} {key} {args}");
        }
    }
}
#[test]
fn room_tuning_runs_on_the_demo() {
    use impulcifer_dsp::pipeline::{StageObserver, StageProgress, run_pipeline};
    use impulcifer_dsp::stages::room_tuning::TuningReport;
    #[derive(Default)]
    struct Observe(Option<TuningReport>);
    impl StageObserver for Observe {
        fn on_stage(&mut self, _: StageProgress) {}
        fn check_cancelled(&self) -> Result<(), impulcifer_dsp::DspError> {
            Ok(())
        }
        fn on_room_tuned(&mut self, r: &TuningReport) -> Result<(), impulcifer_dsp::DspError> {
            self.0 = Some(r.clone());
            Ok(())
        }
    }
    let mut runs = Vec::new();
    let mut mismatches = Vec::new();
    for mode in ["eq", "tuning"] {
        let temp = Temp::demo();
        let config = ProcessingConfig {
            dir_path: Some(temp.0.to_string_lossy().into_owned()),
            room_mode: mode.into(),
            ..Default::default()
        };
        let dir = discover(&temp.0, &config).unwrap();
        let estimator = open_estimator(&dir, Some("default")).unwrap();
        let mut events = Events::default();
        let inputs = load_inputs(&dir, &estimator, &config, &mut events).unwrap();
        let mut observe = Observe::default();
        let output = run_pipeline(&config, inputs, &mut observe).unwrap();
        println!("DEMO {mode} normalization {} dB", output.applied_gain_db);
        if let Some(report) = observe.0 {
            println!("DEMO f_ph {}", report.f_ph);
            for check in &report.pair_checks {
                println!("PAIR {check:?}");
            }
            for s in report.speakers {
                assert_eq!(s.design_origin, report.delay_samples);
                assert!(
                    !s.weak(),
                    "{} representation {:?}",
                    s.speaker,
                    s.representation_rms_db
                );
                for p in &s.pre_echo_channels {
                    println!(
                        "PREECHO {} {:?}: untuned={:.6} tuned={:.6} warning={}",
                        s.speaker,
                        p.side,
                        p.before_db,
                        p.after_db,
                        p.warns()
                    );
                }
                println!(
                    "PROCESSOR {} points={} cutoffs={}/{} trim={} {} representation={:?}",
                    s.speaker,
                    s.points,
                    s.low_cutoff,
                    s.high_cutoff,
                    s.trim_db,
                    s.trim_source,
                    s.representation_rms_db
                );
                println!(
                    "DEMO {} phase={} share={} pre_echo={} weak={} mismatch={} LF EDT {} -> {} ms; median excess GD {} -> {} ms",
                    s.speaker,
                    s.phase_corrected,
                    s.corrected_share,
                    s.pre_echo_db,
                    s.weak(),
                    s.mismatch,
                    s.lf_edt_before_ms,
                    s.lf_edt_after_ms,
                    s.excess_median_before_ms,
                    s.excess_median_after_ms
                );
            }
        }
        let jobs = JobRegistry::new();
        let job = jobs
            .start(JobKind::Brir, true, move |ctx| {
                let mut catalog = Catalog::english();
                catalog.strings.insert(
                    "cli_room_tuning".into(),
                    json!("tuning freq={freq} delay={delay}"),
                );
                run_brir(&config, &catalog, ctx)?;
                Ok(json!({}))
            })
            .unwrap();
        let poll = wait(&jobs, &job.job_id);
        assert_eq!(
            poll.job.status,
            JobStatus::Succeeded,
            "{:?}",
            poll.job.error
        );
        let tuning: Vec<_> = poll
            .events
            .iter()
            .filter(|e| {
                e.payload["key"]
                    .as_str()
                    .is_some_and(|k| k.starts_with("cli_room_tuning"))
            })
            .collect();
        if mode == "eq" {
            assert!(tuning.is_empty());
        } else {
            assert!(!tuning.iter().any(|e| matches!(
                e.payload["key"].as_str(),
                Some("cli_room_tuning_preecho" | "cli_room_tuning_weak")
            )));
            mismatches.extend(
                tuning
                    .iter()
                    .filter(|e| e.payload["key"] == "cli_room_tuning_mismatch")
                    .map(|e| e.payload.clone()),
            );
            assert!(
                tuning.iter().any(|e| e.payload["key"] == "cli_room_tuning"
                    && e.payload["message"]
                        .as_str()
                        .is_some_and(|s| s.ends_with("delay=10"))),
                "{:?}",
                tuning
            );
        }
        let wav = impulcifer_io::read_wav(&temp.0.join("hesuvi.wav")).unwrap();
        assert!(wav.tracks.iter().flatten().all(|v| v.is_finite()));
        let readme = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
        println!(
            "DEMO {mode} README gain lines: {:?}",
            readme
                .lines()
                .filter(|l| l.contains("dB"))
                .collect::<Vec<_>>()
        );
        runs.push((wav, output.applied_gain_db));
    }
    println!("normalisation change {} dB", runs[1].1 - runs[0].1);
    let mut shifts = Vec::new();
    for (a, b) in runs[0].0.tracks.iter().zip(&runs[1].0.tracks) {
        if a.iter().all(|v| *v == 0.0) {
            continue;
        }
        let peak = |x: &[f64]| impulcifer_dsp::peaks::first_peak_index(x, 0, None, 0.12589);
        shifts.push(peak(b) as i64 - peak(a) as i64);
    }
    println!("DEMO direct-peak shifts {shifts:?}; mismatch logs {mismatches:?}");
    // The FIR origin is exactly D_s; changing the waveform near the direct peak
    // can change the selected peak by a few samples (not a per-speaker delay).
    let tolerance = (0.0001 * runs[1].0.sample_rate as f64).round() as i64;
    assert!(
        mismatches.is_empty() && shifts.iter().all(|d| (*d - 480).abs() <= tolerance),
        "demo acceptance: mismatch={}, shifts={shifts:?}",
        mismatches.len()
    );
}

#[test]
fn room_tuning_detects_swapped_files() {
    use impulcifer_dsp::pipeline::{StageObserver, StageProgress, run_pipeline};
    #[derive(Default)]
    struct Observe(Vec<String>);
    impl StageObserver for Observe {
        fn on_stage(&mut self, _: StageProgress) {}
        fn check_cancelled(&self) -> Result<(), impulcifer_dsp::DspError> {
            Ok(())
        }
        fn on_room_tuned(
            &mut self,
            report: &impulcifer_dsp::stages::room_tuning::TuningReport,
        ) -> Result<(), impulcifer_dsp::DspError> {
            self.0 = report
                .speakers
                .iter()
                .filter(|s| s.mismatch)
                .map(|s| s.speaker.clone())
                .collect();
            Ok(())
        }
    }
    let temp = Temp::demo();
    let left = temp.0.join("room-FL,FR-left.wav");
    let right = temp.0.join("room-FL,FR-right.wav");
    let a = std::fs::read(&left).unwrap();
    let b = std::fs::read(&right).unwrap();
    std::fs::write(&left, b).unwrap();
    std::fs::write(&right, a).unwrap();
    let config = ProcessingConfig {
        room_mode: "tuning".into(),
        ..Default::default()
    };
    let dir = discover(&temp.0, &config).unwrap();
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let inputs = load_inputs(&dir, &estimator, &config, &mut Events::default()).unwrap();
    let mut observer = Observe::default();
    run_pipeline(&config, inputs, &mut observer).unwrap();
    observer.0.sort();
    assert_eq!(observer.0, ["FL", "FR"]);
}

#[test]
fn room_tuning_accepts_legacy_range() {
    let temp = Temp::demo();
    let config = ProcessingConfig {
        room_mode: "tuning".into(),
        ..Default::default()
    };
    let (a, _) = load_room(&temp, &config);
    let (b, _) = load_room(
        &temp,
        &ProcessingConfig {
            room_range: "legacy".into(),
            room_max_boost: 0.,
            ..config
        },
    );
    assert!(a.frs.entries.is_empty() && b.frs.entries.is_empty());
    for (a, b) in a
        .tuning
        .unwrap()
        .report
        .speakers
        .iter()
        .zip(b.tuning.unwrap().report.speakers)
    {
        assert_eq!(a.filter, b.filter);
    }
}
#[test]
fn room_tuning_auto_delay_on_the_demo() {
    let temp = Temp::demo();
    let config = ProcessingConfig {
        room_mode: "tuning".into(),
        room_tuning_delay: "auto".into(),
        ..Default::default()
    };
    let (room, _) = load_room(&temp, &config);
    let plan = room.tuning.unwrap();
    println!(
        "DEMO auto D={} ms scores={:?}",
        plan.report.delay_samples as f64 / 48.,
        plan.report.auto_scores
    );
    for curve in &plan.report.auto_file_scores {
        println!(
            "DEMO auto file {:?} {:?}: {:?}",
            curve.speakers, curve.side, curve.metrics
        );
    }
    assert!((96..=480).contains(&plan.report.delay_samples));
    let best = plan
        .report
        .auto_scores
        .iter()
        .map(|v| v.1)
        .fold(f64::INFINITY, f64::min);
    let d = plan
        .report
        .auto_scores
        .iter()
        .find(|v| v.1 == best)
        .unwrap()
        .0;
    assert_eq!(plan.report.delay_samples, d as usize * 48);
}
#[test]
fn room_tuning_off_on_the_demo() {
    let temp = Temp::demo();
    let config = ProcessingConfig {
        room_mode: "tuning".into(),
        room_tuning_phase_limit: "off".into(),
        ..Default::default()
    };
    let (room, _) = load_room(&temp, &config);
    let plan = room.tuning.unwrap();
    assert_eq!(plan.report.delay_samples, 0);
    assert!(plan.report.speakers.iter().all(|s| !s.phase_corrected));
}

fn load_room(
    temp: &Temp,
    config: &ProcessingConfig,
) -> (impulcifer_dsp::stages::room::RoomCorrection, Events) {
    let dir = discover(&temp.0, config).unwrap();
    let estimator = open_estimator(&dir, Some("default")).unwrap();
    let mut events = Events::default();
    let inputs = load_inputs(&dir, &estimator, config, &mut events).unwrap();
    (inputs.room.unwrap(), events)
}

#[test]
fn room_v2_generic_plot_uses_the_applied_gain() {
    let temp = Temp::new();
    // Only generic room data; load_inputs also requires an HRIR recording.
    for (source, target) in [
        ("FL,FR.wav", "FL,FR.wav"),
        ("room-FL,FR-left.wav", "room.wav"),
    ] {
        std::fs::copy(root().join("data/demo").join(source), temp.0.join(target)).unwrap();
    }
    let (room, _) = load_room(
        &temp,
        &ProcessingConfig {
            plot: true,
            ..Default::default()
        },
    );
    assert!(temp.0.join("plots/room/room.png").is_file());
    assert_eq!(room.frs.term, impulcifer_dsp::stages::room::RoomTerm::Gain);
    assert!(!room.frs.entries.is_empty());
    for (_, _, fr) in &room.frs.entries {
        assert_eq!(fr.name, "generic_room");
        assert_eq!(
            fr.error,
            fr.equalization.iter().map(|g| -g).collect::<Vec<_>>()
        );
    }
}

#[test]
fn room_v2_numeric_options_reach_the_room_stage() {
    let temp = Temp::demo();
    let (room, events) = load_room(
        &temp,
        &ProcessingConfig {
            room_volume: Some(100.0),
            ..Default::default()
        },
    );
    assert!(
        events
            .0
            .iter()
            .any(|(_, key, args)| key == "cli_room_schroeder" && args["volume"] == "100.0")
    );
    assert!(
        !events
            .0
            .iter()
            .any(|(_, key, _)| key == "cli_room_schroeder_assumed")
    );
    // Ensure the fixture has boosts before checking that the zero cap removes them.
    assert!(
        room.frs
            .entries
            .iter()
            .any(|(_, _, fr)| fr.equalization.iter().any(|g| *g > 0.0))
    );

    let (_, events) = load_room(
        &temp,
        &ProcessingConfig {
            schroeder_freq: Some(400.0),
            ..Default::default()
        },
    );
    assert!(
        events
            .0
            .iter()
            .any(|(_, key, args)| key == "cli_room_schroeder_override" && args["freq"] == 400)
    );
    assert!(events.0.iter().any(|(_, key, args)| key == "cli_room_range"
        && args["range"] == "schroeder"
        && args["f_hi"] == 400));

    let (room, _) = load_room(
        &temp,
        &ProcessingConfig {
            room_max_boost: 0.0,
            ..Default::default()
        },
    );
    assert_eq!(room.frs.term, impulcifer_dsp::stages::room::RoomTerm::Gain);
    assert!(!room.frs.entries.is_empty());
    for (_, _, fr) in room.frs.entries {
        assert!(!fr.equalization.is_empty());
        assert!(fr.equalization.iter().all(|g| *g <= 0.0));
    }
}

#[test]
fn room_v2_selfcheck_logs_warning() {
    let temp = Temp::demo();
    std::fs::write(
        temp.0.join("room-target.csv"),
        "frequency,raw\n10,40\n200,40\n500,0\n1000,0\n24000,0\n",
    )
    .unwrap();
    let config = ProcessingConfig {
        room_max_boost: 0.0,
        schroeder_freq: Some(400.0),
        ..Default::default()
    };
    let (room, events) = load_room(&temp, &config);
    assert!(
        room.diagnostics
            .unwrap()
            .ears
            .iter()
            .any(|e| e.residual_rms_db.is_some_and(|r| r > 3.0))
    );
    assert!(events.0.iter().any(|(level, key, args)| level == "warning"
        && key == "cli_room_selfcheck_warning"
        && args["rms"].is_string()
        && args["lo"].is_i64()
        && args["hi"].is_i64()));
}

#[test]
fn room_v2_with_vbass_logs_handoff() {
    let temp = Temp::demo();
    let config = ProcessingConfig {
        vbass: true,
        ..Default::default()
    };
    let (room, events) = load_room(&temp, &config);
    assert!(
        events
            .0
            .iter()
            .any(|(_, k, a)| k == "cli_room_vbass_handoff" && a["freq"] == 250)
    );
    for (_, _, fr) in room.frs.entries {
        assert!(
            fr.frequency
                .iter()
                .zip(fr.equalization)
                .filter(|(f, _)| **f < 250.0 / 2.0_f64.sqrt())
                .all(|(_, g)| g == 0.0)
        );
    }
}
