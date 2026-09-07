#![forbid(unsafe_code)]

use impulcifer_dsp::{
    conv::{self, Mode},
    fft::{self, Complex64},
    filters::{self, BType, Sos},
    stats, windows,
};

#[test]
fn real_fft_parseval_and_roundtrip() {
    for n in [1, 2, 8, 9, 257, 1024, 1025] {
        let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.371).sin() + 0.2).collect();
        let spectrum = fft::rfft(&x);
        assert_eq!(spectrum.len(), n / 2 + 1);
        let energy: f64 = spectrum
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let weight = if i == 0 || (n % 2 == 0 && i == n / 2) {
                    1.0
                } else {
                    2.0
                };
                weight * v.norm_sqr()
            })
            .sum::<f64>()
            / n as f64;
        assert!((energy - x.iter().map(|v| v * v).sum::<f64>()).abs() < 1e-11);
        for (a, b) in fft::irfft(&spectrum, n).iter().zip(x) {
            assert!((a - b).abs() < 1e-12);
        }
    }
}
#[test]
fn convolution_linearity_and_identity() {
    for n in [5, 257] {
        let a: Vec<_> = (0..n).map(|i| (i as f64).sin()).collect();
        let b: Vec<_> = (0..n).map(|i| (i as f64).cos()).collect();
        let h = vec![0.125; n];
        let sum: Vec<_> = a.iter().zip(&b).map(|(a, b)| 2.0 * a - b).collect();
        let ca = conv::convolve(&a, &h, Mode::Full);
        let cb = conv::convolve(&b, &h, Mode::Full);
        let cs = conv::convolve(&sum, &h, Mode::Full);
        for ((a, b), s) in ca.iter().zip(cb).zip(cs) {
            assert!((2.0 * a - b - s).abs() < 1e-12);
        }
        assert_eq!(conv::convolve(&a, &[1.0], Mode::Same), a);
    }
}
#[test]
fn butter_lowpass_dc_gain_and_filter_reset() {
    for order in [0, 1, 2, 3, 4, 5, 8] {
        for wn in [15.0 / 24000.0, 250.0 / 24000.0, 0.75] {
            let sos = filters::butter(order, wn, BType::Lowpass);
            let gain: f64 = sos
                .0
                .iter()
                .map(|s| (s[0] + s[1] + s[2]) / (s[3] + s[4] + s[5]))
                .product();
            assert!(
                (gain - 1.0).abs() < 1e-9,
                "order {order}, wn {wn}, gain {gain}"
            );
            let x = [1.0, 0.0, 0.0];
            assert_eq!(filters::sosfilt(&sos, &x), filters::sosfilt(&sos, &x));
            assert!(std::panic::catch_unwind(|| filters::sosfilt(&sos, &[])).is_err());
        }
    }
}
#[test]
fn windows_symmetry_periodicity_and_i0_limits() {
    for n in [0, 1, 2, 9, 64, 257] {
        for beta in [0.0, 5.65326, 20.0, 500.0] {
            let w = windows::kaiser(n, beta, true);
            assert!(w.iter().all(|x| x.is_finite() && *x >= 0.0 && *x <= 1.0));
            for (a, b) in w.iter().zip(w.iter().rev()) {
                assert!((a - b).abs() < 1e-12);
            }
            assert_eq!(w, windows::kaiser(n, -beta, true));
        }
        if n > 1 {
            let mut extended = windows::hann(n + 1, true);
            extended.pop();
            assert_eq!(windows::hann(n, false), extended);
        }
    }
    assert!(windows::get_window("unknown", 0, true).is_err());
    assert_eq!(windows::kaiser(9, 0.0, true), vec![1.0; 9]);
}
#[test]
fn fast_lengths_are_minimal_and_handle_overflow() {
    let smooth = |mut n: usize| {
        for p in [2, 3, 5] {
            while n > 1 && n.is_multiple_of(p) {
                n /= p;
            }
        }
        n <= 1
    };
    for n in 0..4096 {
        let m = fft::next_fast_len(n);
        assert!(m >= n && smooth(m));
        assert!((n..m).all(|x| !smooth(x)));
    }
    assert!(std::panic::catch_unwind(|| fft::next_fast_len(usize::MAX)).is_err());
}
#[test]
fn statistics_edges_and_nonfinite_policy() {
    assert_eq!(stats::running_mean(&[1.0, 2.0, 3.0], 2), vec![1.5, 2.5]);
    assert!(stats::running_mean(&[1.0], 2).is_empty());
    assert_eq!(stats::uniform_filter2d_size3_zero(1, 1, &[9.0]), vec![1.0]);
    assert!(stats::uniform_filter2d_size3_zero(0, 1, &[]).is_empty());
    assert_eq!(stats::expit(f64::NEG_INFINITY), 0.0);
    assert_eq!(stats::expit(f64::INFINITY), 1.0);
    assert!(stats::expit(f64::NAN).is_nan());
    for x in [-100.0, -1.0, 0.0, 1.0, 100.0] {
        assert!((stats::expit(x) + stats::expit(-x) - 1.0).abs() < 1e-15);
    }
    assert!(
        stats::linregress(&[0.0, f64::NAN], &[1.0, 2.0])
            .slope
            .is_nan()
    );
    assert!(
        fft::magnitude_response(&[0.0; 8])
            .iter()
            .all(|v| *v == f64::NEG_INFINITY)
    );
    assert!(fft::rfft(&[f64::NAN, 1.0]).iter().any(|v| v.re.is_nan()));
    assert!(std::panic::catch_unwind(|| fft::irfft(&[], 3)).is_err());
    assert_eq!(fft::irfft(&[Complex64::new(2.0, 9.0)], 1), vec![2.0]);
}
#[test]
fn invalid_arguments_panic_explicitly() {
    let invalid: Vec<Box<dyn Fn()>> = vec![
        Box::new(|| {
            fft::rfft(&[]);
        }),
        Box::new(|| {
            fft::fft(&[]);
        }),
        Box::new(|| {
            fft::ifft(&[]);
        }),
        Box::new(|| {
            fft::irfft(&[], 0);
        }),
        Box::new(|| {
            conv::convolve(&[], &[1.0], Mode::Full);
        }),
        Box::new(|| {
            conv::correlate(&[1.0], &[], Mode::Same);
        }),
        Box::new(|| {
            conv::correlation_lags(0, 1, Mode::Full);
        }),
        Box::new(|| {
            windows::kaiser(8, f64::NAN, true);
        }),
        Box::new(|| {
            filters::butter(4, 0.0, BType::Lowpass);
        }),
        Box::new(|| {
            filters::butter(4, 1.0, BType::Highpass);
        }),
        Box::new(|| {
            filters::butter(4, f64::NAN, BType::Lowpass);
        }),
        Box::new(|| {
            filters::sosfilt(&Sos(vec![]), &[1.0]);
        }),
        Box::new(|| {
            filters::sosfilt(&Sos(vec![[1.0, 0.0, 0.0, 2.0, 0.0, 0.0]]), &[]);
        }),
        Box::new(|| {
            filters::tf2sos(&[1.0], &[0.0]);
        }),
        Box::new(|| {
            filters::tf2sos(&[1.0; 4], &[1.0]);
        }),
        Box::new(|| {
            filters::rbj_peaking(105.0, 0.0, 4.0, 48000.0);
        }),
        Box::new(|| {
            filters::rbj_low_shelf(105.0, 1.0, f64::INFINITY, 48000.0);
        }),
        Box::new(|| {
            filters::rbj_high_shelf(25000.0, 1.0, 4.0, 48000.0);
        }),
        Box::new(|| {
            stats::running_mean(&[1.0], 0);
        }),
        Box::new(|| {
            stats::linregress(&[1.0], &[]);
        }),
        Box::new(|| {
            stats::uniform_filter2d_size3_zero(2, 3, &[0.0]);
        }),
        Box::new(|| {
            stats::uniform_filter2d_size3_zero(1, 1, &[f64::NAN]);
        }),
    ];
    for (i, run) in invalid.into_iter().enumerate() {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).is_err(),
            "invalid case {i} must panic"
        );
    }
}
