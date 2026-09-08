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
fn png_outputs_have_matplotlib_sizes() {
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
        ("results.png", (1200, 900)),
        ("headphones.png", (2200, 1000)),
        ("eq-one.png", (1200, 900)),
        ("eq-two.png", (2200, 900)),
        ("panels.png", (2200, 1000)),
        ("overlay.png", (1200, 700)),
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
        [125, 180, 219],
        [221, 128, 129],
        [31, 119, 180],
        [214, 39, 40],
        [104, 15, 185],
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
