#![forbid(unsafe_code)]
//! PA02 regression coverage for cached plans, grouped SOS, and polyphase dots.
use impulcifer_dsp::{fft, filters, resample};

#[test]
fn cached_plans_preserve_lengths_directions_and_threads() {
    let run = || {
        for n in [1, 2, 7, 64, 65, 128, 7] {
            let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.31).sin()).collect();
            let y = fft::irfft(&fft::rfft(&x), n);
            for (a, b) in x.iter().zip(y) {
                assert!((a - b).abs() < 1e-12);
            }
            let complex: Vec<_> = x.iter().map(|&v| fft::Complex64::new(v, -v)).collect();
            let back = fft::ifft(&fft::fft(&complex));
            for (a, b) in complex.iter().zip(back) {
                assert!((*a - b).norm() < 1e-12);
            }
        }
    };
    run();
    std::thread::spawn(run).join().unwrap();
}

#[test]
fn reused_real_buffers_match_fresh_plans_and_clear_inverse_padding() {
    let mut planner = realfft::RealFftPlanner::<f64>::new();
    for n in [96000, 1, 65, 2, 257, 64, 96000, 7] {
        let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.31).sin()).collect();
        let forward = planner.plan_fft_forward(n);
        let mut input = x.clone();
        let mut expected = forward.make_output_vec();
        forward.process(&mut input, &mut expected).unwrap();
        assert_eq!(fft::rfft(&x), expected);
        // Populate the shared spectrum before testing shorter inverse input.
        let db = fft::magnitude_response(&x);
        for keep in [expected.len(), 1, expected.len().div_ceil(2)] {
            let inverse = planner.plan_fft_inverse(n);
            let mut padded = inverse.make_input_vec();
            padded[..keep].copy_from_slice(&expected[..keep]);
            let mut reference = inverse.make_output_vec();
            inverse.process(&mut padded, &mut reference).unwrap();
            reference.iter_mut().for_each(|v| *v /= n as f64);
            assert_eq!(fft::irfft(&expected[..keep], n), reference);
        }
        assert_eq!(fft::rfft(&x), expected);
        assert_eq!(fft::magnitude_response(&x), db);
    }
    println!("MEASURE reused_real_buffers: max_abs=0 (bit-identical to fresh plans)");
}

#[test]
fn power_db_identity_preserves_hypot_range_and_nonfinite_masks() {
    let mut max_abs = 0.0_f64;
    for amplitude in [
        0.0,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        1e-160,
        1e-150,
        0.5,
        1.0,
        1.0 + f64::EPSILON,
        1e150,
        1e160,
        f64::MAX / 4.0,
        f64::INFINITY,
        f64::NAN,
    ] {
        for n in [1, 2, 7, 64, 65] {
            let mut x = vec![0.0; n];
            x[0] = amplitude;
            let spectrum = fft::rfft(&x);
            let got = fft::magnitude_response(&x);
            assert_eq!(got.len(), n.div_ceil(2));
            for (&got, v) in got.iter().zip(spectrum) {
                let expected = 20.0 * v.norm().log10();
                if expected.is_finite() {
                    let error = (got - expected).abs();
                    max_abs = max_abs.max(error);
                    assert!(got.is_finite() && error <= 1e-12 + 1e-11 * expected.abs());
                } else {
                    assert!(got == expected || (got.is_nan() && expected.is_nan()));
                }
            }
        }
    }
    println!("MEASURE power_db_range: max_abs={max_abs:.17e}");
}

#[test]
fn power_db_identity_matches_hypot_at_demo_size() {
    let mut state = 7_u32;
    let x: Vec<_> = (0..96000)
        .map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            state as f64 / 2147483648.0 - 1.0
        })
        .collect();
    let spectrum = fft::rfft(&x);
    let got = fft::magnitude_response(&x);
    let mut max_abs = 0.0_f64;
    for (&got, v) in got.iter().zip(spectrum) {
        let expected = 20.0 * v.norm().log10();
        let error = (got - expected).abs();
        max_abs = max_abs.max(error);
        assert!(error <= 1e-12 + 1e-11 * expected.abs());
    }
    println!("MEASURE power_db_demo: max_abs={max_abs:.17e}");
}

#[test]
fn grouped_sos_matches_sectionwise_bit_for_bit() {
    let x: Vec<_> = (0..257).map(|i| (i as f64 * 0.71).cos()).collect();
    for sections in 1..=9 {
        let row = filters::butter(2, 0.13, filters::BType::Lowpass).0[0];
        let sos = filters::Sos(vec![row; sections]);
        let mut expected = x.clone();
        for row in &sos.0 {
            expected = filters::sosfilt(&filters::Sos(vec![*row]), &expected);
        }
        assert_eq!(filters::sosfilt(&sos, &x), expected);
    }
}

#[test]
fn contiguous_polyphase_matches_scalar_zero_extension() {
    for (up, down) in [(2, 1), (3, 2), (5, 7), (147, 160)] {
        for n in [1, 3, 129] {
            for len in [1, 2, 7, 32, 401] {
                let x: Vec<_> = (0..n).map(|i| (i as f64 * 0.43).sin()).collect();
                let taps: Vec<_> = (0..len)
                    .map(|i| (i as f64 * 0.27).cos() / len as f64)
                    .collect();
                let got = resample::resample_poly(&x, up, down, &taps).unwrap();
                let half = (len - 1) / 2;
                let pre = down - half % down;
                let remove = (half + pre) / down;
                for (i, &value) in got.iter().enumerate() {
                    let t = (i + remove) * down;
                    let mut expected = 0.0;
                    for (j, &sample) in x.iter().enumerate() {
                        if let Some(k) = t.checked_sub(pre + j * up).filter(|&k| k < len) {
                            expected += sample * (taps[k] * up as f64);
                        }
                    }
                    assert!(
                        (value - expected).abs() < 1e-12,
                        "{up}/{down} n={n} taps={len} i={i}"
                    );
                }
            }
        }
    }
}
