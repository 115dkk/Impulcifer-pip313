//! FITPACK interpolating B-splines, including endpoint polynomial extrapolation.
use crate::DspError;

/// scipy.interpolate.InterpolatedUnivariateSpline(s=0, ext=0), k=1/2/3.
/// Pinned by p05_spline_k*.json, including irregular and minimal grids.
#[derive(Debug, Clone)]
pub struct Spline {
    knots: Vec<f64>,
    coefficients: Vec<f64>,
    k: usize,
}

impl Spline {
    /// Mirrors scipy.interpolate.InterpolatedUnivariateSpline(x,y,k=k).
    /// https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.interpolate.InterpolatedUnivariateSpline.html
    /// p05_spline_k*.json pins FITPACK knot placement: cubic not-a-knot,
    /// quadratic interior midpoints, and linear interior sample locations.
    /// Requires finite matching samples, strictly increasing x, n >= k+1.
    /// Banded collocation elimination costs O(n*k^2) time and O(n*k) space;
    /// positive totally positive interpolation matrices need no pivoting.
    /// Knot-span search during construction is monotone (O(n)), not n searches.
    pub fn new(x: &[f64], y: &[f64], k: usize) -> Result<Spline, DspError> {
        if !(1..=3).contains(&k)
            || x.len() <= k
            || x.len() != y.len()
            || !x.iter().chain(y).all(|v| v.is_finite())
            || x.windows(2).any(|v| v[0] >= v[1])
        {
            return Err(DspError::InvalidArgument(
                "spline requires k=1/2/3, finite matching samples, strictly increasing x and n > k"
                    .into(),
            ));
        }
        let n = x.len();
        let mut knots = vec![x[0]; k + 1];
        match k {
            1 => knots.extend_from_slice(&x[1..n - 1]),
            2 => knots.extend((1..n - 2).map(|i| x[i] * 0.5 + x[i + 1] * 0.5)),
            3 => knots.extend_from_slice(&x[2..n - 2]),
            _ => unreachable!(),
        }
        knots.extend(std::iter::repeat_n(x[n - 1], k + 1));
        // Each row stores offsets [-k,k] in a fixed maximum-width stack array.
        let mut band = vec![[0.0; 7]; n];
        let mut span = k;
        for (i, &at) in x.iter().enumerate() {
            while span < n - 1 && at >= knots[span + 1] {
                span += 1;
            }
            let basis = basis_values(&knots, k, span, at);
            for (j, &value) in basis[..=k].iter().enumerate() {
                let column = span - k + j;
                if value != 0.0 {
                    band[i][column + k - i] = value;
                }
            }
        }
        let mut coefficients = y.to_vec();
        for i in 0..n {
            let pivot = band[i][k];
            if pivot <= 0.0 || !pivot.is_finite() {
                return Err(DspError::InvalidArgument(
                    "singular spline collocation matrix".into(),
                ));
            }
            for row in i + 1..(i + k + 1).min(n) {
                let factor = band[row][i + k - row] / pivot;
                band[row][i + k - row] = 0.0;
                for column in i + 1..(i + k + 1).min(n) {
                    band[row][column + k - row] -= factor * band[i][column + k - i];
                }
                coefficients[row] -= factor * coefficients[i];
            }
        }
        for i in (0..n).rev() {
            for column in i + 1..(i + k + 1).min(n) {
                coefficients[i] -= band[i][column + k - i] * coefficients[column];
            }
            coefficients[i] /= band[i][k];
        }
        Ok(Spline {
            knots,
            coefficients,
            k,
        })
    }

    /// Mirrors InterpolatedUnivariateSpline.__call__(x, ext=0), pinned by
    /// p05_spline_k*.json. O(m*(log(n)+k^2)); evaluation order is preserved.
    pub fn eval(&self, x: &[f64]) -> Vec<f64> {
        x.iter().map(|&v| self.eval_one(v)).collect()
    }

    /// Mirrors FITPACK splev's scalar ext=0 evaluation; p05_spline_k*.json.
    /// De Boor extrapolates the first/last polynomial (never clamps the value).
    /// NaN queries return NaN; infinite queries use polynomial arithmetic.
    pub fn eval_one(&self, x: f64) -> f64 {
        if x.is_nan() {
            return f64::NAN;
        }
        let k = self.k;
        let n = self.coefficients.len();
        let span = self
            .knots
            .partition_point(|&t| t <= x)
            .saturating_sub(1)
            .clamp(k, n - 1);
        let mut d = [0.0; 4];
        d[..=k].copy_from_slice(&self.coefficients[span - k..=span]);
        for r in 1..=k {
            for j in (r..=k).rev() {
                let left = self.knots[span - k + j];
                let right = self.knots[span + 1 + j - r];
                let a = (x - left) / (right - left);
                d[j] = (1.0 - a) * d[j - 1] + a * d[j];
            }
        }
        d[k]
    }
}

/// Mirrors FITPACK fpbspl's local Cox-de Boor basis construction;
/// p05_spline_k*.json pins the collocation rows through their interpolants.
fn basis_values(t: &[f64], k: usize, span: usize, x: f64) -> [f64; 4] {
    let mut basis = [0.0; 4];
    let mut left = [0.0; 4];
    let mut right = [0.0; 4];
    basis[0] = 1.0;
    for j in 1..=k {
        left[j] = x - t[span + 1 - j];
        right[j] = t[span + j] - x;
        let mut saved = 0.0;
        for r in 0..j {
            let temp = basis[r] / (right[r + 1] + left[j - r]);
            basis[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        basis[j] = saved;
    }
    basis
}

/// Mirrors the recurring AutoEQ log10 spline pattern and get_center_value's
/// ValueError-only construction retry, pinned by p05_log_axis_*.json.
/// https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.interpolate.InterpolatedUnivariateSpline.html
/// The actual core/hrir.py starts at k=1 and retries k=1 on ValueError; there
/// is no automatic cubic-first selection. Honor the supplied k. Insufficient
/// points raise FITPACK's error, not ValueError, and must not retry at k=1.
/// Other constructor failures retry linearly (invalid x/y still fail). Nonfinite
/// data and nonpositive frequencies are rejected explicitly. AutoEQ's separate
/// target-query workaround replaces ONLY an initial zero query by .001 Hz;
/// apply that rule without mutating the caller's array.
pub fn interp_log_axis(
    freq: &[f64],
    values: &[f64],
    k: usize,
    at: &[f64],
) -> Result<Vec<f64>, DspError> {
    if !(1..=3).contains(&k)
        || freq.len() <= k
        || freq.iter().any(|v| !v.is_finite() || *v <= 0.0)
        || at
            .iter()
            .enumerate()
            .any(|(i, v)| !v.is_finite() || (*v <= 0.0 && !(i == 0 && *v == 0.0)))
    {
        return Err(DspError::InvalidArgument(
            "log spline requires positive finite frequencies, k=1/2/3 and n > k".into(),
        ));
    }
    let x: Vec<_> = freq.iter().map(|v| v.log10()).collect();
    let spline = Spline::new(&x, values, k).or_else(|_| Spline::new(&x, values, 1))?;
    Ok(spline.eval(
        &at.iter()
            .enumerate()
            .map(|(i, &v)| {
                if i == 0 && v == 0.0 {
                    0.001_f64.log10()
                } else {
                    v.log10()
                }
            })
            .collect::<Vec<_>>(),
    ))
}
