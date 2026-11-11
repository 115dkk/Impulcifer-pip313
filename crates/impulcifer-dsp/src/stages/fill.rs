//! Output-only synthesis for unmeasured speaker and LFE tracks (ADR 0013).
use crate::{
    DspError, fft,
    hrir::{Hrir, SpeakerIrs},
    ir::ImpulseResponse,
    stages::room_tuning,
};
use impulcifer_types::{
    constants::{Side, speaker_side},
    layouts::{SLOTS, Slot},
};
use std::f64::consts::SQRT_2;

const TIE_EPSILON: f64 = 1e-9;

#[derive(Clone, Debug)]
pub struct FillResult {
    pub hrir: Hrir,
    pub filled: Vec<(&'static str, String)>,
}

fn slot(code: &str) -> Option<&'static Slot> {
    SLOTS.iter().find(|slot| slot.code == code)
}

fn unit(slot: &Slot) -> [f64; 3] {
    let azimuth = slot.azimuth.to_radians();
    let elevation = slot.elevation.to_radians();
    [
        elevation.cos() * azimuth.cos(),
        elevation.cos() * azimuth.sin(),
        elevation.sin(),
    ]
}

fn angular_distance(a: &Slot, b: &Slot) -> f64 {
    let (a, b) = (unit(a), unit(b));
    a.iter()
        .zip(b)
        .map(|(x, y)| x * y)
        .sum::<f64>()
        .clamp(-1.0, 1.0)
        .acos()
}

fn candidate_is_better(target: &Slot, candidate: &Slot, best: &Slot) -> bool {
    let candidate_distance = angular_distance(target, candidate);
    let best_distance = angular_distance(target, best);
    if candidate_distance < best_distance - TIE_EPSILON {
        return true;
    }
    if (candidate_distance - best_distance).abs() >= TIE_EPSILON {
        return false;
    }
    let (candidate_azimuth, best_azimuth) = (candidate.azimuth.abs(), best.azimuth.abs());
    if (candidate_azimuth - best_azimuth).abs() >= TIE_EPSILON {
        if target.azimuth.abs() < 90.0 {
            candidate_azimuth < best_azimuth
        } else {
            candidate_azimuth > best_azimuth
        }
    } else {
        let candidate_index = SLOTS
            .iter()
            .position(|slot| slot.code == candidate.code)
            .unwrap();
        let best_index = SLOTS
            .iter()
            .position(|slot| slot.code == best.code)
            .unwrap();
        candidate_index < best_index
    }
}

fn nearest<'a>(target: &Slot, candidates: impl Iterator<Item = &'a Slot>) -> Option<&'a Slot> {
    candidates.fold(None, |best, candidate| match best {
        None => Some(candidate),
        Some(best) if candidate_is_better(target, candidate, best) => Some(candidate),
        Some(best) => Some(best),
    })
}

fn phantom(hrir: &Hrir, left: &str, right: &str, target: &str) -> Result<SpeakerIrs, DspError> {
    let (left_l, left_r) = hrir
        .pair(left)
        .ok_or_else(|| DspError::InvalidArgument(format!("missing measured pair {left}")))?;
    let (right_l, right_r) = hrir
        .pair(right)
        .ok_or_else(|| DspError::InvalidArgument(format!("missing measured pair {right}")))?;
    let combine = |a: &ImpulseResponse, b: &ImpulseResponse| -> Result<ImpulseResponse, DspError> {
        if a.fs != b.fs || a.data.len() != b.data.len() {
            return Err(DspError::InvalidArgument(
                "speaker IRs must have equal sample rates and lengths for phantom fill".into(),
            ));
        }
        Ok(ImpulseResponse {
            data: a
                .data
                .iter()
                .zip(&b.data)
                .map(|(a, b)| (a + b) / SQRT_2)
                .collect(),
            fs: a.fs,
            recording: None,
        })
    };
    Ok(SpeakerIrs {
        speaker: target.into(),
        left: Some(combine(left_l, right_l)?),
        right: Some(combine(left_r, right_r)?),
    })
}

fn measured_slots(hrir: &Hrir) -> impl Iterator<Item = &'static Slot> + '_ {
    SLOTS.iter().filter(|slot| hrir.pair(slot.code).is_some())
}

fn synthesize_neighbor(
    hrir: &Hrir,
    target: &'static Slot,
) -> Result<Option<(SpeakerIrs, String)>, DspError> {
    match speaker_side(target.code) {
        side @ (Side::Left | Side::Right) => {
            let source = nearest(
                target,
                measured_slots(hrir).filter(|candidate| {
                    candidate.layer == target.layer && speaker_side(candidate.code) == side
                }),
            );
            Ok(source.and_then(|source| {
                hrir.get(source.code).cloned().map(|mut pair| {
                    pair.speaker = target.code.into();
                    (pair, source.code.into())
                })
            }))
        }
        Side::Center => {
            let left = nearest(
                target,
                measured_slots(hrir).filter(|candidate| {
                    candidate.layer == target.layer
                        && speaker_side(candidate.code) == Side::Left
                        && measured_slots(hrir).any(|right| {
                            right.layer == candidate.layer
                                && speaker_side(right.code) == Side::Right
                                && (right.azimuth + candidate.azimuth).abs() < TIE_EPSILON
                                && (right.elevation - candidate.elevation).abs() < TIE_EPSILON
                        })
                }),
            );
            if let Some(left) = left {
                let right = measured_slots(hrir)
                    .find(|right| {
                        right.layer == left.layer
                            && speaker_side(right.code) == Side::Right
                            && (right.azimuth + left.azimuth).abs() < TIE_EPSILON
                            && (right.elevation - left.elevation).abs() < TIE_EPSILON
                    })
                    .expect("symmetric pair selected above");
                return Ok(Some((
                    phantom(hrir, left.code, right.code, target.code)?,
                    format!("{}+{}", left.code, right.code),
                )));
            }
            let source = nearest(
                target,
                measured_slots(hrir).filter(|candidate| {
                    candidate.layer == target.layer && speaker_side(candidate.code) == Side::Center
                }),
            );
            Ok(source.and_then(|source| {
                hrir.get(source.code).cloned().map(|mut pair| {
                    pair.speaker = target.code.into();
                    (pair, source.code.into())
                })
            }))
        }
    }
}

/// Fill only `targets`, always choosing sources from the original measured HRIR.
pub fn fill_missing_speakers(hrir: &Hrir, targets: &[&str]) -> Result<FillResult, DspError> {
    let mut output = hrir.clone();
    let mut filled = Vec::new();
    for target in targets {
        if output.pair(target).is_some() || filled.iter().any(|(slot, _)| slot == target) {
            continue;
        }
        let Some(target_slot) = slot(target) else {
            continue;
        };
        if let Some((pair, source)) = synthesize_neighbor(hrir, target_slot)? {
            output.speakers.push(pair);
            filled.push((target_slot.code, source));
        }
    }
    Ok(FillResult {
        hrir: output,
        filled,
    })
}

fn center_lfe(hrir: &Hrir, target: &str) -> Result<Option<(SpeakerIrs, String)>, DspError> {
    if let Some(pair) = hrir.get("FC").filter(|_| hrir.pair("FC").is_some()) {
        let mut pair = pair.clone();
        pair.speaker = target.into();
        return Ok(Some((pair, "FC".into())));
    }
    if hrir.pair("FL").is_some() && hrir.pair("FR").is_some() {
        return Ok(Some((phantom(hrir, "FL", "FR", target)?, "FL+FR".into())));
    }
    Ok(None)
}

fn peak_index(data: &[f64]) -> usize {
    data.iter()
        .enumerate()
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .map_or(0, |(index, _)| index)
}

fn all_lfe(hrir: &Hrir, target: &str) -> Result<Option<(SpeakerIrs, String)>, DspError> {
    let measured: Vec<_> = hrir
        .speakers
        .iter()
        .filter_map(|speaker| Some((speaker.left.as_ref()?, speaker.right.as_ref()?)))
        .collect();
    let Some((first_left, _)) = measured.first() else {
        return Ok(None);
    };
    let length = first_left.data.len();
    if length == 0 {
        return Err(DspError::InvalidArgument(
            "measured speaker IRs must not be empty".into(),
        ));
    }
    if measured
        .iter()
        .flat_map(|(left, right)| [*left, *right])
        .any(|ir| ir.fs != hrir.fs || ir.data.len() != length)
    {
        return Err(DspError::InvalidArgument(
            "measured speaker IRs must have equal sample rates and lengths for LFE synthesis"
                .into(),
        ));
    }
    let nfft = length
        .checked_mul(2)
        .and_then(usize::checked_next_power_of_two)
        .ok_or_else(|| DspError::InvalidArgument("LFE FFT length overflow".into()))?;
    let mut peaks: Vec<_> = measured
        .iter()
        .flat_map(|(left, right)| [peak_index(&left.data), peak_index(&right.data)])
        .collect();
    peaks.sort_unstable();
    let delay = peaks[(peaks.len() - 1) / 2];
    let synthesize = |ear: usize| {
        let mut power = vec![0.0; nfft / 2 + 1];
        for pair in &measured {
            let ir = if ear == 0 { pair.0 } else { pair.1 };
            let mut padded = vec![0.0; nfft];
            padded[..length].copy_from_slice(&ir.data);
            for (sum, bin) in power.iter_mut().zip(fft::rfft(&padded)) {
                *sum += bin.norm_sqr();
            }
        }
        let count = measured.len() as f64;
        let magnitude: Vec<_> = power
            .into_iter()
            .enumerate()
            .map(|(k, power)| {
                let frequency = k as f64 * hrir.fs as f64 / nfft as f64;
                (power / count).sqrt() / (1.0 + (frequency / 120.0).powi(4))
            })
            .collect();
        let minimum = fft::irfft(&room_tuning::minimum_phase(&magnitude, nfft), nfft);
        let mut data = vec![0.0; length];
        if delay < length {
            let copied = (length - delay).min(minimum.len());
            data[delay..delay + copied].copy_from_slice(&minimum[..copied]);
        }
        ImpulseResponse {
            data,
            fs: hrir.fs,
            recording: None,
        }
    };
    Ok(Some((
        SpeakerIrs {
            speaker: target.into(),
            left: Some(synthesize(0)),
            right: Some(synthesize(1)),
        },
        "all".into(),
    )))
}

/// Add one synthetic LFE slot to an output copy. The original HRIR is never changed.
pub fn fill_lfe(
    hrir: &Hrir,
    target: &str,
    mode: &str,
) -> Result<Option<(SpeakerIrs, String)>, DspError> {
    match mode {
        "none" => Ok(None),
        "center" => center_lfe(hrir, target),
        "all" => all_lfe(hrir, target),
        _ => Err(DspError::InvalidArgument(format!(
            "invalid lfe_mode: {mode}"
        ))),
    }
}

fn measured_lfe(measured: &[SpeakerIrs], target: &str) -> Option<(SpeakerIrs, String)> {
    [target, if target == "LFE" { "LFE2" } else { "LFE" }]
        .into_iter()
        .find_map(|source| {
            measured
                .iter()
                .find(|pair| pair.speaker == source && pair.left.is_some() && pair.right.is_some())
                .cloned()
                .map(|pair| (pair, source.to_owned()))
        })
        .map(|(mut pair, source)| {
            pair.speaker = target.into();
            (pair, source)
        })
}

fn complete<'a>(measured: &'a [SpeakerIrs], slot: &str) -> Option<&'a SpeakerIrs> {
    measured
        .iter()
        .find(|pair| pair.speaker == slot && pair.left.is_some() && pair.right.is_some())
}

/// The one LFE channel of a format driven by both measured subwoofers (ADR 0018): the sample sum
/// of the two finished tracks, ear by ear. The source is `sum:<LFE label>+<LFE2 label>`.
fn summed_lfe(
    measured: &[SpeakerIrs],
    labels: &[(String, String)],
    target: &str,
) -> Result<Option<(SpeakerIrs, String)>, DspError> {
    let (Some(first), Some(second)) = (complete(measured, "LFE"), complete(measured, "LFE2"))
    else {
        return Ok(None);
    };
    let sum = |a: &Option<ImpulseResponse>, b: &Option<ImpulseResponse>| {
        let (a, b) = (a.as_ref().unwrap(), b.as_ref().unwrap());
        if a.fs != b.fs || a.data.len() != b.data.len() {
            return Err(DspError::InvalidArgument(
                "LFE and LFE2 tracks must have equal sample rates and lengths to be summed".into(),
            ));
        }
        Ok(ImpulseResponse {
            data: a.data.iter().zip(&b.data).map(|(x, y)| x + y).collect(),
            fs: a.fs,
            recording: None,
        })
    };
    let label = |slot: &str| {
        labels
            .iter()
            .find(|(labelled, _)| labelled == slot)
            .map_or("measured", |(_, label)| label.as_str())
    };
    Ok(Some((
        SpeakerIrs {
            speaker: target.into(),
            left: Some(sum(&first.left, &second.left)?),
            right: Some(sum(&first.right, &second.right)?),
        },
        format!("sum:{}+{}", label("LFE"), label("LFE2")),
    )))
}

/// Assemble an output-only HRIR, filling speakers first and preferring measured LFE.
pub fn fill_output(
    hrir: &Hrir,
    measured_lfe_pairs: &[SpeakerIrs],
    measured_lfe_labels: &[(String, String)],
    targets: &[&str],
    missing_mode: &str,
    lfe_mode: &str,
) -> Result<FillResult, DspError> {
    let mut result = if missing_mode == "neighbor" {
        fill_missing_speakers(hrir, targets)?
    } else if missing_mode == "silent" {
        FillResult {
            hrir: hrir.clone(),
            filled: Vec::new(),
        }
    } else {
        return Err(DspError::InvalidArgument(format!(
            "invalid missing_speakers: {missing_mode}"
        )));
    };
    for lfe in ["LFE", "LFE2"] {
        if !targets.contains(&lfe) {
            continue;
        }
        let other = if lfe == "LFE" { "LFE2" } else { "LFE" };
        let summed = if targets.contains(&other) {
            None
        } else {
            summed_lfe(measured_lfe_pairs, measured_lfe_labels, lfe)?
        };
        if let Some((pair, source)) = summed {
            result.hrir.speakers.push(pair);
            result.filled.push((lfe, source));
        } else if let Some((pair, source_slot)) = measured_lfe(measured_lfe_pairs, lfe) {
            let source = measured_lfe_labels
                .iter()
                .find(|(slot, _)| slot == &source_slot)
                .map_or("measured", |(_, label)| label.as_str())
                .to_owned();
            result.hrir.speakers.push(pair);
            result.filled.push((lfe, source));
        } else if let Some((pair, source)) = fill_lfe(hrir, lfe, lfe_mode)? {
            result.hrir.speakers.push(pair);
            result.filled.push((lfe, source));
        }
    }
    Ok(result)
}
