//! Pure preview naming and sweep validation; never reads or generates audio.
use crate::args::{Args, invalid};
use impulcifer_types::constants::{SPEAKER_NAMES, SWEEP_TRACK_LAYOUTS};
use impulcifer_types::ipc::{self, ErrorCode};
use serde_json::{Value, json};
use std::path::Path;

// Missing from impulcifer-types::constants; mirror core/constants.py:121-122
// locally until a later types packet adds the canonical exports.
pub const DEFAULT_SWEEP_FS: u32 = 48000;
pub const DEFAULT_SWEEP_DURATION: f64 = 5.0;

pub fn resolve(args: &Args) -> Result<Value, Value> {
    args.count(1, 4)?;
    let dir = text(args, 0, "record_dir")?.ok_or_else(|| invalid("record_dir is required."))?;
    let mode = args.string(2, "mode", Some("speakers"))?;
    // Python deliberately bypasses sweep/play validation for headphones.
    let filename = if mode == "headphones" {
        "headphones.wav".to_owned()
    } else if let Some(speakers) = sweep_speakers(args.get(3))? {
        format!("{}.wav", speakers.join(","))
    } else {
        let play = text(args, 1, "play_path")?
            .ok_or_else(|| invalid("play_path is required for speaker recordings."))?;
        derive_filename(&play)
    };
    Ok(json!({"record_path": Path::new(&dir).join(filename).to_string_lossy()}))
}

pub fn text(args: &Args, index: usize, name: &str) -> Result<Option<String>, Value> {
    Ok(args
        .optional_string(index, name)?
        .map(|text| text.trim().to_owned())
        .filter(|s| !s.is_empty()))
}

fn derive_filename(play: &str) -> String {
    // os.path.basename preserves an empty basename for a trailing separator;
    // Path::file_name would instead discard that separator.
    let basename = play.rsplit(std::path::is_separator).next().unwrap_or("");
    let upper = basename.to_uppercase();
    for (index, _) in upper.match_indices("SWEEP-SEG-") {
        let rest = &upper[index + "SWEEP-SEG-".len()..];
        if let Some((speakers, _)) = rest.split_once('-')
            && speakers.split(',').all(|s| SPEAKER_NAMES.contains(&s))
        {
            return format!("{speakers}.wav");
        }
    }
    // Python splitext treats an all-dot prefix as part of a dotfile name.
    let stem = match basename.rfind('.') {
        Some(index) if basename[..index].chars().any(|c| c != '.') => &basename[..index],
        _ => basename,
    };
    format!("{stem}.wav")
}

fn sweep_speakers(sweep: &Value) -> Result<Option<Vec<String>>, Value> {
    if sweep.is_null() {
        return Ok(None);
    }
    let object = sweep
        .as_object()
        .ok_or_else(|| invalid("sweep must be an object."))?;
    let mut unknown: Vec<_> = object
        .keys()
        .filter(|key| !["mode", "fs", "duration", "speakers", "tracks"].contains(&key.as_str()))
        .cloned()
        .collect();
    unknown.sort();
    if !unknown.is_empty() {
        return Err(ipc::error(
            ErrorCode::InvalidRequest,
            "Unknown sweep fields.",
            json!({"fields": unknown}),
            false,
        ));
    }
    let mode = object.get("mode").unwrap_or(&Value::Null);
    let mode = if !object.contains_key("mode") {
        Some("default")
    } else {
        mode.as_str()
    };
    if mode == Some("file") {
        return Ok(None);
    }
    if !matches!(mode, Some("default" | "custom")) {
        return Err(invalid("sweep.mode must be default, custom or file."));
    }
    let raw = object
        .get("speakers")
        .cloned()
        .unwrap_or(json!(["FL", "FR"]));
    let names: Vec<String> = match raw {
        Value::String(value) => value.split(',').map(str::to_owned).collect(),
        Value::Array(value) => value.iter().map(python_string).collect(),
        _ => {
            return Err(invalid(
                "sweep.speakers must be a list or comma-separated string.",
            ));
        }
    };
    let tracks = object.get("tracks").cloned().unwrap_or(json!("stereo"));
    let tracks = tracks
        .as_str()
        .ok_or_else(|| invalid("sweep.tracks must be a string."))?;
    let (fs, duration) = if mode == Some("custom") {
        let fs = object.get("fs").cloned().unwrap_or(json!(DEFAULT_SWEEP_FS));
        let fs = fs
            .as_f64()
            .filter(|n| n.is_finite() && n.fract() == 0.0)
            .ok_or_else(|| invalid("sweep.fs must be an integer."))?;
        let duration = object
            .get("duration")
            .cloned()
            .unwrap_or(json!(DEFAULT_SWEEP_DURATION));
        let duration = duration
            .as_f64()
            .ok_or_else(|| invalid("sweep.duration must be a number."))?;
        (fs, duration)
    } else {
        (f64::from(DEFAULT_SWEEP_FS), DEFAULT_SWEEP_DURATION)
    };
    let names: Vec<String> = names
        .iter()
        .map(|s| s.trim().to_uppercase())
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return Err(invalid("At least one speaker name is required."));
    }
    for name in &names {
        if !SPEAKER_NAMES.contains(&name.as_str()) {
            return Err(invalid(format!(
                "\"{name}\" is not a recognised speaker name."
            )));
        }
    }
    let mut unique = std::collections::HashSet::new();
    if names.iter().any(|s| !unique.insert(s)) {
        return Err(invalid("Speaker names must be unique."));
    }
    if !SWEEP_TRACK_LAYOUTS.contains(&tracks) {
        return Err(invalid(format!(
            "Unsupported track configuration \"{tracks}\". Supported: {}.",
            SWEEP_TRACK_LAYOUTS.join(", ")
        )));
    }
    if tracks == "stereo" && names.len() > 2 {
        return Err(invalid(
            "\"stereo\" track configuration requires one or two speakers.",
        ));
    }
    let order = match tracks {
        "5.1" => Some("FL FR FC LFE BL BR"),
        "7.1" => Some("FL FR FC LFE BL BR SL SR"),
        "7.1.4" => Some("FL FR FC LFE BL BR SL SR TFL TFR TBL TBR"),
        "7.1.6" => Some("FL FR FC LFE BL BR SL SR TFL TFR TSL TSR TBL TBR"),
        _ => None,
    };
    if let Some(order) = order {
        for name in &names {
            if !order.split_whitespace().any(|speaker| speaker == name) {
                return Err(invalid(format!(
                    "Speaker \"{name}\" is not available in the \"{tracks}\" layout."
                )));
            }
        }
    }
    if !(8000.0..=384000.0).contains(&fs) {
        return Err(invalid("Sampling rate must be between 8000 and 384000 Hz."));
    }
    if !(0.1..=60.0).contains(&duration) {
        return Err(invalid(
            "Sweep duration must be between 0.1 and 60 seconds.",
        ));
    }
    Ok(Some(names))
}

fn python_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        _ => value.to_string(),
    }
}
