#![forbid(unsafe_code)]
mod golden_secs2_helpers;
use golden_secs2_helpers::*;
use impulcifer_dsp::stages::room_tuning2::mimo::design_paths;

#[test]
fn golden_secs2_mimo() {
    let mut normal = Errors::default();
    let mut solved = Errors::default();
    let mut guard = Errors::default();
    let mut guard_count = 0;
    let mut final_support = Errors::default();
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
        guard_count += s.guard_band.len();
        guard.compare(
            &format!("{name}/band"),
            &s.guard_band,
            &array(&o["guard_band_db"]),
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
        guard_count += tails.len();
        guard.compare(
            &format!("{name}/tail"),
            &tails,
            &array(&o["guard_tail"]),
            0.,
            1e-9,
        );
        let expected_supports = o["supports"].as_object().unwrap();
        assert_eq!(result.supports.len(), expected_supports.len());
        for support in &result.supports {
            let expected = array(&expected_supports[&support.speaker]);
            final_support.compare(
                &format!("{name}/support/{}", support.speaker),
                &support.fir,
                &expected,
                1e-9,
                1e-12,
            );
        }
        final_support.compare(
            &format!("{name}/level"),
            &[result.level_db],
            &[number(&o["level_db"])],
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
    assert_eq!(guard_count, 85, "every band and evaluated tail guard value");
    guard.finish("MIMO guard");
    final_support.finish("MIMO final supports and level");
}
