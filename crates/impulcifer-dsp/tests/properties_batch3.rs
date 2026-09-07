#![forbid(unsafe_code)]
use impulcifer_dsp::{fft, fir, resample, spectrogram};
use std::f64::consts::PI;

/// Mirrors scipy.resample_poly same-rate copy and nnresample's design error;
/// p06_design_48000_48000 / p06_edge_poly_3 pin the differing contracts.
/// Group-delay alignment is zero for scipy's bypass (no FIR is applied).
/// A delayed-copy property for nnresample 0.2.4.1 cannot hold: it raises first.
#[test]
fn same_rate_copy_and_nnresample_error_are_distinct() {
    let x: Vec<_> = (0..4800).map(|i| (i as f64 * 0.3).sin()).collect();
    let got = resample::resample_poly(&x, 48000, 48000, &[]).unwrap();
    assert_eq!(got.len(), x.len());
    assert_eq!(got, x);
    assert!(resample::nnresample(&x, 48000, 48000).is_err());
}
/// Mirrors nnresample.resample DC interior; p06_dc.json freezes the oracle's
/// 1.53132e-5 phase ripple, which FAILS the packet's per-sample unity 1e-6 claim.
/// At 147/160 half-support is 100 output samples; discard 200 at each end.
/// Without changing taps, phase-repeated outputs are flat, and the interior
/// mean (not every sample) preserves unity within the requested 1e-6 budget.
#[test]
fn resampling_dc_preserves_mean_and_phase_periodicity() {
    let x = vec![1.0; 4800];
    let y = resample::nnresample(&x, 44100, 48000).unwrap();
    let interior = &y[200..y.len() - 200];
    let max = interior.iter().map(|v| (v - 1.0).abs()).fold(0.0, f64::max);
    let mean_error = (interior.iter().sum::<f64>() / interior.len() as f64 - 1.0).abs();
    let phase_error = interior
        .iter()
        .zip(&interior[147..])
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    assert!(mean_error < 1e-6);
    assert!(phase_error < 1e-6);
    // Explicit negative test: do not claim that the unavailable property passes.
    assert!(max > 1e-6);
    println!(
        "MEASURE resampling_dc: unity_max={max:.17e} (requested 1e-6 FAILS oracle), mean_error={mean_error:.17e}, phase_error={phase_error:.17e}"
    );
}
/// Mirrors scipy/nnresample duration and frequency preservation; p06_poly_sine_*.
#[test]
fn resampled_sine_keeps_one_khz_peak() {
    let x: Vec<_> = (0..4800)
        .map(|i| (2.0 * PI * 1000.0 * i as f64 / 48000.0).sin())
        .collect();
    for fs in [44100, 96000] {
        let y = resample::nnresample(&x, fs, 48000).unwrap();
        let bins = fft::rfft(&y);
        let peak = bins
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.norm().total_cmp(&b.norm()))
            .unwrap()
            .0;
        assert_eq!(peak as f64 * fs as f64 / y.len() as f64, 1000.0);
    }
}
/// Mirrors scipy.signal.spectrogram's pure-sine PSD peak; p06_spectrogram_sine
/// pins the same 4800-point Hann window and frequency grid on a windowed sine.
#[test]
fn spectrogram_pure_sine_peaks_at_one_khz() {
    let x: Vec<_> = (0..24000)
        .map(|i| (2.0 * PI * 1000.0 * i as f64 / 48000.0).sin())
        .collect();
    let s = spectrogram::spectrogram(&x, 48000, 4800, 2400).unwrap();
    for segment in 0..s.times.len() {
        let peak = s
            .power
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a[segment].total_cmp(&b[segment]))
            .unwrap()
            .0;
        assert_eq!(s.freqs[peak], 1000.0);
    }
}

/// Mirrors scipy.spectrogram zero PSD / detrend / odd one-sided scaling;
/// p06_spectrogram_{odd,one,noise} complements these exact properties.
#[test]
fn spectrogram_zeros_shapes_and_density_energy() {
    for n in [1, 2, 9, 4800] {
        let x = vec![0.0; n * 3 + 1];
        let s = spectrogram::spectrogram(&x, 48000, n, n / 2).unwrap();
        assert_eq!(s.freqs.len(), n / 2 + 1);
        assert_eq!(s.times.len(), (x.len() - n) / (n - n / 2) + 1);
        assert!(
            s.power
                .iter()
                .all(|row| row.len() == s.times.len() && row.iter().all(|v| *v == 0.0))
        );
        let constant = spectrogram::spectrogram(&vec![3.25; n * 3], 48000, n, n / 2).unwrap();
        assert!(constant.power.iter().flatten().all(|v| *v == 0.0));
    }
    for n in [9, 10] {
        let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.7).sin()).collect();
        let w = impulcifer_dsp::windows::hann(n, false);
        let mean = x.iter().sum::<f64>() / n as f64;
        let expected = x
            .iter()
            .zip(&w)
            .map(|(v, w)| ((v - mean) * w).powi(2))
            .sum::<f64>()
            / w.iter().map(|w| w * w).sum::<f64>();
        let s = spectrogram::spectrogram(&x, 48000, n, 0).unwrap();
        let integral = s.power.iter().map(|row| row[0]).sum::<f64>() * 48000.0 / n as f64;
        assert!((integral - expected).abs() < 1e-14);
    }
}
/// Mirrors scipy/nnresample invalid domains; p06_design_48000_48000 and
/// p06_edge_poly_* additionally pin successful empty/same-rate behavior.
#[test]
fn batch3_rejects_invalid_arguments_and_preserves_inputs() {
    for (u, d) in [(0, 1), (1, 0), (0, 0)] {
        assert!(resample::nnresample_design(u, d).is_err());
        assert!(resample::resample_poly(&[1.0], u, d, &[1.0]).is_err());
        assert!(resample::nnresample(&[1.0], u as u32, d as u32).is_err());
    }
    assert!(resample::resample_poly(&[1.0], 2, 3, &[]).is_err());
    assert!(resample::resample_poly(&[1.0, 2.0], usize::MAX, 2, &[1.0]).is_err());
    for cutoff in [0.0, 1.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(fir::firwin_lowpass(3, cutoff, 5.0).is_err());
    }
    assert!(fir::firwin_lowpass(0, 0.5, 5.0).is_err());
    assert!(fir::firwin_lowpass(3, 0.5, f64::NAN).is_err());
    for (n, overlap, fs) in [(0, 0, 48000), (5, 0, 48000), (3, 3, 48000), (3, 0, 0)] {
        assert!(spectrogram::spectrogram(&[1.0; 3], fs, n, overlap).is_err());
    }
    assert!(spectrogram::spectrogram(&[], 48000, 1, 0).is_err());
    for resolution in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(spectrogram::spectrogram_params(100, 48000, resolution, 200).is_none());
    }
    assert!(spectrogram::spectrogram_params(100, 0, 10.0, 200).is_none());
    let x = [1.0, 2.0, 3.0];
    let taps = [0.25, 0.5, 0.25];
    let _ = resample::resample_poly(&x, 2, 3, &taps).unwrap();
    assert_eq!(x, [1.0, 2.0, 3.0]);
    assert_eq!(taps, [0.25, 0.5, 0.25]);
    assert!(
        spectrogram::spectrogram(&[f64::NAN; 9], 48000, 9, 0)
            .unwrap()
            .power
            .iter()
            .flatten()
            .all(|v| v.is_nan())
    );
}
/// Mirrors nnresample's deterministic cached taps; p06_poly_impulse_44100.
#[test]
fn reduced_ratio_cache_is_deterministic_across_threads() {
    let handles: Vec<_> = (0..4)
        .map(|i| {
            std::thread::spawn(move || {
                let (up, down) = if i % 2 == 0 {
                    (44100, 48000)
                } else {
                    (147, 160)
                };
                resample::nnresample(&[1.0, 0.0, -1.0], up, down).unwrap()
            })
        })
        .collect();
    let outputs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(outputs.windows(2).all(|v| v[0] == v[1]));
}
