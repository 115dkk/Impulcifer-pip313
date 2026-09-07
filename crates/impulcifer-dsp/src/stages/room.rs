//! Room corrections, core/room_correction.py:36-461; p10_room fixtures.
use crate::{
    DspError,
    estimator::SweepEstimator,
    fr::{
        CenterAt, CompensateOptions, FrequencyResponse, generate_frequencies,
        magnitude_to_frequency_response,
    },
    hrir::Hrir,
    ir::ImpulseResponse,
    windows,
};
use impulcifer_types::constants::{HEXADECAGONAL_TRACK_ORDER, SPEAKER_NAMES, Side};

#[derive(Clone, Debug)]
pub struct RoomMeasurementName {
    pub speakers: Vec<String>,
    pub side: Option<Side>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrCombination {
    Average,
    Conservative,
}
#[derive(Clone, Debug)]
pub struct RoomCorrectionOptions {
    pub fr_combination_method: FrCombination,
    pub specific_limit: f64,
    pub generic_limit: f64,
}
#[derive(Clone, Debug)]
pub struct RoomFrs(pub Vec<(String, Side, FrequencyResponse)>);
#[derive(Clone, Debug)]
pub struct RoomCorrection {
    pub frs: RoomFrs,
    pub responses_tracks: Vec<Vec<f64>>,
}

/// Python discover_room_measurements, core/room_correction.py:36-75; p10_generic_room.
pub fn parse_room_measurement_name(file_name: &str) -> Option<RoomMeasurementName> {
    let stem = file_name.strip_prefix("room-")?.strip_suffix(".wav")?;
    let (names, side) = if let Some(s) = stem.strip_suffix("-left") {
        (s, Some(Side::Left))
    } else if let Some(s) = stem.strip_suffix("-right") {
        (s, Some(Side::Right))
    } else {
        (stem, None)
    };
    let speakers: Vec<_> = names.split(',').collect();
    if speakers
        .iter()
        .any(|s| !(2..=3).contains(&s.len()) || !s.bytes().all(|b| b.is_ascii_uppercase()))
    {
        return None;
    }
    Some(RoomMeasurementName {
        speakers: speakers.into_iter().map(String::from).collect(),
        side,
    })
}
/// Python discover_room_measurements, core/room_correction.py:36-75; p10_generic_room.
pub fn discover_room_measurements(file_names: &[&str]) -> Vec<(String, RoomMeasurementName)> {
    file_names
        .iter()
        .filter_map(|n| parse_room_measurement_name(n).map(|m| ((*n).into(), m)))
        .collect()
}
/// Python _correction_limit_mask, core/room_correction.py:295-302; p10_generic_room.
pub fn correction_limit_mask(frequency: &[f64], limit: f64) -> Vec<f64> {
    let start = frequency.iter().position(|f| *f > limit / 2.0).unwrap_or(0);
    let end = frequency.iter().position(|f| *f > limit).unwrap_or(0);
    assert!(end >= start, "Python Hann length would be negative");
    let mut mask = vec![1.0; start];
    mask.extend(windows::hann(end - start, true));
    mask.resize(frequency.len(), 0.0);
    mask
}
/// Python _apply_correction_limit, core/room_correction.py:305-306; p10_room.
pub fn apply_correction_limit(fr: &mut FrequencyResponse, limit: f64) {
    for (e, m) in fr
        .error
        .iter_mut()
        .zip(correction_limit_mask(&fr.frequency, limit))
    {
        *e *= m;
    }
}
/// Python calculate_specific_room_corrections, core/room_correction.py:185-210; p10_room.
pub fn calculate_specific_room_corrections(
    rir: &Hrir,
    target: &FrequencyResponse,
    mic_calibration: Option<&FrequencyResponse>,
    limit: f64,
) -> Result<RoomFrs, DspError> {
    let mut out = Vec::new();
    let mut reference = None;
    for s in &rir.speakers {
        for (side, ir) in [(Side::Left, &s.left), (Side::Right, &s.right)] {
            if let Some(ir) = ir {
                let mut fr =
                    magnitude_to_frequency_response("Frequency response", ir.fs, &ir.data)?;
                if let Some(mic) = mic_calibration {
                    subtract(&mut fr.raw, &mic.raw)?;
                }
                if let Some(g) = reference {
                    for x in &mut fr.raw {
                        *x += g;
                    }
                } else {
                    reference = Some(fr.center(CenterAt::Band(100.0, 10000.0))?);
                }
                fr.compensate(target, &CompensateOptions::default())?;
                if limit > 0.0 {
                    apply_correction_limit(&mut fr, limit);
                }
                out.push((s.speaker.clone(), side, fr));
            }
        }
    }
    Ok(RoomFrs(out))
}
/// Python ndarray subtraction, core/room_correction.py:195; p10_room.
fn subtract(a: &mut [f64], b: &[f64]) -> Result<(), DspError> {
    if a.len() != b.len() {
        return Err(DspError::InvalidArgument(
            "frequency grid lengths differ".into(),
        ));
    }
    for (a, b) in a.iter_mut().zip(b) {
        *a -= b;
    }
    Ok(())
}
/// Python _calculate_generic_room_correction, core/room_correction.py:231-292; p10_generic_room.
pub fn calculate_generic_room_correction(
    irs: &[ImpulseResponse],
    target: &FrequencyResponse,
    mic_calibration: Option<&FrequencyResponse>,
    method: FrCombination,
    limit: f64,
) -> Result<FrequencyResponse, DspError> {
    let first = irs
        .first()
        .ok_or_else(|| DspError::InvalidArgument("generic room requires an IR".into()))?;
    let mut room = FrequencyResponse::constant(
        "generic_room",
        Some(generate_frequencies(10.0, first.fs as f64 / 2.0, 1.01)),
        0.0,
        0.0,
    )?;
    room.target = target.raw.clone();
    let mut errors = Vec::new();
    for ir in irs {
        let mut fr = magnitude_to_frequency_response("Frequency response", ir.fs, &ir.data)?;
        if let Some(mic) = mic_calibration {
            subtract(&mut fr.raw, &mic.raw)?;
        }
        fr.center(CenterAt::Band(100.0, 10000.0))?;
        for (r, v) in room.raw.iter_mut().zip(&fr.raw) {
            *r += v;
        }
        fr.compensate(
            target,
            &CompensateOptions {
                min_mean_error: true,
                ..Default::default()
            },
        )?;
        if method == FrCombination::Conservative && irs.len() > 1 {
            fr.smoothen(1.0 / 3.0, 1.0 / 3.0, 100.0, 10000.0)?;
            errors.push(fr.error_smoothed);
        } else {
            errors.push(fr.error);
        }
    }
    for r in &mut room.raw {
        *r /= irs.len() as f64;
    }
    let conservative = method == FrCombination::Conservative && irs.len() > 1;
    for (i, e) in room.error.iter_mut().enumerate() {
        *e = if conservative {
            if errors.iter().all(|v| v[i] > 0.0) {
                errors.iter().map(|v| v[i]).fold(f64::INFINITY, f64::min)
            } else if errors.iter().all(|v| v[i] <= 0.0) {
                errors
                    .iter()
                    .map(|v| v[i])
                    .fold(f64::NEG_INFINITY, f64::max)
            } else {
                0.0
            }
        } else {
            errors.iter().map(|v| v[i]).sum::<f64>() / irs.len() as f64
        };
    }
    let octave = if conservative { 1.0 / 6.0 } else { 1.0 / 3.0 };
    room.smoothen(octave, octave, 100.0, 10000.0)?;
    if conservative {
        room.error = room.error_smoothed.clone();
    }
    if limit > 0.0 {
        apply_correction_limit(&mut room, limit);
        for (e, m) in room
            .error_smoothed
            .iter_mut()
            .zip(correction_limit_mask(&room.frequency, limit))
        {
            *e *= m;
        }
    }
    Ok(room)
}
/// Python _open_generic_room_measurement, core/room_correction.py:350-386; p10_generic_room.
pub fn split_generic_room_recording(
    estimator: &SweepEstimator,
    fs: u32,
    tracks: &[Vec<f64>],
) -> Result<Vec<ImpulseResponse>, DspError> {
    if fs != estimator.fs {
        return Err(DspError::InvalidArgument(
            "Sampling rate does not match estimator".into(),
        ));
    }
    let mut out = Vec::new();
    for track in tracks {
        let columns = ((track.len() as f64 / fs as f64 - 2.0) / (estimator.duration + 2.0))
            .round_ties_even() as i64;
        for i in 0..columns {
            let start =
                2 * fs as usize + i as usize * (2 * fs as usize + estimator.test_signal.len());
            let end = (start + 2 * fs as usize + estimator.test_signal.len()).min(track.len());
            let recording = track[start.min(end)..end].to_vec();
            let mut ir = ImpulseResponse {
                data: estimator.estimate(&recording),
                fs,
                recording: Some(recording),
            };
            ir.crop_head(1.0);
            out.push(ir);
        }
    }
    Ok(out)
}
/// Python _open_room_target, core/room_correction.py:431-442; p10_room.
pub fn prepare_room_target(
    csv: Option<&FrequencyResponse>,
    fs: u32,
) -> Result<FrequencyResponse, DspError> {
    if let Some(csv) = csv {
        prepare_mic_calibration(csv, fs)
    } else {
        FrequencyResponse::new(
            "room-target",
            Some(generate_frequencies(10.0, fs as f64 / 2.0, 1.01)),
            Some(vec![
                0.0;
                generate_frequencies(10.0, fs as f64 / 2.0, 1.01).len()
            ]),
        )
    }
}
/// Python _open_mic_calibration, core/room_correction.py:450-461; p10_room.
pub fn prepare_mic_calibration(
    csv: &FrequencyResponse,
    fs: u32,
) -> Result<FrequencyResponse, DspError> {
    let mut fr = csv.clone();
    fr.interpolate(None, 1.01, 1, 10.0, fs as f64 / 2.0)?;
    fr.center(CenterAt::Frequency(1000.0))?;
    Ok(fr)
}
/// Python room_correction, core/room_correction.py:78-182; p10_room.
pub fn room_correction(
    rir: &mut Hrir,
    generic_irs: &[ImpulseResponse],
    target: &FrequencyResponse,
    mic_calibration: Option<&FrequencyResponse>,
    estimator: &SweepEstimator,
    options: &RoomCorrectionOptions,
) -> Result<Option<RoomCorrection>, DspError> {
    if rir.speakers.is_empty() && generic_irs.is_empty() {
        return Ok(None);
    }
    let mut frs = RoomFrs(Vec::new());
    let mut responses_tracks = Vec::new();
    if !rir.speakers.is_empty() {
        rir.for_each_ir(|ir| ir.crop_head(1.0));
        rir.crop_tails(estimator)?;
        responses_tracks = rir.stack_tracks(&HEXADECAGONAL_TRACK_ORDER, false)?;
        frs = calculate_specific_room_corrections(
            rir,
            target,
            mic_calibration,
            options.specific_limit,
        )?;
    }
    if !generic_irs.is_empty() {
        let generic = calculate_generic_room_correction(
            generic_irs,
            target,
            mic_calibration,
            options.fr_combination_method,
            options.generic_limit,
        )?;
        for s in SPEAKER_NAMES {
            if rir.get(s).is_none() {
                for side in [Side::Left, Side::Right] {
                    frs.0.push((s.into(), side, generic.clone()));
                }
            }
        }
    }
    Ok(Some(RoomCorrection {
        frs,
        responses_tracks,
    }))
}
