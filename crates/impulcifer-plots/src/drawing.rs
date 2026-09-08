//! Shared static chart anatomy. Coordinates are mapped explicitly so frequency
//! ticks, six-dB intervals, dash lengths and label placement never depend on data.
use crate::{theme::*, *};
use plotters::style::text_anchor::{HPos, Pos, VPos};

pub(crate) fn text(
    area: &Area<'_>,
    value: &str,
    at: (i32, i32),
    size: u32,
    color: RGBColor,
) -> Result<(), PlotError> {
    area.draw(&Text::new(
        value,
        at,
        (FONT, size).into_font().color(&color),
    ))
    .map_err(render_error)
}
pub(crate) fn centered(
    area: &Area<'_>,
    value: &str,
    at: (i32, i32),
    size: u32,
) -> Result<(), PlotError> {
    area.draw(&Text::new(
        value,
        at,
        (FONT, size)
            .into_font()
            .color(&MUTED)
            .pos(Pos::new(HPos::Center, VPos::Top)),
    ))
    .map_err(render_error)
}
pub(crate) fn rule(
    area: &Area<'_>,
    a: (i32, i32),
    b: (i32, i32),
    color: RGBColor,
) -> Result<(), PlotError> {
    area.draw(&PathElement::new(vec![a, b], color.stroke_width(1)))
        .map_err(render_error)
}
pub(crate) fn wrapped(
    area: &Area<'_>,
    value: &str,
    x: i32,
    mut y: i32,
    width: i32,
    size: u32,
    color: RGBColor,
) -> Result<i32, PlotError> {
    let style = (FONT, size).into_font().color(&color);
    let mut line = String::new();
    for word in value.split_whitespace() {
        let next = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if area
            .estimate_text_size(&next, &style)
            .map_err(render_error)?
            .0 as i32
            > width
            && !line.is_empty()
        {
            text(area, &line, (x, y), size, color)?;
            y += size as i32 + PAD;
            line = word.to_owned();
        } else {
            line = next;
        }
    }
    text(area, &line, (x, y), size, color)?;
    Ok(y + size as i32 + PAD)
}
pub(crate) fn header(area: &Area<'_>, title: &str, subtitle: &str) -> Result<i32, PlotError> {
    let width = area.dim_in_pixel().0 as i32;
    let y = wrapped(area, title, 0, 0, width, TITLE, INK)?;
    wrapped(area, subtitle, 0, y, width, SUBTITLE, MUTED)
}
pub(crate) fn footer(area: &Area<'_>, lines: &[String]) -> Result<(), PlotError> {
    let (w, h) = area.dim_in_pixel();
    let mut y = h as i32 - lines.len() as i32 * ROW;
    rule(area, (0, y - PAD), (w as i32, y - PAD), GRID)?;
    for line in lines {
        text(area, line, (0, y), ANNOTATION, INK)?;
        y += ROW;
    }
    Ok(())
}
pub(crate) fn label(
    area: &Area<'_>,
    value: &str,
    at: (i32, i32),
    bounds: (i32, i32, i32, i32),
) -> Result<(), PlotError> {
    label_avoiding(area, value, at, bounds, &mut Vec::new())
}

pub(crate) fn label_avoiding(
    area: &Area<'_>,
    value: &str,
    at: (i32, i32),
    bounds: (i32, i32, i32, i32),
    occupied: &mut Vec<(i32, i32, i32, i32)>,
) -> Result<(), PlotError> {
    let (tw, th) = area
        .estimate_text_size(value, &(FONT, ANNOTATION).into_font().color(&INK))
        .map_err(render_error)?;
    let w = tw as i32 + 2 * PAD;
    let h = th as i32 + 2 * PAD;
    let Some((x, y)) = vacant_label_origin(at, (w, h), bounds, occupied) else {
        return Err(PlotError::Invalid(
            "annotation does not fit inside plot inset".into(),
        ));
    };
    occupied.push((x, y, x + w, y + h));
    label_box(area, value, (x, y, x + w, y + h))
}
pub(crate) fn label_size(area: &Area<'_>, value: &str) -> Result<(i32, i32), PlotError> {
    let (w, h) = area
        .estimate_text_size(value, &(FONT, ANNOTATION).into_font().color(&INK))
        .map_err(render_error)?;
    Ok((w as i32 + 2 * PAD, h as i32 + 2 * PAD))
}
pub(crate) fn label_box(
    area: &Area<'_>,
    value: &str,
    bounds: (i32, i32, i32, i32),
) -> Result<(), PlotError> {
    let (x, y, r, b) = bounds;
    let (w, h) = (r - x, b - y);
    let points: Vec<_> = [
        (x + RADIUS, y + RADIUS, 180.),
        (x + w - RADIUS, y + RADIUS, 270.),
        (x + w - RADIUS, y + h - RADIUS, 0.),
        (x + RADIUS, y + h - RADIUS, 90.),
    ]
    .into_iter()
    .flat_map(|(cx, cy, a)| {
        (0..=8).map(move |i| {
            let r = (a + i as f64 * 90. / 8.).to_radians();
            (
                cx + (RADIUS as f64 * r.cos()).round() as i32,
                cy + (RADIUS as f64 * r.sin()).round() as i32,
            )
        })
    })
    .collect();
    area.draw(&Polygon::new(points.clone(), PANEL.filled()))
        .map_err(render_error)?;
    let mut border = points;
    border.push(border[0]);
    area.draw(&PathElement::new(border, GRID.stroke_width(1)))
        .map_err(render_error)?;
    text(area, value, (x + PAD, y + PAD), ANNOTATION, INK)
}
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Dash {
    Solid,
    Short,
    Long,
    Dots,
}
impl Dash {
    fn pattern(self) -> (f64, f64) {
        match self {
            Self::Solid => (f64::INFINITY, 0.),
            Self::Short => (8., 5.),
            Self::Long => (18., 7.),
            Self::Dots => (3., 5.),
        }
    }
}
pub(crate) struct Line<'a> {
    pub x: &'a [f64],
    pub y: &'a [f64],
    pub label: &'a str,
    pub color: RGBColor,
    pub raw: bool,
    pub dash: Dash,
}
pub(crate) fn line<'a>(
    x: &'a [f64],
    y: &'a [f64],
    label: &'a str,
    color: RGBColor,
    dash: Dash,
) -> Line<'a> {
    Line {
        x,
        y,
        label,
        color,
        raw: false,
        dash,
    }
}
pub(crate) fn raw<'a>(x: &'a [f64], y: &'a [f64], color: RGBColor) -> Line<'a> {
    Line {
        x,
        y,
        label: "",
        color,
        raw: true,
        dash: Dash::Solid,
    }
}
fn stroke(
    area: &Area<'_>,
    points: &[(i32, i32)],
    color: RGBAColor,
    width: u32,
    dash: Dash,
) -> Result<(), PlotError> {
    if dash == Dash::Solid {
        return area
            .draw(&PathElement::new(
                points.to_vec(),
                color.stroke_width(width),
            ))
            .map_err(render_error);
    }
    let (on, off) = dash.pattern();
    let period = on + off;
    let mut phase = 0.;
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let dx = (b.0 - a.0) as f64;
        let dy = (b.1 - a.1) as f64;
        let length = dx.hypot(dy);
        let mut d = 0.;
        while d < length {
            let active = phase < on;
            let step = (if active { on - phase } else { period - phase })
                .min(length - d)
                .max(0.001);
            if active {
                let point = |t: f64| {
                    (
                        a.0 + (dx * t / length).round() as i32,
                        a.1 + (dy * t / length).round() as i32,
                    )
                };
                area.draw(&PathElement::new(
                    vec![point(d), point(d + step)],
                    color.stroke_width(width),
                ))
                .map_err(render_error)?;
            }
            d += step;
            phase = (phase + step) % period;
        }
    }
    Ok(())
}
pub(crate) fn legend(area: &Area<'_>, lines: &[Line<'_>], mut y: i32) -> Result<i32, PlotError> {
    let mut x = 0;
    let width = area.dim_in_pixel().0 as i32;
    for s in lines
        .iter()
        .filter(|s| !s.label.is_empty() && !s.y.is_empty())
    {
        let tw = area
            .estimate_text_size(s.label, &(FONT, LEGEND).into_font().color(&INK))
            .map_err(render_error)?
            .0 as i32;
        let need = GAP + tw + 2 * PAD;
        if x + need > width && x > 0 {
            x = 0;
            y += ROW;
        }
        stroke(
            area,
            &[(x, y + PAD), (x + GAP - PAD, y + PAD)],
            s.color.to_rgba(),
            SMOOTH_WIDTH,
            s.dash,
        )?;
        text(area, s.label, (x + GAP, y), LEGEND, INK)?;
        x += need;
    }
    Ok(y + ROW + PAD)
}
fn label_origin(at: (i32, i32), size: (i32, i32), bounds: (i32, i32, i32, i32)) -> (i32, i32) {
    let place = |anchor: i32, length: i32, lo: i32, hi: i32| {
        let lo = lo + ANNOTATION_INSET;
        let hi = hi - ANNOTATION_INSET - length;
        let candidate = if anchor + PAD > hi {
            anchor - PAD - length
        } else {
            anchor + PAD
        };
        candidate.clamp(lo, hi.max(lo))
    };
    (
        place(at.0, size.0, bounds.0, bounds.2),
        place(at.1, size.1, bounds.1, bounds.3),
    )
}
fn vacant_label_origin(
    at: (i32, i32),
    size: (i32, i32),
    bounds: (i32, i32, i32, i32),
    occupied: &[(i32, i32, i32, i32)],
) -> Option<(i32, i32)> {
    let (l, t, r, b) = (
        bounds.0 + ANNOTATION_INSET,
        bounds.1 + ANNOTATION_INSET,
        bounds.2 - ANNOTATION_INSET - size.0,
        bounds.3 - ANNOTATION_INSET - size.1,
    );
    if r < l || b < t {
        return None;
    }
    let preferred = label_origin(at, size, bounds);
    let fits = |&(x, y): &(i32, i32)| {
        occupied.iter().all(|&(ol, ot, or, ob)| {
            x + size.0 + PAD <= ol || x >= or + PAD || y + size.1 + PAD <= ot || y >= ob + PAD
        })
    };
    if fits(&preferred) {
        return Some(preferred);
    }
    (t..=b)
        .step_by(PAD as usize)
        .flat_map(|y| (l..=r).step_by(PAD as usize).map(move |x| (x, y)))
        .filter(fits)
        .min_by_key(|&(x, y)| (x - preferred.0).pow(2) + (y - preferred.1).pow(2))
}
pub(crate) fn significant(v: f64) -> String {
    if v == 0. {
        return "0".into();
    }
    let exponent = v.abs().log10().floor() as i32;
    if !(-4..3).contains(&exponent) {
        format!("{v:.2e}")
    } else {
        format!("{:.*}", (2 - exponent).max(0) as usize, v)
    }
}
fn db_ticks(range: (f64, f64)) -> Vec<f64> {
    let span = range.1 - range.0;
    let step = DB_STEP
        * if span <= 48. {
            1.
        } else if span <= 96. {
            2.
        } else {
            4.
        };
    let first = (range.0 / step).ceil() as i32;
    let last = (range.1 / step).floor() as i32;
    (first..=last).map(|i| i as f64 * step).collect()
}
/// Round the step, not individual ticks; expand both ends to cover the input.
pub(crate) fn linear_ticks(range: (f64, f64), intervals: usize) -> Vec<f64> {
    let rough = (range.1 - range.0) / intervals as f64;
    let power = 10_f64.powf(rough.log10().floor());
    let step = [1., 2., 5., 10.]
        .into_iter()
        .map(|m| m * power)
        .find(|s| *s >= rough)
        .unwrap_or(power * 10.);
    let first = (range.0 / step).floor();
    let last = (range.1 / step).ceil();
    (0..=(last - first).round() as usize)
        .map(|i| (first + i as f64) * step)
        .collect()
}
pub(crate) fn time_ticks(range: (f64, f64)) -> Vec<f64> {
    if range == OVERLAY_RANGE {
        (0..=12).map(|i| -1. + i as f64 * 0.5).collect()
    } else {
        linear_ticks(range, 5)
    }
}
pub(crate) fn tick_labels(ticks: &[f64]) -> Vec<String> {
    let step_exponent = (ticks[1] - ticks[0]).abs().log10().floor() as i32;
    ticks
        .iter()
        .map(|&v| {
            if v == 0. {
                return "0".into();
            }
            if v.abs() < 0.0001 || v.abs() >= 1e6 {
                let precision =
                    (v.abs().log10().floor() as i32 - step_exponent).clamp(0, 16) as usize;
                format!("{v:.precision$e}")
            } else {
                let precision = (-step_exponent).clamp(0, 16) as usize;
                let s = format!("{v:.precision$}");
                if s.contains('.') {
                    s.trim_end_matches('0').trim_end_matches('.').into()
                } else {
                    s
                }
            }
        })
        .collect()
}
pub(crate) fn tick_range(ticks: &[f64]) -> (f64, f64) {
    (ticks[0], ticks[ticks.len() - 1])
}
/// Every axis (including heatmap and inset) measures its own actual tick strings.
pub(crate) fn y_column(
    area: &Area<'_>,
    labels: &[String],
    title: &str,
) -> Result<(i32, i32), PlotError> {
    let style = (FONT, TICK).into_font().color(&MUTED);
    let mut widest = 0;
    for label in labels {
        widest = widest.max(
            area.estimate_text_size(label, &style)
                .map_err(render_error)?
                .0 as i32,
        );
    }
    let title_width = area
        .estimate_text_size(title, &(FONT, AXIS).into_font().color(&MUTED))
        .map_err(render_error)?
        .1 as i32;
    let left = (3 * PAD + title_width + widest).max(Y_LABEL);
    Ok((left, left - PAD - widest - PAD - title_width))
}
pub(crate) fn db_range(values: impl IntoIterator<Item = f64>) -> (f64, f64) {
    let r = padded_range(values);
    (
        (r.0 / DB_STEP).floor() * DB_STEP,
        (r.1 / DB_STEP).ceil() * DB_STEP,
    )
}
pub(crate) fn frequency(v: f64) -> String {
    if v >= 1000. {
        format!("{:.1} kHz", v / 1000.)
    } else {
        format!("{v:.1} Hz")
    }
}
fn tick_frequency(v: f64) -> String {
    if v >= 1000. {
        format!("{:.0} kHz", v / 1000.)
    } else {
        format!("{v:.0} Hz")
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Axes {
    pub bounds: (i32, i32, i32, i32),
    pub limits: AxisLimits,
    pub log_x: bool,
    pub log_y: bool,
}
impl Axes {
    pub fn map(&self, x: f64, y: f64) -> (i32, i32) {
        let (l, t, r, b) = self.bounds;
        let fraction = |v: f64, range: (f64, f64), log: bool| {
            if log {
                (v.log10() - range.0.log10()) / (range.1.log10() - range.0.log10())
            } else {
                (v - range.0) / (range.1 - range.0)
            }
        };
        (
            l + ((r - l) as f64 * fraction(x, self.limits.x, self.log_x)).round() as i32,
            b - ((b - t) as f64 * fraction(y, self.limits.y, self.log_y)).round() as i32,
        )
    }
    pub fn contains(&self, x: f64, y: f64) -> bool {
        (self.limits.x.0..=self.limits.x.1).contains(&x)
            && (self.limits.y.0..=self.limits.y.1).contains(&y)
    }
}
pub(crate) fn bands(area: &Area<'_>, bounds: (i32, i32, i32, i32)) -> Result<(), PlotError> {
    let (_, t, _, _) = bounds;
    let t = t - PAD;
    let (l, r) = (0, area.dim_in_pixel().0 as i32 - PAD);
    // Equal-width keys keep the full names legible on the six-panel sheet;
    // numerical boundaries state the bands without pretending equal log widths.
    for (i, (name, range, _, _)) in BANDS.iter().enumerate() {
        let x = l + (r - l) * i as i32 / 6;
        let end = l + (r - l) * (i + 1) as i32 / 6;
        area.draw(&Rectangle::new(
            [(x, t - BAND_HEIGHT), (end, t)],
            MINOR.filled(),
        ))
        .map_err(render_error)?;
        centered(area, name, ((x + end) / 2, t - BAND_HEIGHT), TICK)?;
        centered(area, range, ((x + end) / 2, t - ROW), TICK)?;
        rule(area, (x, t - BAND_HEIGHT), (x, t), GRID)?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)] // Explicit chart anatomy, shared by all renderers.
pub(crate) fn axes(
    area: &Area<'_>,
    top: i32,
    bottom: i32,
    limits: AxisLimits,
    frequency_axis: bool,
    db: bool,
    xlabel: &str,
    ylabel: &str,
) -> Result<Axes, PlotError> {
    valid_limits(limits)?;
    let mut limits = limits;
    let xs = if frequency_axis {
        FREQ_TICKS.to_vec()
    } else {
        time_ticks(limits.x)
    };
    let ys = if db {
        db_ticks(limits.y)
    } else {
        linear_ticks(limits.y, 4)
    };
    if !frequency_axis {
        limits.x = tick_range(&xs);
    }
    if !db {
        limits.y = tick_range(&ys);
    }
    let labels = if db {
        ys.iter().map(|v| format!("{v:.0}")).collect()
    } else {
        tick_labels(&ys)
    };
    let (left, title_x) = y_column(area, &labels, ylabel)?;
    let a = Axes {
        bounds: (
            left,
            top + PAD + if frequency_axis { BAND_HEIGHT } else { 0 },
            area.dim_in_pixel().0 as i32 - GAP,
            bottom - X_LABEL,
        ),
        limits,
        log_x: frequency_axis,
        log_y: false,
    };
    let (l, t, r, b) = a.bounds;
    area.draw(&Rectangle::new([(l, t), (r, b)], PANEL.filled()))
        .map_err(render_error)?;
    if frequency_axis {
        bands(area, a.bounds)?;
    }
    let xlabels = if frequency_axis {
        xs.iter().map(|v| tick_frequency(*v)).collect()
    } else {
        tick_labels(&xs)
    };
    for (v, name) in xs.into_iter().zip(xlabels) {
        let x = a.map(v, limits.y.0).0;
        rule(area, (x, t), (x, b), GRID)?;
        // Alternate frequency label rows prevent the closest log ticks colliding.
        let row = if frequency_axis && [50., 200., 2000., 10000.].contains(&v) {
            TICK as i32
        } else {
            0
        };
        centered(area, &name, (x, b + PAD + row), TICK)?;
    }
    for (v, name) in ys.into_iter().zip(labels) {
        let y = a.map(limits.x.0, v).1;
        rule(area, (l, y), (r, y), if v == 0. { ZERO } else { GRID })?;
        area.draw(&Text::new(
            name,
            (l - PAD, y),
            (FONT, TICK)
                .into_font()
                .color(&MUTED)
                .pos(Pos::new(HPos::Right, VPos::Center)),
        ))
        .map_err(render_error)?;
    }
    centered(area, xlabel, ((l + r) / 2, bottom - AXIS as i32), AXIS)?;
    area.draw(&Text::new(
        ylabel,
        (title_x, (t + b) / 2),
        (FONT, AXIS)
            .into_font()
            .color(&MUTED)
            .transform(FontTransform::Rotate270)
            .pos(Pos::new(HPos::Center, VPos::Top)),
    ))
    .map_err(render_error)?;
    Ok(a)
}
pub(crate) fn valid_limits(l: AxisLimits) -> Result<(), PlotError> {
    if [l.x.0, l.x.1, l.y.0, l.y.1].iter().any(|v| !v.is_finite())
        || l.x.0 >= l.x.1
        || l.y.0 >= l.y.1
    {
        Err(PlotError::Invalid("invalid axis limits".into()))
    } else {
        Ok(())
    }
}
/// A data-space segment clipped to the axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ClippedSegment {
    pub start: (f64, f64),
    pub end: (f64, f64),
    /// The original start was inside the axes (nothing was cut there).
    pub starts_inside: bool,
    /// The original end was inside the axes.
    pub ends_inside: bool,
}
/// Liang-Barsky clipping of the segment `p0`-`p1` against the axis limits, done in
/// the axes' own coordinates (log10 on logarithmic axes) so a straight line on the
/// picture stays straight; the endpoints come back in data space. `None` when the
/// segment lies entirely outside.
pub(crate) fn clip_segment(a: &Axes, p0: (f64, f64), p1: (f64, f64)) -> Option<ClippedSegment> {
    let tx = |v: f64| if a.log_x { v.log10() } else { v };
    let ty = |v: f64| if a.log_y { v.log10() } else { v };
    let (u0, v0) = (tx(p0.0), ty(p0.1));
    let (du, dv) = (tx(p1.0) - u0, ty(p1.1) - v0);
    let (u_min, u_max) = (tx(a.limits.x.0), tx(a.limits.x.1));
    let (v_min, v_max) = (ty(a.limits.y.0), ty(a.limits.y.1));
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-du, u0 - u_min),
        (du, u_max - u0),
        (-dv, v0 - v_min),
        (dv, v_max - v0),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            if r > t1 {
                return None;
            }
            t0 = t0.max(r);
        } else {
            if r < t0 {
                return None;
            }
            t1 = t1.min(r);
        }
    }
    let back_x = |u: f64| if a.log_x { 10_f64.powf(u) } else { u };
    let back_y = |v: f64| if a.log_y { 10_f64.powf(v) } else { v };
    let at = |t: f64| {
        if t == 0.0 {
            p0
        } else if t == 1.0 {
            p1
        } else {
            (back_x(u0 + t * du), back_y(v0 + t * dv))
        }
    };
    Some(ClippedSegment {
        start: at(t0),
        end: at(t1),
        starts_inside: t0 == 0.0,
        ends_inside: t1 == 1.0,
    })
}
pub(crate) fn draw_lines(area: &Area<'_>, a: Axes, lines: &[Line<'_>]) -> Result<(), PlotError> {
    for s in lines {
        if s.y.is_empty() {
            continue;
        }
        if s.x.len() != s.y.len() || s.x.iter().chain(s.y).any(|v| !v.is_finite()) {
            return Err(PlotError::Invalid(
                "matching finite line arrays required".into(),
            ));
        }
        let color = s.color.mix(if s.raw { RAW_ALPHA } else { 1. });
        let width = if s.raw { RAW_WIDTH } else { SMOOTH_WIDTH };
        let mut points: Vec<(i32, i32)> = Vec::new();
        if s.x.len() == 1 && a.contains(s.x[0], s.y[0]) {
            points.push(a.map(s.x[0], s.y[0]));
        }
        // Every segment is clipped to the axes, so a curve that leaves the frame is
        // drawn up to the boundary and picked up again where it comes back, instead
        // of vanishing between its last inside sample and its first outside one.
        let mut last_ended_inside = false;
        for i in 1..s.x.len() {
            let Some(c) = clip_segment(&a, (s.x[i - 1], s.y[i - 1]), (s.x[i], s.y[i])) else {
                if !points.is_empty() {
                    stroke(area, &points, color, width, s.dash)?;
                    points.clear();
                }
                last_ended_inside = false;
                continue;
            };
            if !(c.starts_inside && last_ended_inside) && !points.is_empty() {
                stroke(area, &points, color, width, s.dash)?;
                points.clear();
            }
            for (x, y) in [c.start, c.end] {
                let p = a.map(x, y);
                if points.last() != Some(&p) {
                    points.push(p);
                }
            }
            last_ended_inside = c.ends_inside;
        }
        stroke(area, &points, color, width, s.dash)?;
    }
    Ok(())
}
pub(crate) fn signed_fill(area: &Area<'_>, a: Axes, x: &[f64], y: &[f64]) -> Result<(), PlotError> {
    for (xs, ys) in x.windows(2).zip(y.windows(2)) {
        if xs[0] < a.limits.x.0 || xs[1] > a.limits.x.1 {
            continue;
        }
        for positive in [true, false] {
            let clip = |v: f64| if positive { v.max(0.) } else { v.min(0.) };
            let mut x0 = xs[0];
            let mut x1 = xs[1];
            if ys[0] * ys[1] < 0. {
                let f = -ys[0] / (ys[1] - ys[0]);
                let crossing = if a.log_x {
                    (xs[0].ln() + f * (xs[1].ln() - xs[0].ln())).exp()
                } else {
                    xs[0] + f * (xs[1] - xs[0])
                };
                if (ys[0] > 0.) == positive {
                    x1 = crossing;
                } else {
                    x0 = crossing;
                }
            }
            area.draw(&Polygon::new(
                vec![
                    a.map(x0, 0.),
                    a.map(x0, clip(ys[0])),
                    a.map(x1, clip(ys[1])),
                    a.map(x1, 0.),
                ],
                if positive {
                    POSITIVE.filled()
                } else {
                    NEGATIVE.filled()
                },
            ))
            .map_err(render_error)?;
        }
    }
    Ok(())
}
pub(crate) fn empty(area: &Area<'_>, title: &str, reason: &str) -> Result<(), PlotError> {
    let y = header(area, title, "No data available for this panel")?;
    label(
        area,
        reason,
        (PAD, y + GAP),
        (
            0,
            y,
            area.dim_in_pixel().0 as i32,
            area.dim_in_pixel().1 as i32,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn segments_are_clipped_to_the_axes_in_axis_space() {
        let a = Axes {
            bounds: (0, 0, 100, 100),
            limits: AxisLimits {
                x: (0., 10.),
                y: (0., 1.),
            },
            log_x: false,
            log_y: false,
        };
        let inside = clip_segment(&a, (2., 0.2), (4., 0.8)).unwrap();
        assert_eq!(inside.start, (2., 0.2));
        assert_eq!(inside.end, (4., 0.8));
        assert!(inside.starts_inside && inside.ends_inside);
        let top = clip_segment(&a, (2., 0.5), (4., 1.5)).unwrap();
        assert_eq!(top.start, (2., 0.5));
        assert!((top.end.0 - 3.).abs() < 1e-12 && (top.end.1 - 1.).abs() < 1e-12);
        assert!(top.starts_inside && !top.ends_inside);
        let through = clip_segment(&a, (0., -1.), (10., 3.)).unwrap();
        assert!((through.start.0 - 2.5).abs() < 1e-12 && through.start.1.abs() < 1e-12);
        assert!((through.end.0 - 5.).abs() < 1e-12 && (through.end.1 - 1.).abs() < 1e-12);
        assert!(!through.starts_inside && !through.ends_inside);
        assert!(clip_segment(&a, (0., 2.), (10., 3.)).is_none());
        assert!(clip_segment(&a, (11., 0.5), (12., 0.5)).is_none());
        let log = Axes {
            bounds: (0, 0, 100, 100),
            limits: AxisLimits {
                x: (10., 1000.),
                y: (0., 1.),
            },
            log_x: true,
            log_y: false,
        };
        let left = clip_segment(&log, (1., 0.5), (100., 0.5)).unwrap();
        assert!((left.start.0 - 10.).abs() < 1e-9 && (left.start.1 - 0.5).abs() < 1e-12);
        assert_eq!(left.end, (100., 0.5));
        assert!(!left.starts_inside && left.ends_inside);
    }
    #[test]
    fn curves_leaving_the_frame_are_drawn_up_to_the_boundary() {
        font().unwrap();
        let (w, h) = (200_usize, 100_usize);
        let mut buffer = vec![255_u8; w * h * 3];
        {
            let root =
                BitMapBackend::with_buffer(&mut buffer, (w as u32, h as u32)).into_drawing_area();
            let a = Axes {
                bounds: (0, 0, w as i32 - 1, h as i32 - 1),
                limits: AxisLimits {
                    x: (0., 10.),
                    y: (0., 1.),
                },
                log_x: false,
                log_y: false,
            };
            // A spike above the frame: the curve must reach the top edge at x = 2.5
            // and come back at x = 7.5, not stop at the last inside sample.
            let x = [0., 5., 10.];
            let y = [0.5, 1.5, 0.5];
            draw_lines(
                &root,
                a,
                &[Line {
                    x: &x,
                    y: &y,
                    label: "",
                    color: RGBColor(0, 0, 255),
                    raw: false,
                    dash: Dash::Solid,
                }],
            )
            .unwrap();
            root.present().unwrap();
        }
        let inked = |px: usize, py: usize| {
            let i = (py * w + px) * 3;
            buffer[i] != 255 || buffer[i + 1] != 255 || buffer[i + 2] != 255
        };
        for cx in [50_usize, 149] {
            assert!(
                (0..3).any(|dy| (cx.saturating_sub(3)..=cx + 3).any(|px| inked(px, dy))),
                "no ink at the top edge near x={cx}"
            );
        }
    }
    #[test]
    fn nice_linear_steps_cover_asymmetric_tiny_and_large_ranges() {
        for exponent in -12..=12 {
            let scale = 10_f64.powi(exponent);
            for (lo, hi) in [
                (-0.00727, 0.0468),
                (-3.17, -0.2),
                (1.01, 1.09),
                (1.00001, 1.00009),
                (-2., 2.),
            ] {
                let range = (lo * scale, hi * scale);
                let ticks = linear_ticks(range, 4);
                assert!(ticks[0] <= range.0 && ticks[ticks.len() - 1] >= range.1);
                let step = ticks[1] - ticks[0];
                let unit = step / 10_f64.powf(step.log10().floor());
                assert!([1., 2., 5., 10.].iter().any(|m| (unit - m).abs() < 1e-9));
                assert!(
                    ticks
                        .windows(2)
                        .all(|w| ((w[1] - w[0]) / step - 1.).abs() < 1e-9)
                );
                let labels = tick_labels(&ticks);
                assert!(labels.windows(2).all(|w| w[0] != w[1]));
                for (v, s) in ticks.iter().zip(labels) {
                    assert_eq!(*v == 0., s.parse::<f64>().unwrap() == 0.);
                }
            }
        }
        assert_eq!(
            linear_ticks((-0.02, 0.02), 4),
            vec![-0.02, -0.01, 0., 0.01, 0.02]
        );
    }
    #[test]
    fn every_actual_tick_fits_between_rotated_title_and_axis_including_inset() {
        font().unwrap();
        let mut buffer = vec![255; 720 * 480 * 3];
        let root = BitMapBackend::with_buffer(&mut buffer, (720, 480)).into_drawing_area();
        for size in [(720, 480), (480, 240)] {
            let area = root.clone().shrink((0, 0), size);
            for range in [
                (-0.00727, 0.0468),
                (-3e-8, 2e-8),
                (-99999., 20000.),
                (-1., 1.),
            ] {
                let a = axes(
                    &area,
                    ROW,
                    size.1,
                    AxisLimits {
                        x: (0., 10.),
                        y: range,
                    },
                    false,
                    false,
                    "Time (ms)",
                    "Amplitude (FS)",
                )
                .unwrap();
                let labels = tick_labels(&linear_ticks(range, 4));
                let (left, title_x) = y_column(&area, &labels, "Amplitude (FS)").unwrap();
                assert_eq!(a.bounds.0, left);
                let title_width = area
                    .estimate_text_size("Amplitude (FS)", &(FONT, AXIS).into_font().color(&MUTED))
                    .unwrap()
                    .1 as i32;
                for s in labels {
                    let width = area
                        .estimate_text_size(&s, &(FONT, TICK).into_font().color(&MUTED))
                        .unwrap()
                        .0 as i32;
                    assert!(title_x >= PAD);
                    assert!(title_x + title_width + PAD <= left - PAD - width);
                }
                assert!(a.bounds.2 - a.bounds.0 > 200);
            }
        }
        // The widest label can occur between the endpoints; measure every string.
        let labels = vec!["0".into(), "-0.000000001".into(), "1".into()];
        assert!(
            y_column(&root, &labels, "FS").unwrap().0
                > y_column(&root, &["0".into(), "1".into()], "FS").unwrap().0
        );
    }
    #[test]
    fn level_tick_steps_follow_visible_span() {
        for (range, step) in [((-24., 24.), 6.), ((-48., 48.), 12.), ((-200., 6.), 24.)] {
            let ticks = db_ticks(range);
            assert!(ticks.contains(&0.));
            assert!(ticks.windows(2).all(|w| w[1] - w[0] == step));
        }
        assert_eq!(db_ticks((-48., 6.)), vec![-48., -36., -24., -12., 0.]);
    }
    #[test]
    fn amplitudes_never_round_nonzero_values_to_zero() {
        assert_eq!(significant(0.0138), "0.0138");
        assert_eq!(significant(0.), "0");
        for value in [1e-308, -1e-200, 0.00000001, 0.00123, -0.00001, 1e30] {
            let parsed = significant(value).parse::<f64>().unwrap();
            assert_ne!(parsed, 0.);
            assert!((parsed / value - 1.).abs() < 0.005);
        }
    }
    #[test]
    fn overlay_time_ticks_are_half_milliseconds() {
        let ticks = time_ticks(OVERLAY_RANGE);
        assert_eq!(ticks.len(), 13);
        assert_eq!(ticks[0], -1.);
        assert_eq!(ticks[12], 5.);
        assert!(ticks.windows(2).all(|w| w[1] - w[0] == 0.5));
    }
    #[test]
    fn clustered_annotations_fit_without_overlap() {
        let bounds = (80, 200, 660, 400);
        let size = (180, 44);
        let mut occupied = Vec::new();
        for anchor in [(140, 210), (150, 220), (160, 230)] {
            let (x, y) = vacant_label_origin(anchor, size, bounds, &occupied).unwrap();
            assert!(
                occupied
                    .iter()
                    .all(|&(l, t, r, b)| x + size.0 <= l || x >= r || y + size.1 <= t || y >= b)
            );
            assert!(x >= 96 && x + size.0 <= 644 && y >= 216 && y + size.1 <= 384);
            occupied.push((x, y, x + size.0, y + size.1));
        }
        assert!(vacant_label_origin((100, 200), (900, 44), bounds, &[]).is_none());
    }
    #[test]
    fn measured_annotation_flips_and_stays_inside_plot_inset() {
        let bounds = (80, 100, 680, 450);
        let size = (240, 44);
        for anchor in [(80, 100), (680, 450), (500, 100), (400, 300), (-50, -50)] {
            let (x, y) = label_origin(anchor, size, bounds);
            assert!(x >= bounds.0 + ANNOTATION_INSET);
            assert!(y >= bounds.1 + ANNOTATION_INSET);
            assert!(x + size.0 <= bounds.2 - ANNOTATION_INSET);
            assert!(y + size.1 <= bounds.3 - ANNOTATION_INSET);
        }
        assert!(label_origin((650, 400), size, bounds).0 < 650 - size.0);
    }
}
