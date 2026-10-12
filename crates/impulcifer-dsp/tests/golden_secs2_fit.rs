mod golden_secs2_helpers;
use golden_secs2_helpers::*;
use impulcifer_dsp::stages::room_tuning2::peq::fit_with_trace;

// No absolute allowance: every parameter of every iterate must meet README's
// relative 1e-9 contract, including parameters close to zero.
const REL: f64 = 1e-9;
fn run(synthetic: bool) {
    let mut failures = Vec::new();
    for (name, v) in fixtures("fit_") {
        if name.starts_with("fit_synthetic_") != synthetic {
            continue;
        }
        let i = &v["inputs"];
        let o = &v["outputs"];
        let kw = &i["kwargs"];
        assert_eq!(kw["ring_ms"].as_f64().unwrap_or(30.), 30.);
        let g = array(&i["g"]);
        let c = array(&i["corr_db"]);
        let mut trace = None;
        let got = fit_with_trace(
            &g,
            &c,
            integer(&i["n"]),
            kw["lo"].as_f64().unwrap_or(20.),
            kw["hi"].as_f64().unwrap_or(500.),
            kw["boost"].as_f64().unwrap_or(10.),
            kw["f_max"].as_f64(),
            48000.,
            Some(&mut trace),
        )
        .unwrap();
        let expected = bands(&o["bands"]);
        if expected.is_empty() {
            assert!(got.is_empty() && trace.is_none());
            continue;
        }
        let t = trace.unwrap();
        let mut errors = Errors::default();
        let mut first = None;
        let mut compare = |field: &str, a: &[f64], b: &[f64], width: usize| {
            if a.len() != b.len() {
                failures.push(format!("{name}/{field}: shape {} != {}", a.len(), b.len()));
            }
            let count = a.len().min(b.len());
            if first.is_none()
                && let Some(k) = (0..count)
                    .find(|&k| !a[k].is_finite() || (a[k] - b[k]).abs() > REL * b[k].abs())
            {
                first = Some(format!(
                    "FIRST {name}/{field}: evaluation={} parameter={} got={:.17e} expected={:.17e} abs={:.17e}",
                    k / width,
                    k % width,
                    a[k],
                    b[k],
                    (a[k] - b[k]).abs()
                ));
            }
            errors.compare(
                &format!("{name}/{field}"),
                &a[..count],
                &b[..count],
                REL,
                0.,
            );
        };
        let n = integer(&o["params"]);
        compare("x0", &t.x0, &array(&o["x0"]), n);
        compare("lb", &t.lb, &array(&o["lb"]), n);
        compare("ub", &t.ub, &array(&o["ub"]), n);
        compare(
            "f0_cost",
            &[t.solver.initial_cost],
            &[number(&o["f0_cost"])],
            1,
        );
        if !o["J0"].is_null() {
            assert_eq!(
                t.solver.j0_shape,
                [integer(&o["J0_shape"][0]), integer(&o["J0_shape"][1])]
            );
            compare("J0", &t.solver.j0, &array(&o["J0"]), n);
        }
        compare(
            "main_x",
            &t.solver
                .main_x
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>(),
            &array(&o["main_x"]),
            n,
        );
        compare("main_cost", &t.solver.main_cost, &array(&o["main_cost"]), 1);
        compare(
            "jac_x",
            &t.solver.jac_x.iter().flatten().copied().collect::<Vec<_>>(),
            &array(&o["jac_x"]),
            n,
        );
        let r = &o["result"];
        compare("result.x", &t.result.x, &array(&r["x"]), n);
        compare("result.cost", &[t.result.cost], &[number(&r["cost"])], 1);
        compare(
            "result.optimality",
            &[t.result.optimality],
            &[number(&r["optimality"])],
            1,
        );
        compare(
            "bands",
            &got.iter()
                .flat_map(|&(f, q, g)| [f, q, g])
                .collect::<Vec<_>>(),
            &expected
                .iter()
                .flat_map(|&(f, q, g)| [f, q, g])
                .collect::<Vec<_>>(),
            3,
        );
        for (field, a) in [
            ("nfev", t.result.nfev as i64),
            ("njev", t.result.njev as i64),
            ("status", t.result.status as i64),
        ] {
            if a != r[field].as_i64().unwrap() {
                failures.push(format!("{name}/{field}: {a} != {}", r[field]));
            }
        }
        assert_eq!(t.solver.main_x.len() + 1, t.result.nfev);
        assert_eq!(t.solver.jac_x.len() + 1, t.result.njev);
        assert!(
            t.solver
                .main_x
                .iter()
                .chain(&t.solver.jac_x)
                .all(|x| x.len() == n)
        );
        if let Some(f) = first {
            println!("{f}");
        }
        // Keep evaluating the entire family, even if an earlier fixture fails.
        if std::panic::catch_unwind(|| errors.finish(&name)).is_err() {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
#[test]
fn synthetic_initial_jacobian() {
    synthetic_prefix(1);
}
#[test]
fn bumps_solver_prefix() {
    synthetic_prefix(8);
}
fn synthetic_prefix(budget: usize) {
    use impulcifer_dsp::{optimize, stages::room_tuning2::peq};
    for (name, v) in fixtures("fit_synthetic_") {
        if budget > 1 && !name.contains("bumps") {
            continue;
        }
        let i = &v["inputs"];
        let o = &v["outputs"];
        let grid = array(&i["g"]);
        let curve = array(&i["corr_db"]);
        let lo = i["kwargs"]["lo"].as_f64().unwrap_or(20.);
        let hi = i["kwargs"]["hi"].as_f64().unwrap_or(500.);
        let mut trace = optimize::LeastSquaresTrace::default();
        let result = optimize::least_squares_with_trace(
            |x| {
                let bands: Vec<_> = x
                    .chunks_exact(3)
                    .map(|x| (x[0].exp(), x[1].exp(), x[2]))
                    .collect();
                peq::response(&grid, &bands, 48000.)
                    .iter()
                    .enumerate()
                    .map(|(k, p)| {
                        let weight = if grid[k] >= lo && grid[k] <= hi {
                            1.
                        } else {
                            0.15
                        };
                        weight * (20. * (peq::magnitude(*p) + 1e-12).log10() - curve[k])
                    })
                    .collect()
            },
            &array(&o["x0"]),
            &array(&o["lb"]),
            &array(&o["ub"]),
            budget,
            Some(&mut trace),
        )
        .unwrap();
        if budget == 1 {
            assert_eq!(result.njev, 1);
            assert!(trace.jac_x.is_empty());
        }
        assert_eq!(
            integer(&o["jac_x"]["length"]) / integer(&o["params"]) + 1,
            integer(&o["result"]["njev"])
        );
        let mut errors = Errors::default();
        errors.compare(&format!("{name}/J0"), &trace.j0, &array(&o["J0"]), REL, 0.);
        errors.finish(&format!("{name}/initial"));
        if budget > 1 {
            let expected = array(&o["main_x"]);
            let n = trace.j0_shape[1];
            let mut first = None;
            for (evaluation, got) in trace.main_x.iter().enumerate() {
                let want = &expected[evaluation * n..(evaluation + 1) * n];
                let max_relative = got
                    .iter()
                    .zip(want)
                    .map(|(a, b)| ((a - b) / b).abs())
                    .fold(0., f64::max);
                println!("PREFIX {name}: evaluation={evaluation} max_relative={max_relative:.17e}");
                if first.is_none()
                    && let Some(k) =
                        (0..n).find(|&k| (got[k] - want[k]).abs() > REL * want[k].abs())
                {
                    first = Some(format!(
                        "FIRST {name}/main_x: evaluation={evaluation} parameter={k} got={:.17e} expected={:.17e}",
                        got[k], want[k]
                    ));
                }
            }
            assert!(first.is_none(), "{}", first.unwrap_or_default());
        }
    }
}
#[test]
fn a_synthetic_fit_gates() {
    run(true);
}
#[test]
fn b_actual_fit_gates() {
    run(false);
}
#[test]
fn empty_fit_has_no_trace() {
    let mut trace = None;
    assert!(
        fit_with_trace(
            &[20., 100., 500.],
            &[0.; 3],
            16,
            20.,
            500.,
            10.,
            None,
            48000.,
            Some(&mut trace)
        )
        .unwrap()
        .is_empty()
    );
    assert!(trace.is_none());
}
