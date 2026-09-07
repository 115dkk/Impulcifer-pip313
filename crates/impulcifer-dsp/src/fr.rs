#![forbid(unsafe_code)]
//! The application's vendored AutoEQ 1.2.5 FrequencyResponse subset.
//! Python line references refer to autoeq/frequency_response.py unless stated.

use crate::{DspError, filters, interp::Spline, stats};

mod csv;
mod processing;

/// Python __init__/_init_data (42-117); p09_interpolate_*.json.
#[derive(Clone, Debug, PartialEq)]
pub struct FrequencyResponse {
    pub name: String,
    pub frequency: Vec<f64>,
    pub raw: Vec<f64>,
    pub smoothed: Vec<f64>,
    pub error: Vec<f64>,
    pub error_smoothed: Vec<f64>,
    pub equalization: Vec<f64>,
    pub equalized_raw: Vec<f64>,
    pub equalized_smoothed: Vec<f64>,
    pub target: Vec<f64>,
}

/// Optional arrays from Python __init__ (42-73); p09_csv_*.json.
#[derive(Clone, Debug, Default)]
pub struct FrFields {
    pub raw: Option<Vec<f64>>,
    pub smoothed: Option<Vec<f64>>,
    pub error: Option<Vec<f64>>,
    pub error_smoothed: Option<Vec<f64>>,
    pub equalization: Option<Vec<f64>>,
    pub equalized_raw: Option<Vec<f64>>,
    pub equalized_smoothed: Option<Vec<f64>>,
    pub target: Option<Vec<f64>>,
}

/// Python reset (148-179); p09_{interpolate,center,compensate,smoothen}_*.json.
/// The omitted parametric/fixed-band arrays have no-op flags.
#[derive(Clone, Copy, Debug)]
pub struct ResetFlags {
    pub raw: bool,
    pub smoothed: bool,
    pub error: bool,
    pub error_smoothed: bool,
    pub equalization: bool,
    pub parametric_eq: bool,
    pub fixed_band_eq: bool,
    pub equalized_raw: bool,
    pub equalized_smoothed: bool,
    pub target: bool,
}
impl Default for ResetFlags {
    /// Python reset defaults (148-158); p09_center_*.json.
    fn default() -> Self {
        Self {
            raw: false,
            smoothed: true,
            error: true,
            error_smoothed: true,
            equalization: true,
            parametric_eq: true,
            fixed_band_eq: true,
            equalized_raw: true,
            equalized_smoothed: true,
            target: true,
        }
    }
}

/// Python center scalar/band branches (903-940); p09_center_*.json.
#[derive(Clone, Copy, Debug)]
pub enum CenterAt {
    Frequency(f64),
    Band(f64, f64),
}

/// Python compensate arguments (984-991); p09_compensate_*.json.
#[derive(Clone, Debug)]
pub struct CompensateOptions {
    pub bass_boost_gain: f64,
    pub bass_boost_fc: f64,
    pub bass_boost_q: f64,
    pub tilt: Option<f64>,
    pub min_mean_error: bool,
}
impl Default for CompensateOptions {
    /// Python compensate defaults (984-991); p09_compensate_zero.json.
    fn default() -> Self {
        Self {
            bass_boost_gain: 0.0,
            bass_boost_fc: 105.0,
            bass_boost_q: 0.71,
            tilt: None,
            min_mean_error: false,
        }
    }
}

/// Python generate_frequencies (850-857); p09_generate_*.json.
/// Repeated multiplication, not a closed-form geometric progression.
/// Panics for nonfinite bounds, nonpositive minimum or ratio <= 1.
pub fn generate_frequencies(f_min: f64, f_max: f64, f_step: f64) -> Vec<f64> {
    assert!(
        f_min.is_finite() && f_min > 0.0 && f_max.is_finite() && f_step.is_finite() && f_step > 1.0
    );
    let mut out = Vec::new();
    let mut f = f_min;
    while f <= f_max {
        out.push(f);
        f *= f_step;
    }
    out
}

/// Python round in _window_size (1044-1049); p09_window.json and
/// round_half_even_matches_python. Panics on nonfinite/out-of-i64 input.
pub fn round_half_even(x: f64) -> i64 {
    let rounded = x.round_ties_even();
    assert!(rounded.is_finite() && rounded >= i64::MIN as f64 && rounded < -(i64::MIN as f64));
    rounded as i64
}

/// Python _window_size (1033-1050); p09_window.json.
/// Unlike smoothing::fractional_octave_window, derives the ratio from the grid
/// with a naive left-to-right sum. Both agree on the app's 1.01 grids.
/// Panics for fewer than two points, invalid ratios or negative octaves.
pub fn window_size(frequency: &[f64], octaves: f64) -> usize {
    assert!(frequency.len() >= 2 && octaves.is_finite() && octaves >= 0.0);
    let mut sum = 0.0;
    for pair in frequency.windows(2) {
        sum += pair[1] / pair[0];
    }
    let ratio = sum / (frequency.len() - 1) as f64;
    assert!(ratio.is_finite() && ratio > 1.0);
    let n = round_half_even(2.0_f64.powf(octaves).ln() / ratio.ln()) as usize;
    if n.is_multiple_of(2) { n + 1 } else { n }
}

/// Python _sigmoid (1052-1058); p09_sigmoid_*.json. Uses stats::expit.
pub fn sigmoid(
    frequency: &[f64],
    f_lower: f64,
    f_upper: f64,
    a_normal: f64,
    a_treble: f64,
) -> Vec<f64> {
    let center = (f_upper / f_lower).sqrt() * f_lower;
    let half_range = f_upper.log10() - center.log10();
    frequency
        .iter()
        .map(|f| {
            let a = stats::expit((f.log10() - center.log10()) / (half_range / 4.0));
            a * -(a_normal - a_treble) + a_normal
        })
        .collect()
}

/// Python _tilt (942-955); p09_target_*.json. Fixed 20..20000 Hz center.
pub fn tilt(frequency: &[f64], tilt: f64) -> Vec<f64> {
    let c = 20.0 * 1000.0_f64.sqrt();
    frequency.iter().map(|f| (f / c).log2() * tilt).collect()
}

/// Python create_target (957-982), biquad.py low_shelf/digital_coeffs
/// (52-79,112-131); p09_target_*.json. The design AND evaluation rate is always
/// 44100 Hz even for a 48000 Hz project: this preserves the 2.x quirk.
/// Inherits the existing RBJ primitive's argument assertions.
pub fn create_target(
    frequency: &[f64],
    bass_boost_gain: f64,
    bass_boost_fc: f64,
    bass_boost_q: f64,
    slope: Option<f64>,
) -> Vec<f64> {
    let shelf = filters::rbj_low_shelf(bass_boost_fc, bass_boost_q, bass_boost_gain, 44100.0);
    let mut target = filters::biquad_response_db(&shelf, frequency, 44100.0);
    if let Some(slope) = slope {
        for (v, t) in target.iter_mut().zip(tilt(frequency, slope)) {
            *v += t;
        }
    }
    target
}

/// Python array-shape failures in __init__/interpolate (119-146,872-877);
/// p09_interpolate_nan.json and construction property cases.
fn invalid(message: &str) -> DspError {
    DspError::InvalidArgument(message.into())
}

/// Python np.mean band masks in center/compensate (918-921,1015);
/// p09_center_*.json. Empty bands preserve NumPy's NaN result.
fn band_mean(frequency: &[f64], data: &[f64], band: (f64, f64)) -> f64 {
    let mut n = 0;
    let mut sum = 0.0;
    for (&f, &v) in frequency.iter().zip(data) {
        if f >= band.0 && f <= band.1 {
            sum += v;
            n += 1;
        }
    }
    sum / n as f64
}

impl FrequencyResponse {
    /// Python __init__ (42-73); p09_generate_*.json, p09_csv_*.json.
    pub fn new(
        name: &str,
        frequency: Option<Vec<f64>>,
        raw: Option<Vec<f64>>,
    ) -> Result<Self, DspError> {
        Self::with_fields(
            name,
            frequency,
            FrFields {
                raw,
                ..FrFields::default()
            },
        )
    }

    /// Python __init__/_init_data (42-117); p09_csv_*.json. Empty frequency
    /// arrays, like None, select the default grid. Missing data remains empty.
    pub fn with_fields(
        name: &str,
        frequency: Option<Vec<f64>>,
        fields: FrFields,
    ) -> Result<Self, DspError> {
        if name.trim().is_empty() {
            return Err(invalid("name must be non-empty after trimming"));
        }
        let frequency = frequency
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| generate_frequencies(20.0, 20000.0, 1.01));
        let mut fr = Self {
            name: name.trim().into(),
            frequency,
            raw: fields.raw.unwrap_or_default(),
            smoothed: fields.smoothed.unwrap_or_default(),
            error: fields.error.unwrap_or_default(),
            error_smoothed: fields.error_smoothed.unwrap_or_default(),
            equalization: fields.equalization.unwrap_or_default(),
            equalized_raw: fields.equalized_raw.unwrap_or_default(),
            equalized_smoothed: fields.equalized_smoothed.unwrap_or_default(),
            target: fields.target.unwrap_or_default(),
        };
        fr.sort()?;
        Ok(fr)
    }

    /// Python _init_data scalar branch (96-98); p09_compensate_zero.json.
    pub fn constant(
        name: &str,
        frequency: Option<Vec<f64>>,
        raw_value: f64,
        error_value: f64,
    ) -> Result<Self, DspError> {
        let mut fr = Self::new(name, frequency, None)?;
        fr.raw = vec![raw_value; fr.frequency.len()];
        fr.error = vec![error_value; fr.frequency.len()];
        Ok(fr)
    }

    /// Python _sort (119-146); p09_csv_strict.json and construction properties.
    fn sort(&mut self) -> Result<(), DspError> {
        let n = self.frequency.len();
        let mut order: Vec<_> = (0..n).collect();
        order.sort_by(|&a, &b| self.frequency[a].total_cmp(&self.frequency[b]));
        self.frequency = order.iter().map(|&i| self.frequency[i]).collect();
        if let Some(pair) = self.frequency.windows(2).find(|p| p[0] == p[1]) {
            return Err(DspError::InvalidArgument(format!(
                "Duplicate values found at frequency {}. Remove duplicates manually.",
                pair[1]
            )));
        }
        for values in [
            &mut self.raw,
            &mut self.smoothed,
            &mut self.error,
            &mut self.error_smoothed,
            &mut self.equalization,
            &mut self.equalized_raw,
            &mut self.equalized_smoothed,
            &mut self.target,
        ] {
            if !values.is_empty() {
                if values.len() != n {
                    return Err(invalid("array length differs from frequency"));
                }
                *values = order.iter().map(|&i| values[i]).collect();
            }
        }
        Ok(())
    }

    /// Python reset (148-179); p09_{interpolate,center,compensate,smoothen}_*.json.
    /// interpolate clears smoothed; center clears the three EQ result arrays;
    /// compensate also clears error_smoothed; fractional/heavy-light smoothing
    /// clear the three EQ result arrays. All five also clear the omitted
    /// parametric_eq/fixed_band_eq, whose flags here are no-ops.
    pub fn reset(&mut self, flags: ResetFlags) {
        for (clear, values) in [
            (flags.raw, &mut self.raw),
            (flags.smoothed, &mut self.smoothed),
            (flags.error, &mut self.error),
            (flags.error_smoothed, &mut self.error_smoothed),
            (flags.equalization, &mut self.equalization),
            (flags.equalized_raw, &mut self.equalized_raw),
            (flags.equalized_smoothed, &mut self.equalized_smoothed),
            (flags.target, &mut self.target),
        ] {
            if clear {
                values.clear();
            }
        }
    }

    /// Python interpolate (859-901); p09_interpolate_{log,linear,nan}.json.
    /// Only raw/frequency lose NaN rows. Other populated fields then fail the
    /// spline's shape check, as in Python. Smoothed is deliberately discarded.
    pub fn interpolate(
        &mut self,
        f: Option<&[f64]>,
        f_step: f64,
        pol_order: usize,
        f_min: f64,
        f_max: f64,
    ) -> Result<(), DspError> {
        if !self.raw.is_empty() {
            if self.raw.len() != self.frequency.len() {
                return Err(invalid("raw length differs from frequency"));
            }
            let keep: Vec<_> = self
                .raw
                .iter()
                .enumerate()
                .filter_map(|(i, x)| (!x.is_nan()).then_some(i))
                .collect();
            self.raw = keep.iter().map(|&i| self.raw[i]).collect();
            self.frequency = keep.iter().map(|&i| self.frequency[i]).collect();
        }
        let log: Vec<_> = self.frequency.iter().map(|f| f.log10()).collect();
        let grid = f
            .map(<[f64]>::to_vec)
            .unwrap_or_else(|| generate_frequencies(f_min, f_max, f_step));
        if grid.is_empty() {
            return Err(invalid("interpolation grid must be non-empty"));
        }
        let queries: Vec<_> = grid
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                if i == 0 && f == 0.0 {
                    0.001_f64.log10()
                } else {
                    f.log10()
                }
            })
            .collect();
        for values in [
            &mut self.raw,
            &mut self.error,
            &mut self.error_smoothed,
            &mut self.equalization,
            &mut self.equalized_raw,
            &mut self.equalized_smoothed,
            &mut self.target,
        ] {
            if !values.is_empty() {
                *values = Spline::new(&log, values, pol_order)?.eval(&queries);
            }
        }
        self.frequency = grid;
        self.reset(ResetFlags {
            error: false,
            error_smoothed: false,
            equalization: false,
            equalized_raw: false,
            equalized_smoothed: false,
            target: false,
            ..ResetFlags::default()
        });
        Ok(())
    }

    /// Python interpolate defaults (859); p09_center_*.json.
    pub fn interpolate_default(&mut self) -> Result<(), DspError> {
        self.interpolate(None, 1.01, 1, 20.0, 20000.0)
    }

    /// Python center (903-940); p09_center_{scalar,band}.json. Re-grids the
    /// equal-energy copy; errors shift oppositely to raw and target is retained.
    pub fn center(&mut self, at: CenterAt) -> Result<f64, DspError> {
        let mut copy = Self::new(
            "equal_energy",
            Some(self.frequency.clone()),
            Some(self.raw.clone()),
        )?;
        copy.interpolate_default()?;
        let shift = match at {
            CenterAt::Frequency(f) => copy.center_value_at(f)?,
            CenterAt::Band(lo, hi) => copy.center_value((lo, hi)),
        };
        for v in self.raw.iter_mut().chain(&mut self.smoothed) {
            *v += shift;
        }
        for v in self.error.iter_mut().chain(&mut self.error_smoothed) {
            *v -= shift;
        }
        self.clear_eq();
        Ok(shift)
    }

    /// core/hrir.py get_center_value (44-74); p09_center_values.json.
    /// Unlike center, this mean uses the object's own grid without interpolation.
    pub fn center_value(&self, band: (f64, f64)) -> f64 {
        -band_mean(&self.frequency, &self.raw, band)
    }

    /// core/hrir.py get_center_value scalar branch (57-74);
    /// p09_center_values.json. Linear log spline; returns the negative value.
    pub fn center_value_at(&self, frequency: f64) -> Result<f64, DspError> {
        let log: Vec<_> = self.frequency.iter().map(|f| f.log10()).collect();
        Ok(-Spline::new(&log, &self.raw, 1)?.eval_one(frequency.log10()))
    }

    /// Python compensate (984-1031); p09_compensate_{zero,harman}.json.
    /// Same-length grids are used positionally, with no automatic re-gridding.
    pub fn compensate(
        &mut self,
        compensation: &FrequencyResponse,
        options: &CompensateOptions,
    ) -> Result<(), DspError> {
        if compensation.frequency.len() != self.frequency.len()
            || compensation.raw.len() != self.raw.len()
            || self.raw.len() != self.frequency.len()
        {
            return Err(invalid("compensation grid lengths differ"));
        }
        let mut compensation = Self::new(
            "compensation",
            Some(compensation.frequency.clone()),
            Some(compensation.raw.clone()),
        )?;
        compensation.center(CenterAt::Frequency(1000.0))?;
        self.target = create_target(
            &self.frequency,
            options.bass_boost_gain,
            options.bass_boost_fc,
            options.bass_boost_q,
            options.tilt,
        );
        for (v, c) in self.target.iter_mut().zip(compensation.raw) {
            *v += c;
        }
        self.error = self
            .raw
            .iter()
            .zip(&self.target)
            .map(|(r, t)| r - t)
            .collect();
        if options.min_mean_error {
            let delta = band_mean(&self.frequency, &self.error, (100.0, 10000.0));
            for v in &mut self.error {
                *v -= delta;
            }
            for v in &mut self.target {
                *v += delta;
            }
        }
        self.clear_eq();
        self.error_smoothed.clear();
        Ok(())
    }

    /// Python reset clear set in center/smoothen (938,1153-1164,1228-1239);
    /// p09_center_*.json, p09_heavy_light.json.
    fn clear_eq(&mut self) {
        self.reset(ResetFlags {
            smoothed: false,
            error: false,
            error_smoothed: false,
            target: false,
            ..ResetFlags::default()
        });
    }
}

pub use processing::{EqualizeParams, SmoothingParams, magnitude_to_frequency_response};
