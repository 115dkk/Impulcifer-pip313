//! IPC seconds-based validation, application/impulcifer_service.py:1199-1329.
use crate::args::invalid;
use impulcifer_types::{
    config::{FIELD_NAMES, ProcessingConfig},
    constants::SPEAKER_NAMES,
    ipc::{self, ErrorCode},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) struct Request {
    pub config: ProcessingConfig,
    pub sidecars: Vec<(PathBuf, PathBuf)>,
}
pub(crate) fn validate(value: &Value) -> Result<Request, Value> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("BRIR request must be an object."))?;
    let sidecars = [
        ("eq_file", "eq.csv"),
        ("eq_left_file", "eq-left.csv"),
        ("eq_right_file", "eq-right.csv"),
    ];
    let unknown: Vec<_> = object
        .keys()
        .filter(|k| !FIELD_NAMES.contains(&k.as_str()) && !sidecars.iter().any(|(f, _)| f == k))
        .collect();
    if !unknown.is_empty() {
        return Err(ipc::error(
            ErrorCode::InvalidRequest,
            "Unknown BRIR fields.",
            json!({"fields":unknown}),
            false,
        ));
    }
    let dir = object
        .get("dir_path")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if !Path::new(dir).is_dir() {
        return Err(ipc::error(
            ErrorCode::FileNotFound,
            "Measurement directory does not exist.",
            json!({"path":dir}),
            false,
        ));
    }
    let mut params = object.clone();
    params.insert("dir_path".into(), json!(dir));
    for name in [
        "test_signal",
        "room_target",
        "room_mic_calibration",
        "headphone_compensation_file",
    ] {
        if let Some(v) = params.get_mut(name)
            && let Some(s) = v.as_str()
        {
            *v = if s.trim().is_empty() {
                Value::Null
            } else {
                json!(s.trim())
            };
        }
    }
    for name in ["fs", "vbass_freq"] {
        if let Some(v) = params.get_mut(name)
            && !(name == "fs" && v.is_null())
        {
            let n = v
                .as_f64()
                .filter(|n| n.is_finite() && n.fract() == 0.0)
                .ok_or_else(|| invalid(format!("{name} must be an integer.")))?;
            let n = if name == "vbass_freq" {
                n.clamp(30.0, 500.0)
            } else {
                n
            };
            if n < 0.0 || n > u32::MAX as f64 {
                return Err(invalid(format!("{name} is outside supported range.")));
            }
            *v = json!(n as u32);
        }
    }
    for (name, choices) in [
        ("fr_combination_method", &["average", "conservative"][..]),
        ("vbass_polarity", &["auto", "normal", "invert"][..]),
    ] {
        if let Some(v) = params.get(name)
            && !v.as_str().is_some_and(|s| choices.contains(&s))
        {
            return Err(invalid(format!(
                "{name} must be one of: {}.",
                choices.join(", ")
            )));
        }
    }
    if let Some(v) = params.get_mut("channel_balance")
        && !v.is_null()
    {
        if v.is_number() {
            *v = json!(v.to_string());
        } else if !v
            .as_str()
            .is_some_and(|s| ["trend", "left", "right", "avg", "min", "mids"].contains(&s))
        {
            return Err(invalid(
                "channel_balance must be a dB number or one of: trend, left, right, avg, min, mids.",
            ));
        }
    }
    if let Some(v) = params.get_mut("decay")
        && !v.is_null()
    {
        if let Some(n) = v.as_f64().filter(|n| *n > 0.0) {
            *v = json!(
                SPEAKER_NAMES
                    .iter()
                    .map(|s| ((*s).to_owned(), json!(n)))
                    .collect::<serde_json::Map<_, _>>()
            );
        } else if !v.as_object().is_some_and(|m| {
            !m.is_empty()
                && m.iter().all(|(k, v)| {
                    SPEAKER_NAMES.contains(&k.as_str()) && v.as_f64().is_some_and(|n| n > 0.0)
                })
        }) {
            return Err(invalid(
                "decay must be a positive number of seconds or a {channel: seconds} object for FL/FC/FR/SL/SR/BL/BR.",
            ));
        }
    }
    let mut copies = Vec::new();
    for (field, target) in sidecars {
        let raw = params.remove(field).unwrap_or(Value::Null);
        if raw.is_null() {
            continue;
        }
        let text = raw
            .as_str()
            .ok_or_else(|| invalid(format!("{field} must be a string path.")))?
            .trim();
        if text.is_empty() || text == target {
            continue;
        }
        let source = Path::new(dir).join(text);
        let destination = Path::new(dir).join(target);
        if std::path::absolute(&source).ok() == std::path::absolute(&destination).ok() {
            continue;
        }
        if !source.is_file() {
            return Err(ipc::error(
                ErrorCode::FileNotFound,
                "Custom EQ file does not exist.",
                json!({"field":field,"path":text}),
                false,
            ));
        }
        copies.push((source, destination));
    }
    let config = ProcessingConfig::from_kwargs(&params).map_err(|e| invalid(e.to_string()))?;
    Ok(Request {
        config,
        sidecars: copies,
    })
}
