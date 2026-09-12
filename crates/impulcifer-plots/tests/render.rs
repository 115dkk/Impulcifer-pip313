#![forbid(unsafe_code)]
use impulcifer_plots::*;
use std::{collections::HashSet, io::BufReader, path::PathBuf};
fn directory() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let p = std::env::temp_dir().join(format!(
        "impulcifer-p19-render-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn curves() -> (FrSeries, FrCurve) {
    let frequency: Vec<_> = (0..700).map(|i| 20.0 * 1.01_f64.powi(i)).collect();
    let raw: Vec<_> = frequency
        .iter()
        .map(|f| 8.0 * (f.log10() * 3.0).sin())
        .collect();
    (
        FrSeries {
            frequency: frequency.clone(),
            raw: raw.clone(),
            smoothed: raw.clone(),
        },
        FrCurve {
            name: "EQ".into(),
            frequency,
            raw,
            ..Default::default()
        },
    )
}
fn decode(p: &std::path::Path) -> (u32, u32, Vec<u8>) {
    let mut r = png::Decoder::new(BufReader::new(std::fs::File::open(p).unwrap()))
        .read_info()
        .unwrap();
    let mut data = vec![0; r.output_buffer_size().unwrap()];
    let info = r.next_frame(&mut data).unwrap();
    data.truncate(info.buffer_size());
    (info.width, info.height, data)
}
#[test]
fn png_outputs_have_p23_sizes() {
    let d = directory();
    let (s, c) = curves();
    let mut r = s.clone();
    for v in &mut r.smoothed {
        *v += 2.0;
    }
    plot_results(&d.join("results.png"), &s, &r).unwrap();
    plot_headphones(&d.join("headphones.png"), &c, &c, 0.0, -1.0).unwrap();
    plot_eq(&d.join("eq-one.png"), Some(&c), Some(&c)).unwrap();
    let mut right = c.clone();
    right.name = "Right".into();
    plot_eq(&d.join("eq-two.png"), Some(&c), Some(&right)).unwrap();
    plot_interaural_overlay(
        &d.join("overlay.png"),
        &impulcifer_plots::InterauralOverlay {
            speaker: "FL",
            left_ir: &[0.0, 1.0, -0.1, 0.0],
            right_ir: &[0.0, 0.5, 0.0, 0.0],
            left_peak: 1,
            right_peak: 1,
            fs: 48000,
            time_range_ms: (-5.0, 30.0),
        },
    )
    .unwrap();
    let panels = IrPanels {
        title: "FL-left".into(),
        fs: 48000,
        ir: vec![0.0, 1.0, 0.0],
        fr: s,
        limits: PanelLimits([
            None,
            Some(AxisLimits {
                x: (0.0, 1.0),
                y: (-0.1, 1.1),
            }),
            None,
            None,
            Some(AxisLimits {
                x: (20.0, 20000.0),
                y: (-10.0, 10.0),
            }),
            None,
        ]),
        ..Default::default()
    };
    plot_ir_panels(&d.join("panels.png"), &panels).unwrap();
    for (name, size) in [
        ("results.png", (1600, 1000)),
        ("headphones.png", (1600, 1000)),
        ("eq-one.png", (1600, 1000)),
        ("eq-two.png", (1600, 1000)),
        ("panels.png", (2400, 1350)),
        ("overlay.png", (1600, 900)),
    ] {
        let (w, h, _) = decode(&d.join(name));
        assert_eq!((w, h), size);
        println!("{name} {w}x{h}");
    }
    println!("P19_RENDER_DIRECTORY {}", d.display());
}
#[test]
fn png_is_not_blank() {
    let d = directory();
    let (l, _) = curves();
    let mut r = l.clone();
    for v in &mut r.smoothed {
        *v -= 3.0;
    }
    plot_results(&d.join("results.png"), &l, &r).unwrap();
    let (_, _, pixels) = decode(&d.join("results.png"));
    let colors: HashSet<_> = pixels.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
    let mean = pixels.iter().map(|v| f64::from(*v)).sum::<f64>() / pixels.len() as f64;
    let variance = pixels
        .iter()
        .map(|v| (f64::from(*v) - mean).powi(2))
        .sum::<f64>()
        / pixels.len() as f64;
    assert!(colors.len() >= 3);
    assert!(variance > 100.0, "variance={variance}");
    for rgb in [
        [251, 251, 252],
        [27, 31, 36],
        [37, 99, 235],
        [220, 38, 38],
        [17, 24, 39],
    ] {
        assert!(colors.contains(&rgb), "missing fixed series color {rgb:?}");
    }
    println!(
        "pixel variance={variance:.6}; distinct colors={}",
        colors.len()
    );
}
#[test]
fn axis_limits_synchronize_across_panels() {
    let a = PanelLimits([
        Some(AxisLimits {
            x: (-1.0, 4.0),
            y: (-2.0, 3.0),
        }),
        None,
        None,
        None,
        Some(AxisLimits {
            x: (20.0, 20000.0),
            y: (-40.0, 5.0),
        }),
        None,
    ]);
    let b = PanelLimits([
        Some(AxisLimits {
            x: (-3.0, 2.0),
            y: (-1.0, 6.0),
        }),
        None,
        None,
        None,
        Some(AxisLimits {
            x: (20.0, 20000.0),
            y: (-20.0, 10.0),
        }),
        None,
    ]);
    let sync = PanelLimits::synchronize([&a, &b]);
    assert_eq!(
        sync.0[0],
        Some(AxisLimits {
            x: (-3.0, 4.0),
            y: (-2.0, 6.0)
        })
    );
    assert_eq!(
        sync.0[4],
        Some(AxisLimits {
            x: (20.0, 20000.0),
            y: (-40.0, 10.0)
        })
    );
    assert!(sync.0[1].is_none());
    assert_eq!(PanelLimits::synchronize([&sync, &a, &b]), sync);
}
#[test]
fn invalid_grids_and_output_errors_are_reported() {
    let d = directory();
    let (s, _) = curves();
    let mut bad = s.clone();
    bad.raw.pop();
    assert!(plot_results(&d.join("bad.png"), &bad, &s).is_err());
    let mut bad = s.clone();
    bad.frequency[0] = 0.0;
    assert!(plot_results(&d.join("bad.png"), &bad, &s).is_err());
    let mut bad = s.clone();
    bad.smoothed[5] = f64::NAN;
    assert!(plot_results(&d.join("bad.png"), &bad, &s).is_err());
    std::fs::write(d.join("blocked"), b"file").unwrap();
    assert!(plot_results(&d.join("blocked/results.png"), &s, &s).is_err());
    assert!(!d.join("bad.png").exists());
}

fn fixture() -> (FrSeries, FrCurve, IrPanels) {
    let (mut series, mut curve) = curves();
    series.smoothed = series
        .frequency
        .iter()
        .map(|f| 4.0 * (f.log10() * 3.0).sin())
        .collect();
    curve.smoothed = series.smoothed.clone();
    curve.target = vec![0.0; curve.frequency.len()];
    curve.error_smoothed = curve.smoothed.clone();
    curve.equalization = curve.smoothed.iter().map(|v| -v).collect();
    let fs = 48000;
    let ir: Vec<_> = (0..14400)
        .map(|i| {
            let t = i as f64 / 48.0;
            (-t / 30.0).exp() * (t * 3.0).sin()
        })
        .collect();
    let recording: Vec<_> = (0..24000)
        .map(|i| (i as f64 * 0.001 + (i as f64 * 0.0002).powi(2)).sin() * 0.5)
        .collect();
    let spec = Spectrogram {
        frequency: (0..60)
            .map(|i| 20.0 * 1000_f64.powf(i as f64 / 59.0))
            .collect(),
        time: (0..60).map(|i| i as f64 * 0.005).collect(),
        db: (0..60)
            .map(|i| {
                (0..60)
                    .map(|t| (-t as f64 * (0.5 + i as f64 / 60.0)).max(-80.0))
                    .collect()
            })
            .collect(),
    };
    let panels = IrPanels {
        title: "FL · left ear · synthetic".into(),
        fs,
        ir,
        recording: Some(recording),
        spectrogram: Some(spec),
        fr: series.clone(),
        decay: (0..14400).map(|i| -i as f64 / 160.0).collect(),
        decay_average: (0..14000).map(|i| -i as f64 / 180.0).collect(),
        decay_window: 400,
        limits: PanelLimits([
            Some(AxisLimits {
                x: (0.0, 0.5),
                y: (-0.6, 0.6),
            }),
            Some(AxisLimits {
                x: (0.0, 300.0),
                y: (-1.1, 1.1),
            }),
            Some(AxisLimits {
                x: (0.0, 300.0),
                y: (-96.0, 0.0),
            }),
            Some(AxisLimits {
                x: (0.0, 0.3),
                y: (20.0, 20000.0),
            }),
            Some(AxisLimits {
                x: (20.0, 20000.0),
                y: (-12.0, 12.0),
            }),
            None,
        ]),
        ..Default::default()
    };
    (series, curve, panels)
}
fn check_picture(path: &std::path::Path, size: (u32, u32)) {
    assert!(path.is_file());
    let (w, h, pixels) = decode(path);
    assert_eq!((w, h), size);
    assert!(pixels.chunks_exact(3).any(|p| p == [251, 251, 252]));
    assert!(pixels.chunks_exact(3).any(|p| p == [27, 31, 36]));
    // The top title region must contain ink, not only an axis or legend.
    assert!(
        (64..160).any(|y| (64..w as usize - 64).any(|x| pixels
            [(y * w as usize + x) * 3..(y * w as usize + x) * 3 + 3]
            == [27, 31, 36]))
    );
}
#[test]
fn results_chart_renders_without_optional_raw() {
    let d = directory();
    let (mut s, _, _) = fixture();
    s.raw.clear();
    plot_results(&d.join("results.png"), &s, &s).unwrap();
    check_picture(&d.join("results.png"), (1600, 1000));
}
#[test]
fn headphones_chart_renders_without_optional_correction() {
    let d = directory();
    let (_, mut c, _) = fixture();
    c.equalization.clear();
    c.smoothed.clear();
    c.target.clear();
    plot_headphones(&d.join("headphones.png"), &c, &c, 0.0, 0.0).unwrap();
    check_picture(&d.join("headphones.png"), (1600, 1000));
}
#[test]
fn headphones_chart_draws_the_applied_correction_when_supplied() {
    let d = directory();
    let (_, mut left, _) = fixture();
    left.equalization.clear();
    let mut right = left.clone();
    right.smoothed.iter_mut().for_each(|v| *v += 2.);
    let measured_path = d.join("headphones-measured.png");
    plot_headphones(&measured_path, &left, &right, 0., 0.).unwrap();
    let (_, _, measured_pixels) = decode(&measured_path);
    assert!(!measured_pixels.chunks_exact(3).any(|p| p == [124, 58, 237]));

    left.equalization = left
        .frequency
        .iter()
        .map(|f| -12. * (f.log10() * 2.).sin())
        .collect();
    right.equalization = right
        .frequency
        .iter()
        .map(|f| 18. * (f.log10() * 3.).cos())
        .collect();
    let applied_path = d.join("headphones-applied.png");
    plot_headphones(&applied_path, &left, &right, 0., 0.).unwrap();
    for path in [&measured_path, &applied_path] {
        check_picture(path, (1600, 1000));
        let (_, _, pixels) = decode(path);
        let colors: HashSet<_> = pixels.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
        for token in [[37, 99, 235], [220, 38, 38], [156, 163, 175]] {
            assert!(colors.contains(&token), "missing token {token:?}");
        }
        if path == &applied_path {
            assert!(colors.contains(&[124, 58, 237]));
        }
        let mean = pixels.iter().map(|v| f64::from(*v)).sum::<f64>() / pixels.len() as f64;
        let variance = pixels
            .iter()
            .map(|v| (f64::from(*v) - mean).powi(2))
            .sum::<f64>()
            / pixels.len() as f64;
        assert!(colors.len() >= 3);
        assert!(variance > 100., "variance={variance}");
    }
    let mut missing = left.clone();
    missing.equalization.clear();
    for (l, r) in [(&missing, &right), (&left, &missing)] {
        assert!(matches!(
            plot_headphones(&d.join("bad.png"), l, r, 0., 0.),
            Err(PlotError::Invalid(_))
        ));
    }
    let mut wrong = left.clone();
    wrong.equalization.pop();
    for (l, r) in [(&wrong, &right), (&left, &wrong)] {
        assert!(matches!(
            plot_headphones(&d.join("bad.png"), l, r, 0., 0.),
            Err(PlotError::Invalid(_))
        ));
    }
    for value in [f64::NAN, f64::INFINITY] {
        let mut invalid = left.clone();
        invalid.equalization[0] = value;
        assert!(matches!(
            plot_headphones(&d.join("bad.png"), &invalid, &right, 0., 0.),
            Err(PlotError::Invalid(_))
        ));
    }
    assert!(!d.join("bad.png").exists());
    if let Some(out) = std::env::var_os("IMPULCIFER_A02_OUT") {
        let out = PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        for source in [&measured_path, &applied_path] {
            let destination = out.join(source.file_name().unwrap());
            // Never replace a previous visual review artifact.
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .unwrap();
            std::io::copy(&mut std::fs::File::open(source).unwrap(), &mut file).unwrap();
            println!("A02_PNG {}", destination.display());
        }
    }
}
#[test]
fn eq_chart_renders_missing_ears() {
    let d = directory();
    let (_, c, _) = fixture();
    plot_eq(&d.join("absent.png"), None, None).unwrap();
    assert!(!d.join("absent.png").exists());
    for (name, l, r) in [("left", Some(&c), None), ("right", None, Some(&c))] {
        let path = d.join(format!("eq-{name}.png"));
        plot_eq(&path, l, r).unwrap();
        check_picture(&path, (1600, 1000));
    }
}
#[test]
fn room_chart_renders_without_optional_measurements() {
    let d = directory();
    let (_, mut c, _) = fixture();
    c.target.clear();
    c.error_smoothed.clear();
    plot_generic_room(&d.join("room.png"), &c, &[]).unwrap();
    check_picture(&d.join("room.png"), (1600, 1000));
}
#[test]
fn ir_sheet_renders_present_and_absent_optional_panels() {
    let d = directory();
    let (_, _, p) = fixture();
    plot_ir_panels_with_noise_floor(&d.join("panels.png"), &p, -60.0).unwrap();
    check_picture(&d.join("panels.png"), (2400, 1350));
    plot_ir_panels(
        &d.join("empty.png"),
        &IrPanels {
            fs: 48000,
            title: "Unavailable measurements".into(),
            ..Default::default()
        },
    )
    .unwrap();
    check_picture(&d.join("empty.png"), (2400, 1350));
}
#[test]
fn overlay_chart_preserves_supplied_peak_delay() {
    let d = directory();
    let mut left = vec![0.0; 480];
    let mut right = left.clone();
    left[48] = 0.8;
    left[60] = -1.0;
    right[63] = 0.7;
    plot_interaural_overlay(
        &d.join("overlay.png"),
        &InterauralOverlay {
            speaker: "FL",
            left_ir: &left,
            right_ir: &right,
            left_peak: 48,
            right_peak: 63,
            fs: 48000,
            time_range_ms: (-5.0, 30.0),
        },
    )
    .unwrap();
    check_picture(&d.join("overlay.png"), (1600, 900));
}
#[test]
fn invalid_optional_matrices_and_peaks_are_errors() {
    let d = directory();
    let (_, _, mut p) = fixture();
    p.spectrogram.as_mut().unwrap().db[0].pop();
    assert!(plot_ir_panels(&d.join("bad.png"), &p).is_err());
    assert!(
        plot_interaural_overlay(
            &d.join("bad.png"),
            &InterauralOverlay {
                speaker: "FL",
                left_ir: &[0.0],
                right_ir: &[0.0],
                left_peak: 1,
                right_peak: 0,
                fs: 48000,
                time_range_ms: (-1.0, 5.0)
            }
        )
        .is_err()
    );
}
#[test]
fn chart_kinds_use_their_actual_tokens() {
    let d = directory();
    let (s, c, p) = fixture();
    let mut right = s.clone();
    right.smoothed.iter_mut().for_each(|v| *v += 2.);
    let mut rc = c.clone();
    rc.smoothed.iter_mut().for_each(|v| *v += 2.);
    rc.equalization.iter_mut().for_each(|v| *v -= 2.);
    plot_results(&d.join("results.png"), &s, &right).unwrap();
    let mut headphones_left = c.clone();
    let mut headphones_right = rc.clone();
    headphones_left.equalization.clear();
    headphones_right.equalization.clear();
    plot_headphones(
        &d.join("headphones.png"),
        &headphones_left,
        &headphones_right,
        0.,
        0.,
    )
    .unwrap();
    plot_eq(&d.join("eq.png"), Some(&c), Some(&c)).unwrap();
    plot_generic_room(&d.join("room.png"), &c, &[]).unwrap();
    plot_ir_panels_with_noise_floor(&d.join("panels.png"), &p, -60.).unwrap();
    let mut left = vec![0.; 480];
    left[48] = 1.;
    let mut right_ir = vec![0.; 480];
    right_ir[60] = 0.8;
    plot_interaural_overlay(
        &d.join("overlay.png"),
        &InterauralOverlay {
            speaker: "FL",
            left_ir: &left,
            right_ir: &right_ir,
            left_peak: 48,
            right_peak: 60,
            fs: 48000,
            time_range_ms: (-1., 5.),
        },
    )
    .unwrap();
    let blue = [37, 99, 235];
    let red = [220, 38, 38];
    let purple = [124, 58, 237];
    let target = [156, 163, 175];
    let both = [17, 24, 39];
    for (name, expected) in [
        ("results", vec![blue, red, both]),
        ("headphones", vec![blue, red, target]),
        ("eq", vec![both, purple, target]),
        ("room", vec![blue, both, target]),
        ("panels", vec![blue, target]),
        ("overlay", vec![blue, red]),
    ] {
        let (_, _, pixels) = decode(&d.join(format!("{name}.png")));
        let colors: HashSet<_> = pixels.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
        for token in expected.into_iter().chain([[251, 251, 252], [27, 31, 36]]) {
            assert!(colors.contains(&token), "{name}: missing token {token:?}");
        }
        if name == "headphones" {
            assert!(
                !colors.contains(&purple),
                "headphones must never show an inverse correction"
            );
        }
        if name == "panels" {
            assert!(
                !colors.contains(&purple),
                "IR reflections must not retain the discarded band series"
            );
        }
    }
}
#[test]
#[ignore = "writes visual review artifacts"]
fn gallery() {
    let base =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/plots-gallery/synthetic");
    // Inspect before writing and preserve every previous run.
    let d = if base.exists() && std::fs::read_dir(&base).unwrap().next().is_some() {
        base.join(format!(
            "run-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    } else {
        base
    };
    std::fs::create_dir_all(&d).unwrap();
    let (s, c, p) = fixture();
    let mut right = s.clone();
    for (i, v) in right.smoothed.iter_mut().enumerate() {
        *v -= 2.0 * (i as f64 / 60.0).sin();
    }
    let mut rc = c.clone();
    rc.name = "Right ear".into();
    for v in &mut rc.smoothed {
        *v += 2.0;
    }
    for v in &mut rc.equalization {
        *v -= 1.0;
    }
    plot_results(&d.join("results.png"), &s, &right).unwrap();
    plot_headphones(&d.join("headphones.png"), &c, &rc, 0.0, 2.0).unwrap();
    plot_eq(&d.join("eq-single.png"), Some(&c), Some(&c)).unwrap();
    plot_eq(&d.join("eq-split.png"), Some(&c), Some(&rc)).unwrap();
    plot_eq(&d.join("eq-missing-ear.png"), Some(&c), None).unwrap();
    plot_generic_room(&d.join("room.png"), &c, &[rc]).unwrap();
    plot_ir_panels_with_noise_floor(&d.join("panels.png"), &p, -60.0).unwrap();
    let mut room = p.clone();
    room.room_fr = Some(c);
    plot_ir_panels_with_noise_floor(&d.join("room-panels.png"), &room, -60.0).unwrap();
    plot_ir_panels(
        &d.join("panels-empty.png"),
        &IrPanels {
            fs: 48000,
            title: "Missing optional panels".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut left = vec![0.0; 4800];
    let mut right = left.clone();
    left[48] = 0.8;
    left[60] = -1.0;
    right[63] = 0.7;
    plot_interaural_overlay(
        &d.join("overlay.png"),
        &InterauralOverlay {
            speaker: "FL",
            left_ir: &left,
            right_ir: &right,
            left_peak: 48,
            right_peak: 63,
            fs: 48000,
            time_range_ms: (-5.0, 30.0),
        },
    )
    .unwrap();
    println!("P23_SYNTHETIC_GALLERY {}", d.display());
}
