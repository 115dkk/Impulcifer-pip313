//! nnresample 0.2.4.1 null-on-Nyquist design and SciPy 1.18.0 explicit-tap resampling.
use crate::{DspError, fft, fir};
use std::{
    collections::HashMap,
    f64::consts::PI,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone)]
pub struct ResampleDesign {
    pub up: usize,
    pub down: usize,
    pub taps: Vec<f64>,
    pub cutoff: f64,
    pub beta: f64,
}

/// Mirrors math.gcd reduction in nnresample.compute_filt / scipy.resample_poly;
/// p06_design_*.json pins reduced factors. Zero factors are invalid.
fn ratio(up: usize, down: usize) -> Result<(usize, usize), DspError> {
    if up == 0 || down == 0 {
        return Err(DspError::InvalidArgument(
            "resampling factors must be positive".into(),
        ));
    }
    let (mut a, mut b) = (up, down);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    Ok((up / a, down / a))
}

/// Mirrors nnresample.compute_filt(fc='nn') with disambiguate_params defaults
/// in installed 0.2.4.1 / SciPy 1.18.0; p06_design_*.json pins every tap,
/// beta=5.65326 (60 dB, NOT compute_filt's direct default 5), cutoff and null bin.
/// N=32001, F=2^19, H=F/2+1; the null frequency denominator is H, not F/2.
/// Same-rate design returns the installed firwin cutoff=1 error (no 0.2.5 guard).
/// No df-to-N branch is implemented or corrected. Uncached cost O(F log F + N).
pub fn nnresample_design(up: usize, down: usize) -> Result<ResampleDesign, DspError> {
    let (up, down) = ratio(up, down)?;
    let q = up.max(down) as f64;
    let beta = 0.1102 * (60.0 - 8.7);
    let n = 32001;
    let mut initial = fir::firwin_lowpass(n, 1.0 / q, beta)?;
    initial.resize(1 << 19, 0.0);
    let spectrum = fft::rfft(&initial);
    let h = spectrum.len() as f64;
    let c = (1.0 + (beta / PI).powi(2)).sqrt();
    let bot = (h / q).floor() as usize;
    let top = (h * (1.0 / q + 2.0 * c / n as f64)).ceil() as usize;
    let range = spectrum
        .get(bot..top.min(spectrum.len()))
        .ok_or_else(|| DspError::InvalidArgument("null search interval is invalid".into()))?;
    let index = range
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.norm().total_cmp(&b.norm()))
        .ok_or_else(|| DspError::InvalidArgument("null search interval is empty".into()))?
        .0;
    let cutoff = 2.0 / q - (bot + index) as f64 / h;
    let taps = fir::firwin_lowpass(n, cutoff, beta)?;
    Ok(ResampleDesign {
        up,
        down,
        taps,
        cutoff,
        beta,
    })
}

/// Mirrors scipy.signal.resample_poly's _output_len/padding arithmetic;
/// p06_poly_*.json and resample_poly_padding_arithmetic_48k_to_44k1 pin it.
/// Minimal post padding is solved algebraically, equivalent to SciPy's loop.
fn padding(
    n: usize,
    taps: usize,
    up: usize,
    down: usize,
) -> Result<(usize, usize, usize, usize), DspError> {
    let invalid = || DspError::InvalidArgument("resampling length overflow".into());
    let n_out = n.checked_mul(up).ok_or_else(invalid)?.div_ceil(down);
    let half = (taps - 1) / 2;
    let pre = down - half % down;
    let remove = half.checked_add(pre).ok_or_else(invalid)? / down;
    let required = n_out
        .checked_add(remove)
        .and_then(|v| v.checked_sub(1))
        .and_then(|v| v.checked_mul(down))
        .and_then(|v| v.checked_add(1))
        .ok_or_else(invalid)?;
    let available = (n - 1)
        .checked_mul(up)
        .and_then(|v| v.checked_add(taps))
        .and_then(|v| v.checked_add(pre))
        .ok_or_else(invalid)?;
    Ok((n_out, pre, remove, required.saturating_sub(available)))
}

/// Mirrors scipy.signal.resample_poly(x,up,down,window=taps,padtype='constant')
/// in SciPy 1.18.0; full outputs p06_poly_*.json isolate explicit float64 taps.
/// https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.resample_poly.html
/// GCD reduction precedes a same-rate copy, even with empty/nonfinite taps.
/// Otherwise taps must be nonempty. Parity is pinned for finite f64 data;
/// nonfinite values propagate through evaluated products, but implicit zero
/// padding does not promise SciPy's NaN masks for zero*infinity products.
/// Uses zero extension and computes only the retained polyphase outputs, never
/// the zero-inserted signal. Work O(n_out*ceil(taps.len()/up)), storage
/// O(taps.len()+n_out). Zero pre/post taps are implicit; pre_pad is a FULL down
/// when half_len is divisible by down. Input and caller-owned taps are unchanged.
pub fn resample_poly(
    x: &[f64],
    up: usize,
    down: usize,
    taps: &[f64],
) -> Result<Vec<f64>, DspError> {
    let (up, down) = ratio(up, down)?;
    if up == 1 && down == 1 {
        return Ok(x.to_vec());
    }
    if taps.is_empty() {
        return Err(DspError::InvalidArgument(
            "resampling taps must be nonempty".into(),
        ));
    }
    if x.is_empty() {
        return Ok(Vec::new());
    }
    let (n_out, pre, remove, _post) = padding(x.len(), taps.len(), up, down)?;
    // Reverse each phase once so both operands are contiguous in input order.
    let phases: Vec<Vec<f64>> = (0..up.min(taps.len()))
        .map(|phase| {
            taps[phase..]
                .iter()
                .step_by(up)
                .map(|v| v * up as f64)
                .rev()
                .collect()
        })
        .collect();
    let mut y = vec![0.0; n_out];
    for (i, output) in y.iter_mut().enumerate() {
        // padding() already checked the last retained high-rate coordinate.
        let t = (i + remove) * down - pre;
        let first = t.saturating_sub(taps.len() - 1).div_ceil(up);
        let last = (t / up).min(x.len() - 1);
        if first <= last {
            // Ascending input order still matches SciPy; independent accumulators
            // allow vectorization of long dot products without fused operations.
            let phase = &phases[t % up];
            let start = phase.len() - 1 - (t - first * up) / up;
            let coefficients = &phase[start..start + last - first + 1];
            let samples = &x[first..=last];
            let mut sums = [0.0; 4];
            let mut a = samples.chunks_exact(4);
            let mut b = coefficients.chunks_exact(4);
            for (a, b) in a.by_ref().zip(b.by_ref()) {
                for k in 0..4 {
                    sums[k] += a[k] * b[k];
                }
            }
            *output = (sums[0] + sums[1]) + (sums[2] + sums[3]);
            for (a, b) in a.remainder().iter().zip(b.remainder()) {
                *output += a * b;
            }
        }
    }
    Ok(y)
}

type DesignCache = HashMap<(usize, usize), ResampleDesign>;
static DESIGNS: OnceLock<Mutex<DesignCache>> = OnceLock::new();

/// Mirrors nnresample.resample(s,new_fs,old_fs) 0.2.4.1, as called by
/// core/impulse_response.py:121-124; p06_poly_* stores nnresample outputs too.
/// The reduced-ratio process cache holds the lock during design so concurrent
/// first calls perform one design/FFT per successful ratio. It only reuses exact
/// f64 taps, never changes values; filtering happens after releasing the lock.
/// Equal rates retain the installed design error, unlike scipy.resample_poly.
pub fn nnresample(x: &[f64], new_fs: u32, old_fs: u32) -> Result<Vec<f64>, DspError> {
    let key = ratio(new_fs as usize, old_fs as usize)?;
    let taps = {
        let mut cache = DESIGNS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|_| DspError::InvalidArgument("resampling cache lock poisoned".into()))?;
        if let std::collections::hash_map::Entry::Vacant(entry) = cache.entry(key) {
            entry.insert(nnresample_design(key.0, key.1)?);
        }
        cache[&key].taps.clone()
    };
    resample_poly(x, key.0, key.1, &taps)
}

#[cfg(test)]
mod tests {
    /// Mirrors scipy.resample_poly padding; p06_poly_impulse_44100.json.
    #[test]
    fn resample_poly_padding_arithmetic_48k_to_44k1() {
        assert_eq!(super::ratio(44100, 48000).unwrap(), (147, 160));
        assert_eq!((32001 - 1) / 2, 16000);
        assert_eq!(
            super::padding(4800, 32001, 147, 160).unwrap(),
            (4410, 160, 101, 0)
        );
        // Very short custom FIR needs actual post-padding.
        assert_eq!(super::padding(1, 1, 5, 2).unwrap(), (3, 2, 1, 4));
    }
}
