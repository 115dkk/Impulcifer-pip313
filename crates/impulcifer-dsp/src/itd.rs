//! Interaural time-difference analysis and correction (ADR 0014).

use crate::{
    fft,
    filters::{self, BType},
    hrir::Hrir,
};
use impulcifer_types::{constants::SPEAKER_NAMES, layouts::SLOTS};
use serde::Serialize;
use std::f64::consts::PI;

const SPEED_OF_SOUND: f64 = 343.0;
const MIN_HEAD_RADIUS_CM: f64 = 5.0;
const MAX_HEAD_RADIUS_CM: f64 = 15.0;
const DEFAULT_HEAD_RADIUS_CM: f64 = 8.75;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItdEstimate {
    pub itd_us: f64,
    pub inverted: bool,
}

/// First sample in either response whose magnitude reaches half the larger peak.
pub fn direct_arrival(left: &[f64], right: &[f64]) -> Option<usize> {
    let max = left
        .iter()
        .chain(right)
        .fold(0.0_f64, |value, sample| value.max(sample.abs()));
    if max == 0.0 {
        return None;
    }
    let onset = |ir: &[f64]| ir.iter().position(|sample| sample.abs() >= 0.5 * max);
    match (onset(left), onset(right)) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

/// Estimate right-minus-left arrival time. Positive means the left ear is first.
pub fn estimate(left: &[f64], right: &[f64], fs: u32, max_lag_ms: f64) -> Option<ItdEstimate> {
    if left.is_empty()
        || right.is_empty()
        || fs <= 3_000
        || !max_lag_ms.is_finite()
        || max_lag_ms < 0.0
    {
        return None;
    }
    let lowpass = filters::butter(2, 1500.0 / (fs as f64 / 2.0), BType::Lowpass);
    let left = filters::sosfilt(&lowpass, left);
    let right = filters::sosfilt(&lowpass, right);
    let peak = direct_arrival(&left, &right)?;
    let pre = (0.002 * fs as f64).round() as usize;
    let post = (0.003 * fs as f64).round() as usize;
    let start = peak.saturating_sub(pre);
    let end = (peak + post + 1).min(left.len()).min(right.len());
    if end <= start + 2 {
        return None;
    }
    let max_lag = (max_lag_ms * fs as f64 / 1000.0).round() as isize;
    let correlation = |lag: isize| {
        let mut sum = 0.0;
        for (i, sample) in left.iter().enumerate().take(end).skip(start) {
            let j = i as isize + lag;
            if j >= start as isize && j < end as isize {
                sum += *sample * right[j as usize];
            }
        }
        sum
    };
    let values: Vec<_> = (-max_lag..=max_lag).map(correlation).collect();
    let index = values
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))?
        .0;
    let lag = index as isize - max_lag;
    let magnitudes: Vec<_> = values.iter().map(|value| value.abs()).collect();
    let fraction = if index > 0 && index + 1 < magnitudes.len() {
        let before = magnitudes[index - 1];
        let center = magnitudes[index];
        let after = magnitudes[index + 1];
        let denominator = before - 2.0 * center + after;
        if denominator.abs() > f64::EPSILON {
            0.5 * (before - after) / denominator
        } else {
            0.0
        }
    } else {
        0.0
    };
    Some(ItdEstimate {
        itd_us: (lag as f64 + fraction) * 1_000_000.0 / fs as f64,
        inverted: values[index].is_sign_negative(),
    })
}

/// Mean spectral power in a closed frequency band, expressed in decibels.
pub fn band_level_db(ir: &[f64], fs: u32, lo_hz: f64, hi_hz: f64) -> Option<f64> {
    if ir.is_empty() || fs == 0 || !lo_hz.is_finite() || !hi_hz.is_finite() || lo_hz > hi_hz {
        return None;
    }
    let n = fft::next_fast_len(ir.len());
    let mut padded = vec![0.0; n];
    padded[..ir.len()].copy_from_slice(ir);
    let spectrum = fft::rfft(&padded);
    let mut sum = 0.0;
    let mut count = 0;
    for (index, value) in spectrum.iter().enumerate() {
        let frequency = index as f64 * fs as f64 / n as f64;
        if (lo_hz..=hi_hz).contains(&frequency) {
            sum += value.norm_sqr();
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let mean = sum / count as f64;
    (mean > 0.0).then(|| 10.0 * mean.log10())
}

/// Delay a response without changing its length or wrapping the tail to its head.
pub fn fractional_delay(x: &[f64], fs: u32, delay_us: f64) -> Vec<f64> {
    assert!(fs > 0, "sample rate must be positive");
    assert!(
        delay_us.is_finite() && delay_us >= 0.0,
        "delay must be finite and nonnegative"
    );
    if x.is_empty() {
        return Vec::new();
    }
    let delay = delay_us * fs as f64 / 1_000_000.0;
    let m = fft::next_fast_len(x.len() + delay.ceil() as usize + 1);
    let mut padded = vec![0.0; m];
    padded[..x.len()].copy_from_slice(x);
    let mut spectrum = fft::rfft(&padded);
    for (index, value) in spectrum.iter_mut().enumerate() {
        let phase = -2.0 * PI * index as f64 * delay / m as f64;
        *value *= fft::Complex64::from_polar(1.0, phase);
    }
    let mut delayed = fft::irfft(&spectrum, m);
    delayed.truncate(x.len());
    delayed
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItdMode {
    Off,
    Symmetric,
    Model,
}
impl ItdMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "off" => Some(Self::Off),
            "symmetric" => Some(Self::Symmetric),
            "model" => Some(Self::Model),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Symmetric => "symmetric",
            Self::Model => "model",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SpeakerAlignment {
    pub speaker: String,
    pub measured_us: Option<f64>,
    pub model_us: f64,
    pub target_us: Option<f64>,
    pub after_us: Option<f64>,
    pub applied_us: Option<f64>,
    pub lf_ild_db: Option<f64>,
    pub hf_ild_db: Option<f64>,
    pub skipped: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PairAlignment {
    pub left: String,
    pub right: String,
    pub sum_us: f64,
    pub yaw_deg: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AlignmentReport {
    pub mode: String,
    pub head_radius_cm: f64,
    pub head_radius_source: String,
    pub yaw_deg: Option<f64>,
    pub common_offset_us: Option<f64>,
    pub input_balance_db: Option<f64>,
    pub hf_asymmetry_db: Option<f64>,
    pub headphone_balance_db: Option<f64>,
    pub predicted_balance_db: Option<f64>,
    pub speakers: Vec<SpeakerAlignment>,
    pub pairs: Vec<PairAlignment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ear {
    Left,
    Right,
}

#[derive(Clone, Debug)]
pub struct Correction {
    pub speaker: String,
    pub target_us: f64,
    pub applied_us: f64,
    pub ear: Ear,
    pub skipped: bool,
}

fn slot(code: &str) -> Option<&'static impulcifer_types::layouts::Slot> {
    SLOTS.iter().find(|slot| slot.code == code)
}

fn lateral_radians(azimuth: f64, elevation: f64) -> f64 {
    (azimuth.to_radians().sin().abs() * elevation.to_radians().cos()).asin()
}

fn is_center(azimuth: f64, elevation: f64) -> bool {
    lateral_radians(azimuth, elevation).to_degrees() < 5.0
}

fn model_itd_us(azimuth: f64, elevation: f64, radius_cm: f64) -> f64 {
    if is_center(azimuth, elevation) {
        return 0.0;
    }
    let theta = lateral_radians(azimuth, elevation);
    let magnitude = radius_cm / 100.0 / SPEED_OF_SOUND * (theta + theta.sin()) * 1_000_000.0;
    magnitude.copysign(azimuth)
}

fn model_derivative(azimuth: f64, elevation: f64, radius_cm: f64) -> f64 {
    let theta = lateral_radians(azimuth, elevation);
    if theta.cos().abs() < 1e-12 {
        return 0.0;
    }
    radius_cm / 100.0 / SPEED_OF_SOUND
        * (1.0 + theta.cos())
        * (azimuth.to_radians().cos() * elevation.to_radians().cos())
        / theta.cos()
}

fn lower_median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    Some(values[(values.len() - 1) / 2])
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

fn measured(speakers: &[SpeakerAlignment], code: &str) -> Option<f64> {
    speakers
        .iter()
        .find(|speaker| speaker.speaker == code)?
        .measured_us
}

fn ild(speakers: &[SpeakerAlignment], code: &str, high: bool) -> Option<f64> {
    let speaker = speakers.iter().find(|speaker| speaker.speaker == code)?;
    if high {
        speaker.hf_ild_db
    } else {
        speaker.lf_ild_db
    }
}

fn mirror_pairs(speakers: &[SpeakerAlignment]) -> Vec<(&'static str, &'static str)> {
    let present = |code: &str| speakers.iter().any(|speaker| speaker.speaker == code);
    SLOTS
        .iter()
        .filter(|left| left.azimuth > 0.0 && left.azimuth.abs() != 180.0 && present(left.code))
        .filter_map(|left| {
            SLOTS
                .iter()
                .find(|right| {
                    right.azimuth == -left.azimuth
                        && right.elevation == left.elevation
                        && present(right.code)
                })
                .map(|right| (left.code, right.code))
        })
        .collect()
}

pub fn analyze(hrir: &Hrir, head_radius_cm: Option<f64>, max_lag_ms: f64) -> AlignmentReport {
    let mut speakers = Vec::new();
    for code in SPEAKER_NAMES {
        let Some((left, right)) = hrir.pair(code) else {
            continue;
        };
        let measured_us =
            estimate(&left.data, &right.data, hrir.fs, max_lag_ms).map(|estimate| estimate.itd_us);
        let lf_ild_db = band_level_db(&left.data, hrir.fs, 30.0, 120.0)
            .zip(band_level_db(&right.data, hrir.fs, 30.0, 120.0))
            .map(|(left, right)| left - right);
        let first = (hrir.fs as usize / 10)
            .min(left.data.len())
            .min(right.data.len());
        let hf_ild_db =
            (first > 0)
                .then(|| {
                    band_level_db(&left.data[..first], hrir.fs, 4_000.0, 12_000.0).zip(
                        band_level_db(&right.data[..first], hrir.fs, 4_000.0, 12_000.0),
                    )
                })
                .flatten()
                .map(|(left, right)| left - right);
        speakers.push(SpeakerAlignment {
            speaker: code.into(),
            measured_us,
            model_us: 0.0,
            target_us: None,
            after_us: None,
            applied_us: None,
            lf_ild_db,
            hf_ild_db,
            skipped: false,
        });
    }

    let input_balance_db = lower_median(
        speakers
            .iter()
            .filter_map(|speaker| speaker.lf_ild_db)
            .collect(),
    );
    let centre_values: Vec<_> = speakers
        .iter()
        .filter_map(|speaker| {
            let slot = slot(&speaker.speaker)?;
            is_center(slot.azimuth, slot.elevation).then_some(speaker.measured_us?)
        })
        .collect();
    let common_offset_us = (!centre_values.is_empty())
        .then(|| centre_values.iter().sum::<f64>() / centre_values.len() as f64);
    let offset = common_offset_us.unwrap_or(0.0);
    let pairs = mirror_pairs(&speakers);

    let (head_radius_cm, head_radius_source) = if let Some(radius) = head_radius_cm {
        (radius, "given")
    } else {
        let estimate = pairs
            .iter()
            .filter_map(|(left, right)| {
                let geometry = slot(left)?;
                let theta = lateral_radians(geometry.azimuth, geometry.elevation);
                let left = measured(&speakers, left)?;
                let right = measured(&speakers, right)?;
                let time = ((left - offset).abs() + (right - offset).abs()) / 2.0 / 1_000_000.0;
                let radius = SPEED_OF_SOUND * time / (theta + theta.sin()) * 100.0;
                Some((theta, radius))
            })
            .max_by(|(a, _), (b, _)| a.total_cmp(b));
        if let Some((_, radius)) = estimate {
            (
                radius.clamp(MIN_HEAD_RADIUS_CM, MAX_HEAD_RADIUS_CM),
                "estimated",
            )
        } else {
            (DEFAULT_HEAD_RADIUS_CM, "default")
        }
    };

    for speaker in &mut speakers {
        let geometry = slot(&speaker.speaker).expect("known speaker slot");
        speaker.model_us = model_itd_us(geometry.azimuth, geometry.elevation, head_radius_cm);
    }

    let mut pair_reports = Vec::new();
    for (left, right) in pairs {
        let (Some(left_us), Some(right_us)) =
            (measured(&speakers, left), measured(&speakers, right))
        else {
            continue;
        };
        let sum_us = left_us + right_us;
        let geometry = slot(left).expect("known left slot");
        let derivative = model_derivative(geometry.azimuth, geometry.elevation, head_radius_cm);
        let yaw_sum_us = sum_us - 2.0 * offset;
        let yaw_deg = (derivative.abs() >= 0.000_05)
            .then(|| -(yaw_sum_us / 1_000_000.0 / 2.0) / derivative * 180.0 / PI);
        pair_reports.push(PairAlignment {
            left: left.into(),
            right: right.into(),
            sum_us,
            yaw_deg,
        });
    }
    let yaw_deg = median(
        pair_reports
            .iter()
            .filter_map(|pair| pair.yaw_deg)
            .collect(),
    );
    let balance = input_balance_db.unwrap_or(0.0);
    let asymmetries: Vec<_> = pair_reports
        .iter()
        .filter_map(|pair| {
            Some(
                ((ild(&speakers, &pair.left, true)? - balance)
                    + (ild(&speakers, &pair.right, true)? - balance))
                    / 2.0,
            )
        })
        .collect();
    let hf_asymmetry_db = (!asymmetries.is_empty())
        .then(|| asymmetries.iter().sum::<f64>() / asymmetries.len() as f64);

    AlignmentReport {
        mode: ItdMode::Off.as_str().into(),
        head_radius_cm,
        head_radius_source: head_radius_source.into(),
        yaw_deg,
        common_offset_us,
        input_balance_db,
        hf_asymmetry_db,
        headphone_balance_db: None,
        predicted_balance_db: None,
        speakers,
        pairs: pair_reports,
    }
}

pub fn correct(hrir: &mut Hrir, report: &mut AlignmentReport, mode: ItdMode) -> Vec<Correction> {
    report.mode = mode.as_str().into();
    if mode == ItdMode::Off {
        return Vec::new();
    }
    let offset = report.common_offset_us.unwrap_or(0.0);
    let mut pair_targets = std::collections::BTreeMap::new();
    if mode == ItdMode::Symmetric {
        for pair in &report.pairs {
            let (Some(left), Some(right)) = (
                measured(&report.speakers, &pair.left),
                measured(&report.speakers, &pair.right),
            ) else {
                continue;
            };
            let target = ((left - offset).abs() + (right - offset).abs()) / 2.0;
            pair_targets.insert(pair.left.clone(), target);
            pair_targets.insert(pair.right.clone(), -target);
        }
    }

    let mut corrections = Vec::new();
    let fs = hrir.fs;
    for speaker in &mut report.speakers {
        let Some(measured_us) = speaker.measured_us else {
            continue;
        };
        let geometry = slot(&speaker.speaker).expect("known speaker slot");
        let target_us = if is_center(geometry.azimuth, geometry.elevation) {
            0.0
        } else if mode == ItdMode::Model {
            speaker.model_us
        } else {
            pair_targets
                .get(&speaker.speaker)
                .copied()
                .unwrap_or(measured_us - offset)
        };
        speaker.target_us = Some(target_us);
        let delta = target_us - measured_us;
        let ear = if delta >= 0.0 { Ear::Right } else { Ear::Left };
        let skipped = delta.abs() > 1_500.0;
        speaker.skipped = skipped;
        // The report shows what was applied: nothing for a skipped speaker.
        speaker.applied_us = Some(if skipped || delta.abs() < 0.5 {
            0.0
        } else {
            delta
        });
        if !skipped && delta.abs() >= 0.5 {
            let pair = hrir
                .get_mut(&speaker.speaker)
                .expect("analyzed speaker remains in HRIR");
            let ir = match ear {
                Ear::Left => pair.left.as_mut(),
                Ear::Right => pair.right.as_mut(),
            }
            .expect("analyzed speaker has both ears");
            ir.data = fractional_delay(&ir.data, fs, delta.abs());
        }
        corrections.push(Correction {
            speaker: speaker.speaker.clone(),
            target_us,
            applied_us: delta,
            ear,
            skipped,
        });
    }
    corrections
}

pub fn measure_after(hrir: &Hrir, report: &mut AlignmentReport, max_lag_ms: f64) {
    for speaker in &mut report.speakers {
        speaker.after_us = hrir
            .pair(&speaker.speaker)
            .and_then(|(left, right)| estimate(&left.data, &right.data, hrir.fs, max_lag_ms))
            .map(|estimate| estimate.itd_us);
    }
}
