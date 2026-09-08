//! Chart-specific layouts; all inputs remain borrowed and unmodified.
use crate::{drawing::*, theme::*, *};

fn content<'a>(root: &Area<'a>) -> Area<'a> {
    root.margin(MARGIN, MARGIN, MARGIN, MARGIN)
}
fn validate_curve(c: &FrCurve) -> Result<(), PlotError> {
    validate(
        &c.frequency,
        &[
            &c.raw,
            &c.smoothed,
            &c.error,
            &c.error_smoothed,
            &c.equalization,
            &c.equalized_raw,
            &c.target,
        ],
    )
}
fn curve_lines<'a>(c: &'a FrCurve, color: RGBColor, name: &'a str, dash: Dash) -> Vec<Line<'a>> {
    let mut measured = raw(&c.frequency, &c.raw, color);
    if c.smoothed.is_empty() {
        measured.label = name;
        measured.dash = dash;
    }
    vec![measured, line(&c.frequency, &c.smoothed, name, color, dash)]
}
fn correction_lines<'a>(c: &'a FrCurve, name: &'a str, dash: Dash) -> Vec<Line<'a>> {
    vec![
        line(&c.frequency, &c.equalization, name, CORRECTION, dash),
        line(&c.frequency, &c.target, "Target", TARGET, Dash::Long),
    ]
}
fn fr_panel(
    area: &Area<'_>,
    title: &str,
    subtitle: &str,
    lines: &[Line<'_>],
    range: Option<(f64, f64)>,
    fill: Option<(&[f64], &[f64])>,
    guide: bool,
) -> Result<Axes, PlotError> {
    for s in lines {
        validate(s.x, &[s.y])?;
    }
    let y = header(area, title, subtitle)?;
    let top = legend(area, lines, y)?;
    let range = range.unwrap_or_else(|| {
        db_range(
            lines
                .iter()
                .flat_map(|s| {
                    s.x.iter()
                        .zip(s.y)
                        .filter(|(f, _)| (20.0..=20000.0).contains(*f))
                        .map(|(_, v)| *v)
                })
                .chain(fill.into_iter().flat_map(|(_, v)| v.iter().copied()))
                .chain([0.]),
        )
    });
    let a = axes(
        area,
        top,
        area.dim_in_pixel().1 as i32,
        AxisLimits {
            x: (20., 20000.),
            y: range,
        },
        true,
        true,
        "Frequency (Hz)",
        "Level (dB)",
    )?;
    if guide {
        area.draw(&Rectangle::new(
            [a.map(20., 3.), a.map(20000., -3.)],
            GUIDE.filled(),
        ))
        .map_err(render_error)?;
    }
    if let Some((x, y)) = fill {
        signed_fill(area, a, x, y)?;
    }
    if a.contains(20., 0.) {
        rule(area, a.map(20., 0.), a.map(20000., 0.), ZERO)?;
    }
    draw_lines(area, a, lines)?;
    Ok(a)
}
fn correction_footer(curves: &[&FrCurve]) -> String {
    let values: Vec<_> = curves
        .iter()
        .flat_map(|c| {
            c.frequency
                .iter()
                .zip(&c.equalization)
                .filter(|(f, _)| (40.0..=16000.0).contains(*f))
                .map(|(_, v)| *v)
        })
        .collect();
    if values.is_empty() {
        "Correction range: unavailable (no correction curve supplied)".into()
    } else {
        format!(
            "Correction range: {:+.1} … {:+.1} dB",
            values.iter().copied().fold(f64::INFINITY, f64::min),
            values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        )
    }
}
fn difference_summary(f: &[f64], d: &[f64]) -> Vec<String> {
    let band: Vec<_> = f
        .iter()
        .zip(d)
        .filter(|(f, _)| (200.0..=8000.0).contains(*f))
        .map(|(_, v)| *v)
        .collect();
    let average = if band.is_empty() {
        "Average level difference 200 Hz–8 kHz: unavailable (no samples in band)".into()
    } else {
        let mean = band.iter().sum::<f64>() / band.len() as f64;
        format!(
            "Average level difference 200 Hz–8 kHz: {mean:+.1} dB ({})",
            if mean.abs() < 0.05 {
                "balanced to 0.1 dB"
            } else if mean > 0. {
                "left louder"
            } else {
                "right louder"
            }
        )
    };
    let largest = f
        .iter()
        .zip(d)
        .filter(|(f, _)| (40.0..=16000.0).contains(*f))
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
    vec![
        average,
        largest.map_or_else(
            || "Largest difference (40 Hz–16 kHz): unavailable (no samples in view)".into(),
            |(f, v)| {
                format!(
                    "Largest difference (40 Hz–16 kHz): {v:+.1} dB at {}",
                    frequency(*f)
                )
            },
        ),
    ]
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
    png_file(path, SINGLE, |root| {
        let area = content(root);
        let footer_lines = difference_summary(&left.frequency, &difference);
        footer(&area, &footer_lines)?;
        let plot = area.margin(0, ROW * 2 + PAD * 2, 0, 0);
        let height = (plot.dim_in_pixel().1 as i32 - GAP) / 2;
        let (top, bottom) = plot.split_vertically(height);
        let bottom = bottom.margin(GAP, 0, 0, 0);
        fr_panel(
            &top,
            "How each ear hears the room",
            "Faint lines show raw measurements; bold lines show the overall response.",
            &[
                raw(&left.frequency, &left.raw, LEFT),
                raw(&right.frequency, &right.raw, RIGHT),
                line(
                    &left.frequency,
                    &left.smoothed,
                    "Left ear",
                    LEFT,
                    Dash::Solid,
                ),
                line(
                    &right.frequency,
                    &right.smoothed,
                    "Right ear",
                    RIGHT,
                    Dash::Short,
                ),
            ],
            None,
            None,
            false,
        )?;
        let range = db_range(difference.iter().copied().chain([-3., 3.]));
        let a = fr_panel(
            &bottom,
            "Balance: left minus right",
            "Above zero: left louder. Below zero: right louder. Shaded guide: ±3 dB.",
            &[line(
                &left.frequency,
                &difference,
                "Left minus right",
                BOTH,
                Dash::Solid,
            )],
            Some(range),
            Some((&left.frequency, &difference)),
            true,
        )?;
        if let Some((f, v)) = left
            .frequency
            .iter()
            .zip(&difference)
            .filter(|(f, _)| (40.0..=16000.0).contains(*f))
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        {
            let p = a.map(*f, *v);
            label(
                &bottom,
                &format!("{v:+.1} dB at {}", frequency(*f)),
                (p.0, a.bounds.1),
                a.bounds,
            )?;
        }
        Ok(())
    })
}
const HEADPHONES_TITLE: &str = "Your headphones as measured";
const HEADPHONES_SUBTITLE: &str = "Impulcifer flattens this in the final equalization; the flatter the bold line, the less it has to do.";
fn headphones_deviation(c: &FrCurve) -> Vec<(f64, f64)> {
    let measured = if c.smoothed.is_empty() {
        &c.raw
    } else {
        &c.smoothed
    };
    c.frequency
        .iter()
        .zip(measured)
        .zip(&c.target)
        .map(|((&f, &v), &target)| (f, v - target))
        .collect()
}
fn headphones_footer(left: &FrCurve, right: &FrCurve) -> Vec<String> {
    let deviations = [headphones_deviation(left), headphones_deviation(right)];
    let mean = |points: &[(f64, f64)]| {
        let band: Vec<_> = points
            .iter()
            .filter(|(f, _)| (100.0..=10000.0).contains(f))
            .collect();
        if band.is_empty() {
            "unavailable".into()
        } else {
            format!(
                "{:+.1} dB",
                band.iter().map(|(_, v)| v).sum::<f64>() / band.len() as f64
            )
        }
    };
    let largest = deviations
        .iter()
        .flatten()
        .filter(|(f, _)| (40.0..=16000.0).contains(f))
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
    vec![
        format!(
            "Average deviation from target 100 Hz–10 kHz: left {}, right {}",
            mean(&deviations[0]),
            mean(&deviations[1])
        ),
        largest.map_or_else(
            || "Largest deviation (40 Hz–16 kHz): unavailable (no target-relative samples)".into(),
            |(f, v)| {
                format!(
                    "Largest deviation (40 Hz–16 kHz): {v:+.1} dB at {}",
                    frequency(*f)
                )
            },
        ),
    ]
}
pub fn plot_headphones(
    path: &Path,
    left: &FrCurve,
    right: &FrCurve,
    gain_left_db: f64,
    gain_right_db: f64,
) -> Result<(), PlotError> {
    headphones_limits(left, right)?;
    validate_curve(left)?;
    validate_curve(right)?;
    if !gain_left_db.is_finite() || !gain_right_db.is_finite() {
        return Err(PlotError::Invalid("finite headphone gains required".into()));
    }
    let mut lines = curve_lines(left, LEFT, "Left ear", Dash::Solid);
    lines.extend(curve_lines(right, RIGHT, "Right ear", Dash::Short));
    lines.push(line(
        &left.frequency,
        &left.target,
        "Target",
        TARGET,
        Dash::Long,
    ));
    if left.target != right.target {
        lines.push(line(
            &right.frequency,
            &right.target,
            "Target · right",
            TARGET,
            Dash::Dots,
        ));
    }
    png_file(path, SINGLE, |root| {
        let area = content(root);
        footer(&area, &headphones_footer(left, right))?;
        fr_panel(
            &area.margin(0, ROW * 2 + PAD * 2, 0, 0),
            HEADPHONES_TITLE,
            HEADPHONES_SUBTITLE,
            &lines,
            None,
            None,
            false,
        )?;
        Ok(())
    })
}
pub fn plot_eq(
    path: &Path,
    left: Option<&FrCurve>,
    right: Option<&FrCurve>,
) -> Result<(), PlotError> {
    // No EQ input means no output file, as in the service's existing PNG set contract.
    if left.is_none() && right.is_none() {
        return Ok(());
    }
    for c in [left, right].into_iter().flatten() {
        validate_curve(c)?;
    }
    png_file(path, SINGLE, |root| {
        let area = content(root);
        let curves: Vec<_> = [left, right].into_iter().flatten().collect();
        footer(&area, &[correction_footer(&curves)])?;
        let plot = area.margin(0, ROW + PAD * 2, 0, 0);
        if curves.is_empty() {
            return empty(
                &plot,
                "Equalization",
                "No equalization curves were supplied.",
            );
        }
        let single = left.is_some() && left == right;
        if single {
            let c = curves[0];
            let mut lines = curve_lines(c, BOTH, "Both ears", Dash::Solid);
            lines.extend(correction_lines(c, "Correction applied", Dash::Long));
            fr_panel(
                &plot,
                "Equalization",
                "Measured response, target and correction for both ears.",
                &lines,
                None,
                None,
                false,
            )?;
        } else {
            let top = header(
                &plot,
                "Equalization",
                "Each ear is shown separately on the same level scale.",
            )?;
            let body = plot.margin(top, 0, 0, 0);
            let width = (body.dim_in_pixel().0 as i32 - GAP) / 2;
            let (l, r) = body.split_horizontally(width);
            let r = r.margin(0, 0, GAP, 0);
            let range = db_range(
                curves
                    .iter()
                    .flat_map(|c| {
                        c.raw
                            .iter()
                            .chain(&c.smoothed)
                            .chain(&c.equalization)
                            .chain(&c.target)
                            .copied()
                    })
                    .chain([0.]),
            );
            for (area, curve, name, color, dash) in [
                (&l, left, "Left ear", LEFT, Dash::Solid),
                (&r, right, "Right ear", RIGHT, Dash::Short),
            ] {
                if let Some(c) = curve {
                    let mut lines = curve_lines(c, color, name, dash);
                    lines.extend(correction_lines(c, "Correction applied", Dash::Long));
                    fr_panel(
                        area,
                        name,
                        "Faint: raw. Bold: smoothed. Dashed: correction and target.",
                        &lines,
                        Some(range),
                        None,
                        false,
                    )?;
                } else {
                    empty(area, name, "No equalization supplied for this ear.")?;
                }
            }
        }
        Ok(())
    })
}
fn room_error(c: &FrCurve) -> Vec<f64> {
    if !c.error_smoothed.is_empty() {
        c.error_smoothed.clone()
    } else if c.smoothed.len() == c.frequency.len() && c.target.len() == c.frequency.len() {
        c.smoothed
            .iter()
            .zip(&c.target)
            .map(|(v, t)| v - t)
            .collect()
    } else {
        Vec::new()
    }
}
fn room_summary(c: &FrCurve) -> Vec<String> {
    let error = room_error(c);
    let points: Vec<_> = c
        .frequency
        .iter()
        .copied()
        .zip(error)
        .filter(|(f, _)| (40.0..=16000.0).contains(f))
        .collect();
    let extreme = |peak: bool| {
        points
            .iter()
            .filter(|(_, v)| if peak { *v > 0. } else { *v < 0. })
            .max_by(|a, b| {
                if peak {
                    a.1.total_cmp(&b.1)
                } else {
                    b.1.total_cmp(&a.1)
                }
            })
    };
    [true, false]
        .into_iter()
        .map(|peak| {
            let name = if peak {
                "Largest room peak"
            } else {
                "Largest dip"
            };
            extreme(peak).map_or_else(
                || {
                    format!(
                        "{name} (40 Hz–16 kHz): {}",
                        if points.is_empty() {
                            "unavailable (no target-relative error)"
                        } else {
                            "none in the plotted band"
                        }
                    )
                },
                |(f, v)| format!("{name} (40 Hz–16 kHz): {v:+.1} dB at {}", frequency(*f)),
            )
        })
        .collect()
}
fn room_panel(
    area: &Area<'_>,
    room: &FrCurve,
    measurements: &[FrCurve],
    limits: Option<(f64, f64)>,
) -> Result<(), PlotError> {
    validate_curve(room)?;
    let error = room_error(room);
    let mut lines = vec![raw(&room.frequency, &room.raw, LEFT)];
    for c in measurements {
        validate_curve(c)?;
        lines.push(raw(
            &c.frequency,
            if c.raw.is_empty() {
                &c.smoothed
            } else {
                &c.raw
            },
            BOTH,
        ));
    }
    lines.push(line(
        &room.frequency,
        &room.smoothed,
        "Measured",
        LEFT,
        Dash::Solid,
    ));
    lines.push(line(
        &room.frequency,
        &room.target,
        "Target",
        TARGET,
        Dash::Long,
    ));
    lines.push(line(
        &room.frequency,
        &error,
        "Difference from target",
        BOTH,
        Dash::Dots,
    ));
    fr_panel(
        area,
        "Room: measured against the target",
        "Faint fill shows the difference from the target.",
        &lines,
        limits,
        Some((&room.frequency, &error)),
        false,
    )?;
    Ok(())
}
pub fn plot_generic_room(
    path: &Path,
    room: &FrCurve,
    measurements: &[FrCurve],
) -> Result<(), PlotError> {
    png_file(path, SINGLE, |root| {
        let area = content(root);
        footer(&area, &room_summary(room))?;
        room_panel(
            &area.margin(0, ROW * 2 + PAD * 2, 0, 0),
            room,
            measurements,
            None,
        )
    })
}

fn times(n: usize, start: usize, fs: u32, scale: f64) -> Vec<f64> {
    (0..n)
        .map(|i| (start + i) as f64 / f64::from(fs) * scale)
        .collect()
}
#[allow(clippy::too_many_arguments)] // Explicit chart anatomy, matching fr_panel.
fn linear_panel(
    area: &Area<'_>,
    title: &str,
    subtitle: &str,
    xlabel: &str,
    ylabel: &str,
    lines: &[Line<'_>],
    mut limits: AxisLimits,
    db: bool,
) -> Result<Axes, PlotError> {
    if db {
        limits.y = db_range([limits.y.0, limits.y.1]);
    }
    let y = header(area, title, subtitle)?;
    let top = legend(area, lines, y)?;
    let a = axes(
        area,
        top,
        area.dim_in_pixel().1 as i32,
        limits,
        false,
        db,
        xlabel,
        ylabel,
    )?;
    draw_lines(area, a, lines)?;
    Ok(a)
}
fn validate_spec(s: &Spectrogram) -> Result<(), PlotError> {
    validate(&s.frequency, &[])?;
    if s.time.len() < 2
        || s.time.iter().any(|v| !v.is_finite())
        || s.time.windows(2).any(|w| w[0] >= w[1])
        || s.db.len() != s.frequency.len()
        || s.db
            .iter()
            .any(|r| r.len() != s.time.len() || r.iter().any(|v| !v.is_finite()))
    {
        return Err(PlotError::Invalid(
            "spectrogram matrix shape or nonfinite samples".into(),
        ));
    }
    Ok(())
}
fn heatmap(area: &Area<'_>, s: &Spectrogram, limits: AxisLimits) -> Result<(), PlotError> {
    validate_spec(s)?;
    valid_limits(limits)?;
    let top = header(
        area,
        "Sound over time by frequency",
        "Brighter means louder; the scale is fixed from −80 to 0 dB.",
    )?;
    let h = area.dim_in_pixel().1 as i32;
    let w = area.dim_in_pixel().0 as i32;
    let frequency_labels: Vec<_> = FREQ_TICKS
        .iter()
        .map(|f| {
            if *f >= 1000. {
                format!("{:.0}k", f / 1000.)
            } else {
                format!("{f:.0}")
            }
        })
        .collect();
    let (left, title_x) = y_column(area, &frequency_labels, "Frequency (Hz)")?;
    let ticks = time_ticks((limits.x.0 * 1000., limits.x.1 * 1000.));
    let a = Axes {
        bounds: (left, top + BAND_HEIGHT + PAD, w - Y_LABEL, h - X_LABEL),
        limits: AxisLimits {
            x: tick_range(&ticks),
            y: (20., 20000.),
        },
        log_x: false,
        log_y: true,
    };
    bands(area, a.bounds)?;
    let (l, t, r, b) = a.bounds;
    area.draw(&Rectangle::new([(l, t), (r, b)], PANEL.filled()))
        .map_err(render_error)?;
    let edge = |v: &[f64], i: usize| {
        if i == 0 {
            v[0] - (v[1] - v[0]) * 0.5
        } else if i == v.len() {
            v[i - 1] + (v[i - 1] - v[i - 2]) * 0.5
        } else {
            (v[i - 1] + v[i]) * 0.5
        }
    };
    for (fi, row) in s.db.iter().enumerate() {
        let f0 = edge(&s.frequency, fi).max(20.);
        let f1 = edge(&s.frequency, fi + 1).min(20000.);
        if f0 >= f1 {
            continue;
        }
        for (ti, db) in row.iter().enumerate() {
            let x0 = (edge(&s.time, ti) * 1000.).max(a.limits.x.0);
            let x1 = (edge(&s.time, ti + 1) * 1000.).min(a.limits.x.1);
            if x0 >= x1 {
                continue;
            }
            area.draw(&Rectangle::new(
                [a.map(x0, f0), a.map(x1, f1)],
                magma(*db).filled(),
            ))
            .map_err(render_error)?;
        }
    }
    for (f, name) in FREQ_TICKS.into_iter().zip(frequency_labels) {
        let y = a.map(a.limits.x.0, f).1;
        area.draw(&Text::new(
            name,
            (l - PAD, y),
            (FONT, TICK)
                .into_font()
                .color(&MUTED)
                .pos(plotters::style::text_anchor::Pos::new(
                    plotters::style::text_anchor::HPos::Right,
                    plotters::style::text_anchor::VPos::Center,
                )),
        ))
        .map_err(render_error)?;
    }
    let labels = tick_labels(&ticks);
    for (x, name) in ticks.into_iter().zip(labels) {
        centered(area, &name, (a.map(x, 20.).0, b + PAD), TICK)?;
    }
    centered(area, "Time (ms)", ((l + r) / 2, h - AXIS as i32), AXIS)?;
    area.draw(&Text::new(
        "Frequency (Hz)",
        (title_x, (t + b) / 2),
        (FONT, AXIS)
            .into_font()
            .color(&MUTED)
            .transform(FontTransform::Rotate270)
            .pos(plotters::style::text_anchor::Pos::new(
                plotters::style::text_anchor::HPos::Center,
                plotters::style::text_anchor::VPos::Top,
            )),
    ))
    .map_err(render_error)?;
    for py in t..b {
        let db = MAGMA_RANGE.1 - (py - t) as f64 / (b - t) as f64 * (MAGMA_RANGE.1 - MAGMA_RANGE.0);
        rule(area, (r + PAD, py), (r + PAD * 2, py), magma(db))?;
    }
    text(area, "dB", (r + PAD, b + GAP), LEGEND, INK)?;
    for db in [-80., -60., -40., -20., 0.] {
        let y = t + ((-db / 80.) * (b - t) as f64) as i32;
        text(
            area,
            &format!("{db:.0}"),
            (r + PAD * 2 + PAD / 2, y - TICK as i32 / 2),
            TICK,
            MUTED,
        )?;
    }
    Ok(())
}
fn tail_crossing(time: &[f64], average: &[f64], floor: f64) -> Option<(f64, f64)> {
    let peak = average
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))?
        .0;
    for i in peak + 1..average.len() {
        if average[i - 1] > floor && average[i] <= floor {
            let fraction = (floor - average[i - 1]) / (average[i] - average[i - 1]);
            return Some((time[i - 1] + fraction * (time[i] - time[i - 1]), floor));
        }
    }
    None
}
fn speaker_name(code: &str) -> &str {
    match code {
        "FL" => "Front left speaker",
        "FR" => "Front right speaker",
        "FC" => "Center speaker",
        "BL" => "Back left speaker",
        "BR" => "Back right speaker",
        "SL" => "Side left speaker",
        "SR" => "Side right speaker",
        "WL" => "Wide left speaker",
        "WR" => "Wide right speaker",
        "TFL" => "Top front left speaker",
        "TFR" => "Top front right speaker",
        "TSL" => "Top side left speaker",
        "TSR" => "Top side right speaker",
        "TBL" => "Top back left speaker",
        "TBR" => "Top back right speaker",
        "LFE" => "Subwoofer",
        "X" => "Reference microphone",
        _ => code,
    }
}
fn sheet_title(path: &Path, title: &str) -> String {
    let Some((speaker, side)) = title
        .rsplit_once('-')
        .filter(|(_, side)| ["left", "right"].contains(side))
    else {
        return title.into();
    };
    let stage = match path
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
    {
        Some("pre") => " · as recorded",
        Some("post") => " · after processing",
        Some("room") => " · room correction",
        _ => "",
    };
    format!("{} → {side} ear{stage}", speaker_name(speaker))
}
pub fn plot_ir_panels(path: &Path, p: &IrPanels) -> Result<(), PlotError> {
    render_ir_panels(path, p, None)
}
/// Render the already-computed, peak-normalized Lundeby noise floor without
/// changing IrPanels or requiring callers of plot_ir_panels to provide it.
pub fn plot_ir_panels_with_noise_floor(
    path: &Path,
    p: &IrPanels,
    noise_floor_db: f64,
) -> Result<(), PlotError> {
    if !noise_floor_db.is_finite() {
        return Err(PlotError::Invalid("nonfinite noise floor".into()));
    }
    render_ir_panels(path, p, Some(noise_floor_db))
}
fn render_ir_panels(path: &Path, p: &IrPanels, noise_floor: Option<f64>) -> Result<(), PlotError> {
    if p.fs == 0 {
        return Err(PlotError::Invalid("sample rate is zero".into()));
    }
    if p.ir
        .iter()
        .chain(&p.decay)
        .chain(&p.decay_average)
        .chain(p.recording.iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(PlotError::Invalid("nonfinite panel samples".into()));
    }
    for l in p.limits.0.iter().flatten() {
        valid_limits(*l)?;
    }
    if let Some(s) = &p.spectrogram {
        validate_spec(s)?;
    }
    png_file(path, SHEET, |root| {
        let area = content(root);
        let top = header(
            &area,
            &sheet_title(path, &p.title),
            "From the recorded sweep to the room response: the same samples, six views.",
        )?;
        let body = area.margin(top, 0, 0, 0);
        let cells = body.split_evenly((2, 3));
        for (i, cell) in cells.iter().enumerate() {
            let cell = cell.margin(
                if i >= 3 { GAP / 2 } else { 0 },
                if i < 3 { GAP / 2 } else { 0 },
                if i % 3 > 0 { GAP / 2 } else { 0 },
                if i % 3 < 2 { GAP / 2 } else { 0 },
            );
            let plot = cell.margin(0, ROW * 2 + PAD * 2, 0, 0);
            match i {
                0 => {
                    if let (Some(rec), Some(lim)) = (&p.recording, p.limits.0[0]) {
                        if rec.is_empty() {
                            empty(
                                &plot,
                                "Test sweep as recorded",
                                "The recording contains no samples.",
                            )?;
                        } else {
                            let t = times(rec.len(), 0, p.fs, 1.);
                            let mut s = raw(&t, rec, BOTH);
                            s.label = "Recorded sweep";
                            linear_panel(
                                &plot,
                                "Test sweep as recorded",
                                "The microphone signal before the response is extracted.",
                                "Time (s)",
                                "Amplitude (FS)",
                                &[s],
                                lim,
                                false,
                            )?;
                            footer(
                                &cell,
                                &[
                                    format!(
                                        "Recording length: {:.1} s",
                                        rec.len() as f64 / f64::from(p.fs)
                                    ),
                                    format!(
                                        "Peak amplitude: {} FS",
                                        significant(rec.iter().map(|v| v.abs()).fold(0., f64::max))
                                    ),
                                ],
                            )?;
                        }
                    } else {
                        empty(
                            &plot,
                            "Test sweep as recorded",
                            "No recorded sweep was supplied.",
                        )?;
                    }
                }
                1 => {
                    if let Some(lim) = p.limits.0[1].filter(|_| !p.ir.is_empty()) {
                        let t = times(p.ir.len(), 0, p.fs, 1000.);
                        let n = t.partition_point(|v| *v <= 30.);
                        linear_panel(
                            &plot,
                            "Impulse response",
                            "The first 30 ms are bold; the full response stays faint.",
                            "Time (ms)",
                            "Amplitude (FS)",
                            &[
                                raw(&t, &p.ir, BOTH),
                                line(&t[..n], &p.ir[..n], "First 30 ms", LEFT, Dash::Solid),
                            ],
                            lim,
                            false,
                        )?;
                        footer(
                            &cell,
                            &[
                                format!(
                                    "Response length: {:.1} ms",
                                    p.ir.len() as f64 / f64::from(p.fs) * 1000.
                                ),
                                format!(
                                    "Peak amplitude: {} FS",
                                    significant(p.ir.iter().map(|v| v.abs()).fold(0., f64::max))
                                ),
                            ],
                        )?;
                    } else {
                        empty(
                            &plot,
                            "Impulse response",
                            "No impulse response was supplied.",
                        )?;
                    }
                }
                2 => {
                    if let Some(lim) = p.limits.0[2].filter(|_| !p.decay.is_empty()) {
                        let t = times(p.decay.len(), p.decay_start, p.fs, 1000.);
                        let avg_t = times(
                            p.decay_average.len(),
                            p.decay_start + p.decay_window / 2,
                            p.fs,
                            1000.,
                        );
                        let floor_x = [lim.x.0, lim.x.1];
                        let floor_y: Vec<_> =
                            noise_floor.map(|floor| vec![floor; 2]).unwrap_or_default();
                        let a = linear_panel(
                            &plot,
                            "Decay: how fast the room's sound dies away",
                            "Faint: squared response. Bold: moving average.",
                            "Time (ms)",
                            "Level (dB)",
                            &[
                                raw(&t, &p.decay, LEFT),
                                line(&avg_t, &p.decay_average, "Average decay", LEFT, Dash::Solid),
                                line(&floor_x, &floor_y, "Noise floor", TARGET, Dash::Long),
                            ],
                            lim,
                            true,
                        )?;
                        let tail = noise_floor
                            .and_then(|floor| tail_crossing(&avg_t, &p.decay_average, floor));
                        let tail_text = if let Some((time, floor)) = tail {
                            let point = a.map(time, floor);
                            rule(&plot, point, (point.0, a.bounds.1 + PAD), ZERO)?;
                            let value = format!("Tail length ≈ {time:.0} ms");
                            label(&plot, &value, (point.0, a.bounds.1), a.bounds)?;
                            value
                        } else if noise_floor.is_some() {
                            "Tail length: floor not reached in this window".into()
                        } else {
                            "Noise floor / tail length: not supplied".into()
                        };
                        footer(
                            &cell,
                            &[
                                noise_floor.map_or_else(
                                    || {
                                        format!(
                                            "Average window: {:.1} ms",
                                            p.decay_window as f64 / f64::from(p.fs) * 1000.
                                        )
                                    },
                                    |floor| format!("Noise floor: {floor:+.1} dB"),
                                ),
                                tail_text,
                            ],
                        )?;
                    } else {
                        empty(
                            &plot,
                            "Decay: how fast the room's sound dies away",
                            "No decay samples were supplied.",
                        )?;
                    }
                }
                3 => {
                    if let (Some(s), Some(lim)) = (&p.spectrogram, p.limits.0[3]) {
                        heatmap(&plot, s, lim)?;
                        footer(
                            &cell,
                            &[
                                format!(
                                    "Time span: {:.1} … {:.1} ms",
                                    s.time[0] * 1000.,
                                    s.time[s.time.len() - 1] * 1000.
                                ),
                                "Color scale: −80.0 … 0.0 dB (clipped)".into(),
                            ],
                        )?;
                    } else {
                        empty(
                            &plot,
                            "Sound over time by frequency",
                            "No recording spectrogram was supplied.",
                        )?;
                    }
                }
                4 => {
                    if let Some(lim) = p.limits.0[4] {
                        if let Some(c) = &p.room_fr {
                            room_panel(
                                &plot,
                                c,
                                &[],
                                Some(db_range([lim.y.0.min(0.), lim.y.1.max(0.)])),
                            )?;
                            footer(&cell, &room_summary(c))?;
                        } else if !p.fr.frequency.is_empty() {
                            fr_panel(
                                &plot,
                                "Frequency response",
                                "Faint: raw response. Bold: the smoothed response.",
                                &[
                                    raw(&p.fr.frequency, &p.fr.raw, LEFT),
                                    line(
                                        &p.fr.frequency,
                                        &p.fr.smoothed,
                                        "Measured",
                                        LEFT,
                                        Dash::Solid,
                                    ),
                                ],
                                Some(db_range([lim.y.0, lim.y.1])),
                                None,
                                false,
                            )?;
                            let range = (
                                p.fr.frequency
                                    .iter()
                                    .zip(&p.fr.smoothed)
                                    .filter(|(f, _)| (40.0..=16000.0).contains(*f))
                                    .map(|(_, v)| *v)
                                    .fold(f64::INFINITY, f64::min),
                                p.fr.frequency
                                    .iter()
                                    .zip(&p.fr.smoothed)
                                    .filter(|(f, _)| (40.0..=16000.0).contains(*f))
                                    .map(|(_, v)| *v)
                                    .fold(f64::NEG_INFINITY, f64::max),
                            );
                            footer(
                                &cell,
                                &[if !range.0.is_finite() {
                                    "Smoothed response: unavailable".into()
                                } else {
                                    format!(
                                        "Level range (40 Hz–16 kHz): {:+.1} … {:+.1} dB",
                                        range.0, range.1
                                    )
                                }],
                            )?;
                        } else {
                            empty(
                                &plot,
                                "Frequency response",
                                "No frequency response was supplied.",
                            )?;
                        }
                    } else {
                        empty(
                            &plot,
                            "Frequency response",
                            "No frequency response was supplied.",
                        )?;
                    }
                }
                5 => {
                    let title = "First reflections";
                    if let Some(r) = crate::reflections::reflections(&p.ir, p.fs, noise_floor) {
                        let floor_x = [0., 25.];
                        let floor_y = [r.floor; 2];
                        let lines = [
                            line(&r.time, &r.db, "IR envelope", LEFT, Dash::Solid),
                            line(&floor_x, &floor_y, "Noise floor", TARGET, Dash::Long),
                        ];
                        let top = header(
                            &plot,
                            title,
                            "Direct sound at 0 ms; the echoes that follow, relative to it.",
                        )?;
                        let top = legend(&plot, &lines, top)?;
                        let labels: Vec<_> = r
                            .echoes
                            .iter()
                            .map(|(time, db)| format!("+{time:.1} ms, {db:.0} dB"))
                            .collect();
                        let sizes = labels
                            .iter()
                            .map(|s| label_size(&plot, s))
                            .collect::<Result<Vec<_>, _>>()?;
                        let column = sizes.iter().map(|s| s.0).max().unwrap_or(0);
                        let graph = plot.margin(0, 0, 0, column);
                        let a = axes(
                            &graph,
                            top,
                            graph.dim_in_pixel().1 as i32,
                            AxisLimits {
                                x: (0., 25.),
                                y: db_range([
                                    r.floor.min(-36.),
                                    r.db.iter().copied().fold(0., f64::max),
                                ]),
                            },
                            false,
                            true,
                            "Time after direct sound (ms)",
                            "Level re direct (dB)",
                        )?;
                        draw_lines(&graph, a, &lines)?;
                        let boxes = reflection_boxes(a.bounds, &sizes);
                        for ((&(time, db), value), bounds) in
                            r.echoes.iter().zip(&labels).zip(boxes)
                        {
                            let point = a.map(time, db);
                            rule(&plot, point, (bounds.0, (bounds.1 + bounds.3) / 2), ZERO)?;
                            label_box(&plot, value, bounds)?;
                            plot.draw(&Circle::new(point, RADIUS / 2, INK.filled()))
                                .map_err(render_error)?;
                        }
                        footer(
                            &cell,
                            &[
                                reflection_summary(r.first_echo),
                                format!(
                                    "Noise floor: {:.0} dB{}",
                                    r.floor,
                                    if noise_floor.is_none() {
                                        " (tail estimate)"
                                    } else {
                                        ""
                                    }
                                ),
                            ],
                        )?;
                    } else {
                        empty(&plot, title, "No nonzero impulse response was supplied.")?;
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    })
}
fn reflection_boxes(
    bounds: (i32, i32, i32, i32),
    sizes: &[(i32, i32)],
) -> Vec<(i32, i32, i32, i32)> {
    let x = bounds.2 + GAP;
    let mut y = bounds.1 + ANNOTATION_INSET;
    sizes
        .iter()
        .map(|&(w, h)| {
            let rect = (x, y, x + w, y + h);
            y += h + PAD;
            rect
        })
        .collect()
}
fn peak_boxes(
    points: [(i32, i32); 2],
    sizes: [(i32, i32); 2],
    bounds: (i32, i32, i32, i32),
) -> [(i32, i32, i32, i32); 2] {
    let mut boxes = [(0, 0, 0, 0); 2];
    for i in 0..2 {
        let (w, h) = sizes[i];
        let x = if i == 0 {
            points[i].0 - PAD - w
        } else {
            points[i].0 + PAD
        }
        .clamp(bounds.0 + ANNOTATION_INSET, bounds.2 - ANNOTATION_INSET - w);
        let y =
            (points[i].1 + GAP).clamp(bounds.1 + ANNOTATION_INSET, bounds.3 - ANNOTATION_INSET - h);
        boxes[i] = (x, y, x + w, y + h);
    }
    let [l, r] = boxes;
    if l.0 < r.2 + PAD && l.2 + PAD > r.0 && l.1 < r.3 + PAD && l.3 + PAD > r.1 {
        let top =
            l.1.min(bounds.3 - ANNOTATION_INSET - sizes[0].1 - PAD - sizes[1].1);
        boxes[0].1 = top;
        boxes[0].3 = top + sizes[0].1;
        boxes[1].1 = boxes[0].3 + PAD;
        boxes[1].3 = boxes[1].1 + sizes[1].1;
    }
    boxes
}
fn reflection_summary(first_echo: Option<(f64, f64)>) -> String {
    first_echo.map_or_else(
        || "No echo above −30 dB within 25 ms".into(),
        |(time, db)| format!("First echo: +{time:.1} ms ({db:.0} dB)"),
    )
}
fn overlay_times(ir: &[f64], origin: usize, fs: u32) -> Vec<f64> {
    (0..ir.len())
        .map(|i| (i as f64 - origin as f64) * 1000. / f64::from(fs))
        .collect()
}
pub fn plot_interaural_overlay(path: &Path, o: &InterauralOverlay<'_>) -> Result<(), PlotError> {
    if o.fs == 0
        || !o.time_range_ms.0.is_finite()
        || !o.time_range_ms.1.is_finite()
        || o.time_range_ms.0 >= o.time_range_ms.1
        || o.left_ir.is_empty()
        || o.right_ir.is_empty()
        || o.left_peak >= o.left_ir.len()
        || o.right_peak >= o.right_ir.len()
        || o.left_ir.iter().chain(o.right_ir).any(|v| !v.is_finite())
    {
        return Err(PlotError::Invalid(
            "invalid overlay range, peaks or samples".into(),
        ));
    }
    let origin = o.left_peak.min(o.right_peak);
    let lt = overlay_times(o.left_ir, origin, o.fs);
    let rt = overlay_times(o.right_ir, origin, o.fs);
    let delta = (o.right_peak as f64 - o.left_peak as f64) * 1000. / f64::from(o.fs);
    let summary = if delta == 0. {
        "0.0 ms: both ears at the same time".into()
    } else {
        format!(
            "{:.2} ms: {} ear first",
            delta.abs(),
            if delta > 0. { "left" } else { "right" }
        )
    };
    let y = padded_range(
        lt.iter()
            .zip(o.left_ir)
            .chain(rt.iter().zip(o.right_ir))
            .filter(|(t, _)| (OVERLAY_RANGE.0..=OVERLAY_RANGE.1).contains(t))
            .map(|(_, v)| *v),
    );
    png_file(path, OVERLAY, |root| {
        let area = content(root);
        footer(
            &area,
            &[
                format!("{} · {summary}", o.speaker),
                format!(
                    "Direct sound reaches the left ear at {:.1} ms and the right ear at {:.1} ms",
                    o.left_peak as f64 * 1000. / f64::from(o.fs),
                    o.right_peak as f64 * 1000. / f64::from(o.fs)
                ),
            ],
        )?;
        let plot = area.margin(0, ROW * 2 + PAD * 2, 0, 0);
        let a = linear_panel(
            &plot,
            "Which ear hears the speaker first",
            "The earlier supplied peak is time zero; the detail spans −1 to +5 ms.",
            "Time from earlier peak (ms)",
            "Amplitude (FS)",
            &[
                line(&lt, o.left_ir, "Left ear", LEFT, Dash::Solid),
                line(&rt, o.right_ir, "Right ear", RIGHT, Dash::Short),
            ],
            AxisLimits {
                x: OVERLAY_RANGE,
                y,
            },
            false,
        )?;
        let peak_l = (o.left_peak - origin) as f64 * 1000. / f64::from(o.fs);
        let peak_r = (o.right_peak - origin) as f64 * 1000. / f64::from(o.fs);
        let py = a.bounds.1 + GAP;
        let x0 = a.map(peak_l.min(5.), y.0).0;
        let x1 = a.map(peak_r.min(5.), y.0).0;
        rule(&plot, (x0, py), (x1, py), INK)?;
        for (x, direction) in [(x0.min(x1), 1), (x0.max(x1), -1)] {
            rule(&plot, (x, py), (x + direction * RADIUS, py - RADIUS), INK)?;
            rule(&plot, (x, py), (x + direction * RADIUS, py + RADIUS), INK)?;
        }
        label(&plot, &summary, (x0.min(x1), py - ROW * 2), a.bounds)?;
        let names = ["Left peak", "Right peak"];
        let points = [
            a.map(peak_l, o.left_ir[o.left_peak]),
            a.map(peak_r, o.right_ir[o.right_peak]),
        ];
        let boxes = peak_boxes(
            points,
            [label_size(&plot, names[0])?, label_size(&plot, names[1])?],
            a.bounds,
        );
        for ((t, v, name), bounds) in [
            (peak_l, o.left_ir[o.left_peak], names[0]),
            (peak_r, o.right_ir[o.right_peak], names[1]),
        ]
        .into_iter()
        .zip(boxes)
        {
            if a.contains(t, v) {
                let point = a.map(t, v);
                plot.draw(&Circle::new(point, RADIUS, PANEL.filled()))
                    .map_err(render_error)?;
                plot.draw(&Circle::new(point, RADIUS / 2, INK.filled()))
                    .map_err(render_error)?;
                rule(
                    &plot,
                    point,
                    (point.0.clamp(bounds.0, bounds.2), bounds.1),
                    ZERO,
                )?;
                label_box(&plot, name, bounds)?;
            }
        }
        let (w, h) = plot.dim_in_pixel();
        let inset = plot.clone().shrink(
            (w as i32 * 2 / 3, a.bounds.1 + GAP),
            (w as i32 / 3 - PAD, h as i32 / 3),
        );
        inset.fill(&PANEL).map_err(render_error)?;
        text(&inset, "Full response (ms)", (PAD, 0), LEGEND, MUTED)?;
        let full_x = padded_range(lt.iter().chain(&rt).copied());
        let full_y = padded_range(o.left_ir.iter().chain(o.right_ir).copied());
        let inset_a = axes(
            &inset,
            ROW,
            inset.dim_in_pixel().1 as i32,
            AxisLimits {
                x: full_x,
                y: full_y,
            },
            false,
            false,
            "Time (ms)",
            "FS",
        )?;
        draw_lines(
            &inset,
            inset_a,
            &[raw(&lt, o.left_ir, LEFT), raw(&rt, o.right_ir, RIGHT)],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tail_crossing_ignores_the_rise_and_interpolates_the_floor() {
        assert_eq!(
            tail_crossing(&[0., 10., 20., 30.], &[-70., 0., -40., -80.], -60.),
            Some((25., -60.))
        );
        assert_eq!(tail_crossing(&[0., 10.], &[0., -40.], -60.), None);
    }
    #[test]
    fn summaries_are_signed_and_band_limited() {
        let s = difference_summary(
            &[20., 200., 1000., 8000., 20000., 24000.],
            &[1., -2., -4., 0., 3., 100.],
        );
        assert!(s[0].contains("-2.0 dB (right louder)"));
        assert!(s[1].contains("-4.0 dB at 1.0 kHz"));
    }
    #[test]
    fn overlay_uses_shared_supplied_origin() {
        assert_eq!(overlay_times(&[0.; 4], 1, 1000), vec![-1., 0., 1., 2.]);
    }
    #[test]
    fn reflection_footer_reports_first_qualified_echo_not_first_label() {
        let mut ir = vec![0.001; 4800];
        ir[48] = 1.;
        for (offset, db) in [(34, -25.), (144, -8.), (336, -12.), (576, -16.)] {
            ir[48 + offset] = 10_f64.powf(db / 20.);
        }
        let r = crate::reflections::reflections(&ir, 48000, None).unwrap();
        assert_eq!(
            reflection_summary(r.first_echo),
            "First echo: +0.7 ms (-25 dB)"
        );
        assert!(r.echoes.iter().all(|&(time, _)| time >= 3.));
        assert_eq!(
            reflection_summary(None),
            "No echo above −30 dB within 25 ms"
        );
    }
    #[test]
    fn headphones_measured_summary_uses_targets_bands_and_signed_extrema() {
        assert_eq!(HEADPHONES_TITLE, "Your headphones as measured");
        assert_eq!(
            HEADPHONES_SUBTITLE,
            "Impulcifer flattens this in the final equalization; the flatter the bold line, the less it has to do."
        );
        let mut left = FrCurve {
            frequency: vec![39., 40., 100., 10000., 16000., 16001.],
            raw: vec![100.; 6],
            smoothed: vec![99., 3., 1., 3., -8., -99.],
            target: vec![1.; 6],
            equalization: vec![900.; 6],
            ..Default::default()
        };
        let mut right = left.clone();
        right.smoothed.iter_mut().for_each(|v| *v += 2.);
        let summary = headphones_footer(&left, &right);
        assert_eq!(
            summary[0],
            "Average deviation from target 100 Hz–10 kHz: left +1.0 dB, right +3.0 dB"
        );
        assert_eq!(
            summary[1],
            "Largest deviation (40 Hz–16 kHz): -9.0 dB at 16.0 kHz"
        );
        left.smoothed.clear();
        assert!(headphones_footer(&left, &left)[0].contains("left +99.0 dB"));
        left.target.clear();
        assert!(
            headphones_footer(&left, &left)
                .iter()
                .all(|s| s.contains("unavailable"))
        );
    }
    #[test]
    fn peak_labels_are_opposite_below_and_stack_when_reversed_or_clamped() {
        let bounds = (100, 100, 1400, 600);
        let sizes = [(130, 44), (140, 44)];
        let boxes = peak_boxes([(400, 160), (405, 165)], sizes, bounds);
        assert!(boxes[0].2 < 400 && boxes[1].0 > 405);
        assert!(boxes[0].1 > 160 && boxes[1].1 > 165);
        for points in [
            [(400, 160), (200, 160)],
            [(110, 560), (110, 560)],
            [(1390, 560), (1390, 560)],
        ] {
            let [l, r] = peak_boxes(points, sizes, bounds);
            assert!(l.2 + PAD <= r.0 || r.2 + PAD <= l.0 || l.3 + PAD <= r.1 || r.3 + PAD <= l.1);
            for b in [l, r] {
                assert!(b.0 >= bounds.0 + ANNOTATION_INSET && b.2 <= bounds.2 - ANNOTATION_INSET);
                assert!(b.1 >= bounds.1 + ANNOTATION_INSET && b.3 <= bounds.3 - ANNOTATION_INSET);
            }
        }
    }
    #[test]
    fn reflections_column_keeps_boxes_outside_curve_in_time_order() {
        let bounds = (100, 150, 480, 400);
        let boxes = reflection_boxes(bounds, &[(170, 44), (180, 44), (175, 44)]);
        assert!(
            boxes
                .iter()
                .all(|b| b.0 == bounds.2 + GAP && b.3 < bounds.3)
        );
        assert!(boxes.windows(2).all(|w| w[0].3 + PAD <= w[1].1));
    }
    #[test]
    fn correction_summary_does_not_fabricate_missing_values() {
        assert!(correction_footer(&[&FrCurve::default()]).contains("unavailable"));
    }
    #[test]
    fn extrema_ignore_both_band_edges_and_keep_inclusive_boundaries() {
        let f = vec![20., 39., 40., 200., 8000., 16000., 16001., 20000.];
        let values = vec![99., -99., -8., 2., 4., 9., -100., 100.];
        let summaries = difference_summary(&f, &values);
        assert!(summaries[0].contains("+3.0 dB"));
        assert!(summaries[1].contains("(40 Hz–16 kHz): +9.0 dB at 16.0 kHz"));
        let c = FrCurve {
            frequency: f,
            error_smoothed: values.clone(),
            equalization: values,
            ..Default::default()
        };
        let room = room_summary(&c);
        assert!(room[0].contains("+9.0 dB at 16.0 kHz"));
        assert!(room[1].contains("-8.0 dB at 40.0 Hz"));
        assert!(room.iter().all(|s| s.contains("(40 Hz–16 kHz)")));
        assert!(correction_footer(&[&c]).contains("-8.0 … +9.0 dB"));
    }
    #[test]
    fn sheet_titles_expand_speakers_without_changing_identifiers() {
        for (stage, description) in [
            ("pre", "as recorded"),
            ("post", "after processing"),
            ("room", "room correction"),
        ] {
            assert_eq!(
                sheet_title(&Path::new(stage).join("FL-left.png"), "FL-left"),
                format!("Front left speaker → left ear · {description}")
            );
        }
        for code in [
            "FL", "FR", "FC", "BL", "BR", "SL", "SR", "WL", "WR", "TFL", "TFR", "TSL", "TSR",
            "TBL", "TBR", "X", "LFE",
        ] {
            assert_ne!(speaker_name(code), code);
        }
    }
    #[test]
    fn color_identity_has_secondary_dash_encoding() {
        let c = FrCurve::default();
        assert!(
            curve_lines(&c, LEFT, "Left", Dash::Solid)
                .iter()
                .all(|l| l.dash == Dash::Solid)
        );
        assert!(
            correction_lines(&c, "Correction", Dash::Long)
                .iter()
                .all(|l| l.dash == Dash::Long)
        );
    }
}
