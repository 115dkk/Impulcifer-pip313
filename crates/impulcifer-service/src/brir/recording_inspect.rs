//! Read-only sweep locations for one two-ear BRIR recording.

use super::{
    BrirError,
    discovery::{EqDiscovery, MeasurementDir, RoomDiscovery, discover},
    estimator::{self, open_with_events},
    recording_file::{read_recording, read_sweep_reference},
};
use crate::args::invalid;
use impulcifer_dsp::sweep_import::{
    NO_SWEEP_FOUND, NOT_LOGARITHMIC_SWEEP, ReferenceSweep, locate_sweeps, trim_reference,
};
use impulcifer_types::config::ProcessingConfig;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<Option<&'a str>, Value> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok((!value.trim().is_empty()).then(|| value.trim())),
        Some(_) => Err(invalid(format!("{field} must be a string."))),
    }
}

fn empty_dir(path: PathBuf) -> MeasurementDir {
    MeasurementDir {
        dir: path,
        recordings: Vec::new(),
        headphones: None,
        room: RoomDiscovery {
            recordings: Vec::new(),
            generic: None,
            target: None,
            calibration: None,
        },
        eq: EqDiscovery {
            common: None,
            left: None,
            right: None,
            deprecated_wav: false,
        },
        test_signal: None,
    }
}

fn measurement_dir(path: Option<&str>) -> Result<MeasurementDir, BrirError> {
    let path = PathBuf::from(path.unwrap_or_default());
    if path.is_dir() {
        discover(&path, &ProcessingConfig::default())
    } else {
        Ok(empty_dir(path))
    }
}

fn test_signal_reference(
    dir: &MeasurementDir,
    test_signal: Option<&str>,
    data: &Path,
    recording_fs: u32,
) -> Result<(ReferenceSweep, PathBuf), BrirError> {
    let estimator = open_with_events(dir, test_signal, data, true, None, true, None)?;
    let source = match test_signal {
        Some("auto") | None => dir
            .test_signal
            .clone()
            .unwrap_or_else(|| data.join(estimator::DEFAULT_SIGNAL)),
        Some(signal) => PathBuf::from(signal),
    };
    if estimator.fs != recording_fs {
        return Err(BrirError::RecordingSweepRate {
            path: source,
            sweep_fs: estimator.fs,
            recording_fs,
        });
    }
    let reference = trim_reference(&estimator.test_signal, estimator.fs).map_err(|_| {
        BrirError::RecordingSweepFile {
            path: source.clone(),
        }
    })?;
    Ok((reference, source))
}

fn hundredths(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

pub(crate) fn inspect(request: &Value, data: &Path) -> Result<Value, Value> {
    let object = request
        .as_object()
        .ok_or_else(|| invalid("request must be an object."))?;
    let recording_file =
        text(object, "recording_file")?.ok_or_else(|| invalid("recording_file is required."))?;
    let recording_sweep = text(object, "recording_sweep")?;
    let test_signal = text(object, "test_signal")?;
    let dir_path = text(object, "dir_path")?;

    let recording_path = PathBuf::from(recording_file);
    let recording = read_recording(&recording_path).map_err(|error| error.envelope())?;
    if recording.tracks.len() < 2 {
        return Err(BrirError::Unsupported(
            "brir_recording_file must have at least two tracks".into(),
        )
        .envelope());
    }

    let (reference, reference_kind, reference_path) = if let Some(sweep) = recording_sweep {
        let path = PathBuf::from(sweep);
        let reference =
            read_sweep_reference(&path, recording.sample_rate).map_err(|error| error.envelope())?;
        (reference, "file", path)
    } else {
        let dir = measurement_dir(dir_path).map_err(|error| error.envelope())?;
        let (reference, path) =
            test_signal_reference(&dir, test_signal, data, recording.sample_rate)
                .map_err(|error| error.envelope())?;
        (reference, "test_signal", path)
    };

    let locations = match locate_sweeps(&recording.tracks[0], &recording.tracks[1], &reference) {
        Ok(locations) => locations,
        Err(impulcifer_dsp::DspError::InvalidArgument(message)) if message == NO_SWEEP_FOUND => {
            return Ok(json!({
                "found": 0,
                "starts_seconds": [],
                "sweep_seconds": hundredths(reference.samples.len() as f64 / f64::from(reference.fs)),
                "fs": recording.sample_rate,
                "reference": reference_kind,
            }));
        }
        Err(impulcifer_dsp::DspError::InvalidArgument(message))
            if message == NOT_LOGARITHMIC_SWEEP =>
        {
            return Err(BrirError::RecordingSweepFile {
                path: reference_path,
            }
            .envelope());
        }
        Err(error) => return Err(BrirError::Unsupported(error.to_string()).envelope()),
    };
    let starts_seconds: Vec<_> = locations
        .picks
        .iter()
        .map(|pick| hundredths(*pick as f64 / f64::from(recording.sample_rate)))
        .collect();
    Ok(json!({
        "found": locations.picks.len(),
        "starts_seconds": starts_seconds,
        "sweep_seconds": hundredths(reference.samples.len() as f64 / f64::from(reference.fs)),
        "fs": recording.sample_rate,
        "reference": reference_kind,
    }))
}
