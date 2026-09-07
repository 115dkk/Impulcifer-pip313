//! SciPy Type I/II FIR design and the homomorphic minimum-phase transform.
use crate::{
    DspError,
    fft::{self, Complex64},
    windows,
};
use std::f64::consts::PI;

/// Mirrors scipy.signal.firwin2, default symmetric Hamming, antisymmetric=false.
/// Pinned by p05_firwin2_{smooth,notch,flat,duplicate,odd,custom}.json.
/// https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.signal.firwin2.html
/// Rejects nonfinite inputs, invalid endpoints/duplicates, Type II nonzero
/// Nyquist gain and unrepresentable mesh sizes. No gain normalization is applied.
pub fn firwin2(
    numtaps: usize,
    freq: &[f64],
    gain: &[f64],
    nfreqs: Option<usize>,
    fs: f64,
) -> Result<Vec<f64>, DspError> {
    let invalid = |s: &str| DspError::InvalidArgument(s.into());
    if numtaps == 0
        || freq.len() < 2
        || freq.len() != gain.len()
        || !fs.is_finite()
        || fs <= 0.0
        || fs / 2.0 == 0.0
        || !freq.iter().chain(gain).all(|v| v.is_finite())
    {
        return Err(invalid(
            "firwin2 requires positive taps/fs and finite matching frequency/gain arrays",
        ));
    }
    let nyq = fs / 2.0;
    if freq[0] != 0.0
        || freq[freq.len() - 1] != nyq
        || freq.windows(2).any(|v| v[0] > v[1])
        || freq.windows(3).any(|v| v[0] == v[2])
        || freq[1] == 0.0
        || freq[freq.len() - 2] == nyq
    {
        return Err(invalid(
            "frequencies must span 0..fs/2, with at most two equal interior values",
        ));
    }
    if numtaps.is_multiple_of(2) && gain[gain.len() - 1] != 0.0 {
        return Err(invalid("Type II requires zero Nyquist gain"));
    }
    let mesh = match nfreqs {
        Some(n) => n,
        None => numtaps
            .checked_next_power_of_two()
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| invalid("firwin2 mesh overflow"))?,
    };
    if mesh <= numtaps {
        return Err(invalid("nfreqs must exceed numtaps"));
    }
    let n = (mesh - 1)
        .checked_mul(2)
        .ok_or_else(|| invalid("firwin2 FFT length overflow"))?;
    let mut f = freq.to_vec();
    let eps = f64::EPSILON * nyq;
    for i in 0..f.len() - 1 {
        if f[i] == f[i + 1] {
            f[i] -= eps;
            f[i + 1] += eps;
        }
    }
    if f.windows(2).any(|v| v[0] >= v[1]) {
        return Err(invalid(
            "frequencies collide after duplicate epsilon adjustment",
        ));
    }
    let mut j = 0;
    let spectrum: Vec<_> = (0..mesh)
        .map(|i| {
            let x = if i == mesh - 1 {
                nyq
            } else {
                i as f64 * (nyq / (mesh - 1) as f64)
            };
            while j + 1 < f.len() - 1 && x >= f[j + 1] {
                j += 1;
            }
            let g = if x == f[j] {
                gain[j]
            } else if x == f[j + 1] {
                gain[j + 1]
            } else {
                gain[j] + (gain[j + 1] - gain[j]) / (f[j + 1] - f[j]) * (x - f[j])
            };
            Complex64::from_polar(g, -((numtaps - 1) as f64) / 2.0 * PI * x / nyq)
        })
        .collect();
    Ok(fft::irfft(&spectrum, n)
        .into_iter()
        .take(numtaps)
        .zip(windows::hamming(numtaps, true))
        .map(|(h, w)| h * w)
        .collect())
}

/// Mirrors scipy.signal.minimum_phase's default FFT length; p05_minimum_default.json.
/// https://docs.scipy.org/doc/scipy-1.16.1/reference/generated/scipy.signal.minimum_phase.html
/// Panics for len < 3 (the routine's actual input minimum) or usize overflow.
pub fn minimum_phase_default_nfft(len: usize) -> usize {
    assert!(len >= 3, "minimum_phase requires at least three taps");
    (len - 1)
        .checked_mul(200)
        .and_then(usize::checked_next_power_of_two)
        .expect("minimum_phase default FFT length overflow")
}

/// Mirrors scipy.signal.minimum_phase(method="homomorphic") of the frozen
/// oracle, SciPy 1.18.0. Fixtures p05_minimum_{smooth,notch,odd,padded}_{half,full}.json
/// save all stages and their `y` comes from scipy.signal.minimum_phase itself.
/// https://docs.scipy.org/doc/scipy-1.18.0/reference/generated/scipy.signal.minimum_phase.html
/// https://github.com/scipy/scipy/blob/v1.18.0/scipy/signal/_fir_filter_design.py
/// Adds 1e-7*min(positive magnitude) to EVERY bin; the cepstral lifter is
/// `w[0] = 1`, `w[1..n_fft/2] = 2`, `w[n_fft/2] = 1 + n_fft % 2` (even: 1,
/// odd: 2). SciPy 1.17.1 and earlier used 0/1 there; 2.x users run the fixed
/// version, so that rule is not reproduced. Rejects fewer than three taps,
/// nonfinite/all-zero input and n_fft < len(h). Nonsymmetric input is
/// accepted (SciPy warns, but performs the same arithmetic).
pub fn minimum_phase(h: &[f64], n_fft: usize, half: bool) -> Result<Vec<f64>, DspError> {
    let (log, lifter) = homomorphic_stages(h, n_fft, half)?;
    let cepstrum = fft::ifft(
        &log.iter()
            .map(|&v| Complex64::new(v, 0.0))
            .collect::<Vec<_>>(),
    );
    let lifted: Vec<_> = cepstrum
        .iter()
        .zip(lifter)
        .map(|(v, w)| Complex64::new(v.re * w, 0.0))
        .collect();
    let spectrum: Vec<_> = fft::fft(&lifted).into_iter().map(|v| v.exp()).collect();
    Ok(fft::ifft(&spectrum)
        .into_iter()
        .take(if half { h.len().div_ceil(2) } else { h.len() })
        .map(|v| v.re)
        .collect())
}

/// Mirrors pocketfft's real radix butterflies at Nyquist for scipy.signal
/// minimum_phase; p05_minimum_{smooth,notch}_*.json pins this sensitive bin.
/// https://raw.githubusercontent.com/mreineck/pocketfft/cpp/pocketfft_hdronly.h
/// Only 235-smooth even lengths are handled; other sizes use RustFFT unchanged.
fn pocketfft_nyquist(h: &[f64], n: usize) -> Option<f64> {
    if !n.is_multiple_of(2) {
        return None;
    }
    let mut remaining = n;
    let mut factors = Vec::new();
    while remaining.is_multiple_of(4) {
        factors.push(4);
        remaining /= 4;
    }
    if remaining.is_multiple_of(2) {
        remaining /= 2;
        factors.push(2);
        let last = factors.len() - 1;
        factors.swap(0, last);
    }
    for p in [3, 5] {
        while remaining.is_multiple_of(p) {
            factors.push(p);
            remaining /= p;
        }
    }
    if remaining != 1 {
        return None;
    }
    // Forward real stages traverse the factor list in reverse. Its final
    // radix-2/4 butterfly is the alternating sum of the preceding DC branches.
    let last = factors.remove(0);
    factors.reverse();
    let z: Vec<_> = (0..last)
        .map(|i| pocketfft_dc(h, i, last, &factors))
        .collect();
    Some(if last == 2 {
        z[0] - z[1]
    } else {
        (z[0] + z[2]) - (z[3] + z[1])
    })
}

/// Mirrors pocketfft radf2/3/4/5 DC addition ordering, not a compensated sum;
/// p05_minimum_* log spectra pin these scipy.signal.minimum_phase intermediates.
/// Linear work in input length, logarithmic recursion depth, no FFT replacement.
fn pocketfft_dc(h: &[f64], offset: usize, stride: usize, factors: &[usize]) -> f64 {
    if factors.is_empty() {
        return h.get(offset).copied().unwrap_or(0.0);
    }
    let r = factors[0];
    let mut z = [0.0; 5];
    for (i, v) in z[..r].iter_mut().enumerate() {
        *v = pocketfft_dc(h, offset + i * stride, stride * r, &factors[1..]);
    }
    match r {
        2 => z[0] + z[1],
        3 => z[0] + (z[1] + z[2]),
        4 => (z[0] + z[2]) + (z[3] + z[1]),
        5 => (z[0] + (z[4] + z[1])) + (z[3] + z[2]),
        _ => unreachable!(),
    }
}

/// Intermediate scipy.signal.minimum_phase (SciPy 1.18.0) log spectrum and
/// lifter, pinned by p05_minimum_*.json; shared with unit diagnostics, not a
/// public API.
fn homomorphic_stages(
    h: &[f64],
    n_fft: usize,
    half: bool,
) -> Result<(Vec<f64>, Vec<f64>), DspError> {
    if h.len() < 3 || n_fft < h.len() || !h.iter().all(|v| v.is_finite()) {
        return Err(DspError::InvalidArgument(
            "minimum_phase requires finite h, len >= 3 and n_fft >= len".into(),
        ));
    }
    let mut padded = vec![Complex64::new(0.0, 0.0); n_fft];
    for (v, &h) in padded.iter_mut().zip(h) {
        v.re = h;
    }
    let mut magnitude: Vec<_> = fft::fft(&padded).iter().map(|v| v.norm()).collect();
    // A Type II's almost-zero Nyquist bin is log-sensitive. Match the real
    // pocketfft butterfly reduction for 235-smooth even lengths, rather than
    // allowing a different complex FFT's rounding to dominate the cepstrum.
    if let Some(nyquist) = pocketfft_nyquist(h, n_fft) {
        magnitude[n_fft / 2] = nyquist.abs();
    }
    let min = magnitude
        .iter()
        .copied()
        .filter(|v| *v > 0.0)
        .fold(f64::INFINITY, f64::min);
    if !min.is_finite() || magnitude.iter().any(|v| !v.is_finite()) {
        return Err(DspError::InvalidArgument(
            "minimum_phase requires a finite nonzero spectrum".into(),
        ));
    }
    let log = magnitude
        .iter()
        .map(|v| (v + 1e-7 * min).ln() * if half { 0.5 } else { 1.0 })
        .collect();
    let mut lifter = vec![0.0; n_fft];
    lifter[0] = 1.0;
    lifter[1..n_fft / 2].fill(2.0);
    lifter[n_fft / 2] = if n_fft.is_multiple_of(2) { 1.0 } else { 2.0 };
    Ok((log, lifter))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::{fs, path::PathBuf};

    /// Decode scipy.signal.minimum_phase stage arrays; p05_minimum_*.json.
    fn binary(root: &std::path::Path, descriptor: &Value) -> Vec<f64> {
        fs::read(root.join(descriptor["file"].as_str().unwrap()))
            .unwrap()
            .chunks_exact(8)
            .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
            .collect()
    }

    /// Mirror scipy.signal.minimum_phase 1.17.1 log-floor/lifter stages;
    /// p05_minimum_*.json independently pins even/odd and half/full semantics.
    #[test]
    fn golden_homomorphic_log_and_lifter_match_scipy() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/migration/goldens");
        let mut max_log: f64 = 0.0;
        let mut max_db: f64 = 0.0;
        let mut count = 0;
        for path in fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()) {
            let name = path.file_name().unwrap().to_str().unwrap();
            if !name.starts_with("p05_minimum_")
                || !name.ends_with(".json")
                || name == "p05_minimum_default.json"
            {
                continue;
            }
            let v: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let i = &v["inputs"];
            let h = binary(&root, &i["h"]);
            let (log, lifter) = homomorphic_stages(
                &h,
                i["n_fft"].as_u64().unwrap() as usize,
                i["half"].as_bool().unwrap(),
            )
            .unwrap();
            assert_eq!(lifter, binary(&root, &v["outputs"]["lifter"]), "{name}");
            let expected = binary(&root, &v["outputs"]["log"]);
            let peak = expected.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            for (a, b) in log.iter().zip(expected) {
                let error = (a - b).abs();
                max_log = max_log.max(error);
                assert!(error < 1e-9, "{name}: log error {error}");
                if b > peak + 1e-5_f64.ln() {
                    let db = error * 20.0 / std::f64::consts::LN_10;
                    max_db = max_db.max(db);
                    assert!(db < 1e-5, "{name}: log spectrum dB error {db}");
                }
                count += 1;
            }
        }
        assert!(count > 0);
        println!(
            "MEASURE homomorphic_log: max_abs={max_log:.17e}, spectrum_db={max_db:.17e}, values={count}; lifter exact"
        );
    }
}
