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
    hrir::compact_tracks,
    pipeline::PipelineOutputs,
    stages::readme::{ReadmeData, ReverbKind},
};
use impulcifer_io::{brir_layout::append_track_names, write_wav};
use impulcifer_types::{
    config::ProcessingConfig,
    constants::{
        HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER, Side, TRUEHD_11CH_ORDER, TRUEHD_13CH_ORDER,
    },
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

pub fn render_readme(data: &ReadmeData, catalog: &Catalog, date: &str) -> String {
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
    let content = render_readme(&outputs.readme, i18n, &date);
    let content = if cfg!(windows) {
        content.replace('\n', "\r\n")
    } else {
        content
    };
    events.check_cancelled()?;
    std::fs::write(&readme, content)?;
    files.push(readme);
    for (name, order, tracks) in [
        (
            "hrir.wav",
            HEXADECAGONAL_TRACK_ORDER.as_slice(),
            &outputs.hrir_tracks,
        ),
        (
            "hesuvi.wav",
            HESUVI_TRACK_ORDER.as_slice(),
            &outputs.hesuvi_tracks,
        ),
    ] {
        let path = dir.join(name);
        events.check_cancelled()?;
        write_wav(&path, outputs.hrir.fs, tracks, 32)?;
        if config.remove_silent_channels {
            let full = outputs.hrir.stack_tracks(order, false)?;
            let (_, names) = compact_tracks(&full, order);
            append_track_names(&path, &names.iter().map(String::as_str).collect::<Vec<_>>())?;
        }
        files.push(path);
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
    use super::local_date;
    use chrono::{Local, NaiveDateTime, TimeZone};

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
