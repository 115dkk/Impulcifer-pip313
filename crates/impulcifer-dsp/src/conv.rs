//! SciPy one-dimensional real convolution and correlation.
use crate::fft::{irfft, next_fast_len, rfft_owned};

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Full,
    Same,
    Valid,
}

/// SciPy (1.16 reference, verified 1.18): Same is centered on the first
/// argument even if it is shorter. Panics for empty input or length overflow.
/// Nonfinite inputs use direct arithmetic to avoid FFT-wide contamination.
pub fn convolve(a: &[f64], v: &[f64], mode: Mode) -> Vec<f64> {
    assert!(
        !a.is_empty() && !v.is_empty(),
        "convolution inputs must be nonempty"
    );
    let len = a
        .len()
        .checked_add(v.len() - 1)
        .expect("convolution length overflow");
    let full =
        if a.len().saturating_mul(v.len()) <= 16384 || !a.iter().chain(v).all(|x| x.is_finite()) {
            let mut result = vec![0.0; len];
            for (i, x) in a.iter().enumerate() {
                for (j, y) in v.iter().enumerate() {
                    result[i + j] += x * y;
                }
            }
            result
        } else {
            let n = next_fast_len(len);
            let mut ap = vec![0.0; n];
            let mut vp = vec![0.0; n];
            ap[..a.len()].copy_from_slice(a);
            vp[..v.len()].copy_from_slice(v);
            let mut product = rfft_owned(ap);
            for (x, y) in product.iter_mut().zip(rfft_owned(vp)) {
                *x *= y;
            }
            let mut result = irfft(&product, n);
            result.truncate(len);
            result
        };
    let (start, count) = match mode {
        Mode::Full => (0, len),
        Mode::Same => ((v.len() - 1) / 2, a.len()),
        Mode::Valid => (a.len().min(v.len()) - 1, a.len().abs_diff(v.len()) + 1),
    };
    if start == 0 && count == full.len() {
        full
    } else {
        full[start..start + count].to_vec()
    }
}

/// Real scipy.signal.correlate. Same crop/panic policies as `convolve`.
pub fn correlate(a: &[f64], v: &[f64], mode: Mode) -> Vec<f64> {
    convolve(a, &v.iter().rev().copied().collect::<Vec<_>>(), mode)
}

/// scipy.signal.correlation_lags, verified against SciPy 1.18 source.
/// For odd in1 and even in2, Same lags start one later than correlate's crop;
/// preserve SciPy's midpoint slicing rather than correcting that discrepancy.
/// Panics for zero lengths or lengths/ranges exceeding i64.
pub fn correlation_lags(in1_len: usize, in2_len: usize, mode: Mode) -> Vec<i64> {
    assert!(in1_len > 0 && in2_len > 0, "lag lengths must be positive");
    let a = i64::try_from(in1_len).expect("lag length overflow");
    let b = i64::try_from(in2_len).expect("lag length overflow");
    let full = a.checked_add(b - 1).expect("lag range overflow");
    let (start, count) = match mode {
        Mode::Full => (1 - b, full),
        Mode::Same => (1 - b + full / 2 - a / 2, a),
        Mode::Valid => ((a - b).min(0), (a - b).abs() + 1),
    };
    (start..start + count).collect()
}
