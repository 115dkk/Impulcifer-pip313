#![forbid(unsafe_code)]
use impulcifer_dsp::{
    fft::Complex64,
    optimize::{LeastSquaresTrace, least_squares_with_trace},
};
use serde_json::Value;
use std::f64::consts::PI;
fn arr(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}
fn div(a: Complex64, b: Complex64) -> Complex64 {
    if b.re.abs() >= b.im.abs() {
        let r = b.im / b.re;
        let d = 1. / (b.re + b.im * r);
        Complex64::new((a.re + a.im * r) * d, (a.im - a.re * r) * d)
    } else {
        let r = b.re / b.im;
        let d = 1. / (b.re * r + b.im);
        Complex64::new((a.re * r + a.im) * d, (a.im * r - a.re) * d)
    }
}
#[test]
fn trf_matches_scipy() {
    let data: Value = serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/trf_scipy_reference.json"
    ))
    .unwrap();
    for case in data["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let x0 = arr(&case["x0"]);
        let lower = arr(&case["lower"]);
        let upper = arr(&case["upper"]);
        let t = case["t"]
            .as_array()
            .map(|_| arr(&case["t"]))
            .unwrap_or_default();
        let y = case["y"]
            .as_array()
            .map(|_| arr(&case["y"]))
            .unwrap_or_default();
        let grid = case["grid"]
            .as_array()
            .map(|_| arr(&case["grid"]))
            .unwrap_or_default();
        let curve = case["curve"]
            .as_array()
            .map(|_| arr(&case["curve"]))
            .unwrap_or_default();
        let z: Vec<_> = grid
            .iter()
            .map(|f| Complex64::from_polar(1., (-2. * PI * f) * (1. / 48000.)))
            .collect();
        let mut trace = LeastSquaresTrace::default();
        let r = least_squares_with_trace(
            |x| match name {
                "smooth" => t
                    .iter()
                    .zip(&y)
                    .map(|(t, y)| x[0] * (-x[1] * t).exp() + x[2] - y)
                    .collect(),
                "active" => vec![x[0] - 3., 2. * (x[1] + 2.), x[0] + x[1]],
                _ => {
                    let mut p = vec![Complex64::new(1., 0.); grid.len()];
                    for x in x.chunks_exact(3) {
                        let f = x[0].exp();
                        let q = x[1].exp();
                        let a = 10_f64.powf(x[2] / 40.);
                        let w = 2. * PI * f / 48000.;
                        let alpha = w.sin() / (2. * q);
                        let b = [1. + alpha * a, -2. * w.cos(), 1. - alpha * a];
                        let a = [1. + alpha / a, -2. * w.cos(), 1. - alpha / a];
                        for (i, z) in z.iter().enumerate() {
                            p[i] *= div(
                                b[0] + b[1] * z + b[2] * z * z,
                                a[0] + a[1] * z + a[2] * z * z,
                            );
                        }
                    }
                    p.iter()
                        .enumerate()
                        .map(|(i, p)| {
                            (if grid[i] >= 20. && grid[i] <= 500. {
                                1.
                            } else {
                                0.15
                            }) * (20.
                                * (impulcifer_dsp::stages::room_tuning2::peq::magnitude(*p)
                                    + 1e-12)
                                    .log10()
                                - curve[i])
                        })
                        .collect()
                }
            },
            &x0,
            &lower,
            &upper,
            150,
            Some(&mut trace),
        )
        .unwrap();
        let expected = arr(&case["x"]);
        let dx =
            r.x.iter()
                .zip(expected)
                .map(|(a, b)| (a - b).abs())
                .fold(0., f64::max);
        let dc = (r.cost - case["cost"].as_f64().unwrap()).abs();
        println!(
            "TRF {name}: max_x={dx:.12e} cost_diff={dc:.12e} nfev={}/{} status={}/{}",
            r.nfev, case["nfev"], r.status, case["status"]
        );
        if name == "peq16" {
            let expected = case["main_x"].as_array().unwrap();
            let horizon = case["horizon"].as_u64().map(|v| v as usize);
            let gated = horizon.map_or(expected.len(), |h| h.saturating_sub(1));
            assert!(trace.main_x.len() >= gated);
            let mut first = None;
            for (k, (got, want)) in trace.main_x.iter().zip(expected).enumerate() {
                let want = arr(want);
                let rel = got
                    .iter()
                    .zip(&want)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0., f64::max)
                    / want.iter().map(|v| v.abs()).fold(0., f64::max);
                if rel > 1e-9 && first.is_none() {
                    first = Some(k);
                }
                if k < gated {
                    assert!(rel <= 1e-9, "PEQ16 evaluation {k}: {rel:e}");
                }
            }
            println!(
                "MEASURE trf_peq16: first={first:?} horizon={horizon:?} cost={} reference={} gesvd={}",
                r.cost, case["cost"], case["oracle_gesvd"]["cost"]
            );
        } else {
            assert_eq!(r.nfev, case["nfev"].as_u64().unwrap() as usize);
            assert_eq!(r.status, case["status"].as_i64().unwrap() as i32);
            assert!(dx < 1e-9);
            assert!(dc < 1e-14);
        }
    }
}
