#![forbid(unsafe_code)]
//! A07b: primitive-only clean-room gates. No fitting or RC is run here.
mod golden_secs2_helpers;
use golden_secs2_helpers::*;
use impulcifer_dsp::{
    fft, filters,
    stages::room_tuning2::{peq, primitives as p},
};
use std::f64::consts::PI;

#[test]
fn frac_for() {
    let v = fixture("frac_for");
    let mut e = Errors::default();
    e.real(
        "frac_for",
        &p::resolution(&array(&v["inputs"]["f"])),
        &array(&v["outputs"]["frac"]),
    );
    e.finish("frac_for");
}
#[test]
fn smooth() {
    let mut e = Errors::default();
    for (name, v) in fixtures("smooth_") {
        let i = &v["inputs"];
        let f = array(&i["f"]);
        let b = if i["frac"].as_str() == Some("frac_for") {
            p::resolution(&f)
        } else {
            vec![number(&i["frac"])]
        };
        if name.contains("complex") {
            e.complex(
                &name,
                &p::smooth_complex(&complex(&i["x"]), &f, &b),
                &complex(&v["outputs"]["y"]),
            );
        } else {
            e.real(
                &name,
                &p::smooth(&array(&i["x"]), &f, &b),
                &array(&v["outputs"]["y"]),
            );
        }
    }
    e.finish("smooth");
}
fn sampled(h: Vec<fft::Complex64>) -> Vec<fft::Complex64> {
    fixture("bins")["outputs"]["bins"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| h[integer(k)])
        .collect()
}
#[test]
fn minimum_phase() {
    let mut e = Errors::default();
    for (name, v) in fixtures("minimum_phase_") {
        let a = if v["inputs"]["mag"].is_string() {
            (0..=65536)
                .map(|k| 1. + 0.5 * (k as f64 / 300.).cos())
                .collect()
        } else {
            array(&v["inputs"]["mag"])
        };
        let h = p::mp(&a, 2 * (a.len() - 1));
        let h = if a.len() == 65537 { sampled(h) } else { h };
        let expected = complex(&v["outputs"]["h"]);
        assert_eq!(h.len(), expected.len());
        let db: Vec<_> = h.iter().map(|x| 20. * x.norm().log10()).collect();
        let ref_db: Vec<_> = expected.iter().map(|x| 20. * x.norm().log10()).collect();
        e.compare(&format!("{name}/db"), &db, &ref_db, 0., 1e-9);
        let phase: Vec<_> = h
            .iter()
            .zip(&expected)
            .filter(|(_, e)| 20. * e.norm().log10() > -100.)
            .map(|(g, e)| (g.arg() - e.arg() + PI).rem_euclid(2. * PI) - PI)
            .collect();
        e.compare(
            &format!("{name}/phase"),
            &phase,
            &vec![0.; phase.len()],
            0.,
            1e-9,
        );
    }
    e.finish("minimum_phase_db_rad");
}
#[test]
fn peaking_peq() {
    let mut e = Errors::default();
    for (name, v) in fixtures("peaking_")
        .into_iter()
        .chain(fixtures("peq_response"))
    {
        let i = &v["inputs"];
        let f = if i.get("f").is_some() {
            array(&i["f"])
        } else {
            p::grid(131072, 48000.)
        };
        let b = if i.get("bands").is_some() {
            bands(&i["bands"])
        } else {
            vec![(number(&i["f0"]), number(&i["q"]), number(&i["gain_db"]))]
        };
        let h = peq::response(&f, &b, 48000.);
        let h = if f.len() == 65537 { sampled(h) } else { h };
        e.complex(&name, &h, &complex(&v["outputs"]["h"]));
    }
    e.finish("peaking_peq");
}
#[test]
fn ring_caps_boxes() {
    let v = fixture("ring_q_cap");
    let mut e = Errors::default();
    for (i, row) in v["outputs"]["caps"].as_array().unwrap().iter().enumerate() {
        let r = array(row);
        e.real(
            &format!("cap/{i}"),
            &[peq::ring_q_cap(r[0], r[1], number(&v["inputs"]["ring_ms"]))],
            &[r[2]],
        );
    }
    let v = fixture("ring_boxes");
    let i = &v["inputs"];
    let mut lb = array(&i["lb"]);
    let mut ub = array(&i["ub"]);
    peq::ring_boxes(&bands(&i["bands"]), &mut lb, &mut ub, number(&i["ring_ms"]));
    e.real("lb", &lb, &array(&v["outputs"]["lb"]));
    e.real("ub", &ub, &array(&v["outputs"]["ub"]));
    e.finish("ring_caps_boxes");
}
#[test]
fn onset_window() {
    let v = fixture("onset_window");
    let x = array(&v["inputs"]["ir"]);
    let o = p::onset(&x);
    let a = o.saturating_sub(96);
    let mut w = x[a..(a + 24000).min(x.len())].to_vec();
    p::taper(&mut w, 0.2);
    let mut e = Errors::default();
    e.compare(
        "onset/a",
        &[o as f64, a as f64],
        &[number(&v["outputs"]["onset"]), number(&v["outputs"]["a"])],
        0.,
        0.,
    );
    e.real("window", &w, &array(&v["outputs"]["w"]));
    e.finish("onset_window");
}
#[test]
fn to_low() {
    let mut e = Errors::default();
    for (name, v) in fixtures("to_low_") {
        let i = &v["inputs"];
        let n = integer(&i["grid_n"]);
        let x = array(&i["ir"]);
        assert_eq!(x.len(), n);
        e.real(
            &name,
            &p::low_rate(&p::rfft(&x, n), &p::grid(n, 48000.), integer(&i["n"])),
            &array(&v["outputs"]["x"]),
        );
    }
    e.finish("to_low");
}
#[test]
fn late_peaks() {
    let v = fixture("late_peaks");
    let xs: Vec<_> = v["inputs"]["xs"]
        .as_array()
        .unwrap()
        .iter()
        .map(array)
        .collect();
    let mut e = Errors::default();
    // The eight-peak long-mode probe uses the first position only; the
    // count-peak support probe pools both. Independently checked with SciPy.
    for (key, n, signals) in [
        ("peaks", integer(&v["inputs"]["count"]), xs.as_slice()),
        ("peaks8", 8, &xs[..1]),
    ] {
        e.compare(
            key,
            &p::late_peaks(signals, n),
            &array(&v["outputs"][key]),
            0.,
            0.,
        );
    }
    e.finish("late_peaks");
}
#[test]
fn late_peaks_include_band_edges() {
    // SciPy 1.18.1 reference (2026-10-12): x=zeros(4096); x[0]=10;
    // x[start:start+len(taps)]=taps; x[:90]=0; f=rfftfreq(4096, 1/600);
    // power=abs(rfft(x))**2; find_peaks(power*((f>=20)&(f<=160)))[0]
    // returns [1092], [137], [1024], [614] for the four cases below.
    // The two-tap powers are 2-2*cos(w), 2+2*cos(w), 2-2*cos(2*w):
    // respectively rising, falling, and an interior maximum at 150 Hz.
    // An impulse at 2048 gives exactly unit power and tests the even plateau's
    // rounded-down midpoint: (137+1092)/2 -> 614, not either band edge.
    let mut e = Errors::default();
    for (name, taps, start, bin) in [
        ("rising", &[1., -1.][..], 128, 1092),
        ("falling", &[1., 1.][..], 128, 137),
        ("interior", &[1., 0., -1.][..], 128, 1024),
        ("plateau", &[1.][..], 2048, 614),
    ] {
        let mut x = vec![0.; 4096];
        x[0] = 10.;
        x[start..start + taps.len()].copy_from_slice(taps);
        let xs = [x];
        let expected = [bin as f64 * 600. / 4096.];
        assert_eq!(p::late_peaks(&xs, 0), Vec::<f64>::new());
        assert_eq!(p::late_peaks(&xs, 1), expected, "{name}");
        e.compare(name, &p::late_peaks(&xs, 8), &expected, 0., 0.);
    }
    assert!(p::late_peaks(&[vec![0.; 4096]], 8).is_empty());
    assert!(p::late_peaks(&[], 8).is_empty());
    e.finish("late_peaks_include_band_edges");
}
#[test]
fn mode_t60() {
    let v = fixture("mode_t60");
    let x = array(&v["inputs"]["x"]);
    let mut e = Errors::default();
    for (i, c) in v["inputs"]["cases"].as_array().unwrap().iter().enumerate() {
        let c = array(c);
        let got = p::mode_decay(&x, c[0], c[1]);
        let r = &v["outputs"]["t60"][i];
        if r.is_null() {
            assert!(got.is_none(), "case {i}: expected None");
        } else {
            e.real(
                &format!("t60/{i}"),
                &[got.expect("finite T60")],
                &[number(r)],
            );
        }
    }
    e.finish("mode_t60");
}
#[test]
fn firwin_hp4() {
    let mut e = Errors::default();
    for (name, v) in fixtures("firwin_") {
        let i = &v["inputs"];
        e.real(
            &name,
            &p::firwin(integer(&i["taps"]), number(&i["cutoff"]), number(&i["fs"])),
            &array(&v["outputs"]["h"]),
        );
    }
    let v = fixture("hp4");
    e.complex(
        "hp4",
        &p::hp4(&array(&v["inputs"]["f"]), number(&v["inputs"]["fc"])),
        &complex(&v["outputs"]["h"]),
    );
    e.finish("firwin_hp4");
}
#[test]
fn butter_band() {
    let mut e = Errors::default();
    for (name, v) in fixtures("butter_band_") {
        let i = &v["inputs"];
        let b = array(&i["band"]);
        let fs = number(&i["fs"]);
        let sos =
            filters::butter_bandpass(integer(&i["order"]), b[0] / (fs / 2.), b[1] / (fs / 2.));
        let reference: Vec<_> = v["outputs"]["sos"]
            .as_array()
            .unwrap()
            .iter()
            .map(array)
            .collect();
        assert_eq!(sos.0.len(), reference.len());
        for (k, (g, r)) in sos.0.iter().zip(reference).enumerate() {
            e.real(&format!("{name}/sos/{k}"), g, &r);
        }
        if v["outputs"].get("filtered").is_some() {
            let x = array(&fixture("mode_t60")["inputs"]["x"]);
            e.real(
                &format!("{name}/filtered"),
                &filters::sosfilt(&sos, &x),
                &array(&v["outputs"]["filtered"]),
            );
        }
    }
    e.finish("butter_band");
}
#[test]
fn gd_curve() {
    let mut e = Errors::default();
    for (name, v) in fixtures("gd_curve_") {
        let i = &v["inputs"];
        let ir = array(&i["ir"]);
        e.real(
            &name,
            &p::gd_curve(
                &fft::rfft(&ir),
                &p::grid(ir.len(), 48000.),
                &array(&i["g"]),
                number(&i["frac"]),
                48000.,
            ),
            &array(&v["outputs"]["gd_ms"]),
        );
    }
    e.finish("gd_curve");
}
