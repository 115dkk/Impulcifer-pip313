//! Ordered Python request validation. Do not deserialize before checking errors.
use super::{
    naming,
    sweep::{SweepSpec, validate_sweep_spec},
};
use crate::args::invalid;
use impulcifer_types::ipc::{self, ErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Speakers,
    Headphones,
}

/// Raw wire fields remain Values to preserve Python's validation precedence.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RecordingRequest {
    pub mode: Value,
    pub play_path: Value,
    pub record_dir: Value,
    pub input_device: Value,
    pub output_device: Value,
    pub host_api: Value,
    pub channels: Value,
    pub force_channels: Value,
    pub append: Value,
    pub debug_plots: Value,
    pub confirm_warnings: Value,
    pub sweep: Value,
}
#[derive(Clone, Debug, Serialize)]
pub struct ValidatedRecording {
    pub mode: Mode,
    pub play_path: Option<String>,
    pub sweep_spec: Option<SweepSpec>,
    pub record_path: String,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub host_api: Option<String>,
    pub channels: u16,
    pub append: bool,
    pub debug_plots: bool,
}
pub fn python_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(python_repr).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", python_repr(&json!(k)), python_repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => v.to_string(),
    }
}
fn python_repr(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
    } else {
        python_string(v)
    }
}
pub fn optional_string(v: &Value) -> Option<String> {
    if v.is_null() {
        None
    } else {
        let s = python_string(v).trim().to_owned();
        (!s.is_empty()).then_some(s)
    }
}
fn unknown(
    v: &serde_json::Map<String, Value>,
    fields: &[&str],
    message: &str,
) -> Result<(), Value> {
    let mut names: Vec<_> = v.keys().filter(|k| !fields.contains(&k.as_str())).collect();
    names.sort();
    if names.is_empty() {
        Ok(())
    } else {
        Err(ipc::error(
            ErrorCode::InvalidRequest,
            message,
            json!({"fields":names}),
            false,
        ))
    }
}
pub fn validate_sweep(sweep: &Value, mode: Mode) -> Result<Option<SweepSpec>, Value> {
    if sweep.is_null() {
        return Ok(None);
    }
    let o = sweep
        .as_object()
        .ok_or_else(|| invalid("sweep must be an object."))?;
    unknown(
        o,
        &["mode", "fs", "duration", "speakers", "tracks"],
        "Unknown sweep fields.",
    )?;
    let m = o.get("mode").cloned().unwrap_or(json!("default"));
    let m = m
        .as_str()
        .filter(|m| ["default", "custom", "file"].contains(m))
        .ok_or_else(|| invalid("sweep.mode must be default, custom or file."))?;
    if m == "file" {
        return Ok(None);
    }
    let mut spec = SweepSpec::default();
    if mode != Mode::Headphones {
        let names = o.get("speakers").cloned().unwrap_or(json!(["FL", "FR"]));
        spec.speakers = match names {
            Value::String(s) => s.split(',').map(str::to_owned).collect(),
            Value::Array(a) => a.iter().map(python_string).collect(),
            _ => {
                return Err(invalid(
                    "sweep.speakers must be a list or comma-separated string.",
                ));
            }
        };
        spec.tracks = o
            .get("tracks")
            .cloned()
            .unwrap_or(json!("stereo"))
            .as_str()
            .ok_or_else(|| invalid("sweep.tracks must be a string."))?
            .into();
    }
    if m == "custom" {
        let fs = o.get("fs").cloned().unwrap_or(json!(48000));
        let fs = fs
            .as_f64()
            .filter(|n| n.is_finite() && n.fract() == 0.0)
            .ok_or_else(|| invalid("sweep.fs must be an integer."))?;
        spec.duration = o
            .get("duration")
            .cloned()
            .unwrap_or(json!(5.0))
            .as_f64()
            .ok_or_else(|| invalid("sweep.duration must be a number."))?;
        // Out-of-range rates still follow speaker/layout errors, as in Python.
        spec.fs = if (0.0..=u32::MAX as f64).contains(&fs) {
            fs as u32
        } else {
            0
        };
    }
    validate_sweep_spec(spec).map(Some).map_err(invalid)
}
fn boolean(o: &serde_json::Map<String, Value>, name: &str) -> Result<bool, Value> {
    o.get(name).map_or(Ok(false), |v| {
        v.as_bool()
            .ok_or_else(|| invalid(format!("{name} must be a boolean.")))
    })
}
// Python re.search(r'([A-Z]{2,3}(,[A-Z]{2,3})*)', basename), not a known-speaker lookup.
fn filename_speakers(path: &str) -> Option<Vec<String>> {
    let name = path.rsplit(std::path::is_separator).next().unwrap_or("");
    let b = name.as_bytes();
    for start in 0..b.len().saturating_sub(1) {
        if !b[start].is_ascii_uppercase() || !b[start + 1].is_ascii_uppercase() {
            continue;
        }
        let mut pos = start;
        let mut names = Vec::new();
        loop {
            let begin = pos;
            while pos < b.len() && pos - begin < 3 && b[pos].is_ascii_uppercase() {
                pos += 1;
            }
            names.push(name[begin..pos].to_owned());
            if pos + 2 < b.len()
                && b[pos] == b','
                && b[pos + 1].is_ascii_uppercase()
                && b[pos + 2].is_ascii_uppercase()
            {
                pos += 1;
            } else {
                break;
            }
        }
        return Some(names);
    }
    None
}
pub fn validate(request: &Value) -> Result<ValidatedRecording, Value> {
    let o = request
        .as_object()
        .ok_or_else(|| invalid("Recording request must be an object."))?;
    unknown(
        o,
        &[
            "mode",
            "play_path",
            "record_dir",
            "input_device",
            "output_device",
            "host_api",
            "channels",
            "force_channels",
            "append",
            "debug_plots",
            "confirm_warnings",
            "sweep",
        ],
        "Unknown recording fields.",
    )?;
    let mode = match o.get("mode").cloned().unwrap_or(json!("speakers")).as_str() {
        Some("speakers") => Mode::Speakers,
        Some("headphones") => Mode::Headphones,
        _ => return Err(invalid("mode must be speakers or headphones.")),
    };
    let dir = python_string(o.get("record_dir").unwrap_or(&json!("")))
        .trim()
        .to_owned();
    if dir.is_empty() {
        return Err(invalid("record_dir is required."));
    }
    let sweep_spec = validate_sweep(&request["sweep"], mode)?;
    let play_path = if sweep_spec.is_some() {
        None
    } else {
        let play = python_string(o.get("play_path").unwrap_or(&json!("")))
            .trim()
            .to_owned();
        if !Path::new(&play).is_file() {
            return Err(ipc::error(
                ErrorCode::FileNotFound,
                "Playback file does not exist.",
                json!({"path":play}),
                false,
            ));
        }
        Some(play)
    };
    let mut channels =
        o.get("channels")
            .cloned()
            .unwrap_or(json!(2))
            .as_u64()
            .filter(|n| (1..=64).contains(n))
            .ok_or_else(|| invalid("channels must be an integer from 1 to 64."))? as u16;
    let force = boolean(o, "force_channels")?;
    let confirm = boolean(o, "confirm_warnings")?;
    let (append, record_path) = if mode == Mode::Headphones {
        if let Some(play) = &play_path {
            let wav = impulcifer_io::read_wav(Path::new(play));
            let count = wav.as_ref().map(|w| w.tracks.len()).unwrap_or(0);
            if count == 0 || count > 2 {
                let key = if !Path::new(play).exists() {
                    "error_headphones_play_file_missing"
                } else if count > 2 {
                    "error_headphones_play_file_too_many_channels"
                } else {
                    "error_headphones_play_file_unreadable"
                };
                return Err(ipc::error(
                    ErrorCode::InvalidRequest,
                    key,
                    json!({"path":play,"channels":count}),
                    false,
                ));
            }
            if count == 1 && !confirm {
                return Err(ipc::error(
                    ErrorCode::ConfirmationRequired,
                    "Mono playback produces generic L=R headphone compensation.",
                    json!({"warning":"headphones_mono"}),
                    false,
                ));
            }
        }
        channels = 2;
        (false, naming::resolve_headphones_record_path(&dir))
    } else {
        let append = boolean(o, "append")?;
        if !force {
            channels = 2;
        }
        let path = if let Some(spec) = &sweep_spec {
            naming::resolve_record_path_for_speakers(&dir, &spec.speakers).map_err(invalid)?
        } else {
            naming::resolve_record_path(&dir, play_path.as_deref().unwrap_or(""))
        };
        if force
            && !confirm
            && let Some(names) = filename_speakers(&path)
            && names.len() * 2 != usize::from(channels)
        {
            return Err(ipc::error(
                ErrorCode::ConfirmationRequired,
                "Selected recording channels do not match the sweep speaker count.",
                json!({"has_mismatch":true,"expected_channels":names.len()*2,"expected_speakers":names,"selected_channels":channels}),
                false,
            ));
        }
        (append, path)
    };
    let debug_plots = boolean(o, "debug_plots")?;
    Ok(ValidatedRecording {
        mode,
        play_path,
        sweep_spec,
        record_path,
        input_device: optional_string(&request["input_device"]),
        output_device: optional_string(&request["output_device"]),
        host_api: optional_string(&request["host_api"]),
        channels,
        append,
        debug_plots,
    })
}
