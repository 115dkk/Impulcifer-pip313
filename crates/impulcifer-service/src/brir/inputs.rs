use super::{BrirError, BrirEvents, discovery::MeasurementDir};
use impulcifer_dsp::{
    estimator::SweepEstimator,
    fr::FrequencyResponse,
    hrir::Hrir,
    pipeline::PipelineInputs,
    stages::{
        eq_files::{finalize_eq, looks_like_eqapo_config, read_eq_settings_csv, select_eq_pair},
        headphone::headphone_compensation,
        room::{self, FrCombination, RoomCorrectionOptions},
    },
};
use impulcifer_io::{read_wav, write_wav};
use impulcifer_types::{
    config::ProcessingConfig,
    constants::{HEXADECAGONAL_TRACK_ORDER, Side},
};
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
fn eq(
    path: Option<&Path>,
    events: &mut dyn BrirEvents,
) -> Result<Option<FrequencyResponse>, BrirError> {
    let Some(path) = path else {
        return Ok(None);
    };
    let bytes = std::fs::read(path)?;
    // Windows-1252 fallback, matching the oracle's initial format detection.
    let text = match std::str::from_utf8(&bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_owned(),
        Err(_) => bytes.iter().map(|b| cp1252(*b)).collect(),
    }
    .replace("\r\n", "\n")
    .replace('\r', "\n");
    if looks_like_eqapo_config(&text) {
        return Err(BrirError::Unsupported(format!(
            "{}: EqualizerAPO requires P12",
            path.display()
        )));
    }
    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let raw = FrequencyResponse::parse_csv(name, &text)?;
    if raw.error.is_empty() && !raw.raw.is_empty() {
        events.log(
            "info",
            "cli_eq_plain_gain_curve",
            json!({"file":path.file_name().unwrap().to_string_lossy()}),
        );
    }
    Ok(Some(read_eq_settings_csv(name, &text)?))
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
        let target_csv = dir.room.target.as_deref().map(csv).transpose()?;
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
        for (path, names) in &dir.room.recordings {
            events.check_cancelled()?;
            ingest(
                &mut rir,
                path,
                &names
                    .speakers
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                names.side,
                estimator,
            )?;
        }
        let generic = if let Some(path) = &dir.room.generic {
            let wav = read_wav(path)?;
            room::split_generic_room_recording(estimator, wav.sample_rate, &wav.tracks)?
        } else {
            Vec::new()
        };
        room_result = room::room_correction(
            &mut rir,
            &generic,
            &target,
            calibration.as_ref(),
            estimator,
            &RoomCorrectionOptions {
                fr_combination_method: if config.fr_combination_method == "conservative" {
                    FrCombination::Conservative
                } else {
                    FrCombination::Average
                },
                specific_limit: config.specific_limit,
                generic_limit: config.generic_limit,
            },
        )?;
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
        let common = eq(dir.eq.common.as_deref(), events)?;
        let left = eq(dir.eq.left.as_deref(), events)?;
        let right = eq(dir.eq.right.as_deref(), events)?;
        (eq_left, eq_right) = select_eq_pair(common, None, left, right);
        (eq_left, eq_right) = finalize_eq(eq_left, eq_right, fs)?;
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
