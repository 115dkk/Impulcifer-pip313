//! Low-frequency rolloff protection for virtual room tuning 2.
use super::{peq, primitives::median};
use crate::fft::Complex64;
use std::f64::consts::PI;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LimitOutcome {
    pub reduced: Vec<usize>,
    pub cuts: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rolloff {
    pub reference: f64,
    pub threshold: f64,
    pub frequency: Option<f64>,
    pub crossing: Option<(f64, f64, f64, f64)>,
}

fn interp(x: f64, grid: &[f64], values: &[f64]) -> f64 {
    super::primitives::interp(x, grid, values)
}

/// Find the -10 dB low-frequency rolloff from a third-octave-smoothed power response.
pub fn rolloff(power: &[f64], frequencies: &[f64]) -> Rolloff {
    assert_eq!(power.len(), frequencies.len());
    let smoothed = super::primitives::smooth(power, frequencies, &[3.0]);
    let db: Vec<_> = smoothed
        .iter()
        .map(|value| 10.0 * (value + 1e-30).log10())
        .collect();
    rolloff_from_db(&db, frequencies)
}

/// Find the rolloff crossing from an already-smoothed dB response.
pub fn rolloff_from_db(db: &[f64], frequencies: &[f64]) -> Rolloff {
    assert_eq!(db.len(), frequencies.len());
    let reference = median(
        &frequencies
            .iter()
            .enumerate()
            .filter(|(_, frequency)| (200.0..=1000.0).contains(*frequency))
            .map(|(index, _)| db[index])
            .collect::<Vec<_>>(),
    );
    let threshold = reference - 10.0;
    let mut upper = 200.0;
    let mut upper_db = interp(upper, frequencies, db);
    let mut lows: Vec<_> = frequencies
        .iter()
        .copied()
        .filter(|frequency| *frequency >= 20.0 && *frequency < 200.0)
        .collect();
    lows.reverse();
    if lows.last().copied() != Some(20.0) {
        lows.push(20.0);
    }
    for lower in lows {
        let lower_db = interp(lower, frequencies, db);
        if lower_db <= threshold {
            let frequency = if upper_db > threshold {
                lower + (upper - lower) * (threshold - lower_db) / (upper_db - lower_db)
            } else {
                upper
            };
            return Rolloff {
                reference,
                threshold,
                frequency: Some(frequency),
                crossing: Some((upper, upper_db, lower, lower_db)),
            };
        }
        upper = lower;
        upper_db = lower_db;
    }
    Rolloff {
        reference,
        threshold,
        frequency: None,
        crossing: None,
    }
}

/// Remove positive gain from PEQ centres below the measured rolloff.
pub fn clamp(bands: &mut [(f64, f64, f64)], rolloff_hz: f64) {
    for band in bands {
        if band.0 < rolloff_hz {
            band.2 = band.2.min(0.0);
        }
    }
}

fn db(value: Complex64) -> f64 {
    if !value.re.is_finite() || !value.im.is_finite() {
        f64::INFINITY
    } else {
        20.0 * peq::magnitude(value).max(1e-30).log10()
    }
}

fn excess_at(
    bands: &[(f64, f64, f64)],
    fixed: &[Complex64],
    frequencies: &[f64],
    index: usize,
    fs: f64,
) -> f64 {
    db(peq::response(&[frequencies[index]], bands, fs)[0] * fixed[index]) - 0.5
}

/// The worst response excess below the rolloff, optionally applying the reduction eligibility rule.
pub fn worst_excess(
    bands: &[(f64, f64, f64)],
    fixed: &[Complex64],
    frequencies: &[f64],
    rolloff_hz: f64,
    fs: f64,
) -> Option<(usize, f64)> {
    assert_eq!(fixed.len(), frequencies.len());
    let response = peq::response(frequencies, bands, fs);
    let mut worst = None;
    for index in 0..frequencies.len() {
        if frequencies[index] < 10.0 || frequencies[index] >= rolloff_hz {
            continue;
        }
        let excess = db(response[index] * fixed[index]) - 0.5;
        if excess > 0.0 && worst.is_none_or(|(_, value)| excess > value) {
            worst = Some((index, excess));
        }
    }
    worst
}

/// Limit the realized PEQ response below the measured rolloff.
pub fn limit(
    bands: &mut Vec<(f64, f64, f64)>,
    fixed: &[Complex64],
    frequencies: &[f64],
    rolloff_hz: f64,
    mut outcome: LimitOutcome,
    fs: f64,
) -> LimitOutcome {
    assert_eq!(fixed.len(), frequencies.len());
    let mut excluded = vec![false; frequencies.len()];
    let reduction_rounds = 4 * bands.len() + 8;
    for _ in 0..reduction_rounds {
        let response = peq::response(frequencies, bands, fs);
        let positive: Vec<_> = bands
            .iter()
            .enumerate()
            .filter(|(_, band)| band.2 > 0.0)
            .map(|(index, band)| (index, peq::response(frequencies, &[*band], fs)))
            .collect();
        let mut worst = None;
        for index in 0..frequencies.len() {
            if frequencies[index] < 10.0
                || frequencies[index] >= rolloff_hz
                || excluded[index]
                || !positive
                    .iter()
                    .any(|(_, response)| db(response[index]) > 0.05)
            {
                continue;
            }
            let excess = db(response[index] * fixed[index]) - 0.5;
            if excess > 0.0 && worst.is_none_or(|(_, value)| excess > value) {
                worst = Some((index, excess));
            }
        }
        let Some((bin, _)) = worst else { break };
        let mut candidate = None;
        for (index, response) in &positive {
            let contribution = db(response[bin]);
            if contribution > 0.05 && candidate.is_none_or(|(_, value)| contribution > value) {
                candidate = Some((*index, contribution));
            }
        }
        let Some((band_index, _)) = candidate else {
            break;
        };
        let original = bands[band_index].2;
        bands[band_index].2 = 0.0;
        if excess_at(bands, fixed, frequencies, bin, fs) > 0.0 {
            bands[band_index].2 = original;
            excluded[bin] = true;
            continue;
        }
        let (mut low, mut high) = (0.0, original);
        for _ in 0..40 {
            let mid = (low + high) / 2.0;
            bands[band_index].2 = mid;
            if excess_at(bands, fixed, frequencies, bin, fs) <= 0.0 {
                low = mid;
            } else {
                high = mid;
            }
        }
        bands[band_index].2 = low;
        if !outcome.reduced.contains(&band_index) {
            outcome.reduced.push(band_index);
        }
    }

    while outcome.cuts < 4 {
        let Some((bin, excess)) = worst_excess(bands, fixed, frequencies, rolloff_hz, fs) else {
            break;
        };
        if !excess.is_finite() {
            break;
        }
        bands.push((frequencies[bin], 1.4, -(excess + 0.1)));
        outcome.cuts += 1;
    }

    for _ in 0..8 {
        let Some((bin, excess)) = worst_excess(bands, fixed, frequencies, rolloff_hz, fs) else {
            break;
        };
        if !excess.is_finite() {
            break;
        }
        let first_cut = bands.len().saturating_sub(outcome.cuts);
        let mut selected = None;
        for (index, band) in bands.iter().enumerate().skip(first_cut) {
            let attenuation = -db(peq::response(&[frequencies[bin]], &[*band], fs)[0]);
            if attenuation.is_finite()
                && attenuation >= 1.0
                && selected.is_none_or(|(_, value)| attenuation > value)
            {
                selected = Some((index, attenuation));
            }
        }
        let Some((index, attenuation)) = selected else {
            break;
        };
        let relative = attenuation / -bands[index].2;
        bands[index].2 -= (excess / relative).min(12.0);
    }
    outcome.reduced.sort_unstable();
    outcome
}

/// Whether the realized response stays at or below +0.5 dB below the rolloff.
pub fn limited(
    bands: &[(f64, f64, f64)],
    fixed: &[Complex64],
    frequencies: &[f64],
    rolloff_hz: f64,
    fs: f64,
) -> bool {
    worst_excess(bands, fixed, frequencies, rolloff_hz, fs).is_none()
}

/// Remove excess positive DC gain from a realized FIR.
pub fn correct_fir_dc(fir: &mut [f64]) -> (bool, f64) {
    let excess = fir.iter().sum::<f64>() - 1.0;
    let threshold = 10_f64.powf(0.5 / 20.0) - 1.0;
    if fir.len() <= 2 || !excess.is_finite() || excess <= threshold {
        return (false, excess);
    }
    let denominator = (0..fir.len())
        .map(|index| 0.5 * (1.0 - (2.0 * PI * index as f64 / (fir.len() - 1) as f64).cos()))
        .sum::<f64>();
    let length = fir.len();
    for (index, value) in fir.iter_mut().enumerate() {
        let weight = 0.5 * (1.0 - (2.0 * PI * index as f64 / (length - 1) as f64).cos());
        *value -= excess * weight / denominator;
    }
    (true, excess)
}
