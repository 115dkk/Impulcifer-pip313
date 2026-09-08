#![forbid(unsafe_code)]
//! Headless, embedded-font PNG rendering. Data preparation belongs to the service.
use plotters::{coord::Shift, prelude::*};
use std::{fs::File, io::BufWriter, path::Path, sync::OnceLock};

const FONT: &str = "DejaVu Sans";
const BLUE: RGBColor = RGBColor(31, 119, 180);
const RED: RGBColor = RGBColor(214, 39, 40);
const LIGHT_BLUE: RGBColor = RGBColor(125, 180, 219);
const LIGHT_RED: RGBColor = RGBColor(221, 128, 129);
const PURPLE: RGBColor = RGBColor(104, 15, 185);

#[derive(Debug, thiserror::Error)]
pub enum PlotError {
    #[error("invalid plot data: {0}")]
    Invalid(String),
    #[error("plot rendering: {0}")]
    Render(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Png(#[from] png::EncodingError),
}
fn render_error(e: impl std::fmt::Debug) -> PlotError {
    PlotError::Render(format!("{e:?}"))
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrSeries {
    pub frequency: Vec<f64>,
    pub raw: Vec<f64>,
    pub smoothed: Vec<f64>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrCurve {
    pub name: String,
    pub frequency: Vec<f64>,
    pub raw: Vec<f64>,
    pub smoothed: Vec<f64>,
    pub error: Vec<f64>,
    pub error_smoothed: Vec<f64>,
    pub equalization: Vec<f64>,
    pub equalized_raw: Vec<f64>,
    pub target: Vec<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisLimits {
    pub x: (f64, f64),
    pub y: (f64, f64),
}
/// Matplotlib creation order: recording, IR, decay, spectrogram, FR, waterfall.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelLimits(pub [Option<AxisLimits>; 6]);
impl PanelLimits {
    /// Pass one unions corresponding axes only. Disabled panels stay disabled.
    pub fn synchronize<'a>(panels: impl IntoIterator<Item = &'a Self>) -> Self {
        let mut out = Self::default();
        for panel in panels {
            for (dst, src) in out.0.iter_mut().zip(panel.0) {
                if let Some(src) = src {
                    *dst = Some(match *dst {
                        None => src,
                        Some(d) => AxisLimits {
                            x: (d.x.0.min(src.x.0), d.x.1.max(src.x.1)),
                            y: (d.y.0.min(src.y.0), d.y.1.max(src.y.1)),
                        },
                    });
                }
            }
        }
        out
    }
}
#[derive(Clone, Debug)]
pub struct Spectrogram {
    pub frequency: Vec<f64>,
    pub time: Vec<f64>,
    /// Frequency-major dB matrix, with DC already removed.
    pub db: Vec<Vec<f64>>,
}
#[derive(Clone, Debug)]
pub struct Waterfall {
    pub frequency: Vec<f64>,
    pub time_ms: Vec<f64>,
    pub db: Vec<Vec<f64>>,
}
#[derive(Clone, Debug, Default)]
pub struct IrPanels {
    pub title: String,
    pub recording: Option<Vec<f64>>,
    pub spectrogram: Option<Spectrogram>,
    pub ir: Vec<f64>,
    pub fs: u32,
    pub fr: FrSeries,
    /// Room correction uses target/smoothed/error-smoothed instead of raw FR.
    pub room_fr: Option<FrCurve>,
    /// Peak-normalized squared IR in dBr, over the decay window.
    pub decay: Vec<f64>,
    pub decay_start: usize,
    pub decay_average: Vec<f64>,
    pub decay_window: usize,
    pub waterfall: Option<Waterfall>,
    pub limits: PanelLimits,
}
fn font() -> Result<(), PlotError> {
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| {
            plotters::style::register_font(
                FONT,
                FontStyle::Normal,
                include_bytes!("../assets/DejaVuSans.ttf"),
            )
            .map_err(|_| "invalid embedded DejaVu font".to_owned())
        })
        .clone()
        .map_err(PlotError::Render)
}
type Area<'a> = DrawingArea<BitMapBackend<'a>, Shift>;
fn png_file(
    path: &Path,
    size: (u32, u32),
    draw: impl FnOnce(&Area<'_>) -> Result<(), PlotError>,
) -> Result<(), PlotError> {
    font()?;
    let mut pixels = vec![255; size.0 as usize * size.1 as usize * 3];
    {
        let root = BitMapBackend::with_buffer(&mut pixels, size).into_drawing_area();
        root.fill(&WHITE).map_err(render_error)?;
        draw(&root)?;
        root.present().map_err(render_error)?;
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), size.0, size.1);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&pixels)?;
    Ok(())
}
fn validate(f: &[f64], arrays: &[&[f64]]) -> Result<(), PlotError> {
    if f.len() < 2
        || f.iter().any(|v| !v.is_finite() || *v <= 0.0)
        || f.windows(2).any(|p| p[0] >= p[1])
        || arrays
            .iter()
            .any(|v| !v.is_empty() && (v.len() != f.len() || v.iter().any(|v| !v.is_finite())))
    {
        return Err(PlotError::Invalid(
            "finite, increasing frequencies and matching finite arrays required".into(),
        ));
    }
    Ok(())
}
/// Matplotlib linear autoscale margin, including its nonsingular zero/constant case.
pub fn padded_range(values: impl IntoIterator<Item = f64>) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in values.into_iter().filter(|v| v.is_finite()) {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if !lo.is_finite() {
        return (0.0, 1.0);
    }
    if lo == hi {
        let d = if lo == 0.0 { 0.05 } else { lo.abs() * 0.05 };
        lo -= d;
        hi += d;
    }
    let pad = (hi - lo) * 0.05;
    (lo - pad, hi + pad)
}
struct Line<'a> {
    x: &'a [f64],
    y: &'a [f64],
    label: &'a str,
    color: RGBColor,
    width: u32,
}
fn line<'a>(x: &'a [f64], y: &'a [f64], label: &'a str, color: RGBColor, width: u32) -> Line<'a> {
    Line {
        x,
        y,
        label,
        color,
        width,
    }
}
fn fr_chart(
    area: &Area<'_>,
    title: &str,
    ylabel: &str,
    lines: &[Line<'_>],
    ylim: Option<(f64, f64)>,
) -> Result<(), PlotError> {
    for s in lines {
        validate(s.x, &[s.y])?;
    }
    let y = ylim.unwrap_or_else(|| padded_range(lines.iter().flat_map(|s| s.y.iter().copied())));
    if !y.0.is_finite() || !y.1.is_finite() || y.0 >= y.1 {
        return Err(PlotError::Invalid("invalid FR limits".into()));
    }
    let mut chart = ChartBuilder::on(area)
        .margin(18)
        .caption(title, (FONT, 18))
        .x_label_area_size(48)
        .y_label_area_size(62)
        .build_cartesian_2d((20.0_f64..20000.0_f64).log_scale(), y.0..y.1)
        .map_err(render_error)?;
    chart
        .configure_mesh()
        .x_desc("Frequency (Hz)")
        .y_desc(ylabel)
        .label_style((FONT, 13))
        .axis_desc_style((FONT, 15))
        .x_label_formatter(&|v| format!("{v:.0}"))
        .light_line_style(RGBColor(230, 230, 230))
        .bold_line_style(RGBColor(190, 190, 190))
        .draw()
        .map_err(render_error)?;
    for s in lines.iter().filter(|s| !s.y.is_empty()) {
        let color = s.color;
        let width = s.width;
        let annotation = chart
            .draw_series(LineSeries::new(
                s.x.iter().copied().zip(s.y.iter().copied()),
                color.stroke_width(width),
            ))
            .map_err(render_error)?;
        if !s.label.is_empty() {
            annotation.label(s.label).legend(move |(x, y)| {
                PathElement::new(vec![(x, y), (x + 22, y)], color.stroke_width(width))
            });
        }
    }
    chart
        .configure_series_labels()
        .label_font((FONT, 12))
        .background_style(WHITE.mix(0.9))
        .border_style(RGBColor(190, 190, 190))
        .draw()
        .map_err(render_error)?;
    Ok(())
}
pub fn plot_results(path: &Path, left: &FrSeries, right: &FrSeries) -> Result<(), PlotError> {
    validate(&left.frequency, &[&left.raw, &left.smoothed])?;
    validate(&right.frequency, &[&right.raw, &right.smoothed])?;
    if left.frequency != right.frequency
        || left.smoothed.len() != left.frequency.len()
        || right.smoothed.len() != right.frequency.len()
    {
        return Err(PlotError::Invalid(
            "results require matching grids and smoothed curves".into(),
        ));
    }
    let difference: Vec<_> = left
        .smoothed
        .iter()
        .zip(&right.smoothed)
        .map(|(l, r)| l - r)
        .collect();
    png_file(path, (1200, 900), |root| {
        fr_chart(
            root,
            "Frequency response",
            "Amplitude (dB)",
            &[
                line(&left.frequency, &left.raw, "Left raw", LIGHT_BLUE, 1),
                line(&right.frequency, &right.raw, "Right raw", LIGHT_RED, 1),
                line(&left.frequency, &left.smoothed, "Left smoothed", BLUE, 1),
                line(&right.frequency, &right.smoothed, "Right smoothed", RED, 1),
                line(&left.frequency, &difference, "Difference", PURPLE, 2),
            ],
            None,
        )
    })
}
fn curve_lines(fr: &FrCurve) -> Vec<Line<'_>> {
    vec![
        line(
            &fr.frequency,
            &fr.target,
            "Target",
            RGBColor(173, 216, 230),
            7,
        ),
        line(
            &fr.frequency,
            &fr.smoothed,
            "Raw Smoothed",
            RGBColor(211, 211, 211),
            7,
        ),
        line(
            &fr.frequency,
            &fr.error_smoothed,
            "Error Smoothed",
            RGBColor(255, 192, 203),
            7,
        ),
        line(&fr.frequency, &fr.raw, "Raw", BLACK, 1),
        line(&fr.frequency, &fr.error, "Error", RGBColor(255, 0, 0), 1),
        line(
            &fr.frequency,
            &fr.equalization,
            "Equalization",
            RGBColor(144, 238, 144),
            7,
        ),
        line(
            &fr.frequency,
            &fr.equalized_raw,
            "Equalized",
            RGBColor(0, 0, 255),
            1,
        ),
    ]
}
pub fn headphones_limits(left: &FrCurve, right: &FrCurve) -> Result<(f64, f64), PlotError> {
    validate(&left.frequency, &[&left.raw])?;
    validate(&right.frequency, &[&right.raw])?;
    if left.frequency != right.frequency
        || left.raw.len() != left.frequency.len()
        || right.raw.len() != right.frequency.len()
    {
        return Err(PlotError::Invalid(
            "headphones require raw curves on matching grids".into(),
        ));
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for ((f, l), r) in left.frequency.iter().zip(&left.raw).zip(&right.raw) {
        if *f > 20.0 && *f < 20000.0 {
            for v in [*l, *r, l - r] {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    if lo == hi {
        return Ok(padded_range([lo]));
    }
    Ok((lo * 1.1, hi * 1.1))
}
pub fn plot_headphones(
    path: &Path,
    left: &FrCurve,
    right: &FrCurve,
    gain_left_db: f64,
    gain_right_db: f64,
) -> Result<(), PlotError> {
    let limits = headphones_limits(left, right)?;
    let difference: Vec<_> = left
        .raw
        .iter()
        .zip(&right.raw)
        .map(|(l, r)| l - r)
        .collect();
    png_file(path, (2200, 1000), |root| {
        let root = root
            .titled("Headphones", (FONT, 20))
            .map_err(render_error)?;
        let (small, comparison) = root.split_horizontally(2200 / 3);
        let sides = small.split_evenly((2, 1));
        fr_chart(
            &sides[0],
            "Left",
            "Amplitude (dBr)",
            &curve_lines(left),
            Some(limits),
        )?;
        fr_chart(
            &sides[1],
            "Right",
            "Amplitude (dBr)",
            &curve_lines(right),
            Some(limits),
        )?;
        fr_chart(
            &comparison,
            "Comparison",
            "Amplitude (dB)",
            &[
                line(
                    &left.frequency,
                    &left.raw,
                    &format!("Left raw {gain_left_db:+.1} dB"),
                    BLUE,
                    1,
                ),
                line(
                    &right.frequency,
                    &right.raw,
                    &format!("Right raw {gain_right_db:+.1} dB"),
                    RED,
                    1,
                ),
                line(&left.frequency, &difference, "Difference", PURPLE, 1),
            ],
            Some(limits),
        )
    })
}
/// Generic room plot, core/room_correction.py:389-423 (15 x 9 inches, 100 dpi).
pub fn plot_generic_room(
    path: &Path,
    room: &FrCurve,
    measurements: &[FrCurve],
) -> Result<(), PlotError> {
    validate(
        &room.frequency,
        &[&room.target, &room.smoothed, &room.error_smoothed],
    )?;
    let mut lines = vec![line(
        &room.frequency,
        &room.target,
        "Target",
        RGBColor(236, 222, 249),
        7,
    )];
    for raw in measurements {
        lines.push(line(
            &raw.frequency,
            &raw.smoothed,
            "",
            RGBColor(128, 128, 128),
            1,
        ));
    }
    lines.push(line(
        &room.frequency,
        &room.smoothed,
        "Raw smoothed",
        BLUE,
        2,
    ));
    lines.push(line(
        &room.frequency,
        &room.error_smoothed,
        "Error smoothed",
        RED,
        2,
    ));
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for values in [&room.target, &room.smoothed, &room.error_smoothed] {
        for (&f, &v) in room.frequency.iter().zip(values) {
            if (20.0..=20000.0).contains(&f) {
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
    }
    let limits = if lo == hi {
        padded_range([lo])
    } else {
        (lo - (hi - lo) * 0.1, hi + (hi - lo) * 0.1)
    };
    png_file(path, (1500, 900), |root| {
        fr_chart(
            root,
            "Generic room measurement",
            "Amplitude (dB)",
            &lines,
            Some(limits),
        )
    })
}
pub fn plot_eq(
    path: &Path,
    left: Option<&FrCurve>,
    right: Option<&FrCurve>,
) -> Result<(), PlotError> {
    if left.is_none() && right.is_none() {
        return Ok(());
    }
    let single = left.is_some() && left == right;
    png_file(path, (if single { 1200 } else { 2200 }, 900), |root| {
        let panels = root.split_evenly((1, if single { 1 } else { 2 }));
        if let Some(fr) = left {
            fr_chart(
                &panels[0],
                &fr.name,
                "Amplitude (dBr)",
                &curve_lines(fr),
                None,
            )?;
        }
        if !single && let Some(fr) = right {
            fr_chart(
                &panels[1],
                &fr.name,
                "Amplitude (dBr)",
                &curve_lines(fr),
                None,
            )?;
        }
        Ok(())
    })
}
fn linear_chart(
    area: &Area<'_>,
    title: &str,
    xlabel: &str,
    ylabel: &str,
    lines: &[Line<'_>],
    limits: AxisLimits,
) -> Result<(), PlotError> {
    let mut chart = ChartBuilder::on(area)
        .margin(18)
        .caption(title, (FONT, 18))
        .x_label_area_size(48)
        .y_label_area_size(62)
        .build_cartesian_2d(limits.x.0..limits.x.1, limits.y.0..limits.y.1)
        .map_err(render_error)?;
    chart
        .configure_mesh()
        .x_desc(xlabel)
        .y_desc(ylabel)
        .label_style((FONT, 13))
        .axis_desc_style((FONT, 15))
        .light_line_style(RGBColor(230, 230, 230))
        .bold_line_style(RGBColor(190, 190, 190))
        .draw()
        .map_err(render_error)?;
    for s in lines {
        let color = s.color;
        chart
            .draw_series(LineSeries::new(
                s.x.iter().copied().zip(s.y.iter().copied()),
                color.stroke_width(s.width),
            ))
            .map_err(render_error)?
            .label(s.label)
            .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 22, y)], color));
    }
    if lines.iter().any(|s| !s.label.is_empty()) {
        chart
            .configure_series_labels()
            .label_font((FONT, 12))
            .background_style(WHITE.mix(0.9))
            .border_style(RGBColor(190, 190, 190))
            .draw()
            .map_err(render_error)?;
    }
    Ok(())
}
fn sample_times(n: usize, start: usize, fs: u32, scale: f64) -> Vec<f64> {
    (0..n)
        .map(|i| (start + i) as f64 / f64::from(fs) * scale)
        .collect()
}
/// gnuplot2 analytic RGB functions used by matplotlib (30, 31, 32).
fn gnuplot2(t: f64) -> RGBColor {
    let channel = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = if t < 0.25 {
        4.0 * t
    } else if t < 0.92 {
        -2.0 * t + 1.84
    } else {
        t / 0.08 - 11.5
    };
    RGBColor(
        channel(t / 0.32 - 0.78125),
        channel(2.0 * t - 0.84),
        channel(b),
    )
}
fn heatmap(area: &Area<'_>, spec: &Spectrogram, limits: AxisLimits) -> Result<(), PlotError> {
    validate(&spec.frequency, &[])?;
    if spec.time.len() < 2
        || spec.db.len() != spec.frequency.len()
        || spec.db.iter().any(|r| r.len() != spec.time.len())
    {
        return Err(PlotError::Invalid("spectrogram matrix shape".into()));
    }
    let (plot, bar) = area.split_horizontally(area.dim_in_pixel().0.saturating_sub(64));
    let mut chart = ChartBuilder::on(&plot)
        .margin(18)
        .caption("Spectrogram", (FONT, 18))
        .x_label_area_size(48)
        .y_label_area_size(62)
        .build_cartesian_2d(limits.x.0..limits.x.1, (limits.y.0..limits.y.1).log_scale())
        .map_err(render_error)?;
    chart
        .configure_mesh()
        .x_desc("Time (s)")
        .y_desc("Frequency (Hz)")
        .label_style((FONT, 13))
        .axis_desc_style((FONT, 15))
        .y_label_formatter(&|v| format!("{v:.0}"))
        .draw()
        .map_err(render_error)?;
    let max = spec
        .db
        .iter()
        .flatten()
        .copied()
        .fold(-150.0, f64::max)
        .max(-149.0);
    let edge = |values: &[f64], i: usize| {
        if i == 0 {
            values[0] - (values[1] - values[0]) * 0.5
        } else if i == values.len() {
            values[i - 1] + (values[i - 1] - values[i - 2]) * 0.5
        } else {
            (values[i - 1] + values[i]) * 0.5
        }
    };
    chart
        .draw_series(spec.db.iter().enumerate().flat_map(|(f, row)| {
            row.iter().enumerate().map(move |(t, &db)| {
                Rectangle::new(
                    [
                        (
                            edge(&spec.time, t),
                            edge(&spec.frequency, f).max(limits.y.0),
                        ),
                        (edge(&spec.time, t + 1), edge(&spec.frequency, f + 1)),
                    ],
                    gnuplot2((db + 150.0) / (max + 150.0)).filled(),
                )
            })
        }))
        .map_err(render_error)?;
    let mut scale = ChartBuilder::on(&bar)
        .margin_top(42)
        .margin_bottom(66)
        .y_label_area_size(38)
        .build_cartesian_2d(0.0..1.0, -150.0..max)
        .map_err(render_error)?;
    scale
        .draw_series((0..256).map(|i| {
            let y = -150.0 + (max + 150.0) * i as f64 / 256.0;
            Rectangle::new(
                [(0.0, y), (1.0, y + (max + 150.0) / 256.0)],
                gnuplot2(i as f64 / 255.0).filled(),
            )
        }))
        .map_err(render_error)?;
    scale
        .configure_mesh()
        .disable_mesh()
        .x_labels(0)
        .y_labels(6)
        .label_style((FONT, 10))
        .draw()
        .map_err(render_error)?;
    Ok(())
}
fn waterfall_chart(area: &Area<'_>, w: &Waterfall) -> Result<(), PlotError> {
    validate(&w.frequency, &[])?;
    if w.time_ms.len() < 2
        || w.db.len() != w.frequency.len()
        || w.db.iter().any(|r| r.len() != w.time_ms.len())
    {
        return Err(PlotError::Invalid("waterfall matrix shape".into()));
    }
    let magma = |v: f64| {
        let bytes = include_bytes!("../assets/magma.rgb");
        let i = (v.clamp(0.0, 1.0) * 255.0).round() as usize * 3;
        RGBColor(bytes[i], bytes[i + 1], bytes[i + 2])
    };
    let (plot, bar) = area.split_horizontally(area.dim_in_pixel().0.saturating_sub(64));
    let end = w.time_ms.last().copied().unwrap();
    let mut chart = ChartBuilder::on(&plot)
        .margin(18)
        .caption("Waterfall", (FONT, 18))
        .x_label_area_size(48)
        .y_label_area_size(62)
        .build_cartesian_2d(0.0..end, (20.0_f64..20000.0_f64).log_scale())
        .map_err(render_error)?;
    chart
        .configure_mesh()
        .x_desc("Time (ms)")
        .y_desc("Frequency (Hz)")
        .label_style((FONT, 13))
        .axis_desc_style((FONT, 15))
        .y_label_formatter(&|v| format!("{v:.0}"))
        .draw()
        .map_err(render_error)?;
    chart
        .draw_series(w.db.windows(2).enumerate().flat_map(|(f, rows)| {
            rows[0].windows(2).enumerate().map(move |(t, db)| {
                Rectangle::new(
                    [
                        (w.time_ms[t], w.frequency[f]),
                        (w.time_ms[t + 1], w.frequency[f + 1]),
                    ],
                    magma((db[0] + 100.0) / 100.0).filled(),
                )
            })
        }))
        .map_err(render_error)?;
    let mut scale = ChartBuilder::on(&bar)
        .margin_top(42)
        .margin_bottom(66)
        .y_label_area_size(38)
        .build_cartesian_2d(0.0..1.0, -100.0..0.0)
        .map_err(render_error)?;
    scale
        .draw_series((0..256).map(|i| {
            Rectangle::new(
                [
                    (0.0, -100.0 + i as f64 / 256.0 * 100.0),
                    (1.0, -100.0 + (i + 1) as f64 / 256.0 * 100.0),
                ],
                magma(i as f64 / 255.0).filled(),
            )
        }))
        .map_err(render_error)?;
    scale
        .configure_mesh()
        .disable_mesh()
        .x_labels(0)
        .y_labels(6)
        .label_style((FONT, 10))
        .draw()
        .map_err(render_error)?;
    Ok(())
}
pub fn plot_ir_panels(path: &Path, panels: &IrPanels) -> Result<(), PlotError> {
    if panels.fs == 0 {
        return Err(PlotError::Invalid("sample rate is zero".into()));
    }
    png_file(path, (2200, 1000), |root| {
        let root = root
            .titled(&panels.title, (FONT, 20))
            .map_err(render_error)?;
        let a = root.split_evenly((2, 3));
        if let (Some(rec), Some(lim)) = (&panels.recording, panels.limits.0[0]) {
            let t: Vec<_> = (0..rec.len())
                .map(|i| {
                    i as f64 * rec.len() as f64
                        / rec.len().saturating_sub(1).max(1) as f64
                        / f64::from(panels.fs)
                })
                .collect();
            linear_chart(
                &a[0],
                "Sine Sweep",
                "Time (s)",
                "Amplitude",
                &[line(&t, rec, "", BLUE, 1)],
                lim,
            )?;
        }
        if let Some(lim) = panels.limits.0[1] {
            let t = sample_times(panels.ir.len(), 0, panels.fs, 1000.0);
            linear_chart(
                &a[1],
                "Impulse response",
                "Time (ms)",
                "Amplitude",
                &[line(&t, &panels.ir, "", BLUE, 1)],
                lim,
            )?;
        }
        if let Some(lim) = panels.limits.0[2] {
            let t = sample_times(panels.decay.len(), panels.decay_start, panels.fs, 1000.0);
            let avg_t = sample_times(
                panels.decay_average.len(),
                panels.decay_start + panels.decay_window / 2,
                panels.fs,
                1000.0,
            );
            linear_chart(
                &a[2],
                "Decay",
                "Time (ms)",
                "Amplitude (dBr)",
                &[
                    line(&t, &panels.decay, "Squared impulse response", LIGHT_BLUE, 2),
                    line(
                        &avg_t,
                        &panels.decay_average,
                        &format!(
                            "{:.0} ms moving average",
                            panels.decay_window as f64 / f64::from(panels.fs) * 1000.0
                        ),
                        BLUE,
                        2,
                    ),
                ],
                lim,
            )?;
        }
        if let (Some(spec), Some(lim)) = (&panels.spectrogram, panels.limits.0[3]) {
            heatmap(&a[3], spec, lim)?;
        }
        if let Some(lim) = panels.limits.0[4] {
            if let Some(fr) = &panels.room_fr {
                fr_chart(
                    &a[4],
                    &fr.name,
                    "Amplitude (dB)",
                    &[
                        line(
                            &fr.frequency,
                            &fr.target,
                            "Target",
                            RGBColor(236, 222, 249),
                            7,
                        ),
                        line(&fr.frequency, &fr.smoothed, "Raw Smoothed", BLUE, 1),
                        line(&fr.frequency, &fr.error_smoothed, "Error Smoothed", RED, 1),
                    ],
                    Some(lim.y),
                )?;
            } else {
                fr_chart(
                    &a[4],
                    "Frequency response",
                    "Amplitude (dB)",
                    &[
                        line(&panels.fr.frequency, &panels.fr.raw, "Raw", LIGHT_BLUE, 1),
                        line(
                            &panels.fr.frequency,
                            &panels.fr.smoothed,
                            "Raw Smoothed",
                            BLUE,
                            1,
                        ),
                    ],
                    Some(lim.y),
                )?;
            }
        }
        // Permitted 2D heat map: the same magnitude, frequency and time samples.
        // Unlike Python's 3D x-limit typo, display milliseconds only once.
        if let Some(w) = &panels.waterfall {
            waterfall_chart(&a[5], w)?;
        }
        Ok(())
    })
}
/// Input of the interaural overlay: the two ears of one speaker with the time
/// origins the pipeline computed (ImpulseResponse.peak_index semantics).
#[derive(Clone, Copy, Debug)]
pub struct InterauralOverlay<'a> {
    pub speaker: &'a str,
    pub left_ir: &'a [f64],
    pub right_ir: &'a [f64],
    pub left_peak: usize,
    pub right_peak: usize,
    pub fs: u32,
    pub time_range_ms: (f64, f64),
}

pub fn plot_interaural_overlay(
    path: &Path,
    overlay: &InterauralOverlay<'_>,
) -> Result<(), PlotError> {
    let InterauralOverlay {
        speaker,
        left_ir,
        right_ir,
        left_peak,
        right_peak,
        fs,
        time_range_ms,
    } = *overlay;
    if fs == 0
        || !time_range_ms.0.is_finite()
        || !time_range_ms.1.is_finite()
        || time_range_ms.0 >= time_range_ms.1
        || left_ir.is_empty()
        || right_ir.is_empty()
    {
        return Err(PlotError::Invalid(
            "invalid overlay range or samples".into(),
        ));
    }
    // The time origin comes from the caller: ImpulseResponse.peak_index semantics
    // (the first signed extremum above -18 dB, core/impulse_response.py:62-64),
    // computed by impulcifer-dsp. The renderer never re-derives it.
    let segment = |ir: &[f64], peak: usize| {
        let peak = peak.min(ir.len().saturating_sub(1));
        let offset = (time_range_ms.0 * f64::from(fs) / 1000.0) as i64;
        let start = (peak as i64 + offset).max(0) as usize;
        let end = (peak as i64 + (time_range_ms.1 * f64::from(fs) / 1000.0) as i64)
            .clamp(0, ir.len() as i64) as usize;
        let start = start.min(end);
        let t: Vec<_> = (start..end)
            .map(|i| {
                time_range_ms.0 + (i as i64 - peak as i64 - offset) as f64 * 1000.0 / f64::from(fs)
            })
            .collect();
        (t, ir[start..end].to_vec())
    };
    let (lt, l) = segment(left_ir, left_peak);
    let (rt, r) = segment(right_ir, right_peak);
    let max = l.iter().chain(&r).map(|v| v.abs()).fold(0.0, f64::max);
    let lim = AxisLimits {
        x: padded_range(lt.iter().chain(&rt).copied()),
        y: if max > 0.0 {
            (-max * 1.1, max * 1.1)
        } else {
            padded_range([0.0])
        },
    };
    png_file(path, (1200, 700), |root| {
        linear_chart(
            root,
            &format!("{speaker} - Interaural Impulse Response Overlay"),
            "Time relative to peak (ms)",
            "Amplitude",
            &[
                line(&lt, &l, "Left Ear", RGBColor(76, 114, 176), 2),
                line(&rt, &r, "Right Ear", RGBColor(221, 132, 82), 2),
            ],
            lim,
        )
    })
}
