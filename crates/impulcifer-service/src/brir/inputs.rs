use super::{BrirError, BrirEvents, discovery::MeasurementDir};
use impulcifer_dsp::{
    estimator::SweepEstimator,
    fr::FrequencyResponse,
    hrir::{Hrir, ingest_recording},
    pipeline::PipelineInputs,
    stages::{
        eq_files::{finalize_eq, looks_like_eqapo_config, read_eq_settings, select_eq_pair},
        eqapo::EqApoLoader,
        headphone::headphone_compensation,
        room::{self, FrCombination, RoomCorrectionOptions},
    },
};
use impulcifer_io::{read_wav, write_wav};
use impulcifer_types::{
    config::ProcessingConfig,
    constants::{HEXADECAGONAL_TRACK_ORDER, Side},
};
use rayon::prelude::*;
use serde_json::json;
use std::path::Path;

fn empty(fs: u32) -> Hrir {
    Hrir {
        fs,
        speakers: Vec::new(),
    }
}
fn ingest(
    hrir: &mut Hrir,
    path: &Path,
    names: &[&str],
    side: Option<Side>,
    estimator: &SweepEstimator,
) -> Result<(), BrirError> {
    let wav = read_wav(path)?;
    hrir.open_recording_samples(estimator, wav.sample_rate, &wav.tracks, names, side, 2.0)?;
    Ok(())
}
fn csv(path: &Path) -> Result<FrequencyResponse, BrirError> {
    let text = std::fs::read_to_string(path)?;
    Ok(FrequencyResponse::parse_csv(
        path.file_stem().and_then(|s| s.to_str()).unwrap_or(""),
        &text.replace("\r\n", "\n").replace('\r', "\n"),
    )?)
}
/// Python's text decoding for eq files: utf-8-sig first, then cp1252.
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_owned(),
        Err(_) => bytes.iter().map(|b| cp1252(*b)).collect(),
    }
    .replace("\r\n", "\n")
    .replace('\r', "\n")
}
/// File access for `Include:` and `Convolution:` lines of an EqualizerAPO
/// config (the dsp parser is file-free; core/eqapo.py:593-600, 860-874).
pub(crate) struct EqFileLoader;
impl EqApoLoader for EqFileLoader {
    fn read_text(&mut self, path: &Path) -> Result<String, String> {
        std::fs::read(path)
            .map(|bytes| decode_text(&bytes))
            .map_err(|e| e.to_string())
    }
    fn read_wav(&mut self, path: &Path) -> Result<(u32, Vec<Vec<f64>>), String> {
        read_wav(path)
            .map(|w| (w.sample_rate, w.tracks))
            .map_err(|e| e.to_string())
    }
}
/// Python `%g` for the preamp report (core/pipeline_stages.py:254-258).
fn fmt_g(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}
const EQAPO_MAX_BYPASS_WARNINGS: usize = 20;
/// Python _read_eq_settings, core/pipeline_stages.py:199-291: a plain CSV or
/// an EqualizerAPO config, returning (left, right-when-channel-split).
fn eq(
    path: Option<&Path>,
    fs: u32,
    events: &mut dyn BrirEvents,
) -> Result<Option<(FrequencyResponse, Option<FrequencyResponse>)>, BrirError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let text = decode_text(&std::fs::read(path)?);
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let file = path.file_name().unwrap().to_string_lossy().into_owned();
    if !looks_like_eqapo_config(&text) {
        let raw = FrequencyResponse::parse_csv(name, &text)?;
        if raw.error.is_empty() && !raw.raw.is_empty() {
            events.log("info", "cli_eq_plain_gain_curve", json!({"file":file}));
        }
    }
    let frequency = impulcifer_dsp::fr::generate_frequencies(10.0, f64::from(fs) / 2.0, 1.01);
    let (left, right, report) = read_eq_settings(
        name,
        &text,
        fs,
        &frequency,
        path.parent(),
        &mut EqFileLoader,
    )?;
    if let Some(report) = report {
        events.log(
            "info",
            "cli_eqapo_detected",
            json!({"file":file,"applied":report.applied.len(),"bypassed":report.bypassed.len(),"skipped":report.skipped_count}),
        );
        if report.preamp_left != 0.0 || report.preamp_right != 0.0 {
            events.log(
                "info",
                "cli_eqapo_preamp",
                json!({"left":fmt_g(report.preamp_left),"right":fmt_g(report.preamp_right)}),
            );
        }
        for item in report.bypassed.iter().take(EQAPO_MAX_BYPASS_WARNINGS) {
            let reason = events.translate(&format!("cli_eqapo_reason_{}", item.reason));
            events.log(
                "warning",
                "cli_eqapo_bypassed_line",
                json!({"line":item.line_number,"command":item.command,"reason":reason}),
            );
        }
        if report.bypassed.len() > EQAPO_MAX_BYPASS_WARNINGS {
            events.log(
                "warning",
                "cli_eqapo_bypassed_more",
                json!({"count":report.bypassed.len() - EQAPO_MAX_BYPASS_WARNINGS}),
            );
        }
        if report.channel_split {
            events.log("info", "cli_eqapo_channel_split", json!({}));
        }
    }
    Ok(Some((left, right)))
}
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    if (0x80..0xa0).contains(&b) {
        HIGH[(b - 0x80) as usize]
    } else {
        char::from(b)
    }
}
pub fn load_inputs(
    dir: &MeasurementDir,
    estimator: &SweepEstimator,
    config: &ProcessingConfig,
    events: &mut dyn BrirEvents,
) -> Result<PipelineInputs, BrirError> {
    let fs = estimator.fs;
    let mut room_result = None;
    if config.do_room_correction {
        events.step("cli_running_room_correction", json!({}))?;
        // Python _open_room_target: a target path that is not a file, even one
        // asked for with --room_target, is a flat target rather than an error.
        let target_csv = dir
            .room
            .target
            .as_deref()
            .filter(|p| p.is_file())
            .map(csv)
            .transpose()?;
        let target = room::prepare_room_target(target_csv.as_ref(), fs)?;
        let calibration = dir
            .room
            .calibration
            .as_deref()
            .map(csv)
            .transpose()?
            .as_ref()
            .map(|fr| room::prepare_mic_calibration(fr, fs))
            .transpose()?;
        let mut rir = empty(fs);
        let mut room_pairs = Vec::new();
        events.check_cancelled()?;
        let room_recordings: Vec<_> = dir
            .room
            .recordings
            .par_iter()
            .map(|(path, names)| {
                let wav = read_wav(path)?;
                let speakers = names
                    .speakers
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                let incoming = ingest_recording(
                    estimator,
                    fs,
                    wav.sample_rate,
                    &wav.tracks,
                    &speakers,
                    names.side,
                    2.0,
                )?;
                Ok(incoming)
            })
            .collect();
        let room_recordings = room_recordings
            .into_iter()
            .collect::<Result<Vec<_>, BrirError>>()?;
        for ((_, names), incoming) in dir.room.recordings.iter().zip(room_recordings) {
            events.check_cancelled()?;
            rir.merge_recording(incoming);
            if config.room_mode == "tuning" {
                room_pairs.extend(impulcifer_dsp::stages::room_tuning::recording_pairs(
                    &rir,
                    &names.speakers,
                    names.side,
                ));
            }
        }
        let generic = if config.room_mode == "tuning" {
            if dir.room.generic.is_some() {
                events.log("info", "cli_room_tuning_room_wav_ignored", json!({}));
            }
            Vec::new()
        } else if let Some(path) = &dir.room.generic {
            let wav = read_wav(path)?;
            room::split_generic_room_recording(estimator, wav.sample_rate, &wav.tracks)?
        } else {
            Vec::new()
        };
        let options = RoomCorrectionOptions {
            fr_combination_method: if config.fr_combination_method == "conservative" {
                FrCombination::Conservative
            } else {
                FrCombination::Average
            },
            specific_limit: config.specific_limit,
            generic_limit: config.generic_limit,
            range: room::RoomRange::parse(&config.room_range).ok_or_else(|| {
                impulcifer_dsp::DspError::InvalidArgument("invalid room_range".into())
            })?,
            room_volume: config.room_volume,
            schroeder_freq: config.schroeder_freq,
            max_boost_db: config.room_max_boost,
            vbass_crossover: config.vbass.then_some(config.vbass_freq as f64),
        };
        room_result = if config.room_mode == "tuning" {
            Some(room::room_correction_tuning(
                &mut rir,
                &generic,
                &target,
                calibration.as_ref(),
                estimator,
                &options,
                &impulcifer_dsp::stages::room_tuning::TuningOptions {
                    delay: impulcifer_types::config::TuningDelay::parse(&config.room_tuning_delay)
                        .ok_or_else(|| {
                            impulcifer_dsp::DspError::InvalidArgument(
                                "invalid room_tuning_delay".into(),
                            )
                        })?,
                    phase_limit: impulcifer_types::config::PhaseLimit::parse(
                        &config.room_tuning_phase_limit,
                    )
                    .ok_or_else(|| {
                        impulcifer_dsp::DspError::InvalidArgument(
                            "invalid room_tuning_phase_limit".into(),
                        )
                    })?,
                    max_boost: config.room_tuning_max_boost,
                    curtain: config.room_tuning_curtain,
                    level_match: config.room_tuning_level_match,
                    pairs: room_pairs,
                    ..Default::default()
                },
            )?)
        } else {
            room::room_correction(
                &mut rir,
                &generic,
                &target,
                calibration.as_ref(),
                estimator,
                &options,
            )?
        };
        if let Some(d) = room_result.as_ref().and_then(|r| r.diagnostics.as_ref()) {
            use impulcifer_dsp::stages::room_v2::{RolloffSource, SchroederSource};
            events.log(
                "info",
                "cli_room_range",
                json!({"range":d.range.as_str(), "f_hi":d.f_hi.round() as i64}),
            );
            let s = &d.schroeder;
            let (level, key, args) = match s.source {
                SchroederSource::Override => (
                    "info",
                    "cli_room_schroeder_override",
                    json!({"freq":s.freq.round() as i64}),
                ),
                SchroederSource::Fallback => (
                    "warning",
                    "cli_room_schroeder_fallback",
                    json!({"freq":s.freq.round() as i64}),
                ),
                SchroederSource::Volume => (
                    "info",
                    "cli_room_schroeder",
                    json!({"freq":s.freq.round() as i64,"t60":format!("{:.2}",s.t60.unwrap()),"volume":format!("{:.1}",s.volume.unwrap())}),
                ),
                SchroederSource::AssumedVolume => (
                    "info",
                    "cli_room_schroeder_assumed",
                    json!({"freq":s.freq.round() as i64,"t60":format!("{:.2}",s.t60.unwrap())}),
                ),
            };
            events.log(level, key, args);
            for ear in &d.ears {
                let side = match ear.side {
                    Some(Side::Left) => "left",
                    Some(Side::Right) => "right",
                    _ => "both",
                };
                if let Some(r) = ear.rolloff {
                    events.log(
                        "info",
                        match r.source {
                            RolloffSource::Slope => "cli_room_rolloff",
                            RolloffSource::Snr => "cli_room_rolloff_snr",
                        },
                        json!({"speaker":ear.speaker,"side":side,"freq":r.freq.round() as i64}),
                    );
                }
                if !ear.snr_available {
                    events.log(
                        "warning",
                        "cli_room_snr_unavailable",
                        json!({"speaker":ear.speaker,"side":side}),
                    );
                }
                if let Some(rms) = ear.residual_rms_db.filter(|r| *r > 3.0) {
                    events.log("warning","cli_room_selfcheck_warning",json!({"speaker":ear.speaker,"side":side,"rms":format!("{rms:.1}"),"lo":ear.residual_lo.round() as i64,"hi":ear.residual_hi.round() as i64}));
                }
            }
            if let Some(freq) = d.vbass_crossover {
                events.log(
                    "info",
                    "cli_room_vbass_handoff",
                    json!({"freq":freq.round() as i64}),
                );
            }
        }
        if config.plot
            && config.room_mode != "tuning"
            && let Some(room) = &room_result
        {
            let token = events.cancel_token();
            let cancelled = move || token.as_ref().is_some_and(|t| t.is_cancelled());
            super::plots::room(&dir.dir, &rir, room, &cancelled)?;
            super::plots::generic_room(
                &dir.dir,
                &generic,
                &target,
                calibration.as_ref(),
                config,
                &room.frs,
                &cancelled,
            )?;
        }
        if let Some(room) = &room_result
            && !room.responses_tracks.is_empty()
        {
            write_wav(
                &dir.dir.join("room-responses.wav"),
                fs,
                &room.responses_tracks,
                32,
            )?;
        }
        events.check_cancelled()?;
    }
    let mut headphone = None;
    if config.do_headphone_compensation {
        events.step("cli_running_headphone_compensation", json!({}))?;
        if let Some(request) = &config.headphone_compensation_file {
            events.log(
                "info",
                "cli_info_hp_param_provided",
                json!({"file":request}),
            );
            if Path::new(request).is_dir() {
                events.log("info", "cli_info_hp_searching_dir", json!({}));
                if let Some(path) = &dir.headphones {
                    events.log("info", "cli_info_hp_found", json!({"file":path}));
                }
            }
            if !Path::new(request).is_dir() && !dir.dir.join(request).is_file() {
                events.log(
                    "warning",
                    "cli_warning_hp_file_not_found",
                    json!({"file":dir.dir.join(request)}),
                );
            }
        } else {
            events.log(
                "info",
                "cli_info_hp_default",
                json!({"file":dir.dir.join("headphones.wav")}),
            );
        }
        if let Some(path) = &dir.headphones {
            events.log("info", "cli_info_using_hp_file", json!({"file":path}));
            let mut hp = empty(fs);
            ingest(&mut hp, path, &["FL", "FR"], None, estimator)?;
            // Python writes before computing headphone compensation curves.
            write_wav(
                &dir.dir.join("headphone-responses.wav"),
                fs,
                &hp.stack_tracks(&HEXADECAGONAL_TRACK_ORDER, false)?,
                32,
            )?;
            headphone = Some(headphone_compensation(&hp)?);
            if let Some(hp) = &headphone {
                super::plots::headphones(
                    &dir.dir.join("plots/headphones.png"),
                    &hp.left,
                    &hp.right,
                    None,
                )?;
            }
        } else {
            events.log(
                "error",
                "cli_error_hp_file_missing",
                json!({"file":dir.dir.join("headphones.wav")}),
            );
            events.log(
                "error",
                "cli_error_hp_ensure_exists",
                json!({"dir":dir.dir}),
            );
        }
        events.check_cancelled()?;
    }
    let (mut eq_left, mut eq_right) = (None, None);
    if config.do_equalization {
        events.step("cli_creating_equalization", json!({}))?;
        if dir.eq.deprecated_wav {
            events.log("warning", "cli_warning_eq_wav_deprecated", json!({}));
        }
        // Python: eq.csv|txt may be channel-split; eq-left takes a file's left
        // curve, eq-right takes its right curve when split, else its only one.
        let (common, common_right) = match eq(dir.eq.common.as_deref(), fs, events)? {
            Some((l, r)) => (Some(l), r),
            None => (None, None),
        };
        let left = eq(dir.eq.left.as_deref(), fs, events)?.map(|(l, _)| l);
        let right = eq(dir.eq.right.as_deref(), fs, events)?.map(|(l, r)| r.unwrap_or(l));
        (eq_left, eq_right) = select_eq_pair(common, common_right, left, right);
        (eq_left, eq_right) = finalize_eq(eq_left, eq_right, fs)?;
        super::plots::eq(
            &dir.dir.join("plots/eq.png"),
            eq_left.as_ref(),
            eq_right.as_ref(),
        )?;
        events.check_cancelled()?;
    }
    events.step("cli_creating_target", json!({}))?;
    events.step("cli_opening_measurements", json!({}))?;
    let mut hrir = empty(fs);
    for (path, names) in &dir.recordings {
        events.check_cancelled()?;
        ingest(
            &mut hrir,
            path,
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
            None,
            estimator,
        )?;
    }
    if hrir.speakers.is_empty() {
        return Err(BrirError::Missing(
            "No HRIR recordings found in the directory.".into(),
        ));
    }
    events.check_cancelled()?;
    Ok(PipelineInputs {
        estimator: estimator.clone(),
        hrir,
        room: room_result,
        headphone,
        eq_left,
        eq_right,
    })
}
