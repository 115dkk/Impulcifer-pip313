//! Pure preview naming and sweep validation; never reads or generates audio.
use crate::args::{Args, invalid};
use crate::recording::{
    naming,
    request::{Mode, optional_string, validate_sweep},
};
use serde_json::{Value, json};

// Missing from impulcifer-types::constants; mirror core/constants.py:121-122.
pub const DEFAULT_SWEEP_FS: u32 = 48000;
pub const DEFAULT_SWEEP_DURATION: f64 = 5.0;

pub fn resolve(args: &Args) -> Result<Value, Value> {
    args.count(1, 4)?;
    let dir = optional_string(args.get(0)).ok_or_else(|| invalid("record_dir is required."))?;
    let mode = args.string(2, "mode", Some("speakers"))?;
    // Python deliberately bypasses sweep/play validation for headphones.
    let path = if mode == "headphones" {
        naming::resolve_headphones_record_path(&dir)
    } else if let Some(spec) = validate_sweep(args.get(3), Mode::Speakers)? {
        naming::resolve_record_path_for_speakers(&dir, &spec.speakers).map_err(invalid)?
    } else {
        let play = optional_string(args.get(1))
            .ok_or_else(|| invalid("play_path is required for speaker recordings."))?;
        naming::resolve_record_path(&dir, &play)
    };
    Ok(json!({"record_path": path}))
}

pub fn text(args: &Args, index: usize, name: &str) -> Result<Option<String>, Value> {
    Ok(args
        .optional_string(index, name)?
        .map(|text| text.trim().to_owned())
        .filter(|s| !s.is_empty()))
}
