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
