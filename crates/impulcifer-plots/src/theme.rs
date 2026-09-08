//! P23's fixed paper palette and shared geometry, in output pixels.
use plotters::prelude::*;

pub(crate) const FONT: &str = "DejaVu Sans";
pub(crate) const CANVAS: RGBColor = RGBColor(251, 251, 252);
pub(crate) const PANEL: RGBColor = RGBColor(255, 255, 255);
pub(crate) const INK: RGBColor = RGBColor(27, 31, 36);
pub(crate) const MUTED: RGBColor = RGBColor(107, 114, 128);
pub(crate) const GRID: RGBColor = RGBColor(229, 231, 235);
pub(crate) const MINOR: RGBColor = RGBColor(241, 243, 245);
pub(crate) const ZERO: RGBColor = RGBColor(156, 163, 175);
pub(crate) const LEFT: RGBColor = RGBColor(37, 99, 235);
pub(crate) const RIGHT: RGBColor = RGBColor(220, 38, 38);
pub(crate) const BOTH: RGBColor = RGBColor(17, 24, 39);
pub(crate) const TARGET: RGBColor = ZERO;
pub(crate) const CORRECTION: RGBColor = RGBColor(124, 58, 237);
pub(crate) const GUIDE: RGBColor = RGBColor(254, 243, 199);
pub(crate) const POSITIVE: RGBColor = RGBColor(219, 234, 254);
pub(crate) const NEGATIVE: RGBColor = RGBColor(254, 226, 226);
pub(crate) const RAW_ALPHA: f64 = 0.35;
pub(crate) const RAW_WIDTH: u32 = 1;
pub(crate) const SMOOTH_WIDTH: u32 = 3;
pub(crate) const TITLE: u32 = 34;
pub(crate) const SUBTITLE: u32 = 22;
pub(crate) const AXIS: u32 = 22;
pub(crate) const TICK: u32 = 18;
pub(crate) const LEGEND: u32 = 20;
pub(crate) const ANNOTATION: u32 = 20;
pub(crate) const SINGLE: (u32, u32) = (1600, 1000);
pub(crate) const SHEET: (u32, u32) = (2400, 1350);
pub(crate) const OVERLAY: (u32, u32) = (1600, 900);
pub(crate) const MARGIN: i32 = 64;
pub(crate) const GAP: i32 = 48;
pub(crate) const PAD: i32 = 12;
pub(crate) const ROW: i32 = 28;
pub(crate) const RADIUS: i32 = 8;
pub(crate) const ANNOTATION_INSET: i32 = 16;
pub(crate) const Y_LABEL: i32 = 80;
pub(crate) const X_LABEL: i32 = 64;
pub(crate) const BAND_HEIGHT: i32 = 48;
pub(crate) const FREQ_TICKS: [f64; 10] = [
    20., 50., 100., 200., 500., 1000., 2000., 5000., 10000., 20000.,
];
pub(crate) const BANDS: [(&str, &str, f64, f64); 6] = [
    ("SUB-BASS", "20–60", 20., 60.),
    ("BASS", "60–250", 60., 250.),
    ("LOW MIDS", "250–500", 250., 500.),
    ("MIDS", "500–2k", 500., 2000.),
    ("PRESENCE", "2k–6k", 2000., 6000.),
    ("AIR", "6k–20k", 6000., 20000.),
];
pub(crate) const DB_STEP: f64 = 6.;
pub(crate) const MAGMA_RANGE: (f64, f64) = (-80., 0.);
pub(crate) const OVERLAY_RANGE: (f64, f64) = (-1., 5.);
pub(crate) fn magma(db: f64) -> RGBColor {
    let bytes = include_bytes!("../assets/magma.rgb");
    let i = (((db - MAGMA_RANGE.0) / (MAGMA_RANGE.1 - MAGMA_RANGE.0)).clamp(0., 1.) * 255.).round()
        as usize
        * 3;
    RGBColor(bytes[i], bytes[i + 1], bytes[i + 2])
}
