//! External room impulse responses (ADR 0010); no sweep decoding or recording pairs.
use super::BrirError;
use crate::args::invalid;
use impulcifer_dsp::{
    fr::FrequencyResponse,
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
};
use impulcifer_io::external::{
    ExternalFile, ExternalFormat, Skip,
    assign::{EarSide, format_map, parse_map, slot_for_label},
    read_external,
};
use serde_json::{Value, json};
use std::{collections::HashSet, path::Path};

fn read(path: &Path) -> Result<ExternalFile, BrirError> {
    if let Err(error) = std::fs::metadata(path) {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Err(BrirError::Missing(format!(
                "Measurement file does not exist: {}",
                path.display()
            )));
        }
        return Err(BrirError::Unsupported(format!(
            "Could not read measurement file: {error}"
        )));
    }
    read_external(path).map_err(|error| BrirError::Unsupported(error.to_string()))
}

pub(crate) fn load(
    path: &Path,
    map: &str,
    fs: u32,
) -> Result<(Hrir, Option<FrequencyResponse>, ExternalFormat), BrirError> {
    let file = read(path)?;
    let mapping =
        parse_map(&file, map).map_err(|error| BrirError::Unsupported(error.to_string()))?;
    if mapping.is_empty() {
        return Err(BrirError::Unsupported(
            "No room responses assigned; specify room_measurement_map using measurement IDs."
                .into(),
        ));
    }
    let mut hrir = Hrir {
        fs: file.sample_rate,
        speakers: Vec::new(),
    };
    for assignment in mapping {
        let measurement = file
            .measurements
            .iter()
            .find(|m| m.id == assignment.id)
            .expect("validated map");
        if hrir
            .speakers
            .last()
            .is_none_or(|s| s.speaker != assignment.slot)
        {
            hrir.speakers.push(SpeakerIrs {
                speaker: assignment.slot.into(),
                left: None,
                right: None,
            });
        }
        let speaker = hrir.speakers.last_mut().expect("inserted speaker");
        let ir = Some(ImpulseResponse {
            data: measurement.samples.clone(),
            fs: file.sample_rate,
            recording: None,
        });
        match assignment.side {
            EarSide::Left => speaker.left = ir,
            EarSide::Right => speaker.right = ir,
        }
    }
    if hrir.fs != fs {
        hrir.resample(fs)?;
    }
    let calibration = file
        .mic_calibration
        .map(|cal| FrequencyResponse::new("microphone", Some(cal.frequency), Some(cal.gain_db)))
        .transpose()?;
    Ok((hrir, calibration, file.format))
}

/// Read-only IPC preview. The effective mapping is canonical, independent of input order.
pub fn inspect(request: &Value) -> Result<Value, Value> {
    let object = request
        .as_object()
        .ok_or_else(|| invalid("Request must be an object."))?;
    let path = object
        .get("path")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| invalid("path must be a non-empty string."))?;
    let map = match object.get("map") {
        None | Some(Value::Null) => "",
        Some(Value::String(map)) => map,
        _ => return Err(invalid("map must be a string or null.")),
    };
    let path = std::path::absolute(Path::new(path)).map_err(|e| invalid(e.to_string()))?;
    let file = read(&path).map_err(|e| e.envelope())?;
    let mapping = parse_map(&file, map).map_err(|e| invalid(e.to_string()))?;
    let mut used = HashSet::new();
    let speakers: Vec<_> = file
        .speakers
        .iter()
        .map(|s| {
            let slot = (!s.subwoofer)
                .then(|| slot_for_label(&s.name))
                .flatten()
                .filter(|slot| used.insert(*slot));
            json!({"index":s.index,"name":s.name,"subwoofer":s.subwoofer,"slot":slot})
        })
        .collect();
    Ok(json!({
        "format": match file.format { ExternalFormat::Rew => "rew", ExternalFormat::Dirac => "dirac" },
        "path":path, "title":file.title, "sample_rate":file.sample_rate, "microphone":file.microphone,
        "mic_calibration":file.mic_calibration.is_some(), "positions":file.positions, "speakers":speakers,
        "measurements":file.measurements.iter().map(|m| json!({"id":m.id,"name":m.name,"speaker":m.speaker,"position":m.position,"samples":m.samples.len(),"skipped":m.skipped.map(|Skip::NoImpulseResponse| "no_impulse_response")})).collect::<Vec<_>>(),
        "mapping":mapping.iter().map(|a| json!({"slot":a.slot,"side":match a.side { EarSide::Left => "left", EarSide::Right => "right" },"id":a.id})).collect::<Vec<_>>(),
        "map":format_map(&mapping), "auto":map.trim().is_empty() || map.trim().eq_ignore_ascii_case("auto")
    }))
}
