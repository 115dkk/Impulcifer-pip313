#![forbid(unsafe_code)]
//! Virtual bass synthesis, core/virtual_bass.py:31-198.
use crate::{
    DspError, fft,
    filters::{self, BType, Sos},
    hrir::Hrir,
};
use impulcifer_types::constants::{Side, speaker_side};
#[derive(Clone, Debug)]
pub struct VirtualBassOptions {
    pub crossover_freq: u32,
    pub head_ms: f64,
    pub hp_freq: f64,
    pub invert_polarity: Option<bool>,
}
/// Python _delay_signal, core/virtual_bass.py:31-43; p10_vbass.
fn delay_signal(sig: &[f64], delay: i64, length: usize) -> Vec<f64> {
    let mut out = vec![0.0; length];
    if delay >= 0 {
        let start = (delay as usize).min(length);
        let n = (length - start).min(sig.len());
        out[start..start + n].copy_from_slice(&sig[..n]);
    } else {
        let start = delay.unsigned_abs().min(sig.len() as u64) as usize;
        let n = length.min(sig.len() - start);
        out[..n].copy_from_slice(&sig[start..start + n]);
    }
    out
}
/// Python _mag_at, core/virtual_bass.py:46-57; p10_vbass.
fn mag_at(ir: &[f64], fs: u32, freq: f64) -> f64 {
    let spectrum = fft::rfft(ir);
    let index = (0..spectrum.len())
        .min_by(|&a, &b| {
            (a as f64 * (fs as f64 / ir.len() as f64) - freq)
                .abs()
                .total_cmp(&(b as f64 * (fs as f64 / ir.len() as f64) - freq).abs())
        })
        .unwrap();
    spectrum[index].norm()
}
/// Python _duplicate_sos, core/virtual_bass.py:60-62; p10_vbass.
fn duplicate_sos(sos: &Sos, times: usize) -> Sos {
    Sos(sos.0.repeat(times))
}
/// Python _rbj_high_shelf, core/virtual_bass.py:65-79; p10_vbass.
fn rbj_high_shelf(fc: f64, fs: u32, gain: f64, q: f64) -> Sos {
    let a = 10.0_f64.powf(gain / 40.0);
    let w = 2.0 * std::f64::consts::PI * fc / fs as f64;
    let alpha = w.sin() / (2.0 * q);
    let c = w.cos();
    filters::tf2sos(
        &[
            a * ((a + 1.0) + (a - 1.0) * c + 2.0 * a.sqrt() * alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - 2.0 * a.sqrt() * alpha),
        ],
        &[
            (a + 1.0) - (a - 1.0) * c + 2.0 * a.sqrt() * alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - 2.0 * a.sqrt() * alpha,
        ],
    )
}
/// Python synthesize_virtual_bass/apply_virtual_bass_to_hrir,
/// core/virtual_bass.py:82-198; p10_vbass. Nyquist rejection returns unchanged,
/// matching Python's logged early return, not an invented DSP error.
pub fn apply_virtual_bass(hrir: &mut Hrir, options: &VirtualBassOptions) -> Result<(), DspError> {
    let fs = hrir.fs;
    let nyq = fs as f64 / 2.0;
    let xo = options.crossover_freq as f64;
    if xo >= nyq {
        return Ok(());
    }
    if xo <= 0.0
        || !options.hp_freq.is_finite()
        || options.hp_freq <= 0.0
        || options.hp_freq >= nyq
        || !options.head_ms.is_finite()
    {
        return Err(DspError::InvalidArgument(
            "invalid virtual bass cutoff or head duration".into(),
        ));
    }
    let n = hrir
        .speakers
        .iter()
        .flat_map(|s| s.left.iter().chain(s.right.iter()))
        .map(|ir| ir.len())
        .max()
        .unwrap_or(0);
    if n == 0 {
        return Err(DspError::InvalidArgument(
            "virtual bass requires nonempty responses".into(),
        ));
    }
    hrir.for_each_ir(|ir| ir.data.resize(n, 0.0));
    let mut impulse = vec![0.0; n];
    impulse[0] = 1.0;
    let sub = filters::sosfilt(
        &filters::butter(4, options.hp_freq / nyq, BType::Highpass),
        &impulse,
    );
    let bass = filters::sosfilt(
        &duplicate_sos(&filters::butter(4, xo / nyq, BType::Lowpass), 2),
        &sub,
    );
    let ild = Sos([
        (150.0, -1.5, 0.760),
        (400.0, -3.0, 0.660),
        (800.0, -3.5, 0.610),
    ]
    .into_iter()
    .flat_map(|(fc, g, q)| rbj_high_shelf(fc, fs, g, q).0)
    .collect());
    let high = duplicate_sos(&filters::butter(4, xo / nyq, BType::Highpass), 2);
    let mut mags = Vec::new();
    for s in &hrir.speakers {
        if let (Some(l), Some(r)) = (&s.left, &s.right) {
            for ir in [l, r] {
                mags.push(mag_at(&filters::sosfilt(&high, &ir.data), fs, xo));
            }
        }
    }
    if mags.is_empty() {
        return Ok(());
    }
    let gain = (mags.iter().sum::<f64>() / mags.len() as f64) / (mag_at(&bass, fs, xo) + 1e-20);
    let head = (options.head_ms * 1e-3 * fs as f64).round_ties_even() as i64;
    let polarity = if options.invert_polarity == Some(true) {
        -1.0
    } else {
        1.0
    };
    for s in &mut hrir.speakers {
        if let (Some(l), Some(r)) = (&mut s.left, &mut s.right) {
            let on_left = speaker_side(&s.speaker) == Side::Left;
            let itd = r.peak_index(0, None, 0.12589) as i64 - l.peak_index(0, None, 0.12589) as i64;
            let direct: Vec<_> = bass.iter().map(|v| v * gain * polarity).collect();
            let cross = filters::sosfilt(&ild, &direct);
            let direct = delay_signal(&direct, head, n);
            let cross = delay_signal(&cross, head + if on_left { itd } else { -itd }, n);
            l.data = filters::sosfilt(&high, &l.data)
                .iter()
                .zip(if on_left { &direct } else { &cross })
                .map(|(h, b)| h + b)
                .collect();
            r.data = filters::sosfilt(&high, &r.data)
                .iter()
                .zip(if on_left { &cross } else { &direct })
                .map(|(h, b)| h + b)
                .collect();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Python zero-padding, truncation and non-wrapping bounds; core/virtual_bass.py:31-43; p10_vbass.
    #[test]
    fn delay_signal_boundaries() {
        let x = [1.0, 2.0, 3.0];
        for (delay, length, expected) in [
            (0, 5, vec![1.0, 2.0, 3.0, 0.0, 0.0]),
            (0, 2, vec![1.0, 2.0]),
            (1, 3, vec![0.0, 1.0, 2.0]),
            (-1, 3, vec![2.0, 3.0, 0.0]),
            (3, 3, vec![0.0; 3]),
            (-3, 3, vec![0.0; 3]),
            (i64::MAX, 3, vec![0.0; 3]),
            (i64::MIN, 3, vec![0.0; 3]),
            (0, 0, vec![]),
        ] {
            assert_eq!(delay_signal(&x, delay, length), expected);
        }
        assert_eq!(delay_signal(&[], 0, 3), vec![0.0; 3]);
    }

    /// Python nearest rfft bin, first-bin tie and endpoint clamps; core/virtual_bass.py:53-57; p10_vbass.
    #[test]
    fn mag_at_bin_boundaries() {
        // Eight-sample DC signal: only the first bin has magnitude eight.
        let x = [1.0; 8];
        assert_eq!(mag_at(&x, 8000, -100.0), 8.0);
        assert_eq!(mag_at(&x, 8000, 500.0), 8.0);
        assert_eq!(mag_at(&x, 8000, 500.1), 0.0);
        assert_eq!(mag_at(&x, 8000, 10000.0), 0.0);
        assert_eq!(mag_at(&[2.0], 8000, 4000.0), 2.0);
        for freq in [0.0, 500.0, 4000.0, 10000.0] {
            assert!((mag_at(&[1.0, 0.0, 0.0, 0.0, 0.0], 8000, freq) - 1.0).abs() < 1e-14);
        }
    }

    /// Python duplicates entire chains in order, not individual sections; core/virtual_bass.py:60-62; p10_vbass.
    #[test]
    fn duplicate_sos_preserves_chain_order() {
        let sos = filters::butter(4, 0.25, BType::Lowpass);
        assert_eq!(duplicate_sos(&sos, 1).0, sos.0);
        let got = duplicate_sos(&sos, 3);
        assert_eq!(got.0.len(), 3 * sos.0.len());
        for chain in got.0.chunks(sos.0.len()) {
            assert_eq!(chain, sos.0);
        }
    }

    /// Python RBJ shelf has unit DC and requested Nyquist gain; core/virtual_bass.py:65-79; p10_vbass.
    #[test]
    fn rbj_high_shelf_endpoint_gains() {
        for fs in [44100, 48000] {
            for (fc, gain, q) in [
                (150.0, -1.5, 0.760),
                (400.0, -3.0, 0.660),
                (800.0, -3.5, 0.610),
                (400.0, 0.0, 0.660),
                (400.0, 3.0, 0.660),
            ] {
                let sos = rbj_high_shelf(fc, fs, gain, q);
                assert_eq!(sos.0.len(), 1);
                let c = &sos.0[0];
                let dc = (c[0] + c[1] + c[2]) / (c[3] + c[4] + c[5]);
                let nyquist = (c[0] - c[1] + c[2]) / (c[3] - c[4] + c[5]);
                assert!((dc - 1.0).abs() < 1e-9);
                assert!((nyquist - 10.0_f64.powf(gain / 20.0)).abs() < 1e-12);
            }
        }
    }
}
