mod golden_secs2_helpers;
use faer::{Mat, linalg::solvers::Solve};
use golden_secs2_helpers::*;
use impulcifer_dsp::stages::room_tuning2::mimo::{bisect, normal_equations};
#[test]
fn room_tuning2_mimo_fast_normal_equations() {
    let v = fixture("mimo_mixed_fsiso150_auto_FL");
    let o = &v["outputs"];
    let x: Vec<_> = o["x"].as_array().unwrap().iter().map(array).collect();
    let lp = array(&o["lp"]);
    let direct = number(&o["direct"]);
    let g: Vec<_> = o["g"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| array(g).chunks(720).map(|g| g.to_vec()).collect::<Vec<_>>())
        .collect();
    let c: Vec<Vec<_>> = g
        .iter()
        .map(|ear| {
            ear.iter()
                .map(|g| {
                    let mut c = vec![0.; 720];
                    for (i, a) in g.iter().enumerate() {
                        for (j, b) in lp.iter().enumerate() {
                            if i + j < 720 {
                                c[i + j] += a * b;
                            }
                        }
                    }
                    c
                })
                .collect()
        })
        .collect();
    let m = 1080;
    let (fast, rhs) = normal_equations(&x, &c);
    let mut a = Mat::<f64>::zeros(2880, m);
    let mut b = vec![0.; 2880];
    for e in 0..2 {
        let peak = x[e].iter().map(|v| v.abs()).fold(0., f64::max);
        let early = x[e].iter().position(|v| v.abs() > 0.1 * peak).unwrap() + 36;
        for t in 0..720 {
            let row = if t >= early {
                e * 720 + t
            } else {
                1440 + e * 720 + t
            };
            let weight = if t >= early { 1. } else { 10_f64.sqrt() };
            if t >= early {
                b[row] = -x[e][t];
            }
            for j in 0..6 {
                for k in 0..180.min(t + 1) {
                    a[(row, j * 180 + k)] = weight * c[e][j][t - k];
                }
            }
        }
    }
    let dense = a.transpose() * &a;
    let rb: Vec<_> = (0..m)
        .map(|i| (0..2880).map(|t| a[(t, i)] * b[t]).sum::<f64>())
        .collect();
    let mut error = 0.;
    let mut norm = 0.;
    for i in 0..m {
        for j in 0..m {
            error += (fast[(i, j)] - dense[(i, j)]).powi(2);
            norm += dense[(i, j)].powi(2);
        }
    }
    let matrix_rel = (error / norm).sqrt();
    let rhs_rel = (rhs
        .iter()
        .zip(&rb)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        / rb.iter().map(|b| b * b).sum::<f64>())
    .sqrt();
    println!("MEASURE dense normal matrix={matrix_rel:e} rhs={rhs_rel:e}");
    assert!(matrix_rel < 1e-12 && rhs_rel < 1e-12);
    let (logs, ratios, kept, u) = bisect(&fast, &rhs, &lp, direct).unwrap();
    let sigma = (0..m).map(|i| dense[(i, i)]).sum::<f64>() / m as f64;
    let (mut lo, mut hi) = (-8., 4.);
    let mut best = vec![];
    let mut chosen = 0;
    for i in 0..19 {
        let log = if i == 0 { 4. } else { (lo + hi) / 2. };
        assert_eq!(log, logs[i]);
        let mut matrix = dense.clone();
        for k in 0..m {
            matrix[(k, k)] += sigma * 10_f64.powf(log);
        }
        let solve = matrix
            .llt(faer::Side::Lower)
            .unwrap()
            .solve(Mat::from_fn(m, 1, |i, _| rb[i]));
        let values: Vec<_> = (0..m).map(|j| solve[(j, 0)]).collect();
        let ratio = values
            .chunks(180)
            .map(|u| {
                let mut s = vec![0.; 210];
                for (i, a) in u.iter().enumerate() {
                    for (j, b) in lp.iter().enumerate() {
                        s[i + j] += a * b;
                    }
                }
                s.iter().map(|v| v * v).sum::<f64>()
            })
            .sum::<f64>()
            / direct;
        assert_eq!(ratio > 10_f64.powf(-1.2), ratios[i] > 10_f64.powf(-1.2));
        if i == 0 || ratio <= 10_f64.powf(-1.2) {
            best = values;
            chosen = i;
            if i > 0 {
                hi = log;
            }
        } else {
            lo = log;
        }
    }
    let rel = (u
        .iter()
        .zip(&best)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        / best.iter().map(|v| v * v).sum::<f64>())
    .sqrt();
    println!("MEASURE dense kept u={rel:e}, decisions=19/19 kept={kept}/{chosen}");
    assert_eq!(kept, chosen);
    assert!(rel < 1e-9);
}
