#![forbid(unsafe_code)]
//! Headless, embedded-font PNG rendering. Pipeline data preparation belongs to
//! the service; the early-reflection envelope is derived only for display.
use plotters::{coord::Shift, prelude::*};
use std::{fs::File, io::BufWriter, path::Path, sync::OnceLock};

mod charts;
mod drawing;
mod reflections;
mod theme;
pub use charts::*;
use theme::*;

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
/// Stable legacy limit slots: recording, IR, decay, spectrogram, FR, waterfall.
/// Panel six now displays direct-relative reflections using its own fixed time window.
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
        root.fill(&CANVAS).map_err(render_error)?;
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
