use super::{BrirError, BrirEvents, Catalog};
struct Silent;
impl BrirEvents for Silent {
    fn step(&mut self, _: &str, _: serde_json::Value) -> Result<(), BrirError> {
        Ok(())
    }
    fn log(&mut self, _: &str, _: &str, _: serde_json::Value) {}
    fn check_cancelled(&self) -> Result<(), BrirError> {
        Ok(())
    }
}
use impulcifer_dsp::{
    estimator::SweepEstimator,
    pipeline::{LayoutOutput, PipelineOutputs},
    stages::readme::{ReadmeData, ReverbKind},
};
use impulcifer_io::{brir_layout::append_track_names, write_wav};
use impulcifer_types::{
    config::ProcessingConfig,
    constants::{Side, TRUEHD_11CH_ORDER, TRUEHD_13CH_ORDER},
};
use serde_json::json;
use std::path::{Path, PathBuf};

pub struct WrittenOutputs {
    pub hesuvi: PathBuf,
    pub files: Vec<PathBuf>,
}
fn side(s: Side) -> &'static str {
    if s == Side::Left { "left" } else { "right" }
}
fn reverb(k: ReverbKind) -> &'static str {
    match k {
        ReverbKind::Rt60 => "RT60",
        ReverbKind::Rt30 => "RT30",
        ReverbKind::Rt20 => "RT20",
        ReverbKind::Edt => "EDT",
        ReverbKind::Rtxx => "RTxx",
    }
}
fn display_width(s: &str) -> usize {
    // Oracle tabulate 0.10.0 has WIDE_CHARS_MODE=False (no wcwidth dependency).
    s.chars().count()
}
fn pipe_table(headers: &[String], rows: &[Vec<String>]) -> String {
    let widths: Vec<_> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            rows.iter()
                .map(|r| display_width(&r[i]))
                .max()
                .unwrap_or(0)
                .max(display_width(h) + 2)
                .max(3)
        })
        .collect();
    let row = |cells: &[String]| -> String {
        format!(
            "| {} |",
            cells
                .iter()
                .zip(&widths)
                .map(|(v, w)| format!("{v}{}", " ".repeat(w - display_width(v))))
                .collect::<Vec<_>>()
                .join(" | ")
        )
    };
    let mut lines = vec![
        row(headers),
        format!(
            "|{}|",
            widths
                .iter()
                .map(|w| format!(":{}", "-".repeat(w + 1)))
                .collect::<Vec<_>>()
                .join("|")
        ),
    ];
    lines.extend(rows.iter().map(|r| row(r)));
    lines.join("\n")
}
fn local_date() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn missing_channels(channels: &[&impulcifer_types::layouts::FormatChannel]) -> String {
    channels
        .iter()
        .map(|c| format!("{} ({})", c.label, c.slot))
        .collect::<Vec<_>>()
        .join(", ")
}
fn speaker_fills(filled: &[(&'static str, String)]) -> String {
    filled
        .iter()
        .filter(|(slot, _)| !matches!(*slot, "LFE" | "LFE2"))
        .map(|(slot, source)| format!("{slot} ← {source}"))
        .collect::<Vec<_>>()
        .join(", ")
}
fn lfe_source<'a>(filled: &'a [(&'static str, String)]) -> Option<&'a str> {
    filled
        .iter()
        .find(|(slot, _)| matches!(*slot, "LFE" | "LFE2"))
        .map(|(_, source)| source.as_str())
}
fn display_lfe_source(catalog: &Catalog, source: &str) -> String {
    if source == "all" {
        catalog.translate("cli_lfe_source_all", &json!({}))
    } else {
        source.into()
    }
}

pub(super) fn tuning2_mode_name(mode: impulcifer_types::config::Tuning2Mode) -> String {
    use impulcifer_types::config::Tuning2Mode;
    match mode {
        Tuning2Mode::Zero => "zero".into(),
        Tuning2Mode::Mixed => "mixed".into(),
        Tuning2Mode::Linear => "linear".into(),
        Tuning2Mode::Long => "long".into(),
        Tuning2Mode::Ultra(ms) => format!("{ms:.0}"),
    }
}
pub(super) fn tuning2_representation(
    s: &impulcifer_dsp::stages::room_tuning2::Speaker2Report,
) -> Option<f64> {
    (!s.representation_rms_db.is_empty()).then(|| {
        s.representation_rms_db.iter().map(|(_, v)| v).sum::<f64>()
            / s.representation_rms_db.len() as f64
    })
}
pub(crate) fn tuning2_high_key(
    band: &impulcifer_dsp::stages::room_tuning2::CorrectionBand,
) -> Option<&'static str> {
    use impulcifer_dsp::stages::room_tuning2::BandSource;
    if band.source == BandSource::Manual || (band.basis == "strict" && band.transition == 1) {
        None
    } else if band.automatic_basis_configured {
        Some("cli_room_tuning2_high")
    } else {
        Some("cli_room_tuning2_high_no_volume")
    }
}

pub(crate) fn pair_phase_spans(
    bands: &[impulcifer_dsp::stages::room_tuning2::PairPhaseBand],
) -> String {
    let mut spans = Vec::new();
    let mut start = 0;
    while start < bands.len() {
        let mut end = start + 1;
        while end < bands.len() && bands[end - 1].high_hz == bands[end].low_hz {
            end += 1;
        }
        let worst = bands[start..end]
            .iter()
            .min_by(|left, right| left.loss_db.total_cmp(&right.loss_db))
            .unwrap();
        spans.push(format!(
            "{:.0}-{:.0} Hz {:.0}° {:.1} dB",
            bands[start].low_hz,
            bands[end - 1].high_hz,
            worst.phase_deg.abs(),
            worst.loss_db
        ));
        start = end;
    }
    spans.join(", ")
}

fn tuning2_readme(
    report: &impulcifer_dsp::stages::room_tuning2::Tuning2Report,
    catalog: &Catalog,
) -> String {
    use impulcifer_dsp::stages::room_tuning2::BandSource;
    let mut out = format!(
        "## Virtual room tuning v2\n\nMode: {}. Latency: {:.2} ms.",
        tuning2_mode_name(report.mode),
        report.latency_samples as f64 * 1000.0 / report.fs as f64
    );
    if let Some(band) = &report.band {
        let source = match band.source {
            BandSource::Manual => "manual",
            BandSource::Override => "Schroeder override",
            BandSource::Volume => "room volume",
            BandSource::Default => "default",
        };
        out.push_str(&format!(
            " Band: {:.0}–{:.0} Hz ({source}).",
            band.low, band.high
        ));
        match tuning2_high_key(band) {
            Some("cli_room_tuning2_high_no_volume") => {
                out.push('\n');
                out.push_str(&catalog.translate("cli_room_tuning2_high_no_volume", &json!({})));
            }
            Some("cli_room_tuning2_high") => {
                out.push('\n');
                out.push_str(&catalog.translate(
                    "cli_room_tuning2_high",
                    &json!({
                        "basis":band.basis,
                        "factor":band.transition,
                        "schroeder":band.schroeder.map_or_else(|| "none".into(), |value| format!("{:.0}",value)),
                        "band":band.lenient.map_or_else(|| "none".into(), |value| format!("{value:.0}")),
                        "band_t60":band.lenient_t60_s.map_or_else(|| "none".into(), |value| format!("{value:.2}")),
                        "high":format!("{:.0}",band.high),
                    }),
                ));
            }
            Some(_) | None => {}
        }
    }
    if let Some(cutoff) = report.input_hp {
        out.push('\n');
        out.push_str(&catalog.translate(
            "cli_room_tuning2_input_hp",
            &json!({"freq":format!("{cutoff:.1}")}),
        ));
    }
    out.push_str("\n\n| Speaker | Points | Level (dB) | Trim (dB) | Source | PEQ bands | Phase strength | Phase resolution | Long modes (Hz) | Representation RMS (dB) | LF EDT before / after (ms, information) | Median excess GD before / after (ms, information) | Weak |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for s in &report.speakers {
        let resolution = if s.phase_frac == 0.0 {
            "—".into()
        } else {
            format!("1/{:.0} oct", s.phase_frac)
        };
        let modes = if s.long_modes.is_empty() {
            "—".into()
        } else {
            s.long_modes
                .iter()
                .map(|(hz, _)| format!("{hz:.1}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let rms = tuning2_representation(s).map_or_else(|| "—".into(), |v| format!("{v:.2}"));
        out.push_str(&format!("| {} | {} | {:.2} | {:.2} | {} | {} | {:.1} | {} | {} | {} | {:.2} / {:.2} | {:.2} / {:.2} | {} |\n", s.speaker, s.points, s.level_db, s.trim_db, s.trim_source, s.peq_bands, s.phase_strength, resolution, modes, rms, s.lf_edt_before_ms, s.lf_edt_after_ms, s.excess_median_before_ms, s.excess_median_after_ms, s.weak));
    }
    if report.extra_bands
        && let Some(band) = &report.band
    {
        let format_hz = |value: f64| {
            if value.fract() == 0.0 {
                format!("{value:.0}")
            } else {
                value.to_string()
            }
        };
        let bands = band
            .bands
            .iter()
            .map(|(low, high)| format!("{}-{}", format_hz(*low), format_hz(*high)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push('\n');
        out.push_str(&catalog.translate("cli_room_tuning2_bands", &json!({"bands":bands})));
        out.push('\n');
    }
    if report.mode == impulcifer_types::config::Tuning2Mode::Zero {
        let mut inert = Vec::new();
        if report.lf_align {
            inert.push("room_tuning2_lf_align");
        }
        if report.hf_phase {
            inert.push("room_tuning2_hf_phase");
        }
        if !inert.is_empty() {
            out.push('\n');
            out.push_str(&catalog.translate(
                "cli_room_tuning2_zero_inert",
                &json!({"options":inert.join(", ")}),
            ));
        }
    }
    for speaker in &report.speakers {
        if report.low_protect {
            let rolloff = speaker
                .rolloff_hz
                .map_or_else(|| "none".to_owned(), |value| format!("{value:.1}"));
            out.push('\n');
            out.push_str(&catalog.translate(
                "cli_room_tuning2_low_protect",
                &json!({
                    "speaker":speaker.speaker,
                    "rolloff":rolloff,
                    "reduced":speaker.low_protect_reduced,
                    "cuts":speaker.low_protect_cuts,
                    "limited":speaker.low_protect_limited,
                }),
            ));
        }
        if report.dip_limit
            && let Some((hz, depth, fill)) = speaker.limited_dips.first()
        {
            out.push('\n');
            out.push_str(&catalog.translate(
                "cli_room_tuning2_dip_limit",
                &json!({
                    "speaker":speaker.speaker,
                    "count":speaker.limited_dips.len(),
                    "hz":format!("{hz:.1}"),
                    "depth":format!("{depth:.1}"),
                    "fill":format!("{fill:.1}"),
                }),
            ));
        }
        if let Some(result) = &speaker.lf_align {
            out.push('\n');
            out.push_str(&catalog.translate(
                "cli_room_tuning2_lf_align",
                &json!({
                    "speaker":speaker.speaker,
                    "f_lin":format!("{:.1}",result.f_lin_hz),
                    "strength":format!("{:.1}",speaker.phase_strength),
                    "arrival":result.arrival_ms.map_or_else(|| "none".to_owned(), |value| format!("{value:.1}")),
                    "limited":result.limited,
                }),
            ));
        }
        if let Some(result) = &speaker.mode_decay {
            out.push('\n');
            if result.cuts.is_empty() {
                out.push_str(&catalog.translate(
                    "cli_room_tuning2_mode_decay_none",
                    &json!({"speaker":speaker.speaker,"target":format!("{:.2}",result.target_t60_s)}),
                ));
            } else {
                let cuts = result
                    .cuts
                    .iter()
                    .map(|cut| {
                        format!(
                            "{:.1} Hz {:.2} s -{:.1} dB",
                            cut.frequency_hz, cut.measured_t60_s, cut.depth_db
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&catalog.translate(
                    "cli_room_tuning2_mode_decay",
                    &json!({"speaker":speaker.speaker,"target":format!("{:.2}",result.target_t60_s),"count":result.cuts.len(),"cuts":cuts}),
                ));
            }
        }
        if let Some(result) = &speaker.hf_phase {
            out.push('\n');
            if let Some(rotation) = result.rotation_deg {
                out.push_str(&catalog.translate(
                    "cli_room_tuning2_hf_phase",
                    &json!({"speaker":speaker.speaker,"rotation":format!("{rotation:.1}"),"limited":result.limited}),
                ));
            } else {
                let high = speaker.bands.last().map_or(0.0, |band| band.1);
                let low = high * 2_f64.sqrt();
                out.push_str(&catalog.translate(
                    "cli_room_tuning2_hf_phase_none",
                    &json!({"speaker":speaker.speaker,"low":low.round() as i64,"high":(8.0*low).min(0.45*report.fs as f64).round() as i64}),
                ));
            }
        }
    }
    if report.extra_bands
        || report.low_protect
        || report.dip_limit
        || report.lf_align
        || report.mode_decay
        || report.hf_phase
    {
        out.push('\n');
    }
    if !report.pair_phase.is_empty() {
        out.push_str("\n### Pair phase\n");
        for pair in &report.pair_phase {
            for (ear, bands) in [("left", &pair.left), ("right", &pair.right)] {
                if bands.is_empty() {
                    continue;
                }
                out.push('\n');
                out.push_str(&catalog.translate(
                    "cli_room_tuning2_pair_phase",
                    &json!({
                        "a":pair.a,
                        "b":pair.b,
                        "ear":ear,
                        "spans":pair_phase_spans(bands),
                    }),
                ));
            }
        }
        if report.mode != impulcifer_types::config::Tuning2Mode::Zero && !report.lf_align {
            out.push('\n');
            out.push_str(&catalog.translate("cli_room_tuning2_pair_phase_hint", &json!({})));
        }
        out.push('\n');
    }
    out.push_str("\nPre-echo uses the first −18 dB direct peak and a 2 ms guard, against the louder of the speaker's two direct sounds. Warn only above −30 dB and at least 6 dB above the same untuned ear. Weak means mean omni/in-ear magnitude RMS exceeds 4 dB (30–300 Hz, level matched at 100–300 Hz); EDT and excess GD are information only.\n\n| Speaker | Ear | Pre-echo untuned / tuned (dB) | Warning |\n| --- | --- | --- | --- |\n");
    for s in &report.speakers {
        for p in &s.pre_echo_channels {
            out.push_str(&format!(
                "| {} | {} | {:.2} / {:.2} | {} |\n",
                s.speaker,
                side(p.side),
                p.before_db,
                p.after_db,
                p.warns()
            ));
        }
    }
    if let Some(mimo) = &report.mimo {
        out.push_str("\n### MIMO supports\n\n| Input | Supports | Support level (dB) | Guard | Band change (dB) | Tail change (dB) |\n| --- | --- | --- | --- | --- | --- |\n");
        for input in &mimo.inputs {
            let supports = if input.supports.is_empty() {
                "—".into()
            } else {
                input
                    .supports
                    .iter()
                    .map(|s| s.speaker.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let guard = input
                .scale_db
                .map_or_else(|| "off".into(), |v| format!("{v:.0} dB").replace('-', "−"));
            out.push_str(&format!(
                "| {} | {} | {:.1} | {} | {:.2} | {:.2} |\n",
                input.speaker,
                supports,
                input.level_db,
                guard,
                input.band_change_db,
                input.tail_change_db
            ));
        }
    }
    out.push_str("\nVirtual room tuning v2 reproduces the correction method of SECS 2 by the Speaker Gallery (DCinside), released under GPL-3.0. Impulcifer's implementation is independent and contains no SECS code; SECS 2 was used only as a black box to produce the reference numbers.\n\n");
    out
}

fn alignment_readme(report: &impulcifer_dsp::itd::AlignmentReport, catalog: &Catalog) -> String {
    let integer = |value: Option<f64>| {
        value.map_or_else(|| "?".into(), |value| format!("{:.0}", value.round()))
    };
    let decimal =
        |value: Option<f64>| value.map_or_else(|| "?".into(), |value| format!("{value:.1}"));
    let mut out = format!(
        "## {}\n\n",
        catalog.translate("cli_readme_itd_title", &json!({}))
    );
    for speaker in &report.speakers {
        out.push_str(&catalog.translate(
            "cli_readme_itd_row",
            &json!({
                "speaker":speaker.speaker,
                "measured_us":integer(speaker.measured_us),
                "model_us":integer(Some(speaker.model_us)),
                "after_us":integer(speaker.after_us),
            }),
        ));
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&catalog.translate(
        "cli_readme_itd_summary",
        &json!({
            "yaw":decimal(report.yaw_deg),
            "offset_us":integer(report.common_offset_us),
            "balance":decimal(report.input_balance_db),
            "radius_cm":format!("{:.1}",report.head_radius_cm),
        }),
    ));
    out.push('\n');
    if let (Some(speaker), Some(headphone), Some(predicted)) = (
        report.input_balance_db,
        report.headphone_balance_db,
        report.predicted_balance_db,
    ) {
        out.push_str(&catalog.translate(
            "cli_readme_itd_pairing",
            &json!({
                "speaker_db":format!("{speaker:.1}"),
                "headphone_db":format!("{headphone:.1}"),
                "predicted_db":format!("{predicted:.1}"),
            }),
        ));
        out.push('\n');
    }
    out.push('\n');
    out
}

pub fn render_readme(data: &ReadmeData, catalog: &Catalog, date: &str) -> String {
    render_readme_with_layouts(data, None, catalog, date, &[], &[])
}

fn render_readme_with_layouts(
    data: &ReadmeData,
    alignment: Option<&impulcifer_dsp::itd::AlignmentReport>,
    catalog: &Catalog,
    date: &str,
    layouts: &[LayoutOutput],
    fixed_fills: &[(&str, &[(&'static str, String)])],
) -> String {
    let t = |key: &str| catalog.translate(key, &json!({}));
    let mut out = format!(
        "# {}\n\n{}\n\n## {}\n{}\n\n",
        t("cli_readme_title"),
        catalog.translate("cli_readme_processed", &json!({"date":date,"fs":data.fs})),
        t("cli_readme_gain_title"),
        catalog.translate(
            "cli_readme_gain_value",
            &json!({"gain":format!("{:.2}",data.applied_gain_db)})
        )
    );
    let value = |v: f64, unit: &str| {
        if v.is_nan() {
            "N/A".into()
        } else {
            format!("{v:.1} {unit}")
        }
    };
    let rows: Vec<_> = data
        .rows
        .iter()
        .map(|r| {
            vec![
                r.speaker.clone(),
                t(&format!("cli_readme_side_{}", side(r.side))),
                value(r.pnr_db, "dB"),
                value(r.itd_us, "us"),
                value(r.length_ms.unwrap_or(f64::NAN), "ms"),
                value(r.reverb.map_or(f64::NAN, |(_, v)| v), "ms"),
            ]
        })
        .collect();
    if !rows.is_empty() {
        out.push_str(&pipe_table(
            &[
                t("cli_readme_header_speaker"),
                t("cli_readme_header_side"),
                "PNR".into(),
                "ITD".into(),
                t("cli_readme_header_length"),
                reverb(data.reverb_header).into(),
            ],
            &rows,
        ));
        out.push_str("\n\n");
    }
    if let Some(report) = &data.tuning {
        out.push_str(&format!(
            "## Virtual room tuning\n\nDelay: {:.2} ms. Phase limit: {:.0} Hz (0 = off).\n\n",
            report.delay_samples as f64 * 1000.0 / report.fs as f64,
            report.f_ph
        ));
        out.push_str("| Speaker | Points | Low / high cutoff (Hz) | Trim (dB) | Source | Representation RMS (dB) | LF EDT before / after (ms, information) | Median excess GD before / after (ms, information) | Weak |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
        for s in &report.speakers {
            out.push_str(&format!(
                "| {} | {} | {:.0} / {:.0} | {:.2} | {} | {} | {:.2} / {:.2} | {:.2} / {:.2} | {} |\n",
                s.speaker,
                s.points,
                s.low_cutoff,
                s.high_cutoff,
                s.trim_db,
                s.trim_source,
                s.representation_mean_db().map_or_else(|| "—".into(), |v| format!("{v:.2}")),
                s.lf_edt_before_ms,
                s.lf_edt_after_ms,
                s.excess_median_before_ms,
                s.excess_median_after_ms,
                s.weak()
            ));
        }
        out.push_str("\nPre-echo uses the first −18 dB direct peak and a 2 ms guard, against the louder of the speaker's two direct sounds. Warn only above −30 dB and at least 6 dB above the same untuned ear. Weak means mean omni/in-ear magnitude RMS exceeds 4 dB (30–300 Hz, level matched at 100–300 Hz); EDT and excess GD are information only.\n\n| Speaker | Ear | Pre-echo untuned / tuned (dB) | Warning |\n| --- | --- | --- | --- |\n");
        for s in &report.speakers {
            for p in &s.pre_echo_channels {
                out.push_str(&format!(
                    "| {} | {} | {:.2} / {:.2} | {} |\n",
                    s.speaker,
                    side(p.side),
                    p.before_db,
                    p.after_db,
                    p.warns()
                ));
            }
        }
        out.push('\n');
    }
    if let Some(report) = &data.tuning2 {
        out.push_str(&tuning2_readme(report, catalog));
    }
    let has_tuning = data.tuning.is_some() || data.tuning2.is_some();
    if has_tuning && let Some(alignment) = alignment {
        out.push_str(&alignment_readme(alignment, catalog));
    }
    if !data.reflections.is_empty() {
        out.push_str(&format!("## {}\n", t("cli_readme_reflection_title")));
        let mut previous = "";
        for (speaker, s, levels) in &data.reflections {
            if previous != speaker {
                out.push_str(&format!("### {speaker}\n"));
                previous = speaker;
            }
            out.push_str(&format!(
                "- {}: {}: {:.2} dB, {}: {:.2} dB\n",
                t(&format!("cli_readme_{}_ear", side(*s))),
                t("cli_readme_early_label"),
                levels.early_db,
                t("cli_readme_late_label"),
                levels.late_db
            ));
        }
        out.push('\n');
    }
    if fixed_fills.iter().any(|(_, filled)| !filled.is_empty()) {
        out.push_str("## hrir.wav and hesuvi.wav\n\n");
        for (file, filled) in fixed_fills {
            let list = speaker_fills(filled);
            if !list.is_empty() {
                out.push_str(&format!("### {file}\n\n"));
                out.push_str(
                    &catalog.translate("cli_readme_filled_channels", &json!({"list":list})),
                );
                out.push_str("\n\n");
            }
        }
    }
    if !has_tuning && let Some(alignment) = alignment {
        out.push_str(&alignment_readme(alignment, catalog));
    }
    if !layouts.is_empty() {
        out.push_str(&format!("## {}\n\n", t("cli_readme_layouts_title")));
        for output in layouts {
            let layout = output.layout;
            out.push_str(&format!("### {} ({})\n\n", layout.file_name, layout.name));
            // The usual note says LFE tracks are silent; a filled LFE gets the
            // note without that sentence and its source below (ADR 0013).
            let note = if lfe_source(&output.filled).is_some() {
                "cli_readme_layouts_note_lfe_filled"
            } else {
                "cli_readme_layouts_note"
            };
            out.push_str(&format!("{}\n\n", t(note)));
            let order_note = match layout.id {
                "24.1.10" => Some("cli_readme_layouts_atmos_order"),
                "30.2" => Some("cli_readme_layouts_dts_order"),
                "9.1.6" => Some("cli_readme_layouts_atmos_916_order"),
                "9.1" | "9.1.4" => Some("cli_readme_layouts_atmos_subset_order"),
                _ => None,
            };
            if let Some(key) = order_note {
                out.push_str(&format!("{}\n\n", t(key)));
            }
            let fills = speaker_fills(&output.filled);
            if !fills.is_empty() {
                out.push_str(
                    &catalog.translate("cli_readme_filled_channels", &json!({"list":fills})),
                );
                out.push_str("\n\n");
            }
            if let Some(source) = lfe_source(&output.filled) {
                out.push_str(&catalog.translate(
                    "cli_readme_lfe_filled",
                    &json!({"source":display_lfe_source(catalog, source)}),
                ));
                out.push_str("\n\n");
            }
            if !output.missing.is_empty() {
                out.push_str(&catalog.translate(
                    "cli_readme_layouts_partial",
                    &json!({"missing":missing_channels(&output.missing)}),
                ));
                out.push_str("\n\n");
            }
            out.push_str(&format!(
                "| {} | {} | {} |\n| --- | --- | --- |\n",
                t("cli_readme_layouts_header_tracks"),
                t("cli_readme_layouts_header_channel"),
                t("cli_readme_layouts_header_speaker"),
            ));
            for (i, channel) in layout.channels.iter().enumerate() {
                let slot = if channel.layer == impulcifer_types::layouts::Layer::Lfe {
                    "—"
                } else {
                    channel.slot
                };
                out.push_str(&format!(
                    "| {}–{} | {} | {} |\n",
                    2 * i + 1,
                    2 * i + 2,
                    channel.label,
                    slot,
                ));
            }
            out.push('\n');
        }
    }
    out
}
pub fn write_outputs(
    dir: &Path,
    outputs: &PipelineOutputs,
    config: &ProcessingConfig,
    i18n: &Catalog,
    estimator: &SweepEstimator,
) -> Result<WrittenOutputs, BrirError> {
    write_outputs_checked(dir, outputs, config, i18n, estimator, &mut Silent)
}

pub(crate) fn write_outputs_checked(
    dir: &Path,
    outputs: &PipelineOutputs,
    config: &ProcessingConfig,
    i18n: &Catalog,
    estimator: &SweepEstimator,
    events: &mut dyn BrirEvents,
) -> Result<WrittenOutputs, BrirError> {
    let mut files = Vec::new();
    events.check_cancelled()?;
    let responses = dir.join("responses.wav");
    write_wav(&responses, estimator.fs, &outputs.responses_tracks, 32)?;
    files.push(responses);
    let date = i18n.readme_date.clone().unwrap_or_else(local_date);
    let readme = dir.join("README.md");
    // Python open(..., "w") uses the host's native text-mode newline.
    let content = render_readme_with_layouts(
        &outputs.readme,
        Some(&outputs.alignment),
        i18n,
        &date,
        &outputs.layouts,
        &[
            ("hrir.wav", &outputs.hrir_filled),
            ("hesuvi.wav", &outputs.hesuvi_filled),
        ],
    );
    let content = if cfg!(windows) {
        content.replace('\n', "\r\n")
    } else {
        content
    };
    events.check_cancelled()?;
    std::fs::write(&readme, content)?;
    files.push(readme);
    for (name, tracks, names) in [
        ("hrir.wav", &outputs.hrir_tracks, &outputs.hrir_track_names),
        (
            "hesuvi.wav",
            &outputs.hesuvi_tracks,
            &outputs.hesuvi_track_names,
        ),
    ] {
        let path = dir.join(name);
        events.check_cancelled()?;
        write_wav(&path, outputs.hrir.fs, tracks, 32)?;
        if config.remove_silent_channels {
            // The names of the tracks written, filled channels included (ADR 0013).
            append_track_names(&path, &names.iter().map(String::as_str).collect::<Vec<_>>())?;
        }
        let filled = if name == "hrir.wav" {
            &outputs.hrir_filled
        } else {
            &outputs.hesuvi_filled
        };
        let list = speaker_fills(filled);
        if !list.is_empty() {
            events.log(
                "info",
                "cli_info_filled_channels",
                json!({"file":name,"list":list}),
            );
        }
        files.push(path);
    }
    for output in &outputs.layouts {
        let layout = output.layout;
        let path = dir.join(layout.file_name);
        events.check_cancelled()?;
        write_wav(&path, outputs.hrir.fs, &output.tracks, 32)?;
        let names = layout.label_track_names();
        append_track_names(&path, &names.iter().map(String::as_str).collect::<Vec<_>>())?;
        events.log(
            "success",
            "cli_success_layout_file",
            json!({"layout":layout.name,"path":path}),
        );
        let fills = speaker_fills(&output.filled);
        if !fills.is_empty() {
            events.log(
                "info",
                "cli_info_filled_channels",
                json!({"file":layout.file_name,"list":fills}),
            );
        }
        if let Some(source) = lfe_source(&output.filled) {
            events.log(
                "info",
                "cli_info_lfe_filled",
                json!({"file":layout.file_name,"source":display_lfe_source(i18n, source)}),
            );
        }
        if !output.missing.is_empty() {
            events.log(
                "warning",
                "cli_warning_layout_partial",
                json!({"layout":layout.name,"file":layout.file_name,"missing":missing_channels(&output.missing)}),
            );
        }
        files.push(path);
    }
    for (layout, missing) in &outputs.layout_notices {
        events.log(
            "info",
            "cli_info_layout_missing",
            json!({"layout":layout.name,"count":missing.len(),"missing":missing_channels(missing)}),
        );
    }
    if config.output_truehd_layouts || !outputs.truehd.is_empty() {
        for (label, order, minimum) in [
            ("11ch", TRUEHD_11CH_ORDER.as_slice(), 8),
            ("13ch", TRUEHD_13CH_ORDER.as_slice(), 10),
        ] {
            if let Some((name, _, tracks)) = outputs
                .truehd
                .iter()
                .find(|(name, _, _)| name.starts_with(&format!("truehd_{label}_")))
            {
                let path = dir.join(name);
                events.check_cancelled()?;
                write_wav(&path, outputs.hrir.fs, tracks, 32)?;
                events.log(
                    "success",
                    &format!("cli_success_truehd_{label}"),
                    json!({"path":path}),
                );
                files.push(path);
            } else {
                let missing: Vec<_> = order
                    .iter()
                    .filter(|s| outputs.hrir.get(s).is_none())
                    .map(|s| format!("'{s}'"))
                    .collect();
                let count = order.len() - missing.len();
                events.log("warning", &format!("cli_warning_truehd_{label}_fail"), json!({"msg":format!("Insufficient channels: need {minimum}, have {count}. Missing: [{}]", missing.join(", "))}));
            }
        }
    }
    if let Some(tracks) = &outputs.jamesdsp {
        let path = dir.join("jamesdsp.wav");
        events.check_cancelled()?;
        write_wav(&path, outputs.hrir.fs, tracks, 32)?;
        events.log("success", "cli_success_jamesdsp", json!({"path":path}));
        files.push(path);
    }
    for (name, tracks) in &outputs.hangloose {
        let path = dir.join("Hangloose").join(format!("{name}.wav"));
        events.check_cancelled()?;
        write_wav(&path, outputs.hrir.fs, tracks, 32)?;
        events.log(
            "info",
            "cli_success_hangloose_file",
            json!({"file":format!("{name}.wav")}),
        );
        files.push(path);
    }
    if config.hangloose {
        events.log(
            "success",
            "cli_success_hangloose",
            json!({"path":dir.join("Hangloose")}),
        );
    }
    Ok(WrittenOutputs {
        hesuvi: dir.join("hesuvi.wav"),
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::{local_date, tuning2_high_key};
    use chrono::{Local, NaiveDateTime, TimeZone};
    use impulcifer_dsp::{
        ir::ImpulseResponse,
        stages::room_tuning2::{RcOptions, correction_band},
    };

    #[test]
    fn upper_limit_warning_depends_on_setting_presence_not_decay_result() {
        let point = ImpulseResponse {
            fs: 48_000,
            data: vec![0.0; 64],
            recording: None,
        };
        let options = RcOptions {
            high_basis: "lenient".into(),
            high_transition: 2,
            ..Default::default()
        };
        let no_volume = correction_band(&[std::slice::from_ref(&point)], &options).unwrap();
        assert_eq!(
            tuning2_high_key(&no_volume),
            Some("cli_room_tuning2_high_no_volume")
        );
        let with_volume = correction_band(
            &[std::slice::from_ref(&point)],
            &RcOptions {
                room_volume: Some(50.0),
                ..options
            },
        )
        .unwrap();
        assert_eq!(with_volume.schroeder, None);
        assert_eq!(with_volume.lenient, None);
        assert_eq!(
            tuning2_high_key(&with_volume),
            Some("cli_room_tuning2_high")
        );
    }

    #[test]
    fn local_date_has_python_format_and_current_clock() {
        let date = local_date();
        assert!(
            regex::Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$")
                .unwrap()
                .is_match(&date)
        );
        let parsed = NaiveDateTime::parse_from_str(&date, "%Y-%m-%d %H:%M:%S").unwrap();
        let now = Local::now();
        let resolved = Local.from_local_datetime(&parsed);
        // Either offset is valid during the repeated hour at the end of DST.
        assert!(
            [resolved.earliest(), resolved.latest()]
                .into_iter()
                .flatten()
                .any(|date| (now - date).num_milliseconds().abs() <= 5000),
            "local date {date} is not within five seconds of {now}"
        );
    }
}
