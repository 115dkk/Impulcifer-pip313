//! AutoEQ smoothing, gain limiting and FIR synthesis using existing primitives.
use super::{FrequencyResponse, invalid, round_half_even, sigmoid, window_size};
use crate::{DspError, fft, fir, interp::Spline, smoothing};

/// Python smoothen_fractional_octave arguments (1107-1113); p09_smoothen_*.json.
#[derive(Clone, Debug)]
pub struct SmoothingParams {
    pub window_size: f64,
    pub iterations: usize,
    pub treble_window_size: f64,
    pub treble_iterations: usize,
    pub treble_f_lower: f64,
    pub treble_f_upper: f64,
}
impl Default for SmoothingParams {
    /// Python smoothen_fractional_octave defaults (1107-1113); p09_smoothen_*.json.
    fn default() -> Self {
        Self {
            window_size: 1.0 / 3.0,
            iterations: 1,
            treble_window_size: 1.0 / 3.0,
            treble_iterations: 1,
            treble_f_lower: 100.0,
            treble_f_upper: 10000.0,
        }
    }
}

/// Python equalize parameters (1241-1247); p09_equalize_*.json.
#[derive(Clone, Debug)]
pub struct EqualizeParams {
    pub max_gain: f64,
    pub smoothen: bool,
    pub treble_f_lower: f64,
    pub treble_f_upper: f64,
    pub treble_max_gain: f64,
    pub treble_gain_k: f64,
}
impl Default for EqualizeParams {
    /// Python equalize defaults (1241-1247); p09_equalize_*.json.
    fn default() -> Self {
        Self {
            max_gain: 6.0,
            smoothen: true,
            treble_f_lower: 6000.0,
            treble_f_upper: 8000.0,
            treble_max_gain: 6.0,
            treble_gain_k: 1.0,
        }
    }
}

impl FrequencyResponse {
    /// Python _smoothen_fractional_octave (1060-1105); p09_smoothen_*.json.
    /// Both Savitzky-Golay passes operate on the full index range, then blend.
    fn smoothen_fractional_octave_data(
        &self,
        data: &[f64],
        p: &SmoothingParams,
    ) -> Result<Vec<f64>, DspError> {
        if self.frequency.len() < 2
            || data.len() != self.frequency.len()
            || self.frequency.iter().chain(data).any(|v| v.is_nan())
        {
            return Err(invalid("NaN values or mismatched arrays, cannot smoothen"));
        }
        let mut normal = data.to_vec();
        let mut treble = data.to_vec();
        let wn = window_size(&self.frequency, p.window_size);
        let wt = window_size(&self.frequency, p.treble_window_size);
        for _ in 0..p.iterations {
            normal = smoothing::savgol_filter(&normal, wn, 2)?;
        }
        for _ in 0..p.treble_iterations {
            treble = smoothing::savgol_filter(&treble, wt, 2)?;
        }
        let weights = sigmoid(
            &self.frequency,
            p.treble_f_lower,
            p.treble_f_upper,
            0.0,
            1.0,
        );
        Ok(normal
            .iter()
            .zip(treble)
            .zip(weights)
            .map(|((n, t), k)| n * (-k + 1.0) + t * k)
            .collect())
    }

    /// Python smoothen_fractional_octave (1107-1164); p09_smoothen_*.json.
    pub fn smoothen_fractional_octave(&mut self, p: &SmoothingParams) -> Result<(), DspError> {
        if p.treble_f_upper <= p.treble_f_lower {
            return Err(invalid(
                "Upper transition boundary must be greater than lower boundary",
            ));
        }
        self.smoothed = self.smoothen_fractional_octave_data(&self.raw, p)?;
        if !self.error.is_empty() {
            self.error_smoothed = self.smoothen_fractional_octave_data(&self.error, p)?;
        }
        self.clear_eq();
        Ok(())
    }

    /// Python smoothen compatibility wrapper (1166-1179); p09_smoothen_*.json.
    pub fn smoothen(
        &mut self,
        window_size: f64,
        treble_window_size: f64,
        treble_f_lower: f64,
        treble_f_upper: f64,
    ) -> Result<(), DspError> {
        self.smoothen_fractional_octave(&SmoothingParams {
            window_size,
            treble_window_size,
            treble_f_lower,
            treble_f_upper,
            ..SmoothingParams::default()
        })
    }

    /// Python smoothen_heavy_light (1181-1239); p09_heavy_light.json.
    /// Signed elementwise maximum (not maximum magnitude) of light/heavy errors.
    pub fn smoothen_heavy_light(&mut self) -> Result<(), DspError> {
        let light = self.smoothen_fractional_octave_data(
            &self.error,
            &SmoothingParams {
                window_size: 1.0 / 6.0,
                ..SmoothingParams::default()
            },
        )?;
        let heavy = self.smoothen_fractional_octave_data(
            &self.error,
            &SmoothingParams {
                treble_window_size: 1.3,
                treble_f_lower: 1000.0,
                treble_f_upper: 6000.0,
                ..SmoothingParams::default()
            },
        )?;
        let combination: Vec<_> = light.iter().zip(heavy).map(|(&l, h)| l.max(h)).collect();
        self.smoothed =
            self.smoothen_fractional_octave_data(&self.raw, &SmoothingParams::default())?;
        self.error_smoothed =
            self.smoothen_fractional_octave_data(&combination, &SmoothingParams::default())?;
        self.clear_eq();
        Ok(())
    }

    /// Python equalize (1241-1310); p09_equalize_*.json.
    /// Does NOT clear equalized_smoothed at entry, and does NOT re-clamp after
    /// the quadratic kink spline. The final two points are rescued from deletion.
    pub fn equalize(&mut self, p: &EqualizeParams) -> Result<(), DspError> {
        self.equalization.clear();
        self.equalized_raw.clear();
        let error = if !self.error_smoothed.is_empty() {
            &self.error_smoothed
        } else {
            &self.error
        };
        let n = self.frequency.len();
        if error.is_empty()
            || error.len() != n
            || self.raw.len() != n
            || (!self.smoothed.is_empty() && self.smoothed.len() != n)
        {
            return Err(invalid("Error data is missing or array lengths differ"));
        }
        if error.iter().any(|v| v.is_nan()) {
            return Err(invalid("NaN values detected during equalization"));
        }
        let ceiling = sigmoid(
            &self.frequency,
            p.treble_f_lower,
            p.treble_f_upper,
            p.max_gain,
            p.treble_max_gain,
        );
        let scale = sigmoid(
            &self.frequency,
            p.treble_f_lower,
            p.treble_f_upper,
            1.0,
            p.treble_gain_k,
        );
        let gain: Vec<_> = error.iter().zip(scale).map(|(e, k)| -e * k).collect();
        let clipped: Vec<_> = gain.iter().zip(&ceiling).map(|(g, m)| g > m).collect();
        self.equalization = gain
            .iter()
            .zip(ceiling)
            .zip(&clipped)
            .map(|((&g, m), &c)| if c { m } else { g })
            .collect();
        if p.smoothen {
            if n < 3 {
                return Err(invalid("quadratic equalization spline needs three points"));
            }
            let half = (window_size(&self.frequency, 1.0 / 12.0) - 1) / 2;
            let mut keep = vec![true; n];
            for i in 1..n {
                if clipped[i] != clipped[i - 1] {
                    keep[i - i.min(half)..i + 1 + (n - i - 1).min(half)].fill(false);
                }
            }
            keep[n - 1] = true;
            keep[n - 2] = true;
            let mut f = Vec::new();
            let mut e = Vec::new();
            for (i, &keep) in keep.iter().enumerate() {
                if keep {
                    f.push(self.frequency[i].log10());
                    e.push(self.equalization[i]);
                }
            }
            let log: Vec<_> = self.frequency.iter().map(|f| f.log10()).collect();
            self.equalization = Spline::new(&f, &e, 2)?.eval(&log);
        }
        self.equalized_raw = self
            .raw
            .iter()
            .zip(&self.equalization)
            .map(|(r, e)| r + e)
            .collect();
        if !self.smoothed.is_empty() {
            self.equalized_smoothed = self
                .smoothed
                .iter()
                .zip(&self.equalization)
                .map(|(s, e)| s + e)
                .collect();
        }
        Ok(())
    }

    /// Python minimum_phase_impulse_response (637-681); p09_minimum_*.json.
    /// Halves f_res, uses integer fs//2 and legacy 235 FFT lengths, doubles dB
    /// before homomorphic half-length conversion. Normalization retains -0.5 dB.
    pub fn minimum_phase_impulse_response(
        &self,
        fs: u32,
        f_res: f64,
        normalize: bool,
    ) -> Result<Vec<f64>, DspError> {
        if fs < 2 || !f_res.is_finite() || f_res <= 0.0 {
            return Err(invalid("positive fs and resolution required"));
        }
        let resolution = f_res / 2.0;
        let mut fr = Self::new(
            "fr_data",
            Some(self.frequency.clone()),
            Some(self.equalization.clone()),
        )?;
        let f_min = fr.frequency[0].max(resolution);
        let gain_min = -fr.center_value_at(f_min)?;
        let size = (fs / 2) as f64 / resolution;
        if !size.is_finite() || size < 2.0 || size >= i64::MAX as f64 {
            return Err(invalid("FIR length is unrepresentable or less than two"));
        }
        let n = fft::next_fast_len_legacy(round_half_even(size) as usize);
        let taps = n
            .checked_mul(2)
            .ok_or_else(|| invalid("FIR length overflow"))?;
        let nyq = (fs / 2) as f64;
        let mut f: Vec<_> = (0..n).map(|i| i as f64 * (nyq / (n - 1) as f64)).collect();
        f[n - 1] = nyq;
        fr.interpolate(Some(&f), 1.01, 1, 20.0, 20000.0)?;
        for (&f, r) in fr.frequency.iter().zip(&mut fr.raw) {
            if f <= f_min {
                *r = gain_min;
            }
        }
        if normalize {
            let max = fr.raw.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            for v in &mut fr.raw {
                *v -= max;
                *v -= 0.5;
            }
        }
        for v in &mut fr.raw {
            *v *= 2.0;
            *v = 10.0_f64.powf(*v / 20.0);
        }
        fr.raw[n - 1] = 0.0;
        let ir = fir::firwin2(taps, &f, &fr.raw, None, fs as f64)?;
        fir::minimum_phase(&ir, ir.len(), true)
    }
}

/// core/impulse_response.py frequency_response (157-188);
/// p09_magnitude_{demo,one,empty}.json. Drops DC before decimation and k=1
/// interpolation. Degenerate inputs return zeros on the 10..fs/2 grid.
pub fn magnitude_to_frequency_response(
    name: &str,
    fs: u32,
    data: &[f64],
) -> Result<FrequencyResponse, DspError> {
    if fs < 20 {
        return Err(invalid("sample rate must support the 10 Hz grid"));
    }
    let nyq = fs as f64 / 2.0;
    let zero = || {
        let f = super::generate_frequencies(10.0, nyq, 1.01);
        let raw = vec![0.0; f.len()];
        FrequencyResponse::new(name, Some(f), Some(raw))
    };
    if data.len() < 2 {
        return zero();
    }
    let magnitude = fft::magnitude_response(data);
    if magnitude.len() < 2 {
        return zero();
    }
    let target_points = nyq / 4.0;
    let step = if target_points < 2.0 {
        1
    } else {
        round_half_even(magnitude.len() as f64 / target_points).max(1) as usize
    };
    let mut frequency = Vec::new();
    let mut raw = Vec::new();
    for i in (1..magnitude.len()).step_by(step) {
        frequency.push(i as f64 * (fs as f64 / data.len() as f64));
        raw.push(magnitude[i]);
    }
    if frequency.is_empty() {
        return zero();
    }
    let mut fr = FrequencyResponse::new(name, Some(frequency), Some(raw))?;
    fr.interpolate(None, 1.01, 1, 10.0, nyq)?;
    Ok(fr)
}
