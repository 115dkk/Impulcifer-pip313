//! File-free impulse response, core/impulse_response.py:15-155.
use crate::{DspError, conv, decay, fft, peaks, resample};

#[derive(Clone, Debug)]
pub struct ImpulseResponse {
    pub data: Vec<f64>,
    pub fs: u32,
    pub recording: Option<Vec<f64>>,
}

impl ImpulseResponse {
    /// Python __len__, lines 24-26; p08_ir.json.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    /// Python __len__ == 0, lines 24-26; p08_ir.json.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    /// Python duration, lines 28-30; p08_ir.json.
    pub fn duration(&self) -> f64 {
        self.len() as f64 / self.fs as f64
    }
    /// Python peak_index, lines 32-70; p08_demo_*.json.
    pub fn peak_index(&self, start: usize, end: Option<usize>, peak_height: f64) -> usize {
        peaks::first_peak_index(&self.data, start, end, peak_height)
    }
    /// Python crop_head, lines 82-90; p08_ir.json.
    pub fn crop_head(&mut self, head_ms: f64) {
        let start = (self.peak_index(0, None, 0.12589) as i64
            - (self.fs as f64 * head_ms / 1000.0) as i64)
            .max(0) as usize;
        self.data = self.data[start.min(self.len())..].to_vec();
    }
    /// Python shift, lines 92-108; p08_ir.json. No wrapping, length preserved.
    pub fn shift(&mut self, samples: i64) {
        let n = self.len();
        let count = samples.unsigned_abs().min(n as u64) as usize;
        if samples > 0 {
            self.data.copy_within(..n - count, count);
            self.data[..count].fill(0.0);
        } else if samples < 0 {
            self.data.copy_within(count.., 0);
            self.data[n - count..].fill(0.0);
        }
    }
    /// Python equalize, lines 110-119; p08_ir.json.
    pub fn equalize(&mut self, fir: &[f64]) {
        self.data = conv::convolve(&self.data, fir, conv::Mode::Full);
    }
    /// Python resample, lines 121-124; p06_poly_* and P08 properties.
    pub fn resample(&mut self, fs: u32) -> Result<(), DspError> {
        self.data = resample::nnresample(&self.data, fs, self.fs)?;
        self.fs = fs;
        Ok(())
    }
    /// Python convolve, lines 126-135; p08_ir.json.
    pub fn convolve(&self, x: &[f64]) -> Vec<f64> {
        conv::convolve(x, &self.data, conv::Mode::Full)
    }
    /// Python magnitude_response, lines 153-155 and audio_io:100-113; p08_ir.json.
    pub fn magnitude_response(&self) -> (Vec<f64>, Vec<f64>) {
        let magnitude = fft::magnitude_response(&self.data);
        let frequency = (0..magnitude.len())
            .map(|i| i as f64 * (self.fs as f64 / self.len() as f64))
            .collect();
        (frequency, magnitude)
    }
    /// Python decay_params, lines 72-73; p08_decay_*.json.
    pub fn decay_params(&self) -> decay::DecayParams {
        decay::decay_params(&self.data, self.fs)
    }
    /// Python decay_times, lines 75-80; p08_decay_*.json.
    pub fn decay_times(&self, params: Option<&decay::DecayParams>) -> decay::DecayTimes {
        decay::decay_times(&self.data, self.fs, params)
    }
    /// Python decay_adjustment_params, lines 137-138; p08_decay_*.json.
    ///
    /// # Panics
    /// A zero target panics like Python's ZeroDivisionError (the API returns Option).
    pub fn decay_adjustment_params(&self, target_seconds: f64) -> Option<decay::DecayAdjustment> {
        decay::decay_adjustment_params(&self.data, self.fs, target_seconds)
    }
    /// Python adjust_decay, lines 140-151; p08_decay_*.json.
    ///
    /// # Panics
    /// A zero target panics before modifying data, like Python's ZeroDivisionError.
    pub fn adjust_decay(&mut self, target_seconds: f64) {
        let params = self.decay_adjustment_params(target_seconds);
        decay::apply_decay_window(&mut self.data, params.as_ref());
    }
}
