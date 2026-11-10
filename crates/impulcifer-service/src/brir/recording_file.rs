//! BRIR measurements imported from one two-ear WAV recording.

use super::{BrirError, BrirEvents, measurement_file::log_itd};
use impulcifer_dsp::{
    estimator::SweepEstimator,
    hrir::{Hrir, NO_VALID_COLUMNS, SpeakerIrs},
    ir::ImpulseResponse,
    sweep_import::{
        NO_SWEEP_FOUND, NOT_LOGARITHMIC_SWEEP, SweepImport, import_sweeps, trim_reference,
    },
};
use impulcifer_io::{read_wav, wav::Wav};
use impulcifer_types::{config::parse_recording_speakers, layouts::SLOTS};
use serde_json::json;
use std::path::Path;

const SILENCE: f64 = 2.0;

fn read_recording(path: &Path) -> Result<Wav, BrirError> {
    if let Err(error) = std::fs::metadata(path) {
        if error.kind() == std::io::ErrorKind::NotFound {
            return Err(BrirError::Missing(format!(
                "Recording file does not exist: {}",
                path.display()
            )));
        }
        return Err(BrirError::Unsupported(format!(
            "Could not read recording file: {error}"
        )));
    }
    Ok(read_wav(path)?)
}

fn take_lfe(hrir: &mut Hrir) -> Vec<SpeakerIrs> {
    let mut lfe = Vec::new();
    for name in ["LFE", "LFE2"] {
        if let Some(index) = hrir
            .speakers
            .iter()
            .position(|speaker| speaker.speaker == name)
        {
            lfe.push(hrir.speakers.remove(index));
        }
    }
    lfe
}

fn complete(pair: &SpeakerIrs) -> bool {
    pair.left.is_some() && pair.right.is_some()
}

fn folder_method(
    path: &Path,
    wav: &Wav,
    entries: &[String],
    estimator: &SweepEstimator,
) -> Result<(Hrir, Vec<SpeakerIrs>), BrirError> {
    let mut hrir = Hrir {
        fs: estimator.fs,
        speakers: Vec::new(),
    };
    let names: Vec<_> = entries.iter().map(String::as_str).collect();
    // Only the two ear tracks: a folder recording takes every further pair of
    // tracks as further speakers, which this file's later tracks are not.
    let opened = hrir.open_recording_samples(
        estimator,
        wav.sample_rate,
        &wav.tracks[..2],
        &names,
        None,
        SILENCE,
    );
    if !matches!(
        &opened,
        Err(impulcifer_dsp::DspError::InvalidArgument(message))
            if message.starts_with(NO_VALID_COLUMNS)
    ) {
        opened?;
    }
    let lfe = take_lfe(&mut hrir);
    let listed = entries.iter().filter(|entry| entry.as_str() != "X").count();
    let read = entries
        .iter()
        .filter(|entry| entry.as_str() != "X")
        .filter(|entry| {
            let name = entry.as_str();
            hrir.get(name)
                .or_else(|| lfe.iter().find(|pair| pair.speaker == name))
                .is_some_and(complete)
        })
        .count();
    if read != listed {
        return Err(BrirError::RecordingTestSignal {
            path: path.to_owned(),
            read,
            listed,
        });
    }
    Ok((hrir, lfe))
}

fn imported_pair(name: String, samples: [Vec<f64>; 2], fs: u32) -> SpeakerIrs {
    let [left, right] = samples;
    SpeakerIrs {
        speaker: name,
        left: Some(ImpulseResponse {
            data: left,
            fs,
            recording: None,
        }),
        right: Some(ImpulseResponse {
            data: right,
            fs,
            recording: None,
        }),
    }
}

fn located_method(
    recording: &Wav,
    entries: &[String],
    sweep_path: &Path,
    estimator: &SweepEstimator,
    events: &mut dyn BrirEvents,
    recording_path: &Path,
) -> Result<(Hrir, Vec<SpeakerIrs>), BrirError> {
    let sweep = read_recording(sweep_path)?;
    if sweep.sample_rate != recording.sample_rate {
        return Err(BrirError::RecordingSweepRate {
            path: sweep_path.to_owned(),
            sweep_fs: sweep.sample_rate,
            recording_fs: recording.sample_rate,
        });
    }
    let samples = sweep
        .tracks
        .first()
        .ok_or_else(|| BrirError::Unsupported("brir_recording_sweep has no tracks".into()))?;
    let reference =
        trim_reference(samples, sweep.sample_rate).map_err(|_| BrirError::RecordingSweepFile {
            path: sweep_path.to_owned(),
        })?;
    let imported =
        import_sweeps(&recording.tracks[0], &recording.tracks[1], &reference).map_err(|error| {
            match &error {
                impulcifer_dsp::DspError::InvalidArgument(message) if message == NO_SWEEP_FOUND => {
                    BrirError::RecordingSweepCount {
                        path: recording_path.to_owned(),
                        found: 0,
                        listed: entries.len(),
                        starts_seconds: Vec::new(),
                    }
                }
                impulcifer_dsp::DspError::InvalidArgument(message)
                    if message == NOT_LOGARITHMIC_SWEEP =>
                {
                    BrirError::RecordingSweepFile {
                        path: sweep_path.to_owned(),
                    }
                }
                _ => BrirError::Unsupported(error.to_string()),
            }
        })?;
    if imported.picks.len() != entries.len() {
        return Err(BrirError::RecordingSweepCount {
            path: recording_path.to_owned(),
            found: imported.picks.len(),
            listed: entries.len(),
            starts_seconds: imported
                .picks
                .iter()
                .map(|pick| *pick as f64 / f64::from(recording.sample_rate))
                .collect(),
        });
    }
    let SweepImport {
        picks,
        band,
        responses,
        ..
    } = imported;
    let found = picks.len();
    let mut pairs: Vec<_> = entries
        .iter()
        .cloned()
        .zip(responses)
        .filter(|(name, _)| name != "X")
        .map(|(name, response)| imported_pair(name, response, recording.sample_rate))
        .collect();
    let mut hrir = Hrir {
        fs: recording.sample_rate,
        speakers: Vec::new(),
    };
    for slot in SLOTS {
        if let Some(index) = pairs
            .iter()
            .position(|speaker| speaker.speaker == slot.code)
        {
            hrir.speakers.push(pairs.remove(index));
        }
    }
    let mut lfe_hrir = Hrir {
        fs: recording.sample_rate,
        speakers: Vec::new(),
    };
    for name in ["LFE", "LFE2"] {
        if let Some(index) = pairs.iter().position(|speaker| speaker.speaker == name) {
            lfe_hrir.speakers.push(pairs.remove(index));
        }
    }
    debug_assert!(pairs.is_empty(), "validated entries are all assigned");
    if hrir.fs != estimator.fs {
        hrir.resample(estimator.fs)?;
        lfe_hrir.resample(estimator.fs)?;
    }
    events.log(
        "info",
        "cli_brir_recording_loaded",
        json!({
            "file":recording_path.file_name().unwrap_or_default().to_string_lossy(),
            "count":hrir.speakers.len() + lfe_hrir.speakers.len(),
            "sweeps":found,
            "seconds":format!("{:.2}", reference.samples.len() as f64 / f64::from(reference.fs)),
            "low":band.0.round() as i64,
            "high":band.1.round() as i64,
        }),
    );
    log_itd(&hrir, events);
    Ok((hrir, lfe_hrir.speakers))
}

pub(crate) fn load(
    path: &Path,
    speakers: &str,
    sweep: Option<&Path>,
    estimator: &SweepEstimator,
    events: &mut dyn BrirEvents,
) -> Result<(Hrir, Vec<SpeakerIrs>), BrirError> {
    let entries = parse_recording_speakers(speakers).map_err(BrirError::Unsupported)?;
    let recording = read_recording(path)?;
    // The first two tracks are the two ears, in both methods; later tracks are ignored.
    if recording.tracks.len() < 2 {
        return Err(BrirError::Unsupported(
            "brir_recording_file must have at least two tracks".into(),
        ));
    }
    match sweep {
        Some(sweep) => located_method(&recording, &entries, sweep, estimator, events, path),
        None => folder_method(path, &recording, &entries, estimator),
    }
}
