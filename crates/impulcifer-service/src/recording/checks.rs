//! Post-recording level, noise, impulse and direction checks for speaker sweeps.
use super::progress::SweepSegment;
use impulcifer_dsp::{
    estimator::SweepEstimator,
    filters::{self, BType},
    itd::{band_level_db, direct_arrival, estimate},
    polarity::{self, PolarityInput, Side, Verdict},
};
use impulcifer_types::layouts::SLOTS;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, f64::consts::PI};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DirectSound {
    pub fs: u32,
    pub left: Vec<f64>,
    pub right: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReferenceMeasurement {
    pub speaker: String,
    pub itd_us: Option<f64>,
    pub lf_db: Option<f64>,
    pub lf_ild_db: Option<f64>,
    pub azimuth: Option<f64>,
    pub direct: Option<DirectSound>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Measurement {
    pub speaker: String,
    pub peak_db: f64,
    pub snr_db: f64,
    pub itd_us: Option<f64>,
    pub lf_db: Option<f64>,
    pub lf_ild_db: Option<f64>,
    pub azimuth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct: Option<DirectSound>,
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

#[derive(Default)]
struct ImpulseMetrics {
    distortion_db: Option<f64>,
}

#[derive(Clone, Copy)]
struct Direction {
    azimuth: f64,
    lateral: f64,
    side: i8,
}

struct MeasuredTake {
    measurement: Measurement,
    ear_silent: bool,
    wrong_side: bool,
    headphone_left_db: Option<f64>,
    headphone_right_db: Option<f64>,
}

fn rounded(value: f64, decimals: i32) -> f64 {
    let scale = 10_f64.powi(decimals);
    (value * scale).round() / scale
}

fn frames(fs: u32, seconds: f64, len: usize) -> usize {
    ((seconds.max(0.0) * fs as f64).round() as usize).min(len)
}

fn track_rms(track: &[f64], start: usize, end: usize) -> f64 {
    let end = end.min(track.len());
    if start >= end {
        return 0.0;
    }
    (track[start..end].iter().map(|x| x * x).sum::<f64>() / (end - start) as f64).sqrt()
}

fn rms(tracks: &[Vec<f64>], start: usize, end: usize) -> f64 {
    if tracks.is_empty() {
        return 0.0;
    }
    tracks
        .iter()
        .map(|track| track_rms(track, start, end))
        .sum::<f64>()
        / tracks.len() as f64
}

fn db(value: f64) -> Option<f64> {
    (value > 0.0).then(|| 20.0 * value.log10())
}

fn low_frequency_level(tracks: &[Vec<f64>], start: usize, end: usize, fs: u32) -> Option<f64> {
    if tracks.is_empty() || fs <= 1_000 || start >= end {
        return None;
    }
    let nyquist = fs as f64 / 2.0;
    let highpass = filters::butter(2, 100.0 / nyquist, BType::Highpass);
    let lowpass = filters::butter(2, 500.0 / nyquist, BType::Lowpass);
    let levels: Vec<_> = tracks
        .iter()
        .filter_map(|track| {
            let end = end.min(track.len());
            (start < end).then(|| {
                let high = filters::sosfilt(&highpass, &track[start..end]);
                let band = filters::sosfilt(&lowpass, &high);
                track_rms(&band, 0, band.len())
            })
        })
        .collect();
    if levels.is_empty() {
        return None;
    }
    db(levels.iter().sum::<f64>() / levels.len() as f64).map(|value| rounded(value, 1))
}

fn deconvolve_pair(
    left: &[f64],
    right: &[f64],
    estimator: &SweepEstimator,
) -> (Vec<f64>, Vec<f64>) {
    (estimator.estimate(left), estimator.estimate(right))
}

fn window_energy(tracks: [&[f64]; 2], center: usize, before: usize, after: usize) -> f64 {
    tracks
        .iter()
        .map(|track| {
            let start = center.saturating_sub(before);
            let end = center.saturating_add(after).min(track.len());
            track[start..end].iter().map(|x| x * x).sum::<f64>()
        })
        .sum()
}

fn impulse_metrics(
    left: &[f64],
    right: &[f64],
    estimator: &SweepEstimator,
    fs: u32,
) -> ImpulseMetrics {
    let Some(direct) = direct_arrival(left, right) else {
        return ImpulseMetrics::default();
    };
    let linear_energy = window_energy(
        [left, right],
        direct,
        (0.001 * fs as f64).round() as usize,
        (0.005 * fs as f64).round() as usize + 1,
    );
    let harmonic_offset =
        (estimator.duration / (estimator.high / estimator.low).ln() * 2.0_f64.ln() * fs as f64)
            .round() as usize;
    let harmonic_energy = direct.checked_sub(harmonic_offset).map(|center| {
        let radius = (0.002 * fs as f64).round() as usize;
        window_energy([left, right], center, radius, radius + 1)
    });
    let distortion_db = harmonic_energy
        .filter(|_| linear_energy > 0.0)
        .map(|energy| 10.0 * (energy.max(1e-300) / linear_energy).log10());

    ImpulseMetrics { distortion_db }
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

fn direction(speaker: &str, azimuth: Option<f64>) -> Option<Direction> {
    let slot = SLOTS.iter().find(|slot| slot.code == speaker)?;
    let elevation = slot.elevation;
    let azimuth = azimuth.unwrap_or(slot.azimuth);
    let lateral = (azimuth.to_radians().sin().abs() * elevation.to_radians().cos())
        .asin()
        .to_degrees();
    let side = if lateral < 5.0 {
        0
    } else if azimuth.to_radians().sin() > 0.0 {
        1
    } else {
        -1
    };
    Some(Direction {
        azimuth,
        lateral,
        side,
    })
}

fn mirror_slots(a: &str, b: &str) -> bool {
    let Some(a) = SLOTS.iter().find(|slot| slot.code == a) else {
        return false;
    };
    let Some(b) = SLOTS.iter().find(|slot| slot.code == b) else {
        return false;
    };
    a.azimuth != 0.0
        && a.azimuth.abs() != 180.0
        && a.elevation == b.elevation
        && a.azimuth == -b.azimuth
}

fn same_angle(a: &Measurement, b: &Measurement) -> bool {
    match (a.azimuth, b.azimuth) {
        (Some(a_azimuth), Some(b_azimuth)) => {
            let Some(a) = direction(&a.speaker, Some(a_azimuth)) else {
                return false;
            };
            let Some(b) = direction(&b.speaker, Some(b_azimuth)) else {
                return false;
            };
            a.lateral >= 5.0 && b.lateral >= 5.0 && (a.lateral - b.lateral).abs() < 2.0
        }
        _ => mirror_slots(&a.speaker, &b.speaker),
    }
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

fn mismatch_issue(
    checks: &mut Vec<RecordingCheck>,
    measurement: &Measurement,
    reference: &Measurement,
    difference: f64,
) {
    checks.push(RecordingCheck {
        key: "record_check_itd_mismatch",
        speaker: measurement.speaker.clone(),
        severity: "warning",
        values: [
            (
                "itd_us".to_owned(),
                json!(measurement.itd_us.unwrap().round()),
            ),
            (
                "reference_itd_us".to_owned(),
                json!(reference.itd_us.unwrap().round()),
            ),
            ("diff_us".to_owned(), json!(difference.round())),
            ("reference".to_owned(), json!(reference.speaker)),
        ]
        .into_iter()
        .collect(),
    });
}

fn polarity_issue(
    checks: &mut Vec<RecordingCheck>,
    key: &'static str,
    speaker: &str,
    values: Map<String, Value>,
) {
    checks.push(RecordingCheck {
        key,
        speaker: speaker.to_owned(),
        severity: "warning",
        values,
    });
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

pub fn check_recording(
    tracks: &[Vec<f64>],
    fs: u32,
    segments: &[SweepSegment],
    estimator: Option<&SweepEstimator>,
    expected_azimuths: Option<&BTreeMap<String, f64>>,
    reference_measurements: Option<&[ReferenceMeasurement]>,
    headphones: bool,
) -> CheckResults {
    let mut takes = Vec::new();
    let mut checks = Vec::new();
    let used_tracks = &tracks[..tracks.len().min(2)];
    let track_len = tracks.first().map_or(0, Vec::len);
    for segment in segments.iter().filter(|segment| segment.speaker != "X") {
        let start = frames(fs, segment.start, track_len);
        let signal_end = frames(fs, segment.end, track_len);
        let tail_end = frames(fs, segment.end + 0.5, track_len);
        let noise_start = frames(fs, segment.start - 1.5, track_len);
        let noise_end = frames(fs, segment.start - 0.25, track_len);
        let peak = used_tracks
            .iter()
            .flat_map(|track| &track[start.min(track.len())..tail_end.min(track.len())])
            .map(|x| x.abs())
            .fold(0.0, f64::max);
        let peak_db = 20.0 * peak.max(1e-30).log10();
        let signal = rms(used_tracks, start, signal_end);
        let noise = rms(used_tracks, noise_start, noise_end);
        let snr_db = if signal == 0.0 {
            0.0
        } else if noise == 0.0 {
            120.0
        } else {
            20.0 * (signal / noise).max(1e-30).log10()
        };
        let azimuth = expected_azimuths.and_then(|values| values.get(&segment.speaker).copied());
        let lf_db = low_frequency_level(used_tracks, start, signal_end, fs);
        let ear_levels = (tracks.len() == 2).then(|| {
            (
                track_rms(&tracks[0], start, signal_end),
                track_rms(&tracks[1], start, signal_end),
            )
        });
        let ear_silent = ear_levels.is_some_and(|(left, right)| {
            let high = left.max(right);
            let low = left.min(right);
            high > 0.0 && (low == 0.0 || 20.0 * (high / low).log10() >= 25.0)
        });

        let analysis = if tracks.len() == 2 {
            estimator.map(|estimator| {
                let analysis_start = frames(fs, segment.start - 1.0, track_len);
                let left =
                    &tracks[0][analysis_start.min(tracks[0].len())..tail_end.min(tracks[0].len())];
                let right =
                    &tracks[1][analysis_start.min(tracks[1].len())..tail_end.min(tracks[1].len())];
                let (left_ir, right_ir) = deconvolve_pair(left, right, estimator);
                let itd = estimate(&left_ir, &right_ir, fs, 1.0);
                let impulse = impulse_metrics(&left_ir, &right_ir, estimator, fs);
                let lf_ild_db = band_level_db(&left_ir, fs, 30.0, 120.0)
                    .zip(band_level_db(&right_ir, fs, 30.0, 120.0))
                    .map(|(left, right)| left - right);
                let headphone_left_db = band_level_db(&left_ir, fs, 200.0, 2_000.0);
                let headphone_right_db = band_level_db(&right_ir, fs, 200.0, 2_000.0);
                let direct = polarity::direct_window(&left_ir, fs)
                    .zip(polarity::direct_window(&right_ir, fs))
                    .map(|(left, right)| DirectSound {
                        fs,
                        left: left.samples,
                        right: right.samples,
                    });
                (
                    itd,
                    impulse,
                    lf_ild_db,
                    headphone_left_db,
                    headphone_right_db,
                    direct,
                )
            })
        } else {
            None
        };
        let itd = analysis.as_ref().and_then(|(itd, _, _, _, _, _)| *itd);
        let lf_ild_db = analysis
            .as_ref()
            .and_then(|(_, _, level, _, _, _)| *level)
            .filter(|_| snr_db >= 6.0 && !ear_silent);
        let headphone_left_db = analysis
            .as_ref()
            .and_then(|(_, _, _, level, _, _)| *level)
            .filter(|_| snr_db >= 6.0);
        let headphone_right_db = analysis
            .as_ref()
            .and_then(|(_, _, _, _, level, _)| *level)
            .filter(|_| snr_db >= 6.0);
        let mut measurement = Measurement {
            speaker: segment.speaker.clone(),
            peak_db: rounded(peak_db, 1),
            snr_db: rounded(snr_db, 1),
            // A take with no signal or a silent ear has no usable time difference;
            // reporting one would make a bad reference for later takes.
            itd_us: itd
                .filter(|_| snr_db >= 6.0 && !ear_silent)
                .map(|value| rounded(value.itd_us, 0)),
            lf_db,
            lf_ild_db,
            azimuth,
            direct: None,
        };

        if snr_db < 6.0 {
            issue(
                &mut checks,
                "record_check_no_signal",
                &segment.speaker,
                "error",
                &[("snr_db", measurement.snr_db)],
            );
            takes.push(MeasuredTake {
                measurement,
                ear_silent,
                wrong_side: false,
                headphone_left_db,
                headphone_right_db,
            });
            continue;
        }
        if peak_db >= -0.1 {
            issue(
                &mut checks,
                "record_check_clipping",
                &segment.speaker,
                "error",
                &[("peak_db", measurement.peak_db)],
            );
        }
        if peak_db < -40.0 {
            issue(
                &mut checks,
                "record_check_quiet",
                &segment.speaker,
                "warning",
                &[("peak_db", measurement.peak_db)],
            );
        }
        let noisy = snr_db < 30.0;
        if noisy {
            issue(
                &mut checks,
                "record_check_noisy",
                &segment.speaker,
                "warning",
                &[("snr_db", measurement.snr_db)],
            );
        }
        if !headphones
            && let Some((left, right)) = ear_levels
            && ear_silent
        {
            let (key, difference) = if left < right {
                (
                    "record_check_ear_silent_left",
                    20.0 * (right / left.max(1e-300)).log10(),
                )
            } else {
                (
                    "record_check_ear_silent_right",
                    20.0 * (left / right.max(1e-300)).log10(),
                )
            };
            issue(
                &mut checks,
                key,
                &segment.speaker,
                "error",
                &[("db_diff", rounded(difference, 1))],
            );
        }
        // Second-harmonic energy at -20 dB (10 %) or more is audible distortion.
        if !headphones
            && let Some((_, impulse, _, _, _, _)) = &analysis
            && let Some(distortion_db) = impulse.distortion_db
            && distortion_db >= -20.0
        {
            issue(
                &mut checks,
                "record_check_distortion",
                &segment.speaker,
                "warning",
                &[("distortion_db", rounded(distortion_db, 1))],
            );
        }

        let mut wrong_side = false;
        if !headphones
            && !ear_silent
            && let Some(itd) = itd
        {
            if itd.inverted {
                issue(
                    &mut checks,
                    "record_check_polarity",
                    &segment.speaker,
                    "error",
                    &[],
                );
            }
            if let Some(expected) = direction(&segment.speaker, azimuth) {
                let lateral_measured = lateral_angle(itd.itd_us);
                let itd_value = rounded(itd.itd_us, 0);
                wrong_side = if expected.side > 0 && itd.itd_us <= -100.0 {
                    issue(
                        &mut checks,
                        "record_check_wrong_side_left",
                        &segment.speaker,
                        "error",
                        &[("itd_us", itd_value)],
                    );
                    true
                } else if expected.side < 0 && itd.itd_us >= 100.0 {
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
                if !wrong_side && expected.lateral < 5.0 && lateral_measured >= 10.0 {
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
                } else if !wrong_side
                    && azimuth.is_some()
                    && expected.lateral >= 5.0
                    && (lateral_measured - expected.lateral).abs() >= 12.0
                {
                    let absolute_azimuth = expected.azimuth.abs();
                    let key = if absolute_azimuth < 75.0 {
                        if lateral_measured > expected.lateral {
                            "record_check_angle_too_far"
                        } else {
                            "record_check_angle_too_little"
                        }
                    } else if absolute_azimuth > 105.0 {
                        if lateral_measured > expected.lateral {
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
        }
        let has_distortion = analysis.as_ref().is_some_and(|(_, impulse, _, _, _, _)| {
            impulse
                .distortion_db
                .is_some_and(|distortion_db| distortion_db >= -20.0)
        });
        if !headphones
            && !ear_silent
            && !wrong_side
            && !itd.is_some_and(|value| value.inverted)
            && !has_distortion
        {
            measurement.direct = analysis
                .as_ref()
                .and_then(|(_, _, _, _, _, direct)| direct.clone());
        }
        takes.push(MeasuredTake {
            measurement,
            ear_silent,
            wrong_side,
            headphone_left_db,
            headphone_right_db,
        });
    }

    let current_speakers: Vec<_> = takes
        .iter()
        .map(|take| take.measurement.speaker.as_str())
        .collect();
    let references: Vec<Measurement> = reference_measurements
        .unwrap_or(&[])
        .iter()
        .map(|reference| Measurement {
            speaker: reference.speaker.clone(),
            peak_db: 0.0,
            snr_db: 0.0,
            itd_us: reference.itd_us,
            lf_db: reference.lf_db,
            lf_ild_db: reference.lf_ild_db,
            azimuth: reference.azimuth,
            direct: reference.direct.clone(),
        })
        .collect();
    let comparison_references: Vec<_> = references
        .iter()
        .filter(|reference| !current_speakers.contains(&reference.speaker.as_str()))
        .collect();

    if headphones {
        let headphone_balance_db = takes
            .iter()
            .find(|take| take.measurement.speaker == "FL")
            .and_then(|take| take.headphone_left_db)
            .zip(
                takes
                    .iter()
                    .find(|take| take.measurement.speaker == "FR")
                    .and_then(|take| take.headphone_right_db),
            )
            .map(|(left, right)| left - right);
        let speaker_balance_db = median(
            reference_measurements
                .unwrap_or(&[])
                .iter()
                .filter_map(|reference| reference.lf_ild_db)
                .collect(),
        );
        if let Some((headphone, speaker)) = headphone_balance_db.zip(speaker_balance_db) {
            let predicted = speaker - headphone;
            if predicted.abs() >= 2.0 {
                issue(
                    &mut checks,
                    "record_check_headphone_balance",
                    takes.first().map_or("", |take| &take.measurement.speaker),
                    "warning",
                    &[
                        ("db_diff", rounded(headphone, 1)),
                        ("predicted_db", rounded(predicted, 1)),
                    ],
                );
            }
        }
        return CheckResults {
            measurements: takes.into_iter().map(|take| take.measurement).collect(),
            checks,
        };
    }

    let input_balance_db = median(
        takes
            .iter()
            .filter(|take| take.measurement.snr_db >= 6.0 && !take.ear_silent && !take.wrong_side)
            .filter_map(|take| take.measurement.lf_ild_db)
            .collect(),
    );
    if let Some(balance) = input_balance_db.filter(|balance| balance.abs() >= 3.0)
        && let Some(first) = takes.first()
    {
        issue(
            &mut checks,
            "record_check_input_balance",
            &first.measurement.speaker,
            "warning",
            &[("db_diff", rounded(balance, 1))],
        );
    }

    for index in 0..takes.len() {
        if takes[index].ear_silent
            || takes[index].wrong_side
            || takes[index].measurement.itd_us.is_none()
        {
            continue;
        }
        let measurement = &takes[index].measurement;
        let candidate = takes[..index]
            .iter()
            .filter(|take| {
                !take.ear_silent
                    && !take.wrong_side
                    && take.measurement.itd_us.is_some()
                    && same_angle(measurement, &take.measurement)
            })
            .map(|take| &take.measurement)
            .chain(comparison_references.iter().copied().filter(|reference| {
                reference.itd_us.is_some() && same_angle(measurement, reference)
            }))
            .map(|reference| {
                let difference =
                    (measurement.itd_us.unwrap().abs() - reference.itd_us.unwrap().abs()).abs();
                (reference, difference)
            })
            .max_by(|(_, a), (_, b)| a.total_cmp(b));
        if let Some((reference, difference)) = candidate
            && difference >= 100.0
        {
            mismatch_issue(&mut checks, measurement, reference, difference);
        }
    }

    for (index, take) in takes.iter().enumerate() {
        let measurement = &take.measurement;
        let (Some(_), Some(level)) = (measurement.azimuth, measurement.lf_db) else {
            continue;
        };
        let comparison = comparison_references
            .iter()
            .copied()
            .chain(
                takes
                    .iter()
                    .enumerate()
                    .filter(|(other, _)| *other != index)
                    .map(|(_, take)| &take.measurement),
            )
            .filter_map(|other| other.azimuth.zip(other.lf_db).map(|(_, level)| level))
            .collect();
        if let Some(reference) = median(comparison) {
            let difference = level - reference;
            if difference.abs() >= 4.0 {
                issue(
                    &mut checks,
                    "record_check_level_changed",
                    &measurement.speaker,
                    "warning",
                    &[("db_diff", rounded(difference, 1))],
                );
            }
        }
    }

    let directional: Vec<_> = takes
        .iter()
        .filter(|take| !take.ear_silent)
        .map(|take| &take.measurement)
        // A speaker recorded again now replaces its earlier reference here too.
        .chain(comparison_references.iter().copied())
        .filter_map(|measurement| {
            let itd = measurement.itd_us?;
            let expected = direction(&measurement.speaker, measurement.azimuth)?;
            (expected.side != 0 && itd.abs() >= 100.0).then_some((expected.side, itd))
        })
        .collect();
    if directional.len() >= 2
        && directional.iter().any(|(side, _)| *side > 0)
        && directional.iter().any(|(side, _)| *side < 0)
        && directional
            .iter()
            .all(|(side, itd)| (*side > 0 && *itd < 0.0) || (*side < 0 && *itd > 0.0))
        && let Some(first) = takes.first()
    {
        issue(
            &mut checks,
            "record_check_mics_swapped",
            &first.measurement.speaker,
            "error",
            &[],
        );
    }

    let polarity_measurements = SLOTS
        .iter()
        .filter_map(|slot| {
            takes
                .iter()
                .map(|take| &take.measurement)
                .chain(comparison_references.iter().copied())
                .find(|measurement| measurement.speaker == slot.code)
                .filter(|measurement| measurement.direct.is_some())
        })
        .collect::<Vec<_>>();
    let polarity_inputs = polarity_measurements
        .iter()
        .map(|measurement| {
            let direct = measurement.direct.as_ref().unwrap();
            let side = match direction(&measurement.speaker, measurement.azimuth)
                .expect("known speaker has a direction")
                .side
            {
                1 => Side::Left,
                -1 => Side::Right,
                _ => Side::Centre,
            };
            PolarityInput {
                fs: direct.fs,
                side,
                left: Some(&direct.left),
                right: Some(&direct.right),
            }
        })
        .collect::<Vec<_>>();
    let polarity_pairs = polarity::pair_matrix(&polarity_inputs);
    for verdict in polarity::verdicts(&polarity_pairs) {
        match verdict {
            Verdict::Reversed {
                index,
                confidence,
                compared,
                opposite,
            } => polarity_issue(
                &mut checks,
                "record_check_speaker_polarity",
                &polarity_measurements[index].speaker,
                [
                    ("compared".to_owned(), json!(compared)),
                    ("opposite".to_owned(), json!(opposite)),
                    ("confidence".to_owned(), json!(format!("{confidence:.2}"))),
                ]
                .into_iter()
                .collect(),
            ),
            Verdict::Split { a, b, confidence } => {
                let group = |indices: &[usize]| {
                    indices
                        .iter()
                        .map(|index| polarity_measurements[*index].speaker.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                polarity_issue(
                    &mut checks,
                    "record_check_speaker_polarity_split",
                    &polarity_measurements[a[0]].speaker,
                    [
                        ("group_a".to_owned(), json!(group(&a))),
                        ("group_b".to_owned(), json!(group(&b))),
                        ("confidence".to_owned(), json!(format!("{confidence:.2}"))),
                    ]
                    .into_iter()
                    .collect(),
                );
            }
        }
    }

    CheckResults {
        measurements: takes.into_iter().map(|take| take.measurement).collect(),
        checks,
    }
}
