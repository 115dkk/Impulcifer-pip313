//! Plot-only preparation from borrowed pipeline state; never mutates DSP inputs.
use impulcifer_dsp::{
    DspError,
    estimator::SweepEstimator,
    fr::{FrequencyResponse, magnitude_to_frequency_response},
    hrir::Hrir,
    interp::Spline,
    ir::ImpulseResponse,
    spectrogram,
};
use impulcifer_plots::{
    AxisLimits, FrCurve, FrSeries, IrPanels, PanelLimits, Spectrogram, Waterfall, padded_range,
};
use impulcifer_types::stages::StageKey;
use rayon::prelude::*;
use std::path::Path;

fn error(e: impl std::fmt::Display) -> DspError {
    DspError::InvalidArgument(format!("plot: {e}"))
}
pub fn curve(fr: &FrequencyResponse) -> FrCurve {
    FrCurve {
        name: fr.name.clone(),
        frequency: fr.frequency.clone(),
        raw: fr.raw.clone(),
        smoothed: fr.smoothed.clone(),
        error: fr.error.clone(),
        error_smoothed: fr.error_smoothed.clone(),
        equalization: fr.equalization.clone(),
        equalized_raw: fr.equalized_raw.clone(),
        target: fr.target.clone(),
    }
}
pub fn series(data: &[f64], fs: u32, results: bool) -> Result<FrSeries, DspError> {
    let mut fr = magnitude_to_frequency_response("Frequency response", fs, data)?;
    fr.smoothen(
        1.0 / 3.0,
        if results { 1.0 / 5.0 } else { 1.0 / 3.0 },
        20000.0,
        23999.0,
    )?;
    Ok(FrSeries {
        frequency: fr.frequency,
        raw: fr.raw,
        smoothed: fr.smoothed,
    })
}
pub fn results_series(hrir: &Hrir) -> Result<(FrSeries, FrSeries), DspError> {
    let sum = |left: bool| -> Result<Vec<f64>, DspError> {
        let mut sum: Option<Vec<f64>> = None;
        for speaker in &hrir.speakers {
            if let Some(ir) = if left { &speaker.left } else { &speaker.right } {
                if let Some(sum) = &mut sum {
                    if sum.len() != ir.len() {
                        return Err(error("stacked IR lengths differ"));
                    }
                    for (s, v) in sum.iter_mut().zip(&ir.data) {
                        *s += v;
                    }
                } else {
                    sum = Some(ir.data.clone());
                }
            }
        }
        sum.ok_or_else(|| error("empty ear stack"))
    };
    Ok((
        series(&sum(true)?, hrir.fs, true)?,
        series(&sum(false)?, hrir.fs, true)?,
    ))
}
pub fn headphones(
    path: &Path,
    left: &FrequencyResponse,
    right: &FrequencyResponse,
) -> Result<(), DspError> {
    let display = |fr: &FrequencyResponse| -> Result<FrCurve, DspError> {
        let mut smoothed = fr.clone();
        // HRIRPlotter.plot_result uses these display windows. There is no
        // plot_headphones method in 2.x; pipeline_stages plots raw headphones.
        smoothed.smoothen(1.0 / 3.0, 1.0 / 5.0, 20000.0, 23999.0)?;
        Ok(FrCurve {
            name: fr.name.clone(),
            frequency: fr.frequency.clone(),
            raw: fr.raw.clone(),
            smoothed: smoothed.smoothed,
            target: fr.target.clone(),
            ..Default::default()
        })
    };
    impulcifer_plots::plot_headphones(
        path,
        &display(left)?,
        &display(right)?,
        left.center_value((100.0, 10000.0)),
        right.center_value((100.0, 10000.0)),
    )
    .map_err(error)
}
pub fn eq(
    path: &Path,
    left: Option<&FrequencyResponse>,
    right: Option<&FrequencyResponse>,
) -> Result<(), DspError> {
    impulcifer_plots::plot_eq(path, left.map(curve).as_ref(), right.map(curve).as_ref())
        .map_err(error)
}
fn waterfall(ir: &ImpulseResponse) -> Result<Waterfall, DspError> {
    let mut data = vec![0.0; 256 * 5 + 512];
    let n = data.len().min(ir.len());
    data[..n].copy_from_slice(&ir.data[..n]);
    let spec = spectrogram::spectrogram(&data, ir.fs, 256, 128)?;
    let n = ((f64::from(ir.fs) / 2.0 / 10.0).ln() / 1.03_f64.ln()) as usize;
    let frequency: Vec<_> = (0..n).map(|i| 10.0 * 1.03_f64.powi(i as i32)).collect();
    let log: Vec<_> = spec.freqs[1..].iter().map(|v| v.log10()).collect();
    let query: Vec<_> = frequency.iter().map(|v| v.log10()).collect();
    let mut z = vec![vec![0.0; spec.times.len()]; n];
    for t in 0..spec.times.len() {
        // SciPy magnitude/density = sqrt(PSD / one-sided doubling).
        let magnitude: Vec<_> = spec.power[1..]
            .iter()
            .enumerate()
            .map(|(i, row)| (row[t] / if i + 1 == 128 { 1.0 } else { 2.0 }).sqrt())
            .collect();
        for (row, v) in z
            .iter_mut()
            .zip(Spline::new(&log, &magnitude, 1)?.eval(&query))
        {
            row[t] = v;
        }
    }
    let max = z.iter().flatten().copied().fold(0.0, f64::max);
    for v in z.iter_mut().flatten() {
        *v = 20.0 * (*v / max.max(f64::MIN_POSITIVE)).clamp(1e-5, 1.0).log10();
    }
    // scipy.ndimage.uniform_filter(size=3, mode='constant'), then Python's crop.
    let mut smooth = vec![vec![0.0; spec.times.len() - 1]; n.saturating_sub(2)];
    for (f, output) in smooth.iter_mut().enumerate() {
        for (t, value) in output.iter_mut().enumerate() {
            let mut sum = 0.0;
            for row in &z[f..=f + 2] {
                for v in &row[t.saturating_sub(1)..=t + 1] {
                    sum += v;
                }
            }
            *value = sum / 9.0;
        }
    }
    Ok(Waterfall {
        frequency: frequency[1..n - 1].to_vec(),
        time_ms: spec.times[..spec.times.len() - 1]
            .iter()
            .map(|v| v * 1000.0)
            .collect(),
        db: smooth,
    })
}
pub fn prepare_panels(
    ir: &ImpulseResponse,
    title: String,
    recording: Option<Vec<f64>>,
) -> Result<IrPanels, DspError> {
    prepare_panels_with_floor(ir, title, recording).map(|(panels, _)| panels)
}
fn prepare_panels_with_floor(
    ir: &ImpulseResponse,
    title: String,
    recording: Option<Vec<f64>>,
) -> Result<(IrPanels, f64), DspError> {
    let fs = f64::from(ir.fs);
    let fr = series(&ir.data, ir.fs, false)?;
    let params = ir.decay_params();
    let span = params.knee_index.saturating_sub(params.peak_index) * 2;
    let start = params.peak_index.saturating_sub(span).min(ir.len());
    let end = (params.peak_index + span).min(ir.len()).max(start);
    let maximum = ir
        .data
        .iter()
        .map(|v| v.abs())
        .fold(0.0, f64::max)
        .max(f64::MIN_POSITIVE);
    let squared: Vec<_> = ir.data[start..end]
        .iter()
        .map(|v| (v / maximum).powi(2))
        .collect();
    let window = params.window_size.max(1).min(squared.len().max(1));
    // core.audio_io.running_mean uses cumulative sums and subtraction, not
    // a rolling accumulator (rounding changes the noise-floor axis limit).
    let mut cumulative = Vec::with_capacity(squared.len() + 1);
    cumulative.push(0.0);
    for v in &squared {
        cumulative.push(cumulative.last().copied().unwrap() + v);
    }
    let average: Vec<_> = (window..cumulative.len())
        .map(|i| 10.0 * ((cumulative[i] - cumulative[i - window]) / window as f64 + 1e-24).log10())
        .collect();
    let decay = squared.iter().map(|v| 10.0 * (v + 1e-24).log10()).collect();
    let mut limits = PanelLimits::default();
    if let Some(rec) = &recording
        && rec.iter().any(|v| *v != 0.0)
    {
        limits.0[0] = Some(AxisLimits {
            x: padded_range([0.0, rec.len() as f64 / fs]),
            y: padded_range(rec.iter().copied()),
        });
    }
    limits.0[1] = Some(AxisLimits {
        x: padded_range([0.0, ir.len().saturating_sub(1) as f64 / fs * 1000.0]),
        y: padded_range(ir.data.iter().copied()),
    });
    if !average.is_empty() {
        limits.0[2] = Some(AxisLimits {
            x: (start as f64 / fs * 1000.0, end as f64 / fs * 1000.0),
            y: (
                average.iter().copied().fold(f64::INFINITY, f64::min) * 1.2,
                0.0,
            ),
        });
    }
    let spectrogram = recording
        .as_ref()
        .and_then(|rec| {
            spectrogram::spectrogram_params(rec.len(), ir.fs, 10.0, 200).map(|p| (rec, p))
        })
        .map(|(rec, p)| -> Result<_, DspError> {
            let s = spectrogram::spectrogram(rec, ir.fs, p.nfft, p.noverlap)?;
            Ok(Spectrogram {
                frequency: s.freqs[1..].to_vec(),
                time: s.times,
                db: s.power[1..]
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|v| 10.0 * (v.abs() + 1e-9).log10())
                            .collect()
                    })
                    .collect(),
            })
        })
        .transpose()?;
    if let Some(s) = &spectrogram
        && s.time.len() > 1
        && s.frequency.len() > 1
    {
        limits.0[3] = Some(AxisLimits {
            x: (
                s.time[0] - (s.time[1] - s.time[0]) / 2.0,
                s.time[s.time.len() - 1] + (s.time[1] - s.time[0]) / 2.0,
            ),
            y: (
                s.frequency[0] - (s.frequency[1] - s.frequency[0]) / 2.0,
                s.frequency[s.frequency.len() - 1] + (s.frequency[1] - s.frequency[0]) / 2.0,
            ),
        });
    }
    limits.0[4] = Some(AxisLimits {
        x: (20.0, 20000.0),
        y: padded_range(fr.raw.iter().chain(&fr.smoothed).copied()),
    });
    let waterfall = waterfall(ir)?;
    limits.0[5] = Some(AxisLimits {
        x: (
            0.0,
            waterfall.time_ms.last().copied().unwrap_or(0.1) * 1000.0,
        ),
        y: (20.0_f64.log10(), 20000.0_f64.log10()),
    });
    Ok((
        IrPanels {
            title,
            recording,
            spectrogram,
            ir: ir.data.clone(),
            fs: ir.fs,
            fr,
            room_fr: None,
            decay,
            decay_start: start,
            decay_average: average,
            decay_window: window,
            waterfall: Some(waterfall),
            limits,
        },
        params.noise_floor_db,
    ))
}
pub fn room(
    dir: &Path,
    rir: &Hrir,
    room: &impulcifer_dsp::stages::room::RoomCorrection,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DspError> {
    let mut panels = Vec::new();
    for s in &rir.speakers {
        check(cancelled)?;
        for (side, ir) in [
            (impulcifer_types::constants::Side::Left, &s.left),
            (impulcifer_types::constants::Side::Right, &s.right),
        ] {
            if let Some(ir) = ir {
                let label = if side == impulcifer_types::constants::Side::Left {
                    "left"
                } else {
                    "right"
                };
                let (mut p, floor) = prepare_panels_with_floor(
                    ir,
                    format!("{}-{label}", s.speaker),
                    ir.recording.clone(),
                )?;
                if let Some((_, _, fr)) = room
                    .frs
                    .0
                    .iter()
                    .find(|(name, ear, _)| name == &s.speaker && *ear == side)
                {
                    let mut fr = fr.clone();
                    fr.smoothen(1.0 / 3.0, 1.0 / 3.0, 100.0, 10000.0)?;
                    let mut values = Vec::new();
                    for v in [&fr.target, &fr.smoothed, &fr.error_smoothed] {
                        values.extend(
                            fr.frequency
                                .iter()
                                .zip(v)
                                .filter(|(f, _)| **f >= 20.0 && **f <= 20000.0)
                                .map(|(_, v)| *v),
                        );
                    }
                    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    p.limits.0[4] = Some(AxisLimits {
                        x: (20.0, 20000.0),
                        y: (lo - (hi - lo) * 0.1, hi + (hi - lo) * 0.1),
                    });
                    p.room_fr = Some(curve(&fr));
                }
                panels.push((p, floor));
            }
        }
    }
    let limits = PanelLimits::synchronize(panels.iter().map(|(p, _)| &p.limits));
    for (p, _) in &mut panels {
        p.limits = limits.clone();
    }
    panels.par_iter().try_for_each(|(p, floor)| {
        check(cancelled)?;
        impulcifer_plots::plot_ir_panels_with_noise_floor(
            &dir.join("plots/room").join(format!("{}.png", p.title)),
            p,
            *floor,
        )
        .map_err(error)
    })
}
/// Python _plot_generic_room_measurement: recompute plot-only copies, including
/// individual centered measurements. The pipeline's correction objects are untouched.
pub fn generic_room(
    dir: &Path,
    irs: &[ImpulseResponse],
    target: &FrequencyResponse,
    calibration: Option<&FrequencyResponse>,
    config: &impulcifer_types::config::ProcessingConfig,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DspError> {
    if irs.is_empty() {
        return Ok(());
    }
    check(cancelled)?;
    use impulcifer_dsp::{
        fr::CenterAt,
        stages::room::{FrCombination, calculate_generic_room_correction},
    };
    let room = calculate_generic_room_correction(
        irs,
        target,
        calibration,
        if config.fr_combination_method == "conservative" {
            FrCombination::Conservative
        } else {
            FrCombination::Average
        },
        config.generic_limit,
    )?;
    let raws = irs
        .iter()
        .map(|ir| {
            let mut fr = magnitude_to_frequency_response("Measurement", ir.fs, &ir.data)?;
            if let Some(mic) = calibration {
                for (raw, correction) in fr.raw.iter_mut().zip(&mic.raw) {
                    *raw -= correction;
                }
            }
            fr.center(CenterAt::Band(100.0, 10000.0))?;
            fr.smoothen(1.0 / 3.0, 1.0 / 3.0, 100.0, 10000.0)?;
            Ok(curve(&fr))
        })
        .collect::<Result<Vec<_>, DspError>>()?;
    impulcifer_plots::plot_generic_room(&dir.join("plots/room/room.png"), &curve(&room), &raws)
        .map_err(error)
}
/// The cooperative-cancellation contract (ARCHITECTURE section 4): every
/// per-speaker task of a rayon batch consults the job before it renders, so a
/// cancel request never waits for a whole batch. The message matches the
/// observer's `check_cancelled` mapping.
fn check(cancelled: &(dyn Fn() -> bool + Sync)) -> Result<(), DspError> {
    if cancelled() {
        Err(DspError::InvalidArgument("cancelled".into()))
    } else {
        Ok(())
    }
}
pub fn render_stage(
    dir: &Path,
    key: StageKey,
    hrir: &Hrir,
    estimator: &SweepEstimator,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), DspError> {
    let plots = dir.join("plots");
    match key {
        StageKey::PlotResults => {
            let (l, r) = results_series(hrir)?;
            impulcifer_plots::plot_results(&plots.join("results.png"), &l, &r).map_err(error)?;
        }
        StageKey::PlotPre | StageKey::PlotPost => {
            let stage = if key == StageKey::PlotPre {
                "pre"
            } else {
                "post"
            };
            let tasks: Vec<_> = hrir
                .speakers
                .iter()
                .flat_map(|s| {
                    [("left", &s.left), ("right", &s.right)]
                        .into_iter()
                        .filter_map(move |(side, ir)| {
                            ir.as_ref().map(|ir| (format!("{}-{side}", s.speaker), ir))
                        })
                })
                .collect();
            let mut panels: Vec<_> = tasks
                .par_iter()
                .map(|(name, ir)| {
                    check(cancelled)?;
                    let recording = if key == StageKey::PlotPost {
                        Some(ir.convolve(&estimator.test_signal))
                    } else {
                        ir.recording.clone()
                    };
                    prepare_panels_with_floor(ir, name.clone(), recording)
                })
                .collect::<Result<_, _>>()?;
            let sync = PanelLimits::synchronize(panels.iter().map(|(p, _)| &p.limits));
            // Pass two applies the union to every corresponding enabled axis.
            for (p, _) in &mut panels {
                p.limits = sync.clone();
            }
            panels.par_iter().try_for_each(|(p, floor)| {
                check(cancelled)?;
                impulcifer_plots::plot_ir_panels_with_noise_floor(
                    &plots.join(stage).join(format!("{}.png", p.title)),
                    p,
                    *floor,
                )
                .map_err(error)
            })?;
        }
        StageKey::PlotAdditional => {
            hrir.speakers
                .par_iter()
                .try_for_each(|s| -> Result<(), DspError> {
                    check(cancelled)?;
                    if let (Some(l), Some(r)) = (&s.left, &s.right) {
                        impulcifer_plots::plot_interaural_overlay(
                            &plots
                                .join("interaural_overlay")
                                .join(format!("{}_interaural_overlay.png", s.speaker)),
                            &impulcifer_plots::InterauralOverlay {
                                speaker: &s.speaker,
                                left_ir: &l.data,
                                right_ir: &r.data,
                                left_peak: impulcifer_dsp::peaks::first_peak_index(
                                    &l.data, 0, None, 0.12589,
                                ),
                                right_peak: impulcifer_dsp::peaks::first_peak_index(
                                    &r.data, 0, None, 0.12589,
                                ),
                                fs: hrir.fs,
                                time_range_ms: (-5.0, 30.0),
                            },
                        )
                        .map_err(error)?;
                    }
                    Ok(())
                })?;
        }
        _ => (),
    }
    Ok(())
}
