//! SciPy symmetric/periodic windows (N=0 empty, N=1 unity).
use crate::DspError;
use std::f64::consts::PI;

fn cosine(n: usize, sym: bool, a: f64) -> Vec<f64> {
    if n <= 1 {
        return vec![1.0; n];
    }
    let denominator = if sym { n - 1 } else { n } as f64;
    // SciPy general_cosine uses a linspace from -pi to pi.
    (0..n)
        .map(|i| a + (1.0 - a) * (-PI + i as f64 * (2.0 * PI / denominator)).cos())
        .collect()
}

pub fn hann(n: usize, sym: bool) -> Vec<f64> {
    cosine(n, sym, 0.5)
}
pub fn hamming(n: usize, sym: bool) -> Vec<f64> {
    cosine(n, sym, 0.54)
}

/// I0(x) = sum(k=0..infinity) (x²/4)^k / (k!)². Positive-term series
/// terminates when a term no longer changes the sum. Overflow returns infinity
/// (like SciPy's unscaled I0), so very large Kaiser beta can yield NaNs.
fn i0(x: f64) -> f64 {
    // NumPy/SciPy evaluate exp(|x|) before the scaled approximation: preserve
    // that intermediate overflow even when the mathematical I0 still fits.
    if !x.abs().exp().is_finite() {
        return f64::INFINITY;
    }
    let mut sum = 1.0;
    let mut term = 1.0;
    for k in 1.. {
        term *= (x / (2.0 * k as f64)).powi(2);
        let next = sum + term;
        if next == sum || !next.is_finite() {
            return next;
        }
        sum = next;
    }
    unreachable!()
}

/// SciPy Kaiser window. Negative beta is equivalent to positive beta.
/// https://docs.scipy.org/doc/scipy-1.16.0/reference/generated/scipy.signal.windows.kaiser.html
/// Panics on nonfinite beta. Like SciPy's unscaled I0 ratio, very large finite
/// beta can overflow and produce NaNs; no substitute normalized window is used.
pub fn kaiser(n: usize, beta: f64, sym: bool) -> Vec<f64> {
    assert!(beta.is_finite(), "Kaiser beta must be finite");
    if n <= 1 {
        return vec![1.0; n];
    }
    let alpha = (if sym { n - 1 } else { n }) as f64 / 2.0;
    let denominator = i0(beta);
    let mut output = vec![0.0; n];
    // Squared distances from alpha are identical on the mirrored half.
    for i in 0..=n / 2 {
        let arg = beta * (1.0 - ((i as f64 - alpha) / alpha).powi(2)).max(0.0).sqrt();
        let value = i0(arg) / denominator;
        output[i] = value;
        let mirror = if sym { n - 1 - i } else { n - i };
        if mirror < n {
            output[mirror] = value;
        }
    }
    output
}

/// Named subset of scipy.signal.get_window: fftbins=true means periodic.
/// Unknown names or N=0 return InvalidArgument. SciPy 1.18 get_window rejects
/// N=0 even though the individual window functions accept it (1.16 differed).
pub fn get_window(name: &str, n: usize, fftbins: bool) -> Result<Vec<f64>, DspError> {
    if n == 0 {
        return Err(DspError::InvalidArgument(
            "window length must be positive".into(),
        ));
    }
    match name {
        "hann" => Ok(hann(n, !fftbins)),
        "hamming" => Ok(hamming(n, !fftbins)),
        "boxcar" => Ok(vec![1.0; n]),
        _ => Err(DspError::InvalidArgument(format!(
            "unsupported window: {name}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bessel_i0_matches_scipy_reference() {
        // scipy.special.i0, SciPy 1.18.0, float64 oracle.
        for (x, expected) in [
            (0.0, 1.0),
            (1.0, 1.2660658777520082),
            (2.0, 2.279585302336067),
            (5.65326, 49.04845930258202),
            (10.0, 2815.716628466254),
            (20.0, 43558282.559553534),
            (-5.65326, 49.04845930258202),
        ] {
            let actual = super::i0(x);
            let tolerance = 1e-14 * expected;
            assert!(
                (actual - expected).abs() <= tolerance,
                "I0({x}): got {actual}, expected {expected}, tolerance {tolerance}"
            );
        }
    }
}
