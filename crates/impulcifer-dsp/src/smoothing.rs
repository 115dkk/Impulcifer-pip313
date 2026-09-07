//! Savitzky-Golay least-squares smoothing with polynomial (interp) edges.
use crate::DspError;

/// Mirrors scipy.signal.savgol_filter(deriv=0, delta=1, mode="interp").
/// Pinned by p05_savgol_{noise,quadratic,constant,linear}_*.json.
/// https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.signal.savgol_filter.html
/// Requires an odd positive window <= x.len(), polyorder < window, and finite
/// input (AutoEQ rejects NaNs before smoothing). Uses a scaled Vandermonde QR
/// projection, not normal equations; endpoint outputs fit the whole end window.
/// Setup O(window*polyorder^2), filtering O(len*window), memory O(window*polyorder).
pub fn savgol_filter(
    x: &[f64],
    window_length: usize,
    polyorder: usize,
) -> Result<Vec<f64>, DspError> {
    if window_length == 0
        || window_length.is_multiple_of(2)
        || window_length > x.len()
        || polyorder >= window_length
        || !x.iter().all(|v| v.is_finite())
    {
        return Err(DspError::InvalidArgument(
            "savgol requires finite input, an odd window <= len and polyorder < window".into(),
        ));
    }
    let w = window_length;
    let mid = w / 2;
    let basis = polynomial_basis(w, polyorder)?;
    let weights = |at: usize| -> Vec<f64> {
        (0..w)
            .map(|j| basis.iter().map(|q| q[at] * q[j]).sum())
            .collect()
    };
    let center = weights(mid);
    let mut y = vec![0.0; x.len()];
    for i in mid..x.len() - mid {
        y[i] = center
            .iter()
            .zip(&x[i - mid..i + mid + 1])
            .map(|(a, b)| a * b)
            .sum();
    }
    for i in 0..mid {
        y[i] = weights(i).iter().zip(&x[..w]).map(|(a, b)| a * b).sum();
        y[x.len() - mid + i] = weights(w - mid + i)
            .iter()
            .zip(&x[x.len() - w..])
            .map(|(a, b)| a * b)
            .sum();
    }
    Ok(y)
}

/// QR basis for scipy.signal.savgol_filter's polynomial least-squares design;
/// p05_savgol_*.json pins its center/edge projections. Two-pass modified
/// Gram-Schmidt avoids loss of orthogonality without adding a matrix dependency.
fn polynomial_basis(w: usize, order: usize) -> Result<Vec<Vec<f64>>, DspError> {
    let mid = (w / 2) as f64;
    let grid: Vec<_> = (0..w).map(|i| (i as f64 - mid) / mid.max(1.0)).collect();
    let mut basis: Vec<Vec<f64>> = Vec::with_capacity(order + 1);
    let mut power = vec![1.0; w];
    for _ in 0..=order {
        let mut q = power.clone();
        for _ in 0..2 {
            for previous in &basis {
                let dot: f64 = q.iter().zip(previous).map(|(a, b)| a * b).sum();
                for (a, b) in q.iter_mut().zip(previous) {
                    *a -= dot * b;
                }
            }
        }
        let norm = q.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm == 0.0 || !norm.is_finite() {
            return Err(DspError::InvalidArgument(
                "singular polynomial design".into(),
            ));
        }
        q.iter_mut().for_each(|v| *v /= norm);
        basis.push(q);
        for (p, t) in power.iter_mut().zip(&grid) {
            *p *= t;
        }
    }
    Ok(basis)
}

/// Mirrors AutoEQ FrequencyResponse._window_size, with its mean frequency ratio
/// supplied by the caller. Pinned by p05_fractional_window.json. Python round is
/// ties-to-even; do not replace log(2**octaves) by octaves*log(2).
/// Panics for nonfinite/negative octaves, ratio <= 1, or unrepresentable size.
pub fn fractional_octave_window(octaves: f64, ratio: f64) -> usize {
    assert!(
        octaves.is_finite() && octaves >= 0.0 && ratio.is_finite() && ratio > 1.0,
        "fractional octave window requires octaves >= 0 and ratio > 1"
    );
    let size = (2.0_f64.powf(octaves).ln() / ratio.ln()).round_ties_even();
    assert!(
        size.is_finite() && size >= 0.0 && size < usize::MAX as f64,
        "fractional octave window overflow"
    );
    let n = size as usize;
    if n.is_multiple_of(2) {
        n.checked_add(1).expect("window overflow")
    } else {
        n
    }
}
