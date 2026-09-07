#![forbid(unsafe_code)]
use impulcifer_dsp::{
    fir,
    interp::{self, Spline},
    peaks, smoothing,
};

/// Mirrors scipy.signal.firwin2 symmetry; complements p05_firwin2_*.json.
#[test]
fn firwin2_output_symmetry() {
    let mut max: f64 = 0.0;
    for n in [1, 2, 17, 32, 129, 9600] {
        let h = fir::firwin2(n, &[0.0, 0.2, 0.8, 1.0], &[1.0, 0.8, 0.3, 0.0], None, 2.0).unwrap();
        for (a, b) in h.iter().zip(h.iter().rev()) {
            max = max.max((a - b).abs());
        }
    }
    assert!(max < 1e-11);
    println!("MEASURE firwin2_symmetry: max_abs={max:.17e}");
}
/// Mirrors scipy.signal.minimum_phase energy concentration; p05_minimum_smooth_*.
#[test]
fn minimum_phase_energy_concentrated() {
    let h = fir::firwin2(
        9600,
        &[0.0, 0.3, 0.7, 1.0],
        &[1.0, 0.8, 0.6, 0.0],
        None,
        2.0,
    )
    .unwrap();
    let y = fir::minimum_phase(&h, h.len(), true).unwrap();
    let energy: f64 = y.iter().map(|v| v * v).sum();
    let early: f64 = y[..y.len() / 2].iter().map(|v| v * v).sum();
    assert!(early / energy > 0.99);
    println!(
        "MEASURE minimum_phase_energy_first_half: fraction={:.17e}",
        early / energy
    );
}
/// Mirrors FITPACK interpolation at knots; p05_spline_k*, including n=8000
/// to exercise the banded solve rather than an impractical dense matrix.
#[test]
fn spline_hits_knots_and_scales_to_eight_thousand() {
    let mut max: f64 = 0.0;
    for n in [4, 31, 8000] {
        let x: Vec<_> = (0..n).map(|i| (1.0 + i as f64 * 0.017).ln()).collect();
        let y: Vec<_> = x.iter().map(|v| (v * 3.1).sin() + v * v).collect();
        for k in 1..=3 {
            let s = Spline::new(&x, &y, k).unwrap();
            for (a, b) in s.eval(&x).iter().zip(&y) {
                max = max.max((a - b).abs());
            }
        }
    }
    assert!(max < 1e-9);
    println!("MEASURE spline_knot_interpolation: max_abs={max:.17e}");
}
/// Mirrors scipy.signal.find_peaks endpoint exclusion; p05_peaks_*.json.
#[test]
fn find_peaks_endpoints_excluded() {
    for x in [
        vec![],
        vec![1.0],
        vec![1.0, 1.0],
        vec![2.0, 0.0, 1.0, 0.0, 2.0],
        vec![1.0; 9],
    ] {
        for i in peaks::find_peaks(&x, None) {
            assert!(i > 0 && i + 1 < x.len());
        }
    }
}
/// Mirrors scipy FIR/SG/FITPACK domain validation; p05_* complements invalid cases.
#[test]
fn batch2_rejects_invalid_arguments() {
    for (n, f, g, q, fs) in [
        (0, vec![0.0, 1.0], vec![1.0, 0.0], None, 2.0),
        (4, vec![], vec![], None, 2.0),
        (4, vec![0.0, 1.0], vec![1.0], None, 2.0),
        (4, vec![0.0, 1.0], vec![1.0, 1.0], None, 2.0),
        (4, vec![0.1, 1.0], vec![1.0, 0.0], None, 2.0),
        (
            4,
            vec![0.0, 0.7, 0.3, 1.0],
            vec![1.0, 1.0, 1.0, 0.0],
            None,
            2.0,
        ),
        (
            4,
            vec![0.0, 0.5, 0.5, 0.5, 1.0],
            vec![1.0, 1.0, 0.5, 0.5, 0.0],
            None,
            2.0,
        ),
        (4, vec![0.0, 0.0, 1.0], vec![1.0, 1.0, 0.0], None, 2.0),
        (4, vec![0.0, 1.0, 1.0], vec![1.0, 0.0, 0.0], None, 2.0),
        (
            4,
            vec![0.0, 0.5, 0.5, 0.5 + f64::EPSILON / 2.0, 1.0],
            vec![1.0, 1.0, 0.5, 0.5, 0.0],
            None,
            2.0,
        ),
        (4, vec![0.0, 1.0], vec![1.0, 0.0], Some(4), 2.0),
        (4, vec![0.0, 1.0], vec![f64::NAN, 0.0], None, 2.0),
        (4, vec![0.0, 1.0], vec![1.0, 0.0], None, f64::INFINITY),
        (usize::MAX, vec![0.0, 1.0], vec![1.0, 0.0], None, 2.0),
    ] {
        assert!(fir::firwin2(n, &f, &g, q, fs).is_err());
    }
    for (h, n) in [
        (vec![], 3),
        (vec![1.0, 1.0], 2),
        (vec![0.0; 3], 3),
        (vec![1.0; 3], 2),
        (vec![f64::NAN; 3], 3),
    ] {
        assert!(fir::minimum_phase(&h, n, true).is_err());
    }
    for (w, p) in [(0, 0), (2, 1), (5, 5), (7, 2)] {
        assert!(smoothing::savgol_filter(&[1.0; 5], w, p).is_err());
    }
    assert!(smoothing::savgol_filter(&[f64::NAN; 5], 3, 2).is_err());
    for (x, y, k) in [
        (vec![], vec![], 1),
        (vec![0.0, 1.0], vec![0.0, 1.0], 3),
        (vec![0.0, 0.0], vec![0.0, 1.0], 1),
        (vec![1.0, 0.0], vec![0.0, 1.0], 1),
        (vec![0.0, 1.0], vec![0.0], 1),
        (vec![0.0, 1.0], vec![0.0, f64::NAN], 1),
        (vec![0.0, 1.0], vec![0.0, 1.0], 0),
        (vec![0.0, 1.0], vec![0.0, 1.0], 4),
    ] {
        assert!(Spline::new(&x, &y, k).is_err());
    }
    // Actual hrir retries only ValueError: FITPACK m>k is not that exception.
    assert!(interp::interp_log_axis(&[10.0, 100.0], &[0.0, 1.0], 3, &[20.0]).is_err());
    assert!(interp::interp_log_axis(&[10.0, 10.0], &[0.0, 1.0], 1, &[20.0]).is_err());
    assert!(interp::interp_log_axis(&[0.0, 10.0], &[0.0, 1.0], 1, &[20.0]).is_err());
    assert!(interp::interp_log_axis(&[10.0, 100.0], &[0.0, 1.0], 1, &[20.0, 0.0]).is_err());
    assert!(std::panic::catch_unwind(|| fir::minimum_phase_default_nfft(2)).is_err());
    assert!(std::panic::catch_unwind(|| fir::minimum_phase_default_nfft(usize::MAX)).is_err());
    for (o, r) in [(-1.0, 1.01), (1.0, 1.0), (f64::NAN, 1.01), (2000.0, 1.01)] {
        assert!(std::panic::catch_unwind(|| smoothing::fractional_octave_window(o, r)).is_err());
    }
}
