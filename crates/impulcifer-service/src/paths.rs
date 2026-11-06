//! Pure preview naming and sweep validation; never reads or generates audio.
//! The preview reads the folder only to say where the run's recording lands
//! (recording::session) and changes nothing.
use crate::args::{Args, invalid};
use crate::recording::{
    naming,
    request::{Mode, optional_string, validate_sweep},
    session::Sessions,
};
use serde_json::{Value, json};
use std::path::Path;

// Missing from impulcifer-types::constants; mirror core/constants.py:121-122.
pub const DEFAULT_SWEEP_FS: u32 = 48000;
pub const DEFAULT_SWEEP_DURATION: f64 = 5.0;

pub fn resolve(args: &Args, sessions: &Sessions) -> Result<Value, Value> {
    args.count(1, 5)?;
    let dir = optional_string(args.get(0)).ok_or_else(|| invalid("record_dir is required."))?;
    let mode = args.string(2, "mode", Some("speakers"))?;
    let append = match args.get(4) {
        Value::Null => false,
        value => value
            .as_bool()
            .ok_or_else(|| invalid("append must be a boolean."))?,
    };
    // Python deliberately bypasses sweep/play validation for headphones.
    let (mode, path) = if mode == "headphones" {
        (
            Mode::Headphones,
            naming::resolve_headphones_record_path(&dir),
        )
    } else if let Some(spec) = validate_sweep(args.get(3), Mode::Speakers)? {
        (
            Mode::Speakers,
            naming::resolve_record_path_for_speakers(&dir, &spec.speakers).map_err(invalid)?,
        )
    } else {
        let play = optional_string(args.get(1))
            .ok_or_else(|| invalid("play_path is required for speaker recordings."))?;
        (Mode::Speakers, naming::resolve_record_path(&dir, &play))
    };
    let name = Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let placement = sessions.place(mode, append, &dir, &name);
    Ok(json!({"record_path": placement.record_path}))
}

pub fn text(args: &Args, index: usize, name: &str) -> Result<Option<String>, Value> {
    Ok(args
        .optional_string(index, name)?
        .map(|text| text.trim().to_owned())
        .filter(|s| !s.is_empty()))
}
