//! Post-recording level, noise and direction checks for generated speaker sweeps.
use super::progress::SweepSegment;
use impulcifer_dsp::{
    estimator::SweepEstimator,
    filters::{self, BType},
};
use impulcifer_types::layouts::SLOTS;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, f64::consts::PI};

#[derive(Debug, Serialize)]
pub struct Measurement {
    pub speaker: String,
    pub peak_db: f64,
    pub snr_db: f64,
    pub itd_us: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct RecordingCheck {
    pub key: &'static str,
    pub speaker: String,
    pub severity: &'static str,
    pub values: Map<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct CheckResults {
    pub measurements: Vec<Measurement>,
    pub checks: Vec<RecordingCheck>,
}

fn rounded(value: f64, decimals: i32) -> f64 {
    let scale = 10_f64.powi(decimals);
    (value * scale).round() / scale
}

fn frames(fs: u32, seconds: f64, len: usize) -> usize {
    ((seconds.max(0.0) * fs as f64).round() as usize).min(len)
}

fn rms(tracks: &[Vec<f64>], start: usize, end: usize) -> f64 {
    if start >= end || tracks.is_empty() {
        return 0.0;
    }
    tracks
        .iter()
        .map(|track| {
            let end = end.min(track.len());
            if start >= end {
                return 0.0;
            }
            (track[start..end].iter().map(|x| x * x).sum::<f64>() / (end - start) as f64).sqrt()
        })
        .sum::<f64>()
        / tracks.len() as f64
}

fn estimate_itd(left: &[f64], right: &[f64], estimator: &SweepEstimator, fs: u32) -> Option<f64> {
    if left.is_empty() || right.is_empty() || fs == 0 {
        return None;
    }
    let lowpass = filters::butter(2, 1500.0 / (fs as f64 / 2.0), BType::Lowpass);
    let left = filters::sosfilt(&lowpass, &estimator.estimate(left));
    let right = filters::sosfilt(&lowpass, &estimator.estimate(right));
    // The direct sound is the first arrival above half the loudest peak: a
    // reflection can be louder than the direct sound but never earlier.
    let max = left
        .iter()
        .chain(&right)
        .fold(0.0_f64, |m, x| m.max(x.abs()));
    if max == 0.0 {
        return None;
    }
    let onset = |ir: &[f64]| ir.iter().position(|x| x.abs() >= 0.5 * max);
    let peak = match (onset(&left), onset(&right)) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return None,
    };
    let pre = (0.002 * fs as f64).round() as usize;
    let post = (0.003 * fs as f64).round() as usize;
    let start = peak.saturating_sub(pre);
    let end = (peak + post + 1).min(left.len()).min(right.len());
    if end <= start + 2 {
        return None;
    }
    let max_lag = (0.001 * fs as f64).round() as isize;
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
    let mut values = Vec::with_capacity((2 * max_lag + 1) as usize);
    for lag in -max_lag..=max_lag {
        values.push(correlation(lag));
    }
    let index = values
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))?
        .0;
    let lag = index as isize - max_lag;
    let fraction = if index > 0 && index + 1 < values.len() {
        let before = values[index - 1];
        let center = values[index];
        let after = values[index + 1];
        let denominator = before - 2.0 * center + after;
        if denominator.abs() > f64::EPSILON {
            0.5 * (before - after) / denominator
        } else {
            0.0
        }
    } else {
        0.0
    };
    // Positive lag aligns a delayed right ear with the earlier left ear.
    Some((lag as f64 + fraction) * 1_000_000.0 / fs as f64)
}

fn lateral_angle(itd_us: f64) -> f64 {
    let target = itd_us.abs().min(255.0 * (PI / 2.0 + 1.0));
    let mut lo: f64 = 0.0;
    let mut hi: f64 = PI / 2.0;
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if 255.0 * (mid + mid.sin()) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0 * 180.0 / PI
}

fn issue(
    checks: &mut Vec<RecordingCheck>,
    key: &'static str,
    speaker: &str,
    severity: &'static str,
    values: &[(&str, f64)],
) {
    checks.push(RecordingCheck {
        key,
        speaker: speaker.to_owned(),
        severity,
        values: values
            .iter()
            .map(|(key, value)| ((*key).to_owned(), json!(value)))
            .collect(),
    });
}

pub fn check_recording(
    tracks: &[Vec<f64>],
    fs: u32,
    segments: &[SweepSegment],
    estimator: Option<&SweepEstimator>,
    expected_azimuths: Option<&BTreeMap<String, f64>>,
) -> CheckResults {
    let mut measurements = Vec::new();
    let mut checks = Vec::new();
    let used_tracks = &tracks[..tracks.len().min(2)];
    for segment in segments.iter().filter(|segment| segment.speaker != "X") {
        let start = frames(fs, segment.start, tracks.first().map_or(0, Vec::len));
        let signal_end = frames(fs, segment.end, tracks.first().map_or(0, Vec::len));
        let tail_end = frames(fs, segment.end + 0.5, tracks.first().map_or(0, Vec::len));
        let noise_start = frames(fs, segment.start - 1.5, tracks.first().map_or(0, Vec::len));
        let noise_end = frames(fs, segment.start - 0.25, tracks.first().map_or(0, Vec::len));
        let peak = used_tracks
            .iter()
            .flat_map(|track| &track[start.min(track.len())..tail_end.min(track.len())])
            .map(|x| x.abs())
            .fold(0.0, f64::max);
        let peak_db = 20.0 * peak.max(1e-30).log10();
        let signal = rms(used_tracks, start, signal_end);
        let noise = rms(used_tracks, noise_start, noise_end);
        // Digital silence (or a segment past the end of the take) has no signal.
        let snr_db = if signal == 0.0 {
            0.0
        } else if noise == 0.0 {
            120.0
        } else {
            20.0 * (signal / noise).max(1e-30).log10()
        };
        let itd_us = if tracks.len() == 2 {
            estimator.and_then(|estimator| {
                let left = &tracks[0][start.min(tracks[0].len())..tail_end.min(tracks[0].len())];
                let right = &tracks[1][start.min(tracks[1].len())..tail_end.min(tracks[1].len())];
                estimate_itd(left, right, estimator, fs)
            })
        } else {
            None
        };
        let peak_value = rounded(peak_db, 1);
        let snr_value = rounded(snr_db, 1);
        measurements.push(Measurement {
            speaker: segment.speaker.clone(),
            peak_db: peak_value,
            snr_db: snr_value,
            itd_us: itd_us.map(|value| rounded(value, 0)),
        });
        if snr_db < 6.0 {
            issue(
                &mut checks,
                "record_check_no_signal",
                &segment.speaker,
                "error",
                &[("snr_db", snr_value)],
            );
            continue;
        }
        if peak_db >= -0.1 {
            issue(
                &mut checks,
                "record_check_clipping",
                &segment.speaker,
                "error",
                &[("peak_db", peak_value)],
            );
        }
        if peak_db < -40.0 {
            issue(
                &mut checks,
                "record_check_quiet",
                &segment.speaker,
                "warning",
                &[("peak_db", peak_value)],
            );
        }
        if snr_db < 30.0 {
            issue(
                &mut checks,
                "record_check_noisy",
                &segment.speaker,
                "warning",
                &[("snr_db", snr_value)],
            );
        }
        let Some(itd_us) = itd_us else { continue };
        let Some(slot) = SLOTS.iter().find(|slot| slot.code == segment.speaker) else {
            continue;
        };
        let expected_override = expected_azimuths.and_then(|map| map.get(&segment.speaker));
        let azimuth = expected_override.copied().unwrap_or(slot.azimuth);
        let elevation = if expected_override.is_some() {
            0.0
        } else {
            slot.elevation
        };
        let azimuth_rad = azimuth.to_radians();
        let lateral_expected = (azimuth_rad.sin().abs() * elevation.to_radians().cos())
            .asin()
            .to_degrees();
        let lateral_measured = lateral_angle(itd_us);
        let itd_value = rounded(itd_us, 0);
        // A centre slot (front or back) has no side; sin(180°) is not exactly 0.
        let sided = lateral_expected >= 5.0;
        let wrong_side = if sided && azimuth_rad.sin() > 0.0 && itd_us <= -100.0 {
            issue(
                &mut checks,
                "record_check_wrong_side_left",
                &segment.speaker,
                "error",
                &[("itd_us", itd_value)],
            );
            true
        } else if sided && azimuth_rad.sin() < 0.0 && itd_us >= 100.0 {
            issue(
                &mut checks,
                "record_check_wrong_side_right",
                &segment.speaker,
                "error",
                &[("itd_us", itd_value)],
            );
            true
        } else {
            false
        };
        if wrong_side {
            continue;
        }
        if lateral_expected < 5.0 && lateral_measured >= 10.0 {
            issue(
                &mut checks,
                "record_check_off_center",
                &segment.speaker,
                "warning",
                &[
                    ("itd_us", itd_value),
                    ("measured", lateral_measured.round()),
                ],
            );
        } else if expected_override.is_some()
            && lateral_expected >= 5.0
            && (lateral_measured - lateral_expected).abs() >= 12.0
        {
            let absolute_azimuth = azimuth.abs();
            let key = if absolute_azimuth < 75.0 {
                if lateral_measured > lateral_expected {
                    "record_check_angle_too_far"
                } else {
                    "record_check_angle_too_little"
                }
            } else if absolute_azimuth > 105.0 {
                if lateral_measured > lateral_expected {
                    "record_check_angle_too_little"
                } else {
                    "record_check_angle_too_far"
                }
            } else {
                "record_check_angle_off"
            };
            let display_measured = if absolute_azimuth > 90.0 {
                180.0 - lateral_measured
            } else {
                lateral_measured
            };
            issue(
                &mut checks,
                key,
                &segment.speaker,
                "warning",
                &[
                    ("itd_us", itd_value),
                    ("measured", display_measured.round()),
                    ("expected", absolute_azimuth.round()),
                ],
            );
        }
    }
    CheckResults {
        measurements,
        checks,
    }
}
