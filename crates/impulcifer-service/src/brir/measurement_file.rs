//! External impulse responses (ADR 0010/0011); no sweep decoding or recording pairs.
use super::{BrirError, BrirEvents};
use crate::args::invalid;
use impulcifer_dsp::{
    fr::FrequencyResponse,
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
};
use impulcifer_io::external::{
    ExternalFile, ExternalFormat, Skip,
    assign::{
        EarSide, Purpose, auto_assign_for, check_assignments, format_map_for, parse_map_for,
        slot_for_label,
    },
    read_external,
};
use impulcifer_types::layouts::SLOTS;
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

pub(crate) fn load_room(
    path: &Path,
    map: &str,
    fs: u32,
) -> Result<(Hrir, Option<FrequencyResponse>, ExternalFormat), BrirError> {
    let file = read(path)?;
    let mapping = parse_map_for(&file, map, Purpose::Room)
        .map_err(|error| BrirError::Unsupported(error.to_string()))?;
    if mapping.is_empty() {
        return Err(BrirError::Unsupported(
            "No room responses assigned; specify room_measurement_map using measurement IDs."
                .into(),
        ));
    }
    let mut hrir = build_direct(&file, &mapping);
    if hrir.fs != fs {
        hrir.resample(fs)?;
    }
    let calibration = file
        .mic_calibration
        .map(|cal| FrequencyResponse::new("microphone", Some(cal.frequency), Some(cal.gain_db)))
        .transpose()?;
    Ok((hrir, calibration, file.format))
}

pub(crate) fn load_brir(
    path: &Path,
    map: &str,
    fs: u32,
    events: &mut dyn BrirEvents,
) -> Result<(Hrir, Vec<SpeakerIrs>, ExternalFormat), BrirError> {
    let file = read(path)?;
    let mapping = parse_map_for(&file, map, Purpose::Brir)
        .map_err(|error| BrirError::Unsupported(error.to_string()))?;
    check_assignments(&mapping, Purpose::Brir)
        .map_err(|error| BrirError::Unsupported(error.to_string()))?;
    let t0 = mapping
        .iter()
        .map(|a| measurement(&file, &a.id).start_time)
        .fold(f64::INFINITY, f64::min);
    let mut responses = Vec::with_capacity(mapping.len());
    let mut largest_padding = 0usize;
    for assignment in &mapping {
        let source = measurement(&file, &assignment.id);
        let difference = source.start_time - t0;
        if difference > 10.0 {
            return Err(BrirError::Unsupported(
                "measurements are more than 10 s apart in time".into(),
            ));
        }
        let padding = (difference * f64::from(file.sample_rate)).round() as usize;
        largest_padding = largest_padding.max(padding);
        let mut samples = vec![0.0; padding];
        samples.extend_from_slice(&source.samples);
        responses.push((assignment, samples));
    }
    let common = responses
        .iter()
        .map(|(_, samples)| samples.len())
        .min()
        .unwrap_or(0);
    if common <= largest_padding {
        return Err(BrirError::Unsupported(
            "measurements do not overlap in time".into(),
        ));
    }
    let build_pair = |slot: &'static str| {
        let selected: Vec<_> = responses
            .iter()
            .filter(|(assignment, _)| assignment.slot == slot)
            .collect();
        if selected.is_empty() {
            return None;
        }
        let mut speaker = SpeakerIrs {
            speaker: slot.into(),
            left: None,
            right: None,
        };
        for (assignment, samples) in selected {
            let ir = Some(ImpulseResponse {
                data: samples[..common].to_vec(),
                fs: file.sample_rate,
                recording: None,
            });
            match assignment.side {
                EarSide::Left => speaker.left = ir,
                EarSide::Right => speaker.right = ir,
            }
        }
        Some(speaker)
    };
    let mut hrir = Hrir {
        fs: file.sample_rate,
        speakers: SLOTS
            .iter()
            .filter_map(|slot| build_pair(slot.code))
            .collect(),
    };
    let mut lfe_hrir = Hrir {
        fs: file.sample_rate,
        speakers: ["LFE", "LFE2"].into_iter().filter_map(build_pair).collect(),
    };
    if hrir.fs != fs {
        hrir.resample(fs)?;
        lfe_hrir.resample(fs)?;
    }
    events.log(
        "info",
        "cli_brir_measurement_file_loaded",
        json!({
            "file": path.file_name().unwrap_or_default().to_string_lossy(),
            "format": match file.format {
                ExternalFormat::Rew => "REW",
                ExternalFormat::Dirac => "Dirac Live",
            },
            "count": hrir.speakers.len(),
        }),
    );
    if !lfe_hrir.speakers.is_empty() {
        events.log(
            "info",
            "cli_brir_measurement_file_lfe",
            json!({"list":lfe_hrir.speakers.iter().map(|speaker| speaker.speaker.as_str()).collect::<Vec<_>>().join(", ")}),
        );
    }
    log_itd(&hrir, events);
    Ok((hrir, lfe_hrir.speakers, file.format))
}

pub(crate) fn load_headphones(
    path: &Path,
    map: &str,
    fs: u32,
) -> Result<(Hrir, ExternalFormat, String, String), BrirError> {
    let file = read(path)?;
    let mapping = parse_map_for(&file, map, Purpose::Headphones)
        .map_err(|error| BrirError::Unsupported(error.to_string()))?;
    check_assignments(&mapping, Purpose::Headphones)
        .map_err(|error| BrirError::Unsupported(error.to_string()))?;
    let left = mapping.iter().find(|a| a.side == EarSide::Left).unwrap();
    let right = mapping.iter().find(|a| a.side == EarSide::Right).unwrap();
    let left_measurement = measurement(&file, &left.id);
    let right_measurement = measurement(&file, &right.id);
    let length = left_measurement
        .samples
        .len()
        .min(right_measurement.samples.len());
    let mut hrir = Hrir {
        fs: file.sample_rate,
        speakers: vec![
            SpeakerIrs {
                speaker: "FL".into(),
                left: Some(ImpulseResponse {
                    data: left_measurement.samples[..length].to_vec(),
                    fs: file.sample_rate,
                    recording: None,
                }),
                right: None,
            },
            SpeakerIrs {
                speaker: "FR".into(),
                left: None,
                right: Some(ImpulseResponse {
                    data: right_measurement.samples[..length].to_vec(),
                    fs: file.sample_rate,
                    recording: None,
                }),
            },
        ],
    };
    if hrir.fs != fs {
        hrir.resample(fs)?;
    }
    Ok((
        hrir,
        file.format,
        left_measurement.name.clone(),
        right_measurement.name.clone(),
    ))
}

fn measurement<'a>(
    file: &'a ExternalFile,
    id: &str,
) -> &'a impulcifer_io::external::ExternalMeasurement {
    file.measurements
        .iter()
        .find(|measurement| measurement.id == id)
        .expect("validated map")
}

fn build_direct(
    file: &ExternalFile,
    mapping: &[impulcifer_io::external::assign::Assignment],
) -> Hrir {
    let mut hrir = Hrir {
        fs: file.sample_rate,
        speakers: Vec::new(),
    };
    for assignment in mapping {
        let source = measurement(file, &assignment.id);
        if hrir
            .speakers
            .last()
            .is_none_or(|speaker| speaker.speaker != assignment.slot)
        {
            hrir.speakers.push(SpeakerIrs {
                speaker: assignment.slot.into(),
                left: None,
                right: None,
            });
        }
        let speaker = hrir.speakers.last_mut().expect("inserted speaker");
        let ir = Some(ImpulseResponse {
            data: source.samples.clone(),
            fs: file.sample_rate,
            recording: None,
        });
        match assignment.side {
            EarSide::Left => speaker.left = ir,
            EarSide::Right => speaker.right = ir,
        }
    }
    hrir
}

pub(crate) fn log_itd(hrir: &Hrir, events: &mut dyn BrirEvents) {
    let mut list = Vec::new();
    let mut off_median = Vec::new();
    for speaker in &hrir.speakers {
        let (Some(left), Some(right)) = (&speaker.left, &speaker.right) else {
            continue;
        };
        let samples =
            right.peak_index(0, None, 0.12589) as i64 - left.peak_index(0, None, 0.12589) as i64;
        let ms = samples as f64 * 1000.0 / f64::from(hrir.fs);
        list.push(format!("{} {ms:+.2}", speaker.speaker));
        if ms.abs() > 1.0 {
            events.log(
                "warning",
                "cli_brir_measurement_file_itd_large",
                json!({"speaker":speaker.speaker,"ms":format!("{ms:+.2}")}),
            );
        }
        if SLOTS
            .iter()
            .find(|slot| slot.code == speaker.speaker)
            .is_some_and(|slot| slot.azimuth != 0.0 && slot.azimuth.abs() != 180.0)
        {
            off_median.push(samples);
        }
    }
    events.log(
        "info",
        "cli_brir_measurement_file_itd",
        json!({"list":list.join(", ")}),
    );
    if off_median.len() >= 2 && off_median.iter().all(|samples| samples.abs() <= 1) {
        events.log("warning", "cli_brir_measurement_file_itd_zero", json!({}));
    }
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
    let purpose = match object.get("purpose") {
        None | Some(Value::Null) => Purpose::Room,
        Some(Value::String(purpose)) => Purpose::parse(purpose)
            .ok_or_else(|| invalid("purpose must be room, brir or headphones."))?,
        _ => return Err(invalid("purpose must be room, brir or headphones.")),
    };
    let path = std::path::absolute(Path::new(path)).map_err(|error| invalid(error.to_string()))?;
    let file = read(&path).map_err(|error| error.envelope())?;
    let mapping = parse_map_for(&file, map, purpose).map_err(|error| invalid(error.to_string()))?;
    let automatic = auto_assign_for(&file, purpose);
    let mut used = HashSet::new();
    let speakers: Vec<_> = file
        .speakers
        .iter()
        .map(|speaker| {
            let slot = if speaker.subwoofer && purpose == Purpose::Brir {
                let prefix = format!("{}@", speaker.index);
                automatic
                    .iter()
                    .find(|assignment| assignment.id.starts_with(&prefix))
                    .map(|assignment| assignment.slot)
            } else if speaker.subwoofer {
                None
            } else {
                slot_for_label(&speaker.name).filter(|slot| used.insert(*slot))
            };
            json!({"index":speaker.index,"name":speaker.name,"subwoofer":speaker.subwoofer,"slot":slot})
        })
        .collect();
    Ok(json!({
        "format": match file.format { ExternalFormat::Rew => "rew", ExternalFormat::Dirac => "dirac" },
        "purpose":purpose.as_str(), "path":path, "title":file.title, "sample_rate":file.sample_rate,
        "microphone":file.microphone, "mic_calibration":file.mic_calibration.is_some(),
        "positions":file.positions, "speakers":speakers,
        "measurements":file.measurements.iter().map(|m| json!({"id":m.id,"name":m.name,"speaker":m.speaker,"position":m.position,"samples":m.samples.len(),"skipped":m.skipped.map(|Skip::NoImpulseResponse| "no_impulse_response")})).collect::<Vec<_>>(),
        "mapping":mapping.iter().map(|a| json!({"slot":a.slot,"side":match a.side { EarSide::Left => "left", EarSide::Right => "right" },"id":a.id})).collect::<Vec<_>>(),
        "map":format_map_for(&mapping, purpose),
        "auto":map.trim().is_empty() || map.trim().eq_ignore_ascii_case("auto")
    }))
}
