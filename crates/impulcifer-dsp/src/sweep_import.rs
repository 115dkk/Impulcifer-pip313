//! Import a sequence recorded with an externally supplied logarithmic sweep.
//! The module is file-free; callers own WAV decoding and speaker assignment.

use crate::{
    DspError,
    fft::{Complex64, irfft, rfft},
};

/// Stable error messages for callers that distinguish invalid references from missing sweeps.
pub const NOT_LOGARITHMIC_SWEEP: &str = "not a logarithmic sweep covering 200–2000 Hz";
pub const NO_SWEEP_FOUND: &str = "no sweep found";

/// The active part of the played sweep.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceSweep {
    pub samples: Vec<f64>,
    pub fs: u32,
}

/// Located sweep starts and their deconvolved two-ear responses.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepImport {
    pub picks: Vec<usize>,
    pub band: (f64, f64),
    pub pre: usize,
    pub responses: Vec<[Vec<f64>; 2]>,
}

fn invalid(message: impl Into<String>) -> DspError {
    DspError::InvalidArgument(message.into())
}

/// Trim leading and trailing silence/noise from a played sweep.
pub fn trim_reference(samples: &[f64], fs: u32) -> Result<ReferenceSweep, DspError> {
    let window = (0.05 * f64::from(fs)).floor() as usize;
    if window == 0 || samples.len() < window {
        return Err(invalid("reference sweep is shorter than 50 ms"));
    }
    let mut prefix = Vec::with_capacity(samples.len() + 1);
    prefix.push(0.0);
    for sample in samples {
        prefix.push(prefix.last().copied().unwrap_or_default() + sample.abs());
    }
    let means: Vec<_> = (0..=samples.len() - window)
        .map(|start| (prefix[start + window] - prefix[start]) / window as f64)
        .collect();
    let peak = means.iter().copied().fold(0.0_f64, f64::max);
    if !peak.is_finite() || peak <= 0.0 {
        return Err(invalid("reference sweep has no active part"));
    }
    let threshold = 0.01 * peak;
    let first = means
        .iter()
        .position(|value| *value > threshold)
        .ok_or_else(|| invalid("reference sweep has no active part"))?;
    let last = means
        .iter()
        .rposition(|value| *value > threshold)
        .expect("first active window exists");
    let active = &samples[first..last + window];
    if active.len() < (0.5 * f64::from(fs)).floor() as usize {
        return Err(invalid(
            "reference active part is shorter than 0.5 s; not a sweep",
        ));
    }
    Ok(ReferenceSweep {
        samples: active.to_vec(),
        fs,
    })
}

fn padded_spectrum(samples: &[f64], n: usize) -> Vec<Complex64> {
    let mut padded = vec![0.0; n];
    padded[..samples.len()].copy_from_slice(samples);
    rfft(&padded)
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        0.5 * (values[middle - 1] + values[middle])
    } else {
        values[middle]
    })
}

fn first_max(values: &[f64]) -> Option<(usize, f64)> {
    let mut best = None;
    for (index, value) in values.iter().copied().enumerate() {
        if best.is_none_or(|(_, current)| value > current) {
            best = Some((index, value));
        }
    }
    best
}

fn correlation_energy(
    recording: &[f64],
    reference_spectrum: &[Complex64],
    n: usize,
    energy: &mut [f64],
) {
    let mut spectrum = padded_spectrum(recording, n);
    for (value, reference) in spectrum.iter_mut().zip(reference_spectrum) {
        *value *= reference.conj();
    }
    let correlation = irfft(&spectrum, n);
    drop(spectrum);
    for (sum, value) in energy.iter_mut().zip(correlation) {
        *sum += value * value;
    }
}

fn deconvolve(
    recording: &[f64],
    reference_spectrum: &[Complex64],
    window: &[f64],
    q_ref: f64,
    fs: u32,
    n: usize,
) -> Vec<f64> {
    let mut spectrum = padded_spectrum(recording, n);
    let bin_hz = f64::from(fs) / n as f64;
    for (index, ((value, reference), taper)) in spectrum
        .iter_mut()
        .zip(reference_spectrum)
        .zip(window)
        .enumerate()
    {
        let frequency = index as f64 * bin_hz;
        let regularization = 1e-6 * q_ref / frequency.max(bin_hz);
        *value = *value * reference.conj() * *taper / (reference.norm_sqr() + regularization);
    }
    irfft(&spectrum, n)
}

/// Locate every sweep in a two-ear recording and deconvolve the whole tracks.
pub fn import_sweeps(
    left: &[f64],
    right: &[f64],
    reference: &ReferenceSweep,
) -> Result<SweepImport, DspError> {
    if left.len() != right.len() {
        return Err(invalid(
            "left and right recording tracks must have equal lengths",
        ));
    }
    if left.len() < reference.samples.len() {
        return Err(invalid("recording is shorter than the sweep"));
    }
    let n = left
        .len()
        .checked_add(reference.samples.len())
        .and_then(usize::checked_next_power_of_two)
        .ok_or_else(|| invalid("recording and sweep are too long"))?;
    let reference_spectrum = padded_spectrum(&reference.samples, n);
    let bin_hz = f64::from(reference.fs) / n as f64;

    // Prefix sums of P[k] * f[k] make every logarithmic analysis band O(1).
    let mut q_prefix = Vec::with_capacity(reference_spectrum.len() + 1);
    q_prefix.push(0.0);
    for (index, value) in reference_spectrum.iter().enumerate() {
        let q = value.norm_sqr() * index as f64 * bin_hz;
        q_prefix.push(q_prefix.last().copied().unwrap_or_default() + q);
    }
    let bin_range = |low: f64, high: f64| {
        let start = (low / bin_hz).ceil() as usize;
        let end = (high / bin_hz).ceil() as usize;
        (
            start.min(reference_spectrum.len()),
            end.min(reference_spectrum.len()),
        )
    };
    let middle_start = (200.0 / bin_hz).ceil() as usize;
    let middle_end = ((2000.0 / bin_hz).floor() as usize + 1).min(reference_spectrum.len());
    let q_ref = median(
        (middle_start..middle_end)
            .map(|index| reference_spectrum[index].norm_sqr() * index as f64 * bin_hz)
            .collect(),
    )
    .filter(|value| value.is_finite() && *value > 0.0)
    .ok_or_else(|| invalid(NOT_LOGARITHMIC_SWEEP))?;

    let half_band = 2.0_f64.powf(1.0 / 12.0);
    let mut grid = Vec::new();
    let mut index = 0_u32;
    loop {
        let frequency = 10.0 * 2.0_f64.powf(f64::from(index) / 24.0);
        if frequency > f64::from(reference.fs) / 2.0 {
            break;
        }
        let (start, end) = bin_range(frequency / half_band, frequency * half_band);
        let q = if end > start {
            (q_prefix[end] - q_prefix[start]) / (end - start) as f64
        } else {
            0.0
        };
        grid.push((frequency, q));
        index += 1;
    }
    let logarithmic = grid
        .iter()
        .filter(|(frequency, _)| (200.0..=2000.0).contains(frequency))
        .all(|(_, q)| *q > 0.0 && (10.0 * (*q / q_ref).log10()).abs() <= 3.0);
    if !logarithmic {
        return Err(invalid(NOT_LOGARITHMIC_SWEEP));
    }
    let center = grid
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            (left.0 - 632.46).abs().total_cmp(&(right.0 - 632.46).abs())
        })
        .map(|(index, _)| index)
        .ok_or_else(|| invalid(NOT_LOGARITHMIC_SWEEP))?;
    let threshold = q_ref * 0.1;
    if grid[center].1 < threshold {
        return Err(invalid(NOT_LOGARITHMIC_SWEEP));
    }
    let mut low = center;
    while low > 0 && grid[low - 1].1 >= threshold {
        low -= 1;
    }
    let mut high = center;
    while high + 1 < grid.len() && grid[high + 1].1 >= threshold {
        high += 1;
    }
    let band = (grid[low].0, grid[high].0);
    drop(q_prefix);
    drop(grid);

    let mut band_window = Vec::with_capacity(reference_spectrum.len());
    let lower_outer = band.0 * 2.0_f64.powf(-1.0 / 3.0);
    let upper_outer = band.1 * 2.0_f64.powf(1.0 / 3.0);
    for index in 0..reference_spectrum.len() {
        let frequency = index as f64 * bin_hz;
        if frequency == 0.0 {
            band_window.push(0.0);
            continue;
        }
        let low_taper = (3.0 * (frequency / lower_outer).log2()).clamp(0.0, 1.0);
        let high_taper = (3.0 * (upper_outer / frequency).log2()).clamp(0.0, 1.0);
        let low_value = 0.5 * (1.0 - (std::f64::consts::PI * low_taper).cos());
        let high_value = 0.5 * (1.0 - (std::f64::consts::PI * high_taper).cos());
        band_window.push(low_value * high_value);
    }

    let mut energy = vec![0.0; left.len()];
    correlation_energy(left, &reference_spectrum, n, &mut energy);
    correlation_energy(right, &reference_spectrum, n, &mut energy);
    let (first_pick, top) = first_max(&energy).ok_or_else(|| invalid(NO_SWEEP_FOUND))?;
    if !top.is_finite() || top <= 0.0 {
        return Err(invalid(NO_SWEEP_FOUND));
    }
    let exclusion = (0.9 * reference.samples.len() as f64).floor() as usize;
    let mut picks = Vec::new();
    let mut candidate = Some((first_pick, top));
    while let Some((pick, value)) = candidate {
        if value < 1e-3 * top {
            break;
        }
        picks.push(pick);
        let start = pick.saturating_sub(exclusion);
        let end = pick
            .saturating_add(exclusion)
            .saturating_add(1)
            .min(energy.len());
        energy[start..end].fill(0.0);
        candidate = first_max(&energy);
    }
    drop(energy);
    if picks.is_empty() {
        return Err(invalid(NO_SWEEP_FOUND));
    }
    picks.sort_unstable();
    if picks
        .windows(2)
        .any(|pair| pair[1] - pair[0] < reference.samples.len())
    {
        return Err(invalid("overlapping sweeps"));
    }

    let harmonic_guard = (0.25 * reference.samples.len() as f64).floor() as usize;
    let mut response_length = n - picks[picks.len() - 1];
    response_length = response_length.min((3.0 * f64::from(reference.fs)).floor() as usize);
    for pair in picks.windows(2) {
        response_length = response_length.min(pair[1] - pair[0] - harmonic_guard);
    }
    if response_length < (0.2 * f64::from(reference.fs)).floor() as usize {
        return Err(invalid("sweeps too close for a response"));
    }
    let pre = ((0.1 * f64::from(reference.fs)).floor() as usize).min(picks[0]);

    let left_response = deconvolve(
        left,
        &reference_spectrum,
        &band_window,
        q_ref,
        reference.fs,
        n,
    );
    let right_response = deconvolve(
        right,
        &reference_spectrum,
        &band_window,
        q_ref,
        reference.fs,
        n,
    );
    drop(reference_spectrum);
    drop(band_window);
    let responses = picks
        .iter()
        .map(|pick| {
            let start = pick - pre;
            let end = pick + response_length;
            [
                left_response[start..end].to_vec(),
                right_response[start..end].to_vec(),
            ]
        })
        .collect();
    drop(left_response);
    drop(right_response);

    Ok(SweepImport {
        picks,
        band,
        pre,
        responses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_sweep(fs: u32, seconds: f64, low: f64, high: f64) -> Vec<f64> {
        let length = (seconds * f64::from(fs)).round() as usize;
        let ratio = high / low;
        let scale = 2.0 * std::f64::consts::PI * low * seconds / ratio.ln();
        (0..length)
            .map(|index| {
                let time = index as f64 / f64::from(fs);
                (scale * (ratio.powf(time / seconds) - 1.0)).sin()
            })
            .collect()
    }

    fn linear_sweep(fs: u32, seconds: f64, low: f64, high: f64) -> Vec<f64> {
        let length = (seconds * f64::from(fs)).round() as usize;
        let slope = (high - low) / seconds;
        (0..length)
            .map(|index| {
                let time = index as f64 / f64::from(fs);
                (2.0 * std::f64::consts::PI * (low * time + 0.5 * slope * time * time)).sin()
            })
            .collect()
    }

    fn noise(length: usize, amplitude: f64) -> Vec<f64> {
        let mut state = 0x1234_5678_u64;
        (0..length)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let value = ((state >> 11) as f64 / (1_u64 << 53) as f64) * 2.0 - 1.0;
                amplitude * value
            })
            .collect()
    }

    fn add_response(
        track: &mut [f64],
        sweep: &[f64],
        start: usize,
        delay: usize,
        gain: f64,
        second_delay: usize,
        second_gain: f64,
    ) {
        for (index, sample) in sweep.iter().copied().enumerate() {
            track[start + delay + index] += gain * sample;
            track[start + second_delay + index] += second_gain * sample;
        }
    }

    fn magnitude_at(samples: &[f64], fs: u32, frequency: f64) -> f64 {
        let omega = -2.0 * std::f64::consts::PI * frequency / f64::from(fs);
        let value = samples
            .iter()
            .enumerate()
            .fold(Complex64::new(0.0, 0.0), |sum, (index, sample)| {
                sum + Complex64::from_polar(*sample, omega * index as f64)
            });
        value.norm()
    }

    struct Synthetic {
        reference: ReferenceSweep,
        left: Vec<f64>,
        right: Vec<f64>,
        starts: Vec<usize>,
        responses: Vec<[(usize, f64, usize, f64); 2]>,
    }

    fn synthetic() -> Synthetic {
        let fs = 48_000;
        let reference = ReferenceSweep {
            samples: log_sweep(fs, 2.0, 10.0, 20_000.0),
            fs,
        };
        let starts = vec![12_000, 134_400, 273_600, 403_200, 542_400];
        let levels = [0.0, -3.0, -7.0, -11.0, -15.0];
        let responses: Vec<_> = levels
            .iter()
            .enumerate()
            .map(|(index, db)| {
                let gain = 10.0_f64.powf(*db / 20.0);
                [
                    (7 + index, gain, 47 + index, 0.21 * gain),
                    (12 + index, 0.72 * gain, 61 + index, -0.16 * gain),
                ]
            })
            .collect();
        let length = starts.last().unwrap() + reference.samples.len() + 1000;
        let mut left = noise(length, 0.001);
        let mut right = noise(length, 0.001);
        for (start, response) in starts.iter().copied().zip(&responses) {
            for (track, &(delay, gain, second_delay, second_gain)) in
                [&mut left, &mut right].into_iter().zip(response)
            {
                add_response(
                    track,
                    &reference.samples,
                    start,
                    delay,
                    gain,
                    second_delay,
                    second_gain,
                );
            }
        }
        Synthetic {
            reference,
            left,
            right,
            starts,
            responses,
        }
    }

    #[test]
    fn reference_trim_keeps_the_active_part() {
        let fs = 48_000;
        let sweep = log_sweep(fs, 2.0, 10.0, 20_000.0);
        let mut samples = noise(3 * fs as usize, 10.0_f64.powf(-54.0 / 20.0));
        samples.extend_from_slice(&sweep);
        samples.extend(noise(3 * fs as usize, 10.0_f64.powf(-54.0 / 20.0)));
        let trimmed = trim_reference(&samples, fs).unwrap();
        let start = samples
            .windows(2)
            .position(|window| window == &trimmed.samples[..2])
            .unwrap();
        let end = start + trimmed.samples.len();
        let tolerance = (0.06 * fs as f64) as usize;
        assert!(start.abs_diff(3 * fs as usize) <= tolerance);
        assert!(end.abs_diff(3 * fs as usize + sweep.len()) <= tolerance);
        assert!(trim_reference(&vec![0.0; fs as usize], fs).is_err());
    }

    #[test]
    fn import_finds_irregular_sweeps_and_recovers_the_responses() {
        let Synthetic {
            reference,
            left,
            right,
            starts,
            responses: known,
        } = synthetic();
        let imported = import_sweeps(&left, &right, &reference).unwrap();
        assert_eq!(imported.picks.len(), starts.len());
        let tap_scale = imported.responses[0][0][imported.pre].abs() / known[0][0].1.abs();
        for (index, ((pick, response), known)) in imported
            .picks
            .iter()
            .zip(&imported.responses)
            .zip(&known)
            .enumerate()
        {
            let expected_pick = starts[index] + known[0].0;
            assert!(
                pick.abs_diff(expected_pick) <= 1,
                "pick {index}: {pick} vs {expected_pick}"
            );
            for ear in 0..2 {
                let expected_position = imported.pre + known[ear].0 - known[0].0;
                let actual_position = response[ear]
                    .iter()
                    .enumerate()
                    .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
                    .unwrap()
                    .0;
                assert!(
                    actual_position.abs_diff(expected_position) <= 1,
                    "response {index}/{ear}: {actual_position} vs {expected_position}"
                );
                let actual_db = 20.0
                    * (response[ear][actual_position].abs() / (tap_scale * known[ear].1.abs()))
                        .log10();
                assert!(actual_db.abs() <= 0.2, "tap {index}/{ear}: {actual_db} dB");
                let mut ideal = vec![0.0; response[ear].len()];
                ideal[expected_position] = known[ear].1;
                ideal[imported.pre + known[ear].2 - known[0].0] = known[ear].3;
                for frequency in [100.0, 1000.0, 10_000.0] {
                    let actual = magnitude_at(&response[ear], reference.fs, frequency);
                    let expected = magnitude_at(&ideal, reference.fs, frequency);
                    let error = 20.0 * (actual / expected).log10();
                    assert!(
                        error.abs() <= 0.3,
                        "magnitude {index}/{ear}/{frequency}: {error} dB"
                    );
                }
            }
        }
    }

    #[test]
    fn import_reports_the_sweeps_it_finds_not_the_number_asked_for() {
        let Synthetic {
            reference,
            mut left,
            mut right,
            ..
        } = synthetic();
        let burst_start = 230_000;
        for index in burst_start..burst_start + 1000 {
            left[index] += 0.01;
            right[index] -= 0.01;
        }
        let imported = import_sweeps(&left, &right, &reference).unwrap();
        assert_eq!(imported.picks.len(), 5);
    }

    #[test]
    fn linear_sweep_is_rejected() {
        let fs = 48_000;
        let reference = trim_reference(&linear_sweep(fs, 2.0, 10.0, 20_000.0), fs).unwrap();
        let mut recording = vec![0.0; reference.samples.len() + fs as usize];
        recording[1000..1000 + reference.samples.len()].copy_from_slice(&reference.samples);
        let error = import_sweeps(&recording, &recording, &reference).unwrap_err();
        assert!(error.to_string().contains("not a logarithmic sweep"));
    }

    #[test]
    fn overlapping_or_too_close_sweeps_are_errors() {
        let fs = 8_000;
        let reference = ReferenceSweep {
            samples: log_sweep(fs, 1.0, 10.0, 3_800.0),
            fs,
        };
        let spacing = (0.95 * reference.samples.len() as f64) as usize;
        let mut recording = vec![0.0; spacing + reference.samples.len() + 100];
        for start in [50, 50 + spacing] {
            for (target, sample) in recording[start..].iter_mut().zip(&reference.samples) {
                *target += sample;
            }
        }
        let error = import_sweeps(&recording, &recording, &reference).unwrap_err();
        assert!(error.to_string().contains("overlapping sweeps"));
    }

    #[test]
    fn import_is_deterministic() {
        let fs = 8_000;
        let reference = ReferenceSweep {
            samples: log_sweep(fs, 1.0, 10.0, 3_800.0),
            fs,
        };
        let starts = [1000, 11_000, 22_000];
        let mut left = noise(31_000, 0.0001);
        let mut right = left.clone();
        for start in starts {
            add_response(&mut left, &reference.samples, start, 3, 1.0, 21, 0.2);
            add_response(&mut right, &reference.samples, start, 6, 0.8, 27, -0.1);
        }
        assert_eq!(
            import_sweeps(&left, &right, &reference).unwrap(),
            import_sweeps(&left, &right, &reference).unwrap()
        );
    }
}
