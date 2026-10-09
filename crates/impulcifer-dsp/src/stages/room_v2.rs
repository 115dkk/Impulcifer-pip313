//! Range-limited room gains. Legacy error curves remain in `room`.
use super::room::{self, FrCombination, RoomCorrection, RoomCorrectionOptions, RoomFrs, RoomTerm};
use crate::{
    DspError, decay,
    estimator::SweepEstimator,
    fft::Complex64,
    filters::{self, BType},
    fr::{CenterAt, FrequencyResponse, magnitude_to_frequency_response},
    hrir::Hrir,
    ir::ImpulseResponse,
    virtual_bass,
};
use impulcifer_types::constants::{HEXADECAGONAL_TRACK_ORDER, SPEAKER_NAMES, Side};
use std::f64::consts::{PI, SQRT_2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomRange {
    Legacy,
    Modes,
    Schroeder,
    Extreme,
}
impl RoomRange {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "legacy" => Some(Self::Legacy),
            "modes" => Some(Self::Modes),
            "schroeder" => Some(Self::Schroeder),
            "extreme" => Some(Self::Extreme),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Modes => "modes",
            Self::Schroeder => "schroeder",
            Self::Extreme => "extreme",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchroederSource {
    Override,
    Volume,
    AssumedVolume,
    Fallback,
}
#[derive(Clone, Debug)]
pub struct SchroederEstimate {
    pub freq: f64,
    pub t60: Option<f64>,
    pub volume: Option<f64>,
    pub source: SchroederSource,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RolloffSource {
    Slope,
    Snr,
}
#[derive(Clone, Copy, Debug)]
pub struct Rolloff {
    pub freq: f64,
    pub knee: f64,
    pub source: RolloffSource,
}
#[derive(Clone, Debug)]
pub struct EarDiagnostics {
    pub speaker: String,
    pub side: Option<Side>,
    pub rolloff: Option<Rolloff>,
    pub snr_available: bool,
    pub residual_rms_db: Option<f64>,
    pub residual_lo: f64,
    pub residual_hi: f64,
}
#[derive(Clone, Debug)]
pub struct RoomDiagnostics {
    pub range: RoomRange,
    pub schroeder: SchroederEstimate,
    pub f_hi: f64,
    pub vbass_crossover: Option<f64>,
    pub ears: Vec<EarDiagnostics>,
}

fn smooth(frequency: &[f64], data: &[f64], width: f64) -> Result<Vec<f64>, DspError> {
    let mut fr = FrequencyResponse::new(
        "room smoothing",
        Some(frequency.to_vec()),
        Some(data.to_vec()),
    )?;
    fr.smoothen(width, width, 100.0, 10000.0)?;
    Ok(fr.smoothed)
}
fn median(mut data: Vec<f64>) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    data.sort_by(f64::total_cmp);
    Some((data[(data.len() - 1) / 2] + data[data.len() / 2]) / 2.0)
}
fn half_hann(f: f64, start: f64, end: f64) -> f64 {
    let t = ((f / start).log2() / (end / start).log2()).clamp(0.0, 1.0);
    0.5 * (1.0 + (PI * t).cos())
}
fn transition(f: f64, at: f64, low: f64, high: f64) -> f64 {
    let t = ((f / at).log2() * 2.0 + 0.5).clamp(0.0, 1.0);
    low + (high - low) * t
}

pub fn estimate_schroeder(
    irs: &[&ImpulseResponse],
    volume: Option<f64>,
    override_hz: Option<f64>,
) -> SchroederEstimate {
    let mut times = Vec::new();
    for ir in irs {
        if ir.data.len() < 8 || ir.data.iter().all(|v| *v == 0.0) {
            continue;
        }
        let nyq = ir.fs as f64 / 2.0;
        for center in [125.0, 250.0, 500.0] {
            if center * SQRT_2 >= nyq {
                continue;
            }
            let high = filters::sosfilt(
                &filters::butter(4, center / SQRT_2 / nyq, BType::Highpass),
                &ir.data,
            );
            let band = filters::sosfilt(
                &filters::butter(4, center * SQRT_2 / nyq, BType::Lowpass),
                &high,
            );
            let decay = decay::decay_times(&band, ir.fs, None);
            // The existing helper reports elapsed -30/-20 dB times, not extrapolated T60.
            let t = decay
                .rt30
                .map(|t| t * 2.0)
                .or_else(|| decay.rt20.map(|t| t * 3.0));
            if let Some(t) = t.filter(|t| (0.05..=3.0).contains(t)) {
                times.push(t);
            }
        }
    }
    let t60 = median(times);
    let (freq, source) = if let Some(f) = override_hz {
        (f.clamp(50.0, 1000.0), SchroederSource::Override)
    } else if let Some(t) = t60 {
        if let Some(v) = volume {
            (
                (2000.0 * (t / v).sqrt()).clamp(80.0, 500.0),
                SchroederSource::Volume,
            )
        } else {
            (
                (2000.0 * (t / 50.0).sqrt()).clamp(120.0, 300.0),
                SchroederSource::AssumedVolume,
            )
        }
    } else {
        (200.0, SchroederSource::Fallback)
    };
    SchroederEstimate {
        freq,
        t60,
        volume: volume.or(Some(50.0)),
        source,
    }
}

pub fn snr_db(ir: &ImpulseResponse, frequency: &[f64]) -> Option<Vec<f64>> {
    if ir.data.len() < ir.fs as usize {
        return None;
    }
    let length = (ir.data.len() / 4).min(ir.fs as usize);
    if length < 4 {
        return None;
    }
    // Identical rectangular windows preserve the direct impulse near sample zero.
    let spectrum = |data: &[f64]| -> Option<Vec<f64>> {
        if data.iter().all(|v| *v == 0.0) {
            return Some(vec![-600.0; frequency.len()]);
        }
        let mut fr = magnitude_to_frequency_response("SNR", ir.fs, data).ok()?;
        // Exact silence and FFT zeros have -inf dB; keep the smoothing finite.
        for value in &mut fr.raw {
            if !value.is_finite() {
                *value = -600.0;
            }
        }
        fr.interpolate(Some(frequency), 1.01, 1, 10.0, ir.fs as f64 / 2.0)
            .ok()?;
        smooth(frequency, &fr.raw, 1.0 / 6.0).ok()
    };
    let signal = spectrum(&ir.data[..length])?;
    let noise = spectrum(&ir.data[ir.data.len() - length..])?;
    Some(signal.iter().zip(noise).map(|(s, n)| s - n).collect())
}

pub fn detect_rolloff(
    frequency: &[f64],
    measured_db: &[f64],
    snr: Option<&[f64]>,
) -> Option<Rolloff> {
    if snr.is_some_and(|s| s.len() != frequency.len()) {
        return None;
    }
    let m = smooth(frequency, measured_db, 1.0 / 6.0).ok()?;
    for step in 0..=36 {
        let fc = 160.0 * 2.0_f64.powf(-(step as f64) / 12.0);
        let points: Vec<_> = frequency
            .iter()
            .zip(&m)
            .filter(|(f, _)| **f >= fc / 2.0 && **f <= fc)
            .collect();
        if points.len() < 2 {
            continue;
        }
        let x = points.iter().map(|(f, _)| f.log2()).sum::<f64>() / points.len() as f64;
        let y = points.iter().map(|(_, m)| **m).sum::<f64>() / points.len() as f64;
        let slope = points
            .iter()
            .map(|(f, m)| (f.log2() - x) * (**m - y))
            .sum::<f64>()
            / points
                .iter()
                .map(|(f, _)| (f.log2() - x).powi(2))
                .sum::<f64>();
        let index = frequency.partition_point(|f| *f <= fc).saturating_sub(1);
        let below = frequency
            .iter()
            .enumerate()
            .filter(|(i, f)| **f <= fc / 2.0 && snr.is_none_or(|s| s[*i] >= 10.0));
        if slope >= 9.0
            && below.clone().next().is_some()
            && below.into_iter().all(|(i, _)| m[i] <= m[index] - 6.0)
        {
            let plateau = median(
                frequency
                    .iter()
                    .zip(&m)
                    .filter(|(f, _)| **f >= fc && **f <= 2.0 * fc)
                    .map(|(_, m)| *m)
                    .collect(),
            )?;
            let roll = frequency
                .iter()
                .zip(&m)
                .rev()
                .find(|(f, m)| **f <= fc && **m <= plateau - 6.0)
                .map_or(fc, |(f, _)| *f);
            return Some(Rolloff {
                freq: roll,
                knee: fc.max(roll * 2.0_f64.powf(0.25)),
                source: RolloffSource::Slope,
            });
        }
    }
    if let Some(snr) = snr {
        let snr = smooth(frequency, snr, 1.0 / 3.0).ok()?;
        let end = snr.iter().take_while(|s| **s < 10.0).count();
        if end > 0 {
            let freq = frequency[end - 1].min(160.0);
            return Some(Rolloff {
                freq,
                knee: freq * SQRT_2,
                source: RolloffSource::Snr,
            });
        }
    }
    None
}

pub fn vbass_handoff_mask(frequency: &[f64], crossover: f64, fs: u32) -> Vec<f64> {
    if crossover <= 0.0 || crossover >= fs as f64 / 2.0 {
        return vec![1.0; frequency.len()];
    }
    let sos = virtual_bass::highpass_sos(crossover, fs);
    frequency
        .iter()
        .map(|f| {
            if *f < crossover / SQRT_2 {
                return 0.0;
            }
            let z = Complex64::from_polar(1.0, -2.0 * PI * f / fs as f64);
            sos.0
                .iter()
                .map(|s| {
                    ((s[0] + s[1] * z + s[2] * z * z) / (s[3] + s[4] * z + s[5] * z * z)).norm()
                })
                .product()
        })
        .collect()
}

fn modes_error(f: &[f64], error: &[f64]) -> Result<Vec<f64>, DspError> {
    let narrow = smooth(f, error, 1.0 / 24.0)?;
    // S_1's quadratic smoother retains most of a Q=5 resonance. A robust
    // +/- one-octave median baseline preserves the shelf instead of fitting the peak.
    let baseline: Vec<_> = f
        .iter()
        .map(|center| {
            median(
                f.iter()
                    .zip(error)
                    .filter(|(f, _)| **f >= center / 2.0 && **f <= center * 2.0)
                    .map(|(_, e)| *e)
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    Ok(narrow.iter().zip(baseline).map(|(e, b)| e - b).collect())
}

fn upper_frequency(range: RoomRange, schroeder: f64) -> f64 {
    match range {
        RoomRange::Modes => schroeder / SQRT_2,
        RoomRange::Extreme => 10000.0,
        _ => schroeder,
    }
}

/// Convert an unmasked, calibrated room error to a gain. Exposed for synthetic DSP tests.
pub fn correction_gain(
    fr: &mut FrequencyResponse,
    snr: Option<&[f64]>,
    rolloff: Option<Rolloff>,
    schroeder: f64,
    generic: bool,
    fs: u32,
    options: &RoomCorrectionOptions,
) -> Result<(), DspError> {
    let f = &fr.frequency;
    let e = &fr.error;
    if snr.is_some_and(|s| s.len() != f.len()) {
        return Err(DspError::InvalidArgument("SNR grid lengths differ".into()));
    }
    let x = if options.range == RoomRange::Modes {
        modes_error(f, e)?
    } else {
        e.clone()
    };
    let mut cuts = if options.range == RoomRange::Modes {
        x.clone()
    } else {
        smooth(f, &x, 1.0 / 12.0)?
    };
    let mut boosts = smooth(f, &x, 1.0 / 6.0)?;
    if options.range == RoomRange::Extreme {
        let widths: [f64; 5] = [1.0 / 12.0, 1.0 / 6.0, 1.0 / 3.0, 0.5, 1.0];
        let views: Vec<_> = widths
            .iter()
            .map(|w| smooth(f, &x, *w))
            .collect::<Result<_, _>>()?;
        for (i, freq) in f.iter().enumerate().filter(|(_, f)| **f > schroeder) {
            let width = if *freq >= 4000.0 {
                1.0
            } else if *freq >= 1000.0 {
                (1.0 / 3.0_f64).log2() + (freq / 1000.0).log2() / 2.0 * 3.0_f64.log2()
            } else {
                (1.0 / 12.0_f64).log2()
                    + (freq / schroeder).log2() / (1000.0 / schroeder).log2() * 2.0
            };
            let width = if *freq >= 4000.0 {
                width
            } else {
                2.0_f64.powf(width)
            };
            let upper = widths.partition_point(|w| *w < width).clamp(1, 4);
            let lower = upper - 1;
            let t = ((width / widths[lower]).log2() / (widths[upper] / widths[lower]).log2())
                .clamp(0.0, 1.0);
            let value = views[lower][i] * (1.0 - t) + views[upper][i] * t;
            cuts[i] = value;
            boosts[i] = value;
        }
    }
    let f_hi = upper_frequency(options.range, schroeder);
    let handoff = options
        .vbass_crossover
        .map(|fc| vbass_handoff_mask(f, fc, fs));
    fr.equalization = f
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let cut_cap = if options.range == RoomRange::Extreme {
                24.0 - transition(*f, schroeder, 0.0, 12.0) - transition(*f, 1000.0, 0.0, 6.0)
            } else {
                24.0
            };
            let max = options.max_boost_db;
            let mut boost_cap = if generic {
                let cap = if options.fr_combination_method == FrCombination::Average {
                    0.0
                } else {
                    max.min(3.0)
                };
                transition(*f, schroeder / SQRT_2, cap, 0.0)
            } else if options.range == RoomRange::Modes {
                max.min(6.0)
            } else {
                max - transition(*f, schroeder, 0.0, max - max.min(6.0))
                    - transition(*f, 1000.0, 0.0, max.min(6.0) - max.min(3.0))
            };
            if let Some(roll) = rolloff {
                boost_cap *= 1.0 - half_hann(*f, roll.freq, roll.knee);
            }
            if let Some(snr) = snr {
                boost_cap = boost_cap.min((snr[i] - 20.0).max(0.0));
            }
            let gain = if cuts[i] > 0.0 {
                -cuts[i].min(cut_cap)
            } else {
                (-boosts[i]).max(0.0).min(boost_cap)
            };
            let start = if options.range == RoomRange::Extreme {
                5000.0
            } else {
                f_hi / SQRT_2
            };
            gain * half_hann(*f, start, f_hi) * handoff.as_ref().map_or(1.0, |mask| mask[i])
        })
        .collect();
    fr.error = fr.equalization.iter().map(|g| -g).collect();
    fr.error_smoothed = fr.error.clone();
    Ok(())
}

pub fn residual(
    fr: &FrequencyResponse,
    original: &[f64],
    rolloff: Option<Rolloff>,
    options: &RoomCorrectionOptions,
    f_hi: f64,
) -> Result<(Option<f64>, f64, f64), DspError> {
    let x = if options.range == RoomRange::Modes {
        modes_error(&fr.frequency, original)?
    } else {
        original.to_vec()
    };
    let residual: Vec<_> = x.iter().zip(&fr.equalization).map(|(e, g)| e + g).collect();
    let residual = smooth(&fr.frequency, &residual, 1.0 / 6.0)?;
    let lo = 20.0_f64
        .max(rolloff.map_or(0.0, |r| r.knee))
        .max(options.vbass_crossover.map_or(0.0, |f| f * SQRT_2));
    let hi = f_hi / SQRT_2;
    let values: Vec<_> = fr
        .frequency
        .iter()
        .zip(residual)
        .filter(|(f, _)| **f >= lo && **f <= hi)
        .map(|(_, r)| r * r)
        .collect();
    Ok((
        (!values.is_empty()).then(|| (values.iter().sum::<f64>() / values.len() as f64).sqrt()),
        lo,
        hi,
    ))
}

/// Apply the final diotic blend, after per-ear limits, fades and bass hand-off.
pub fn blend_diotic(frs: &mut RoomFrs) {
    for speaker in SPEAKER_NAMES {
        let left = frs
            .entries
            .iter()
            .position(|(s, side, _)| s == speaker && *side == Side::Left);
        let right = frs
            .entries
            .iter()
            .position(|(s, side, _)| s == speaker && *side == Side::Right);
        if let (Some(l), Some(r)) = (left, right) {
            for i in 0..frs.entries[l].2.frequency.len() {
                let alpha = 1.0 - half_hann(frs.entries[l].2.frequency[i], 500.0, 700.0);
                let mean =
                    (frs.entries[l].2.equalization[i] + frs.entries[r].2.equalization[i]) / 2.0;
                for index in [l, r] {
                    let fr = &mut frs.entries[index].2;
                    fr.equalization[i] = (1.0 - alpha) * fr.equalization[i] + alpha * mean;
                    fr.error[i] = -fr.equalization[i];
                    fr.error_smoothed[i] = fr.error[i];
                }
            }
        }
    }
}

pub(super) fn room_correction(
    rir: &mut Hrir,
    generic_irs: &[ImpulseResponse],
    target: &FrequencyResponse,
    mic: Option<&FrequencyResponse>,
    estimator: &SweepEstimator,
    options: &RoomCorrectionOptions,
) -> Result<RoomCorrection, DspError> {
    rir.for_each_ir(|ir| ir.crop_head(1.0));
    let uncropped = rir.clone();
    let specific: Vec<_> = uncropped
        .speakers
        .iter()
        .flat_map(|s| s.left.iter().chain(s.right.iter()))
        .collect();
    let estimation = if specific.is_empty() {
        generic_irs.iter().collect()
    } else {
        specific
    };
    let schroeder = estimate_schroeder(&estimation, options.room_volume, options.schroeder_freq);
    let f_hi = upper_frequency(options.range, schroeder.freq);
    let mut frs = RoomFrs {
        term: RoomTerm::Gain,
        entries: Vec::new(),
    };
    let mut tracks = Vec::new();
    if !rir.speakers.is_empty() {
        rir.crop_tails(estimator)?;
        tracks = rir.stack_tracks(&HEXADECAGONAL_TRACK_ORDER, false)?;
        frs.entries = room::calculate_specific_room_corrections(rir, target, mic, 0.0)?.entries;
    }
    let mut diagnostics = Vec::new();
    let mut originals = Vec::new();
    for (speaker, side, fr) in &mut frs.entries {
        let pair = uncropped.get(speaker).unwrap();
        let ir = if *side == Side::Left {
            pair.left.as_ref()
        } else {
            pair.right.as_ref()
        }
        .unwrap();
        let snr = snr_db(ir, &fr.frequency);
        let rolloff = detect_rolloff(&fr.frequency, &fr.raw, snr.as_deref());
        originals.push(fr.error.clone());
        correction_gain(
            fr,
            snr.as_deref(),
            rolloff,
            schroeder.freq,
            false,
            rir.fs,
            options,
        )?;
        diagnostics.push(EarDiagnostics {
            speaker: speaker.clone(),
            side: Some(*side),
            rolloff,
            snr_available: snr.is_some(),
            residual_rms_db: None,
            residual_lo: 0.0,
            residual_hi: 0.0,
        });
    }
    if options.range == RoomRange::Extreme {
        blend_diotic(&mut frs);
    }
    for ((_, _, fr), (diag, original)) in frs
        .entries
        .iter()
        .zip(diagnostics.iter_mut().zip(&originals))
    {
        (diag.residual_rms_db, diag.residual_lo, diag.residual_hi) =
            residual(fr, original, diag.rolloff, options, f_hi)?;
    }
    if !generic_irs.is_empty() {
        let mut fr = room::calculate_generic_room_correction(
            generic_irs,
            target,
            mic,
            options.fr_combination_method,
            0.0,
        )?;
        let mut rolloff: Option<Rolloff> = None;
        let mut combined_snr: Option<Vec<f64>> = None;
        let mut available = true;
        for ir in generic_irs {
            let mut measured = magnitude_to_frequency_response("generic rolloff", ir.fs, &ir.data)?;
            if let Some(mic) = mic {
                for (r, c) in measured.raw.iter_mut().zip(&mic.raw) {
                    *r -= c;
                }
            }
            measured.center(CenterAt::Band(100.0, 10000.0))?;
            let snr = snr_db(ir, &fr.frequency);
            if let Some(r) = detect_rolloff(&fr.frequency, &measured.raw, snr.as_deref())
                && rolloff.is_none_or(|old| r.freq > old.freq)
            {
                rolloff = Some(r);
            }
            if let Some(snr) = snr {
                if let Some(combined) = &mut combined_snr {
                    for (v, s) in combined.iter_mut().zip(snr) {
                        *v = v.min(s);
                    }
                } else {
                    combined_snr = Some(snr);
                }
            } else {
                available = false;
            }
        }
        if !available {
            combined_snr = None;
        }
        let original = fr.error.clone();
        correction_gain(
            &mut fr,
            combined_snr.as_deref(),
            rolloff,
            schroeder.freq,
            true,
            rir.fs,
            options,
        )?;
        let (rms, lo, hi) = residual(&fr, &original, rolloff, options, f_hi)?;
        diagnostics.push(EarDiagnostics {
            speaker: "room.wav".into(),
            side: None,
            rolloff,
            snr_available: available,
            residual_rms_db: rms,
            residual_lo: lo,
            residual_hi: hi,
        });
        for s in SPEAKER_NAMES {
            if rir.get(s).is_none() {
                for side in [Side::Left, Side::Right] {
                    frs.entries.push((s.into(), side, fr.clone()));
                }
            }
        }
    }
    Ok(RoomCorrection {
        frs,
        responses_tracks: tracks,
        diagnostics: Some(RoomDiagnostics {
            range: options.range,
            schroeder,
            f_hi,
            vbass_crossover: options.vbass_crossover,
            ears: diagnostics,
        }),
    })
}
