//! File-free Farina sweep estimator, ported from core/impulse_response_estimator.py.
use crate::{DspError, conv, fft, windows};
use impulcifer_types::constants::{SEQUENCE_TRACK_ORDERS, SPEAKER_NAMES};
use std::f64::consts::PI;

#[derive(Clone, Debug)]
pub struct SweepEstimator {
    pub fs: u32,
    pub low: f64,
    pub high: f64,
    pub n_octaves: f64,
    pub test_signal: Vec<f64>,
    pub duration: f64,
    pub inverse_filter: Vec<f64>,
}

impl SweepEstimator {
    /// Python ImpulseResponseEstimator.__init__, lines 33-50; p08_estimator.json.
    pub fn new(min_duration: f64, fs: u32) -> Result<Self, DspError> {
        if fs <= 10 || !min_duration.is_finite() || min_duration <= 0.0 {
            return Err(DspError::InvalidArgument(
                "positive duration and fs > 10 required".into(),
            ));
        }
        let high = fs as f64 / 2.0;
        let n_octaves = (high / 5.0).log2().ceil();
        let low = high / 2.0_f64.powf(n_octaves);
        let test_signal = Self::generate_test_signal(fs, n_octaves, min_duration, Some(0.5), None);
        let duration = test_signal.len() as f64 / fs as f64;
        let inverse_filter = Self::generate_inverse_filter(&test_signal, n_octaves);
        Ok(Self {
            fs,
            low,
            high,
            n_octaves,
            test_signal,
            duration,
            inverse_filter,
        })
    }

    /// Python generate_test_signal, lines 86-147; p08_estimator.json.
    ///
    /// # Panics
    /// Negative/nonfinite fades or fade halves exceeding the combined sweep length
    /// panic. Python rejects invalid Hann/ones dimensions; this Vec API cannot
    /// return an error. Negative fades are rejected even if int() would round to zero.
    pub fn generate_test_signal(
        fs: u32,
        n_octaves: f64,
        min_duration: f64,
        fade_in: Option<f64>,
        fade_out: Option<f64>,
    ) -> Vec<f64> {
        let power = 2.0_f64.powf(n_octaves);
        let logarithm = power.ln();
        let m = (min_duration * fs as f64 * (PI / power) / (PI * 2.0 * logarithm)).ceil();
        let l = m * PI * 2.0 * logarithm / (PI / power);
        let n = l.round_ties_even() as usize;
        let scale = PI / power * l / logarithm;
        let mut signal: Vec<_> = (0..n)
            .map(|i| (scale * (i as f64 / n as f64 * logarithm).exp()).sin())
            .collect();
        let seconds_per_octave = n as f64 / fs as f64 / n_octaves;
        let fade_half = |fade: Option<f64>| {
            fade.map_or(0, |fade| {
                assert!(
                    fade.is_finite() && fade >= 0.0,
                    "fade must be finite and nonnegative"
                );
                (fs as f64 * seconds_per_octave * fade) as usize
            })
        };
        let fade_in_half = fade_half(fade_in);
        let fade_out_half = fade_half(fade_out);
        assert!(
            fade_in_half <= n && fade_out_half <= n - fade_in_half,
            "combined fades exceed sweep length"
        );
        if fade_in.is_some() {
            let half = fade_in_half;
            for (x, w) in signal[..half].iter_mut().zip(windows::hann(2 * half, true)) {
                *x *= w;
            }
        }
        if fade_out.is_some() {
            let half = fade_out_half;
            for (x, w) in signal[n - half..]
                .iter_mut()
                .zip(&windows::hann(2 * half, true)[half..])
            {
                *x *= w;
            }
        }
        signal
    }

    /// Python generate_inverse_filter, lines 73-84; p08_estimator.json.
    pub fn generate_inverse_filter(test_signal: &[f64], n_octaves: f64) -> Vec<f64> {
        let base = 2.0_f64.powf(n_octaves / test_signal.len() as f64);
        let mut inverse: Vec<_> = test_signal
            .iter()
            .rev()
            .enumerate()
            .map(|(i, x)| {
                x * base.powf(-(i as f64)) * n_octaves * 2.0_f64.ln()
                    / (1.0 - 2.0_f64.powf(-n_octaves))
            })
            .collect();
        let full: Vec<_> = conv::convolve(&inverse, test_signal, conv::Mode::Full)
            .into_iter()
            .map(|x| fft::Complex64::new(x, 0.0))
            .collect();
        let bin = (full.len() as f64 / 4.0).round_ties_even() as usize;
        let energy = fft::fft(&full)[bin].norm();
        inverse.iter_mut().for_each(|x| *x /= energy);
        inverse
    }

    /// Python estimate, lines 149-151; p08_estimator.json.
    pub fn estimate(&self, recording: &[f64]) -> Vec<f64> {
        conv::convolve(recording, &self.inverse_filter, conv::Mode::Same)
    }

    /// Python sweep_sequence, lines 153-232; p08_sequence.json.
    /// Duplicates are deliberately accepted: Python never fills unique_speakers.
    pub fn sweep_sequence(
        &self,
        speakers: &[&str],
        tracks: &str,
    ) -> Result<Vec<Vec<f64>>, DspError> {
        let mono = ["FL"];
        let (speakers, order, count): (&[&str], &[&str], usize) = if let Some((_, order)) =
            SEQUENCE_TRACK_ORDERS
                .iter()
                .find(|(name, _)| *name == tracks)
        {
            (speakers, order, order.len())
        } else if tracks == "stereo" {
            if !(1..=2).contains(&speakers.len()) {
                return Err(DspError::InvalidArgument(
                    "\"stereo\" track configuration requires one or two speakers.".into(),
                ));
            }
            for speaker in speakers {
                if !SPEAKER_NAMES.contains(speaker) {
                    return Err(DspError::InvalidArgument(format!(
                        "Speaker name \"{speaker}\" is not a recognised speaker."
                    )));
                }
            }
            (speakers, speakers, 2)
        } else if tracks == "mono" {
            (&mono, &mono, 1)
        } else {
            return Err(DspError::InvalidArgument(format!(
                "Unsupported track configuration \"{tracks}\"."
            )));
        };
        let silence = 2 * self.fs as usize;
        let stride = silence + self.test_signal.len();
        let mut data = vec![vec![0.0; stride * speakers.len() + silence]; count];
        for (i, speaker) in speakers.iter().enumerate() {
            let index = order.iter().position(|s| s == speaker).ok_or_else(|| {
                DspError::InvalidArgument(format!(
                    "Speaker name \"{speaker}\" not supported with track configuration \"{tracks}\""
                ))
            })?;
            // Assignment replaces the entire row, including an earlier duplicate.
            data[index].fill(0.0);
            let start = stride * i + silence;
            data[index][start..start + self.test_signal.len()].copy_from_slice(&self.test_signal);
        }
        Ok(data)
    }

    /// Python from_wav, lines 234-262, already-read first track; p08_repair.json.
    pub fn from_samples(fs: u32, data: &[f64]) -> Result<Self, DspError> {
        if data.len() < 2 {
            return Err(DspError::InvalidArgument(
                "sweep requires at least two samples".into(),
            ));
        }
        let mut estimator = Self::new((data.len() - 1) as f64 / fs as f64, fs)?;
        let length_mismatch = estimator.test_signal.len() != data.len();
        if length_mismatch
            || estimator
                .test_signal
                .iter()
                .zip(data)
                .any(|(a, b)| (a - b).abs() > 1e-4)
        {
            estimator.test_signal = data.to_vec();
            if length_mismatch {
                estimator.duration = data.len() as f64 / fs as f64;
            }
            estimator.inverse_filter = Self::generate_inverse_filter(data, estimator.n_octaves);
        }
        Ok(estimator)
    }

    /// Python file_name, lines 264-273; p08_estimator.json and p08_repair.json.
    pub fn file_name(&self, bit_depth: u16) -> String {
        format!(
            "{:.2}s-{}Hz-{}bit-{:.2}Hz-{:.0}Hz",
            self.duration, self.fs, bit_depth, self.low, self.high
        )
    }
}
