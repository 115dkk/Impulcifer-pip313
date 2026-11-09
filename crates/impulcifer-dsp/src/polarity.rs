//! Relative speaker-polarity analysis for recording checks.

use crate::fft::{Complex64, irfft, rfft};
use std::{collections::VecDeque, f64::consts::PI};

pub const FFT_LEN: usize = 4096;
pub const DECISION: f64 = 0.5;

/// The window and the indices that located it, for the fixture gate.
#[derive(Clone, Debug, PartialEq)]
pub struct DirectWindow {
    pub onset: usize,
    pub peak: usize,
    pub start: isize,
    pub taper: usize,
    pub samples: Vec<f64>,
}

fn samples_for_ms(fs: u32, ms: f64) -> usize {
    (ms * f64::from(fs) / 1000.0).round() as usize
}

pub fn direct_window(h: &[f64], fs: u32) -> Option<DirectWindow> {
    if h.is_empty() {
        return None;
    }
    let length = samples_for_ms(fs, 6.0);
    if length == 0 || length > FFT_LEN {
        return None;
    }

    let mut first_peak = 0;
    let mut first_peak_abs = h[0].abs();
    for (index, sample) in h.iter().enumerate().skip(1) {
        let magnitude = sample.abs();
        if magnitude > first_peak_abs {
            first_peak = index;
            first_peak_abs = magnitude;
        }
    }
    let threshold = 0.1 * first_peak_abs;
    let onset = h[..=first_peak]
        .iter()
        .position(|sample| sample.abs() >= threshold)
        .unwrap_or(0);

    let search_end = onset
        .saturating_add(samples_for_ms(fs, 5.0))
        .min(h.len() - 1);
    let mut peak = onset;
    let mut peak_abs = h[onset].abs();
    for (index, sample) in h[onset..=search_end].iter().enumerate() {
        let magnitude = sample.abs();
        if magnitude >= peak_abs {
            peak = onset + index;
            peak_abs = magnitude;
        }
    }

    let start = peak as isize - samples_for_ms(fs, 1.5) as isize;
    let mut samples = (0..length)
        .map(|index| {
            let source = start + index as isize;
            if source >= 0 && (source as usize) < h.len() {
                h[source as usize]
            } else {
                0.0
            }
        })
        .collect::<Vec<_>>();
    let taper = ((0.1 * length as f64).round() as usize).max(1);
    for index in 0..taper.min(length) {
        let gain = 0.5 * (1.0 - (PI * index as f64 / taper as f64).cos());
        samples[index] *= gain;
        samples[length - 1 - index] *= gain;
    }

    Some(DirectWindow {
        onset,
        peak,
        start,
        taper,
        samples,
    })
}

pub fn band_weight(f: f64) -> f64 {
    let low = 300.0 / 2.0_f64.sqrt();
    let high = 3000.0 * 2.0_f64.sqrt();
    if !f.is_finite() || f < low || f > high {
        0.0
    } else if f < 300.0 {
        0.5 * (1.0 - (2.0 * PI * (f / low).log2()).cos())
    } else if f <= 3000.0 {
        1.0
    } else {
        0.5 * (1.0 + (2.0 * PI * (f / 3000.0).log2()).cos())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Correlation {
    pub value: f64,
    pub lag: isize,
    pub energy: f64,
}

/// Correlates two unpadded windows at the same sample rate.
pub fn window_correlation(a: &[f64], b: &[f64], fs: u32) -> Option<Correlation> {
    if a.len() > FFT_LEN || b.len() > FFT_LEN {
        return None;
    }
    let mut padded_a = vec![0.0; FFT_LEN];
    let mut padded_b = vec![0.0; FFT_LEN];
    padded_a[..a.len()].copy_from_slice(a);
    padded_b[..b.len()].copy_from_slice(b);
    let spectrum_a = rfft(&padded_a);
    let spectrum_b = rfft(&padded_b);
    let weights = (0..spectrum_a.len())
        .map(|bin| band_weight(bin as f64 * f64::from(fs) / FFT_LEN as f64))
        .collect::<Vec<_>>();

    let weighted_energy = |spectrum: &[Complex64]| {
        spectrum
            .iter()
            .zip(&weights)
            .enumerate()
            .map(|(bin, (value, weight))| {
                let multiplicity = if bin == 0 || bin == FFT_LEN / 2 {
                    1.0
                } else {
                    2.0
                };
                multiplicity * value.norm_sqr() * weight / FFT_LEN as f64
            })
            .sum::<f64>()
    };
    let energy = (weighted_energy(&spectrum_a) * weighted_energy(&spectrum_b)).sqrt();
    if !energy.is_finite() || energy <= f64::EPSILON {
        return None;
    }

    let cross = spectrum_a
        .iter()
        .zip(&spectrum_b)
        .zip(weights)
        .map(|((a, b), weight)| *a * b.conj() * weight)
        .collect::<Vec<_>>();
    let correlation = irfft(&cross, FFT_LEN);
    let max_lag = samples_for_ms(fs, 1.0).min(FFT_LEN / 2) as isize;
    let mut best_lag = -max_lag;
    let initial_index = if max_lag == 0 {
        0
    } else {
        (FFT_LEN as isize - max_lag) as usize
    };
    let mut best_value = correlation[initial_index] / energy;
    for lag in (-max_lag + 1)..=max_lag {
        let index = if lag < 0 {
            (FFT_LEN as isize + lag) as usize
        } else {
            lag as usize
        };
        let value = correlation[index] / energy;
        if value.abs() >= best_value.abs() {
            best_lag = lag;
            best_value = value;
        }
    }
    Some(Correlation {
        value: best_value.clamp(-1.0, 1.0),
        lag: best_lag,
        energy,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Centre,
}

pub struct PolarityInput<'a> {
    pub fs: u32,
    pub side: Side,
    pub left: Option<&'a [f64]>,
    pub right: Option<&'a [f64]>,
}

/// Decided averaged pair values (None when undecided or not compared), inputs in caller order.
pub fn pair_matrix(inputs: &[PolarityInput<'_>]) -> Vec<Vec<Option<f64>>> {
    let mut pairs = vec![vec![None; inputs.len()]; inputs.len()];
    for first in 0..inputs.len() {
        for second in first + 1..inputs.len() {
            if inputs[first].fs != inputs[second].fs {
                continue;
            }
            let ears: &[bool] = match (inputs[first].side, inputs[second].side) {
                (Side::Left, Side::Left | Side::Centre) | (Side::Centre, Side::Left) => &[true],
                (Side::Right, Side::Right | Side::Centre) | (Side::Centre, Side::Right) => &[false],
                (Side::Centre, Side::Centre) => &[true, false],
                _ => &[],
            };
            let values = ears
                .iter()
                .filter_map(|left| {
                    let (a, b) = if *left {
                        (inputs[first].left?, inputs[second].left?)
                    } else {
                        (inputs[first].right?, inputs[second].right?)
                    };
                    window_correlation(a, b, inputs[first].fs).map(|result| result.value)
                })
                .collect::<Vec<_>>();
            if !values.is_empty() {
                let value = values.iter().sum::<f64>() / values.len() as f64;
                if value.abs() >= DECISION {
                    pairs[first][second] = Some(value);
                    pairs[second][first] = Some(value);
                }
            }
        }
    }
    pairs
}

#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Reversed {
        index: usize,
        confidence: f64,
        compared: usize,
        opposite: usize,
    },
    Split {
        a: Vec<usize>,
        b: Vec<usize>,
        confidence: f64,
    },
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

pub fn verdicts(pairs: &[Vec<Option<f64>>]) -> Vec<Verdict> {
    let count = pairs.len();
    assert!(pairs.iter().all(|row| row.len() == count));
    let mut reached = vec![false; count];
    let mut out = Vec::new();

    for start in 0..count {
        if reached[start] || !pairs[start].iter().any(Option::is_some) {
            continue;
        }
        let mut signs = vec![None; count];
        signs[start] = Some(true);
        reached[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut contradiction = false;
        while let Some(current) = queue.pop_front() {
            let current_sign = signs[current].unwrap();
            for neighbour in 0..count {
                let Some(value) = pairs[current][neighbour] else {
                    continue;
                };
                let sign = if value < 0.0 {
                    !current_sign
                } else {
                    current_sign
                };
                match signs[neighbour] {
                    Some(existing) if existing != sign => contradiction = true,
                    Some(_) => {}
                    None => {
                        signs[neighbour] = Some(sign);
                        reached[neighbour] = true;
                        queue.push_back(neighbour);
                    }
                }
            }
        }
        if contradiction {
            continue;
        }
        let members = signs
            .iter()
            .enumerate()
            .filter_map(|(index, sign)| sign.map(|_| index))
            .collect::<Vec<_>>();
        let first_sign = signs[members[0]].unwrap();
        let a = members
            .iter()
            .copied()
            .filter(|index| signs[*index] == Some(first_sign))
            .collect::<Vec<_>>();
        let b = members
            .iter()
            .copied()
            .filter(|index| signs[*index] != Some(first_sign))
            .collect::<Vec<_>>();
        if b.is_empty() {
            continue;
        }
        if a.len() == b.len() {
            let between = a
                .iter()
                .flat_map(|first| b.iter().filter_map(|second| pairs[*first][*second]))
                .map(f64::abs)
                .collect::<Vec<_>>();
            out.push(Verdict::Split {
                a,
                b,
                confidence: median(between),
            });
        } else {
            let reversed = if a.len() < b.len() { &a } else { &b };
            for index in reversed {
                let decided = pairs[*index].iter().flatten().copied().collect::<Vec<_>>();
                out.push(Verdict::Reversed {
                    index: *index,
                    confidence: median(decided.iter().map(|value| value.abs()).collect()),
                    compared: decided.len(),
                    opposite: decided.iter().filter(|value| **value < 0.0).count(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contradicting_triangle_has_no_verdict() {
        let pairs = vec![
            vec![None, Some(0.8), Some(-0.8)],
            vec![Some(0.8), None, Some(0.8)],
            vec![Some(-0.8), Some(0.8), None],
        ];
        assert!(verdicts(&pairs).is_empty());
    }

    #[test]
    fn disconnected_components_are_judged_separately() {
        let pairs = vec![
            vec![None, Some(-0.8), None, None],
            vec![Some(-0.8), None, None, None],
            vec![None, None, None, Some(-0.9)],
            vec![None, None, Some(-0.9), None],
        ];
        assert_eq!(
            verdicts(&pairs),
            vec![
                Verdict::Split {
                    a: vec![0],
                    b: vec![1],
                    confidence: 0.8,
                },
                Verdict::Split {
                    a: vec![2],
                    b: vec![3],
                    confidence: 0.9,
                },
            ]
        );
    }

    #[test]
    fn mismatched_sample_rates_have_no_pair_value() {
        let window = [1.0, 0.0];
        let inputs = [
            PolarityInput {
                fs: 48_000,
                side: Side::Left,
                left: Some(&window),
                right: None,
            },
            PolarityInput {
                fs: 44_100,
                side: Side::Left,
                left: Some(&window),
                right: None,
            },
        ];
        assert_eq!(pair_matrix(&inputs), vec![vec![None; 2]; 2]);
    }
}
