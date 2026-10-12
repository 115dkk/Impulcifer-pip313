mod golden_secs2_helpers;
use golden_secs2_helpers::*;
use impulcifer_dsp::stages::room_tuning2::peq::fit_with_trace;

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
        let mut trace = None;
        let got = fit_with_trace(
            &array(&i["g"]),
            &array(&i["corr_db"]),
            integer(&i["n"]),
            kw["lo"].as_f64().unwrap_or(20.),
            kw["hi"].as_f64().unwrap_or(500.),
            kw["boost"].as_f64().unwrap_or(10.),
            kw["f_max"].as_f64(),
            48000.,
            Some(&mut trace),
        )
        .unwrap();
        if bands(&o["bands"]).is_empty() {
            assert!(got.is_empty() && trace.is_none());
            continue;
        }
        let t = trace.unwrap();
        let mut errors = Errors::default();
        for (field, values) in [("x0", &t.x0), ("lb", &t.lb), ("ub", &t.ub)] {
            errors.real(&format!("{name}/{field}"), values, &array(&o[field]));
            assert_eq!(values, &array(&o[field]), "{name}/{field}");
        }
        errors.real(
            "initial cost",
            &[t.solver.initial_cost],
            &[number(&o["f0_cost"])],
        );
        if !o["J0"].is_null() {
            assert_eq!(
                t.solver.j0_shape,
                [integer(&o["J0_shape"][0]), integer(&o["J0_shape"][1])]
            );
            // Exact residual arithmetic, not an optimizer-path tolerance.
            assert_eq!(t.solver.j0, array(&o["J0"]), "{name}/J0");
        }
        let n = integer(&o["params"]);
        let expected = array(&o["main_x"]);
        assert_eq!(expected.len() % n, 0);
        let count = expected.len() / n;
        let horizon = o["horizon"].as_u64().map(|v| v as usize);
        let gated = horizon.map_or(count, |h| h.saturating_sub(1));
        assert!(gated <= count, "{name}: invalid horizon");
        if t.solver.main_x.len() < gated {
            failures.push(format!(
                "{name}: only {} evaluations, need {gated}",
                t.solver.main_x.len()
            ));
        }
        let mut first = None;
        let mut max_prefix = 0_f64;
        for (k, (a, b)) in t
            .solver
            .main_x
            .iter()
            .zip(expected.chunks_exact(n))
            .enumerate()
        {
            assert_eq!(a.len(), n);
            assert!(a.iter().all(|v| v.is_finite()));
            let distance = a
                .iter()
                .zip(b)
                .map(|(a, b)| (a - b).abs())
                .fold(0., f64::max)
                / b.iter()
                    .map(|b| b.abs())
                    .fold(0., f64::max)
                    .max(f64::MIN_POSITIVE);
            if distance > REL && first.is_none() {
                first = Some(k);
            }
            if k < gated {
                max_prefix = max_prefix.max(distance);
                if distance > REL {
                    failures.push(format!(
                        "{name}: evaluation {k} distance {distance:e}, horizon {horizon:?}"
                    ));
                }
            }
        }
        if first.is_none() && t.solver.main_x.len() != count {
            first = Some(t.solver.main_x.len().min(count));
        }
        if horizon.is_none() {
            assert_eq!(
                t.solver.main_x.len(),
                count,
                "{name}: horizon-free trace count"
            );
        }
        println!(
            "MEASURE {name}/prefix: max_relative={max_prefix:.17e}, evaluations={gated}, budget={REL:e}"
        );
        println!(
            "FIT {name}: first={first:?} horizon={horizon:?} cost={:.17e} reference={:.17e} variants={} nfev={}/{}",
            t.result.cost,
            number(&o["result"]["cost"]),
            serde_json::to_string(
                &o["oracle_variants"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k, &v["cost"]))
                    .collect::<std::collections::BTreeMap<_, _>>()
            )
            .unwrap(),
            t.result.nfev,
            o["result"]["nfev"]
        );
        assert_eq!(t.solver.main_x.len() + 1, t.result.nfev);
        assert_eq!(t.solver.jac_x.len() + 1, t.result.njev);
        if std::panic::catch_unwind(|| errors.finish(&name)).is_err() {
            failures.push(name);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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
