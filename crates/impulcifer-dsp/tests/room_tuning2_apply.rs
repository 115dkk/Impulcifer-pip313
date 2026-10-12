use impulcifer_dsp::{
    estimator::SweepEstimator,
    fft,
    hrir::Hrir,
    stages::{room, room_tuning2::*},
};
use impulcifer_io::read_wav;
use impulcifer_types::constants::IPSILATERAL_PAIRS;
use std::{path::PathBuf, sync::OnceLock};
fn demo() -> (Hrir, Hrir) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wav = read_wav(&root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")).unwrap();
    let estimator = SweepEstimator::from_samples(wav.sample_rate, &wav.tracks[0]).unwrap();
    let mut omni = Hrir {
        fs: 48000,
        speakers: vec![],
    };
    let mut brir = omni.clone();
    for file in ["FL,FR.wav", "FC.wav", "BL,SL.wav", "SR,BR.wav"] {
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let names: Vec<_> = file.strip_suffix(".wav").unwrap().split(',').collect();
        brir.open_recording_samples(&estimator, wav.sample_rate, &wav.tracks, &names, None, 2.)
            .unwrap();
    }
    for file in [
        "room-FL,FR-left.wav",
        "room-FL,FR-right.wav",
        "room-FC-left.wav",
        "room-FC-right.wav",
        "room-BL,SL-left.wav",
        "room-BL,SL-right.wav",
        "room-SR,BR-left.wav",
        "room-SR,BR-right.wav",
    ] {
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let parsed = room::parse_room_measurement_name(file).unwrap();
        let names: Vec<_> = parsed.speakers.iter().map(String::as_str).collect();
        omni.open_recording_samples(
            &estimator,
            wav.sample_rate,
            &wav.tracks,
            &names,
            parsed.side,
            2.,
        )
        .unwrap();
    }
    brir.crop_heads(1.).unwrap();
    brir.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.);
    brir.align_onset_groups_peak_leftref(None).unwrap();
    brir.crop_tails(&estimator).unwrap();
    (omni, brir)
}
fn options() -> Tuning2Options {
    Tuning2Options {
        rc: RcOptions::default(),
        level_match: true,
        mimo: None,
        pairs: vec![],
    }
}
fn cached() -> &'static (Hrir, Tuning2Plan) {
    static CACHE: OnceLock<(Hrir, Tuning2Plan)> = OnceLock::new();
    CACHE.get_or_init(|| {
        let (omni, brir) = demo();
        let plan = prepare_tuning2(&omni, &options(), None, None).unwrap();
        (brir, plan)
    })
}
#[test]
fn room_tuning2_matches_secs2_mimo() {
    use rayon::prelude::*;
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/room_tuning_secs2_mimo.json"
    ))
    .unwrap();
    let variance: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/migration/goldens/room_tuning_secs2_variance.json"
    ))
    .unwrap();
    let (_, brir) = demo();
    let mut failures = vec![];
    for (setting, entry) in reference["entries"].as_object().unwrap() {
        let rc = RcOptions {
            mode: Tuning2Mode::parse(entry["rc_mode"].as_str().unwrap()).unwrap(),
            policy: PositionPolicy::Listening {
                index: 0,
                weight: 1.,
            },
            ..Default::default()
        };
        let points: Vec<Vec<_>> = brir
            .speakers
            .iter()
            .map(|s| s.left.iter().chain(&s.right).cloned().collect())
            .collect();
        let band =
            correction_band(&points.iter().map(Vec::as_slice).collect::<Vec<_>>(), &rc).unwrap();
        let designs: Vec<_> = points
            .par_iter()
            .map(|p| design_rc(p, &rc, &band, None, None).unwrap())
            .collect();
        let min = designs
            .iter()
            .map(|s| s.level_db)
            .fold(f64::INFINITY, f64::min);
        let inputs: Vec<_> = brir
            .speakers
            .iter()
            .zip(&designs)
            .map(|(s, rc)| MimoInput {
                speaker: &s.speaker,
                gain_db: min - rc.level_db,
                response: &rc.response,
            })
            .collect();
        for i in &inputs {
            assert!((i.gain_db - entry["gain_db"][i.speaker].as_f64().unwrap()).abs() < 1e-10);
        }
        let start = std::time::Instant::now();
        let design = design_mimo(
            &brir,
            &inputs,
            &MimoOptions {
                fsiso: entry["fsiso"].as_f64().unwrap(),
                low: entry["f_low"].as_f64(),
            },
        )
        .unwrap();
        println!(
            "REFERENCE {setting} design_seconds={}",
            start.elapsed().as_secs_f64()
        );
        for (name, range) in &design.ranges {
            let expect = &entry["ranges"][name];
            assert_eq!(
                *range,
                if expect.is_null() {
                    None
                } else {
                    Some((expect[0].as_f64().unwrap(), expect[1].as_f64().unwrap()))
                },
                "{setting}/{name}"
            );
        }
        let mut changed = 0;
        for i in &design.inputs {
            let expected = entry["level_db"][&i.speaker].as_f64().unwrap();
            let delta = i.level_db - expected;
            if delta.abs() > 1. {
                changed += 1;
            }
            let mut error = 0.;
            let mut energy = 0.;
            let mut max: f64 = 0.;
            for (name, fir) in entry["supports"][&i.speaker].as_object().unwrap() {
                let actual = i.supports.iter().find(|s| s.speaker == *name);
                for (k, v) in fir.as_array().unwrap().iter().enumerate() {
                    let v = v.as_f64().unwrap();
                    let d = actual.map_or(0., |s| s.fir[k]) - v;
                    error += d * d;
                    energy += v * v;
                    max = max.max(d.abs());
                }
            }
            let rel = if energy > 0. {
                (error / energy).sqrt()
            } else {
                0.
            };
            println!(
                "REFERENCE {setting}/{} level_delta={delta:.9} support_l2={rel:.9} max_abs={max:e} scale={:?} late={:.9}/{:.9} expected_late={}",
                i.speaker,
                i.scale_db,
                i.band_change_db,
                0.15 * i.tail_change_db,
                entry["late"][&i.speaker]
            );
            for (variant, rows) in variance["mimo"].as_object().unwrap() {
                let row = &rows[format!("{setting}/{}", i.speaker)];
                println!(
                    "VARIANT {variant} {setting}/{} level_delta={} support_l2={}",
                    i.speaker,
                    row["level_db"][1].as_f64().unwrap() - row["level_db"][0].as_f64().unwrap(),
                    row["support_rel_l2"]
                );
            }
        }
        let worst = variance["mimo"]
            .as_object()
            .unwrap()
            .values()
            .map(|rows| {
                rows.as_object()
                    .unwrap()
                    .iter()
                    .filter(|(k, v)| {
                        k.starts_with(&format!("{setting}/"))
                            && (v["level_db"][1].as_f64().unwrap()
                                - v["level_db"][0].as_f64().unwrap())
                            .abs()
                                > 1.
                    })
                    .count()
            })
            .max()
            .unwrap();
        println!("REFERENCE {setting} changed_steps={changed} worst_variant={worst}");
        if changed > worst {
            failures.push(format!("{setting}: {changed}>{worst}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn room_tuning2_mimo_reduces_late_energy_at_the_ears() {
    let (brir, plan) = cached();
    let mut rc = brir.clone();
    apply_tuning2(&mut rc, plan).unwrap();
    let mut p = plan.clone();
    p.options.mimo = Some(MimoOptions {
        fsiso: 150.,
        low: None,
    });
    let mut tuned = brir.clone();
    let report = apply_tuning2(&mut tuned, &p).unwrap();
    for input in &report.mimo.unwrap().inputs {
        if input.supports.is_empty() {
            continue;
        }
        let before = rc.get(&input.speaker).unwrap();
        let after = tuned.get(&input.speaker).unwrap();
        let mut eb = 0.;
        let mut ea = 0.;
        for (b, a) in before
            .left
            .iter()
            .chain(&before.right)
            .zip(after.left.iter().chain(&after.right))
        {
            let start = (primitives::onset(&b.data) + 2880).min(b.len());
            let mut b = b.data.clone();
            let mut a = a.data.clone();
            b[..start].fill(0.);
            a[..start].fill(0.);
            let b = primitives::rfft(&b, 131072);
            let a = primitives::rfft(&a, 131072);
            for (k, (b, a)) in b.iter().zip(a).enumerate() {
                let f = k as f64 * 48000. / 131072.;
                if (20. ..=150.).contains(&f) {
                    eb += b.norm_sqr();
                    ea += a.norm_sqr();
                }
            }
        }
        let db = 10. * (ea / eb).log10();
        println!("LATE {} {db:.6} dB", input.speaker);
        assert!(db < 0.);
    }
}

#[test]
fn room_tuning2_no_data_changes_nothing() {
    let empty = Hrir {
        fs: 48000,
        speakers: vec![],
    };
    let plan = prepare_tuning2(&empty, &options(), None, None).unwrap();
    let (_, mut brir) = demo();
    let before = brir.clone();
    let report = apply_tuning2(&mut brir, &plan).unwrap();
    assert_eq!(report.latency_samples, 0);
    for (a, b) in brir.speakers.iter().zip(&before.speakers) {
        assert_eq!(a.left.as_ref().unwrap().data, b.left.as_ref().unwrap().data);
        assert_eq!(
            a.right.as_ref().unwrap().data,
            b.right.as_ref().unwrap().data
        );
    }
}
#[test]
fn room_tuning2_one_filter_per_speaker() {
    let (brir, plan) = cached();
    for s in &plan.speakers {
        let ears = brir.get(&s.speaker).unwrap();
        let l = &ears.left.as_ref().unwrap().data;
        let r = &ears.right.as_ref().unwrap().data;
        let f = fft::irfft(&s.rc.response, analysis_len(brir.fs));
        let yl = impulcifer_dsp::conv::convolve(l, &f, impulcifer_dsp::conv::Mode::Full);
        let yr = impulcifer_dsp::conv::convolve(r, &f, impulcifer_dsp::conv::Mode::Full);
        let n = (yl.len() + r.len()).next_power_of_two();
        let a = primitives::rfft(&yl, n);
        let b = primitives::rfft(&yr, n);
        let l = primitives::rfft(l, n);
        let r = primitives::rfft(r, n);
        let err = a
            .iter()
            .zip(&b)
            .zip(l.iter().zip(&r))
            .map(|((a, b), (l, r))| (a * r - b * l).norm_sqr())
            .sum::<f64>();
        let norm = a
            .iter()
            .zip(r)
            .map(|(a, r)| (a * r).norm_sqr())
            .sum::<f64>();
        assert!((err / norm).sqrt() < 1e-9);
    }
}
#[test]
fn room_tuning2_latency() {
    let (brir, plan) = cached();
    let mut plan = plan.clone();
    plan.speakers.truncate(1);
    let mut tuned = brir.clone();
    let report = apply_tuning2(&mut tuned, &plan).unwrap();
    assert_eq!(report.latency_samples, 384);
    for (a, b) in tuned.speakers.iter().zip(&brir.speakers) {
        for (a, b) in a
            .left
            .iter()
            .chain(&a.right)
            .zip(b.left.iter().chain(&b.right))
        {
            assert_eq!(a.len(), b.len() + 384);
        }
    }
    for s in tuned.speakers.iter().skip(1) {
        let a = &s.left.as_ref().unwrap().data;
        let b = &brir.get(&s.speaker).unwrap().left.as_ref().unwrap().data;
        assert!(a[..384].iter().all(|v| *v == 0.));
        assert_eq!(&a[384..], b);
    }
}
#[test]
fn room_tuning2_level_rules() {
    let (brir, plan) = cached();
    let mut h = brir.clone();
    let report = apply_tuning2(&mut h, plan).unwrap();
    let min = report
        .speakers
        .iter()
        .filter(|s| s.trim_source == "omni")
        .map(|s| s.level_db)
        .fold(f64::INFINITY, f64::min);
    for s in &report.speakers {
        if s.trim_source == "omni" {
            assert_eq!(s.trim_db, min - s.level_db);
        } else {
            assert_eq!(s.trim_db, 0.);
        }
    }
    let mut plan = plan.clone();
    plan.options.level_match = false;
    let r = apply_tuning2(&mut brir.clone(), &plan).unwrap();
    assert!(
        r.speakers
            .iter()
            .all(|s| s.trim_db == 0. && s.trim_source == "off")
    );
    plan.options.level_match = true;
    plan.speakers[0].rc.level_db += 20.;
    let r = apply_tuning2(&mut brir.clone(), &plan).unwrap();
    assert_eq!(r.speakers[0].trim_source, "skipped");
    assert_eq!(r.speakers[0].trim_db, 0.);
}
#[test]
fn room_tuning2_runs_on_the_demo_dsp() {
    let (brir, plan) = cached();
    for mimo in [
        None,
        Some(MimoOptions {
            fsiso: 150.,
            low: None,
        }),
    ] {
        let mut p = plan.clone();
        p.options.mimo = mimo;
        let mut h = brir.clone();
        let start = std::time::Instant::now();
        let r = apply_tuning2(&mut h, &p).unwrap();
        println!(
            "DEMO MIMO={} apply_seconds={}",
            r.mimo.is_some(),
            start.elapsed().as_secs_f64()
        );
        for s in r.speakers {
            println!(
                "DEMO {} level={:.6} trim={:.6} {} peq={} strength={} pre={:?} edt={:.3}/{:.3} gd={:.3}/{:.3} rms={:?} weak={}",
                s.speaker,
                s.level_db,
                s.trim_db,
                s.trim_source,
                s.peq_bands,
                s.phase_strength,
                s.pre_echo_channels,
                s.lf_edt_before_ms,
                s.lf_edt_after_ms,
                s.excess_median_before_ms,
                s.excess_median_after_ms,
                s.representation_rms_db,
                s.weak
            );
            assert!(!s.mismatch);
            assert!(!s.weak);
        }
        if let Some(m) = r.mimo {
            for i in m.inputs {
                println!(
                    "MIMO {} supports={} level={} scale={:?} band={} tail={}",
                    i.speaker,
                    i.supports.len(),
                    i.level_db,
                    i.scale_db,
                    i.band_change_db,
                    i.tail_change_db
                );
            }
        }
        assert!(
            h.speakers
                .iter()
                .flat_map(|s| s.left.iter().chain(&s.right))
                .flat_map(|ir| &ir.data)
                .all(|v| v.is_finite())
        );
    }
}
#[test]
fn room_tuning2_is_thread_count_free() {
    let (brir, plan) = cached();
    let mut plan = plan.clone();
    plan.options.mimo = Some(MimoOptions {
        fsiso: 150.,
        low: None,
    });
    let run = |threads| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                let mut h = brir.clone();
                apply_tuning2(&mut h, &plan).unwrap();
                h
            })
    };
    let a = run(1);
    let b = run(4);
    for (a, b) in a.speakers.iter().zip(b.speakers) {
        for (a, b) in a
            .left
            .iter()
            .chain(&a.right)
            .zip(b.left.iter().chain(&b.right))
        {
            assert_eq!(a.data, b.data);
        }
    }
}
#[test]
fn room_tuning2_supports_are_band_limited() {
    let support = MimoSupport {
        speaker: "FL".into(),
        low: 50.,
        fir: primitives::firwin(210, 150., 600.),
    };
    let response = support_response(&support, 48000, 131072);
    assert!(
        response
            .iter()
            .enumerate()
            .filter(|(k, _)| *k as f64 * 48000. / 131072. > 300.)
            .all(|(_, v)| v.norm_sqr() == 0.)
    );
}
