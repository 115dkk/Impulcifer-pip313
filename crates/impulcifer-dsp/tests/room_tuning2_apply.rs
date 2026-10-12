#![forbid(unsafe_code)]
use impulcifer_dsp::{
    estimator::SweepEstimator,
    fft,
    hrir::Hrir,
    stages::{room, room_tuning2::*},
};
mod golden_secs2_helpers;
use impulcifer_io::read_wav;
use impulcifer_types::constants::IPSILATERAL_PAIRS;
use std::{path::PathBuf, sync::OnceLock};
fn demo() -> (Hrir, Hrir) {
    let (omni, brir, _) = demo_with_pairs();
    (omni, brir)
}
fn demo_with_pairs() -> (
    Hrir,
    Hrir,
    Vec<impulcifer_dsp::stages::room_tuning::PairMeasurement>,
) {
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
    let mut pairs = vec![];
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
        pairs.extend(impulcifer_dsp::stages::room_tuning::recording_pairs(
            &omni,
            &parsed.speakers,
            parsed.side,
        ));
    }
    brir.crop_heads(1.).unwrap();
    brir.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.);
    brir.align_onset_groups_peak_leftref(None).unwrap();
    brir.crop_tails(&estimator).unwrap();
    (omni, brir, pairs)
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
// provisional: set by the owner after review (A08c measured maxima × 1.5).
const SUPPORT_REL_L2_BOUND: f64 = 0.997751334 * 1.5;
const SUPPORT_MAX_ABS_BOUND: f64 = 0.049634475 * 1.5;
const LEVEL_DB_BOUND: f64 = 6.000058928 * 1.5;
const LATE_BAND_DB_BOUND: f64 = 8.287972908 * 1.5;
const LATE_TAIL_BOUND: f64 = 0.169831498 * 1.5;
// With band-edge peaks fixed, A08d measures 1/2/4 changed inputs versus the
// worst oracle variants' 2/3/2. Two is the smallest end-to-end allowance;
// frozen-RC guards and supports have no exceptions (85 guard values at 1e-9).
const GUARD_STEP_ALLOWANCE: usize = 2;
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
        let design = mimo::design_mimo_observed(
            &brir,
            &inputs,
            &MimoOptions {
                fsiso: entry["fsiso"].as_f64().unwrap(),
                low: entry["f_low"].as_f64(),
            },
            &|speaker, stages| {
                if setting.starts_with("mixed/") {
                    use golden_secs2_helpers::{fixture,array,Errors};
                    let v=fixture(&format!("mimo_{}_{}",setting.replace('/',"_"),speaker));
                    let o=&v["outputs"];
                    let mut paths=Errors::default();
                    for (e,g) in stages.g.iter().enumerate() {
                        let got:Vec<_>=g.iter().flatten().copied().collect();
                        // Fixture candidates use canonical speaker order, not recording order.
                        let names:Vec<_>=["FL","FR","FC","BL","BR","SL","SR"].into_iter().filter(|s| *s!=speaker).collect();
                        let source:Vec<_>=brir.speakers.iter().filter(|s|s.speaker!=speaker).collect();
                        let got:Vec<_>=names.iter().flat_map(|name| {let i=source.iter().position(|s|s.speaker==*name).unwrap();got[i*720..(i+1)*720].to_vec()}).collect();
                        paths.compare(&format!("{setting}/{speaker}/g{e}"),&got,&array(&o["g"][e]),1e-9,1e-12);
                    }
                    paths.compare(&format!("{setting}/{speaker}/lp"),&stages.lp,&array(&o["lp"]),0.,1e-12);
                    paths.finish(&format!("MIMO paths {setting}/{speaker}"));
                    let x:Vec<_>=stages.x.iter().flatten().copied().collect();
                    let expected:Vec<_>=o["x"].as_array().unwrap().iter().flat_map(array).collect();
                    let rel=(x.iter().zip(&expected).map(|(a,b)|(a-b).powi(2)).sum::<f64>()/expected.iter().map(|v|v*v).sum::<f64>()).sqrt();
                    println!("RC_ATTRIBUTION {setting}/{speaker} path_x_relative={rel:e} direct_relative={:e}",(stages.direct-o["direct"].as_f64().unwrap()).abs()/o["direct"].as_f64().unwrap());
                    // Full MIMO RC spectra are not exported. Substitute their exact sufficient
                    // statistics (x and direct) at the frozen RC/path boundary instead.
                    let candidates:Vec<_>=brir.speakers.iter().filter(|s|s.speaker!=speaker).map(|s|(s.speaker.clone(),entry["ranges"][&s.speaker][0].as_f64().unwrap())).collect();
                    let (sub,_)=mimo::design_paths(speaker,&candidates,o["x"].as_array().unwrap().iter().map(array).collect(),stages.g.clone(),stages.lp.clone(),o["direct"].as_f64().unwrap(),entry["fsiso"].as_f64().unwrap()).unwrap();
                    let mut error=0.;let mut norm=0.;let mut max:f64=0.;
                    for s in &sub.supports {for (a,b) in s.fir.iter().zip(array(&o["supports"][&s.speaker])) {error+=(a-b).powi(2);norm+=b*b;max=max.max((a-b).abs());}}
                    let remaining=if norm>0. {(error/norm).sqrt()} else {0.};
                    println!("RC_ATTRIBUTION {setting}/{speaker} frozen_rc_remaining_l2={remaining:e} max={max:e} level_delta={:e}",sub.level_db-o["level_db"].as_f64().unwrap());
                    assert!(remaining<1e-9 && max<if golden_secs2_helpers::oracle_libm() {1e-12} else {1e-9});
                }
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
            assert!(
                rel <= SUPPORT_REL_L2_BOUND
                    && max <= SUPPORT_MAX_ABS_BOUND
                    && delta.abs() <= LEVEL_DB_BOUND
            );
            if let Some(band) = entry["late"][&i.speaker]["worst_band_db"].as_f64() {
                assert!((i.band_change_db - band).abs() <= LATE_BAND_DB_BOUND);
                assert!(
                    (i.tail_change_db * 0.15
                        - entry["late"][&i.speaker]["tail_change"].as_f64().unwrap())
                    .abs()
                        <= LATE_TAIL_BOUND
                );
            }
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
            .iter()
            .map(|(variant, rows)| {
                let count = rows
                    .as_object()
                    .unwrap()
                    .iter()
                    .filter(|(k, v)| {
                        k.starts_with(&format!("{setting}/"))
                            && (v["level_db"][1].as_f64().unwrap()
                                - v["level_db"][0].as_f64().unwrap())
                            .abs()
                                > 1.
                    })
                    .count();
                println!(
                    "MEASURE MIMO guard steps {setting} variant={variant} changed_steps={count}"
                );
                count
            })
            .max()
            .unwrap();
        println!(
            "MEASURE MIMO guard steps {setting} changed_steps={changed} worst_variant={worst}"
        );
        // The MIMO stage itself is exact on the oracle's own RC outputs (the
        // frozen-boundary check above, 1e-9). End to end, our RC is one more
        // computer, and the guard's tail metric is discontinuous (a top-six peak
        // set), so a different RC moves some near-threshold inputs by one step,
        // as the oracle's variants do (1-3 per setting, 6 of 21 at worst).
        if changed > worst + GUARD_STEP_ALLOWANCE {
            failures.push(format!(
                "{setting}: {changed}>{worst}+{GUARD_STEP_ALLOWANCE}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("; "));
}

#[test]
fn room_tuning2_observer() {
    use impulcifer_dsp::{
        DspError,
        pipeline::{self, PipelineInputs, StageObserver, StageProgress},
    };
    use impulcifer_types::config::ProcessingConfig;
    struct Observer(Vec<Tuning2Report>);
    impl StageObserver for Observer {
        fn on_stage(&mut self, _: StageProgress) {}
        fn check_cancelled(&self) -> Result<(), DspError> {
            Ok(())
        }
        fn on_room_tuned2(&mut self, r: &Tuning2Report) -> Result<(), DspError> {
            self.0.push(r.clone());
            Ok(())
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wav = read_wav(&root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")).unwrap();
    let estimator = SweepEstimator::from_samples(wav.sample_rate, &wav.tracks[0]).unwrap();
    let mut hrir = Hrir {
        fs: 48000,
        speakers: vec![],
    };
    let wav = read_wav(&root.join("data/demo/FL,FR.wav")).unwrap();
    hrir.open_recording_samples(
        &estimator,
        wav.sample_rate,
        &wav.tracks,
        &["FL", "FR"],
        None,
        2.,
    )
    .unwrap();
    for (mode, enabled, count) in [
        ("tuning2", true, 1),
        ("tuning", true, 0),
        ("eq", true, 0),
        ("tuning2", false, 0),
    ] {
        let config = ProcessingConfig {
            room_mode: mode.into(),
            do_room_correction: enabled,
            do_headphone_compensation: false,
            ..Default::default()
        };
        let inputs = PipelineInputs {
            estimator: estimator.clone(),
            hrir: hrir.clone(),
            vbass: None,
            headphone: None,
            eq_left: None,
            eq_right: None,
            room: Some(room::RoomCorrection {
                frs: room::RoomFrs {
                    term: room::RoomTerm::Gain,
                    entries: vec![],
                },
                responses_tracks: vec![],
                diagnostics: None,
                tuning: None,
                tuning2: Some(cached().1.clone()),
            }),
        };
        let mut observer = Observer(vec![]);
        pipeline::run_pipeline(&config, inputs, &mut observer).unwrap();
        assert_eq!(observer.0.len(), count);
        if count == 1 {
            assert_eq!(observer.0[0].speakers.len(), 2);
            assert_eq!(observer.0[0].latency_samples, 384);
        }
    }
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
fn room_tuning2_geometry() {
    let (_, brir, pairs) = demo_with_pairs();
    let mut plan = cached().1.clone();
    plan.options.pairs = pairs;
    let report = apply_tuning2(&mut brir.clone(), &plan).unwrap();
    assert_eq!(report.pair_checks.len(), 2);
    assert!(report.pair_checks.iter().all(|p| !p.mismatch));
    assert!(report.speakers.iter().all(|s| !s.mismatch));
    let p = &mut plan.options.pairs[0];
    p.irs[0].data = vec![0.; 1000];
    p.irs[0].data[800] = 1.;
    p.irs[1].data = vec![0.; 1000];
    p.irs[1].data[100] = 1.;
    let report = apply_tuning2(&mut brir.clone(), &plan).unwrap();
    assert!(report.pair_checks[0].mismatch);
    assert!(
        report
            .speakers
            .iter()
            .filter(|s| ["FL", "FR"].contains(&s.speaker.as_str()))
            .all(|s| s.mismatch)
    );
}
#[test]
fn room_tuning2_zero_latency() {
    let (mut omni, brir) = demo();
    omni.speakers.truncate(1);
    let mut opts = options();
    opts.rc.mode = Tuning2Mode::Zero;
    let plan = prepare_tuning2(&omni, &opts, None, None).unwrap();
    let mut tuned = brir.clone();
    let report = apply_tuning2(&mut tuned, &plan).unwrap();
    assert_eq!(report.latency_samples, 0);
    for (a, b) in tuned.speakers.iter().zip(&brir.speakers) {
        for (a, b) in a
            .left
            .iter()
            .chain(&a.right)
            .zip(b.left.iter().chain(&b.right))
        {
            assert_eq!(a.len(), b.len());
        }
    }
    for (a, b) in tuned.speakers.iter().zip(&brir.speakers).skip(1) {
        assert_eq!(a.left.as_ref().unwrap().data, b.left.as_ref().unwrap().data);
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
    use impulcifer_dsp::conv::{Mode, convolve};
    let (brir, plan) = cached();
    let mut rc = brir.clone();
    apply_tuning2_full(&mut rc, plan).unwrap();
    let mut plan = plan.clone();
    plan.options.mimo = Some(MimoOptions {
        fsiso: 150.,
        low: None,
    });
    let mut tuned = brir.clone();
    let report = apply_tuning2_full(&mut tuned, &plan).unwrap();
    let mut checked = 0;
    let mut failures = vec![];
    for input in &report.mimo.unwrap().inputs {
        if input.supports.is_empty() {
            continue;
        }
        for right in [false, true] {
            let ear = |h: &Hrir, name: &str| {
                let s = h.get(name).unwrap();
                if right {
                    s.right.as_ref().unwrap().data.clone()
                } else {
                    s.left.as_ref().unwrap().data.clone()
                }
            };
            let a = ear(&tuned, &input.speaker);
            let b = ear(&rc, &input.speaker);
            let delta: Vec<_> = a.iter().zip(b).map(|(a, b)| a - b).collect();
            let mut sum = vec![0.; delta.len()];
            for s in &input.supports {
                let q = fft::irfft(
                    &support_response(s, brir.fs, analysis_len(brir.fs)).unwrap(),
                    analysis_len(brir.fs),
                );
                let y = convolve(&ear(brir, &s.speaker), &q, Mode::Full);
                assert_eq!(sum.len(), y.len());
                for (a, b) in sum.iter_mut().zip(y) {
                    *a += b;
                }
            }
            let rel = (delta
                .iter()
                .zip(&sum)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                / sum.iter().map(|v| v * v).sum::<f64>())
            .sqrt();
            let n = delta.len().next_power_of_two();
            let spectrum = primitives::rfft(&delta, n);
            let (mut inside, mut outside) = (0., 0.);
            for (k, v) in spectrum.iter().enumerate() {
                if k as f64 * brir.fs as f64 / n as f64 > 300. {
                    outside += v.norm_sqr();
                } else {
                    inside += v.norm_sqr();
                }
            }
            let leakage = 10. * (outside / inside).log10();
            println!(
                "MEASURE full_support {}/right={right} sum_relative={rel:e} leakage_db={leakage}",
                input.speaker
            );
            assert!(rel < 1e-9);
            // The supports themselves stop at 300 Hz; what remains above it is
            // the cut where the summed BRIR ends (measured -78 to -71 dB on the
            // demo, gone when the ends are faded). -65 dB keeps that inaudible.
            if leakage >= -65. {
                failures.push(format!("{}/right={right}: {leakage} dB", input.speaker));
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 14);
    assert!(failures.is_empty(), "{}", failures.join("; "));
}
