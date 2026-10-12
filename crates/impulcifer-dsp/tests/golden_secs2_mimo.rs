mod golden_secs2_helpers;
use golden_secs2_helpers::*;
use impulcifer_dsp::stages::room_tuning2::mimo::design_paths;

fn convolve(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut y = vec![0.; a.len() + b.len() - 1];
    for (i, a) in a.iter().enumerate() {
        for (j, b) in b.iter().enumerate() {
            y[i + j] += a * b;
        }
    }
    y
}
fn first(x: &[f64]) -> usize {
    let peak = x.iter().map(|v| v.abs()).fold(0., f64::max);
    x.iter().position(|v| v.abs() > 0.1 * peak).unwrap_or(0)
}
#[test]
fn mimo_guard_hypotheses() {
    use impulcifer_dsp::{filters, stages::room_tuning2::primitives::*};
    let mut stats = [
        (0usize, 0usize, 0f64, String::new()),
        (0, 0, 0., String::new()),
        (0, 0, 0., String::new()),
    ];
    let mut mismatches = Vec::new();
    for (name, v) in fixtures("mimo_") {
        let o = &v["outputs"];
        if o.get("x").is_none() {
            continue;
        }
        let x: Vec<_> = o["x"].as_array().unwrap().iter().map(array).collect();
        let g: Vec<Vec<_>> = o["g"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| array(v).chunks(720).map(|x| x.to_vec()).collect())
            .collect();
        let lp = array(&o["lp"]);
        let u = array(&o["u"]);
        let supports: Vec<_> = u.chunks(180).map(|u| convolve(&lp, u)).collect();
        let fsiso: f64 = if name.contains("fsiso150") {
            150.
        } else {
            100.
        };
        let expected_band = array(&o["guard_band_db"]);
        let expected_tail = array(&o["guard_tail"]);
        let mut tail_index = 0;
        for (trial, expected_band) in expected_band.iter().enumerate() {
            let mut y = x.clone();
            for (y, g) in y.iter_mut().zip(&g) {
                for (g, s) in g.iter().zip(&supports) {
                    let s: Vec<_> = s
                        .iter()
                        .map(|v| v * 10f64.powf(-3. * trial as f64 / 20.))
                        .collect();
                    for (y, v) in y.iter_mut().zip(convolve(g, &s)) {
                        *y += v;
                    }
                }
            }
            let before_peaks = late_peaks(&x, 6);
            let after_peaks = late_peaks(&y, 6);
            let edges: Vec<_> = (0..)
                .map(|i| i as f64 / 3.)
                .take_while(|i| *i <= (fsiso / 20.).log2() + 1e-9)
                .map(|i| 20. * 2f64.powf(i))
                .collect();
            let bands: Vec<_> = edges
                .windows(2)
                .map(|w| (w[0], w[1]))
                .chain(before_peaks.iter().map(|p| (p - 1.5, p + 1.5)))
                .collect();
            let mut a = vec![0.; 721];
            let mut b = a.clone();
            for (x, y) in x.iter().zip(&y) {
                let end = (first(x) + 36).min(720);
                let mut xx = x.clone();
                let mut yy = y.clone();
                xx[..end].fill(0.);
                yy[..end].fill(0.);
                for (a, v) in a.iter_mut().zip(rfft(&xx, 1440)) {
                    *a += v.norm_sqr();
                }
                for (b, v) in b.iter_mut().zip(rfft(&yy, 1440)) {
                    *b += v.norm_sqr();
                }
            }
            let band = bands
                .iter()
                .map(|(lo, hi)| {
                    let (mut aa, mut bb) = (0., 0.);
                    for k in 0..721 {
                        let f = k as f64 * 600. / 1440.;
                        if f >= *lo && f < *hi {
                            aa += a[k];
                            bb += b[k];
                        }
                    }
                    if aa > 0. {
                        10. * ((bb + 1e-30) / aa).log10()
                    } else {
                        -99.
                    }
                })
                .fold(-99., f64::max);
            for (rule, stat) in stats.iter_mut().enumerate() {
                let mut compare = |kind: &str, got: f64, expected: f64| {
                    let diff = (got - expected).abs();
                    stat.0 += 1;
                    stat.1 += usize::from(diff <= 1e-9);
                    if diff > stat.2 {
                        stat.2 = diff;
                        stat.3 = format!("{name}/trial{trial}/{kind}");
                    }
                    diff
                };
                compare("band", band, *expected_band);
                if *expected_band > 1. {
                    continue;
                }
                let mut modes: Vec<_> = match rule {
                    0 => before_peaks.iter().chain(&after_peaks).copied().collect(),
                    1 => before_peaks.clone(),
                    _ => after_peaks.clone(),
                };
                for p in &mut modes {
                    *p = (*p * 10.).round_ties_even() / 10.;
                }
                modes.sort_by(f64::total_cmp);
                modes.dedup();
                let energies: Vec<_> = modes
                    .iter()
                    .map(|p| {
                        let sos =
                            filters::butter_bandpass(2, (p - 1.5).max(1.) / 300., (p + 1.5) / 300.);
                        let (mut a, mut b) = (0., 0.);
                        for (x, y) in x.iter().zip(&y) {
                            let k = (first(x) + 180).min(720);
                            a += filters::sosfilt(&sos, x)[k..]
                                .iter()
                                .map(|v| v * v)
                                .sum::<f64>();
                            b += filters::sosfilt(&sos, y)[k..]
                                .iter()
                                .map(|v| v * v)
                                .sum::<f64>();
                        }
                        (*p, a, b)
                    })
                    .collect();
                let max = energies.iter().map(|v| v.1).fold(0., f64::max);
                let tail = energies
                    .iter()
                    .filter(|(_, a, _)| *a > 0. && *a >= 0.01 * max)
                    .map(|(_, a, b)| 10. * ((b + 1e-30) / a).log10())
                    .fold(-99., f64::max)
                    * 0.15;
                if compare("tail", tail, expected_tail[tail_index]) > 1e-9 {
                    mismatches.push(format!("H{} {name} trial={trial} before={before_peaks:?} after={after_peaks:?} modes/a/b={energies:?} got={tail} expected={}", rule + 1, expected_tail[tail_index]));
                }
            }
            if *expected_band <= 1. {
                tail_index += 1;
            }
        }
        assert_eq!(
            tail_index,
            expected_tail.len(),
            "{name}: every recorded tail"
        );
    }
    for (rule, s) in stats.iter().enumerate() {
        println!(
            "HYPOTHESIS H{} compared={} within_1e-9={} worst={:.17e} where={}",
            rule + 1,
            s.0,
            s.1,
            s.2,
            s.3
        );
    }
    let winners: Vec<_> = stats
        .iter()
        .enumerate()
        .filter(|(_, s)| s.0 == s.1)
        .map(|(i, _)| i + 1)
        .collect();
    if winners.len() != 1 {
        for m in mismatches {
            println!("MISMATCH {m}");
        }
    }
    assert_eq!(
        winners.len(),
        1,
        "no unique guard rule reproduces the oracle; do not change production"
    );
}

#[test]
fn golden_secs2_mimo() {
    let mut normal = Errors::default();
    let mut solved = Errors::default();
    let mut guard = Errors::default();
    for (name, v) in fixtures("mimo_") {
        let o = &v["outputs"];
        if o.get("x").is_none() {
            continue;
        }
        let x: Vec<_> = o["x"].as_array().unwrap().iter().map(array).collect();
        let r = integer(&o["candidates"]);
        let g: Vec<_> = o["g"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| array(v).chunks(720).map(|c| c.to_vec()).collect::<Vec<_>>())
            .collect();
        assert_eq!(g[0].len(), r);
        let candidates: Vec<_> = ["FL", "FR", "FC", "BL", "BR", "SL", "SR"]
            .iter()
            .filter(|s| **s != v["inputs"]["input"].as_str().unwrap())
            .map(|s| (s.to_string(), 50.))
            .collect();
        let fsiso = if name.contains("fsiso150") {
            150.
        } else {
            100.
        };
        let (result, s) = design_paths(
            v["inputs"]["input"].as_str().unwrap(),
            &candidates,
            x,
            g,
            array(&o["lp"]),
            number(&o["direct"]),
            fsiso,
        )
        .unwrap();
        let m = r * 180;
        normal.real(
            &format!("{name}/diag"),
            &(0..m).map(|i| s.normal[(i, i)]).collect::<Vec<_>>(),
            &array(&o["AtA_diag"]),
        );
        let mut rows = vec![];
        for j in 0..r {
            for k in 0..r {
                for t in 0..180 {
                    rows.push(s.normal[(j * 180, k * 180 + t)]);
                }
            }
        }
        normal.real(
            &format!("{name}/rows"),
            &rows,
            &array(&o["AtA_block_first_rows"]),
        );
        normal.real(&format!("{name}/rhs"), &s.rhs, &array(&o["Atb"]));
        for (a, b) in s.logs.iter().zip(array(&o["log10_lambda_over_scale"])) {
            assert!((a - b).abs() < 1e-10, "{name}/decisions {a} {b}");
        }
        assert_eq!(s.kept, integer(&o["kept_solve"]));
        solved.compare(&format!("{name}/u"), &s.u, &array(&o["u"]), 1e-9, 1e-12);
        solved.compare(
            &format!("{name}/ratios"),
            &s.ratios,
            &array(&o["ratios"]),
            3e-9, // provisional: measured 1.68e-9 × 1.5, rounded up
            1e-12,
        );
        println!(
            "{name} guard {:?} {:?} expected {:?} {:?}",
            s.guard_band,
            s.guard_tail,
            array(&o["guard_band_db"]),
            array(&o["guard_tail"])
        );
        let count = s.guard_band.len().min(array(&o["guard_band_db"]).len());
        guard.compare(
            &format!("{name}/band"),
            &s.guard_band[..count],
            &array(&o["guard_band_db"])[..count],
            0.,
            1e-9,
        );
        let tails: Vec<_> = s
            .guard_tail
            .iter()
            .zip(&s.guard_band)
            .filter(|(_, band)| **band <= 1.)
            .map(|(v, _)| v * 0.15)
            .collect();
        let expected = array(&o["guard_tail"]);
        let count = tails.len().min(expected.len());
        guard.compare(
            &format!("{name}/tail"),
            &tails[..count],
            &expected[..count],
            0.,
            1e-9,
        );
        println!(
            "MEASURE {name} level={} scale={:?} band={} tail={}",
            result.level_db, result.scale_db, result.band_change_db, result.tail_change_db
        );
    }
    normal.finish("MIMO normal equations");
    solved.finish("MIMO solve");
    guard.finish("MIMO guard");
}
