#![forbid(unsafe_code)]
//! ADR 0007 end to end: measurements named after slots become format files in
//! the official channel order, next to hrir.wav and hesuvi.wav.
mod brir_support;
use brir_support::*;
use impulcifer_io::{brir_layout::read_track_names, read_wav, write_wav};
use impulcifer_jobs::registry::{JobRegistry, PollResult};
use impulcifer_types::{
    constants::{HEXADECAGONAL_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER_2X, SPEAKER_NAMES},
    job::JobStatus,
    layouts::{IMMERSIVE_LAYOUTS, Layer, layout},
};
use serde_json::{Value, json};
use std::path::Path;

const FORMAT_FILES: [&str; 4] = [
    "nhk_22.2.wav",
    "auro_13.1.wav",
    "atmos_24.1.10.wav",
    "dtsx_30.2.wav",
];
/// The Korean community's 21-direction measurement in slot codes: 7.1, the
/// Auro height layer (HBL/HBR for HLs/HRs), the Atmos tops, the zenith and
/// the wides (ADR 0007).
const COMMUNITY_21: [&str; 21] = [
    "FL", "FC", "FR", "SL", "SR", "BL", "BR", "HFL", "HFR", "HFC", "HBL", "HBR", "TFL", "TFR",
    "TSL", "TSR", "TBL", "TBR", "TC", "WL", "WR",
];

/// Distinct gain per slot and ear: a swapped speaker or ear changes a ratio.
fn tag(speaker: &str, ear: usize) -> f64 {
    let k = SPEAKER_NAMES.iter().position(|s| *s == speaker).unwrap();
    0.1 + 0.005 * (2 * k + ear) as f64
}
/// One single-speaker recording per slot, made from the demo centre
/// recording's left microphone on both ears so every track has one shape.
fn measurements(temp: &Temp, speakers: &[&str]) {
    let fc = read_wav(&temp.0.join("FC.wav")).unwrap();
    for name in ["FL,FR.wav", "BL,SL.wav", "SR,BR.wav", "FC.wav"] {
        std::fs::remove_file(temp.0.join(name)).unwrap();
    }
    for speaker in speakers {
        let tracks: Vec<Vec<f64>> = (0..2)
            .map(|ear| fc.tracks[0].iter().map(|x| x * tag(speaker, ear)).collect())
            .collect();
        write_wav(
            &temp.0.join(format!("{speaker}.wav")),
            fc.sample_rate,
            &tracks,
            32,
        )
        .unwrap();
    }
}
fn run(temp: &Temp, extra: Value) -> PollResult {
    let jobs = JobRegistry::new();
    let service = service(temp, jobs.clone());
    let mut request = json!({"dir_path":temp.0,"do_room_correction":false,
        "do_headphone_compensation":false,"do_equalization":false});
    request
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let result = service.call("start_brir", vec![request]);
    assert_eq!(result["ok"], true, "{result}");
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    poll
}
fn peak(track: &[f64]) -> f64 {
    track.iter().fold(0.0, |m, x| x.abs().max(m))
}
fn events<'a>(poll: &'a PollResult, key: &str) -> Vec<&'a Value> {
    poll.events
        .iter()
        .filter(|e| e.payload["key"] == key)
        .map(|e| &e.payload)
        .collect()
}
/// Each track of a format file is the hrir.wav track of its slot and ear,
/// and carries that slot's and ear's tag; LFE tracks and the tracks of
/// speakers that were not measured (`measured`) are silent.
fn assert_format_file(dir: &Path, id: &str, measured: &[&str]) {
    let l = layout(id).unwrap();
    let hrir = read_wav(&dir.join("hrir.wav")).unwrap();
    let file = dir.join(l.file_name);
    let wav = read_wav(&file).unwrap();
    assert_eq!(wav.sample_rate, hrir.sample_rate);
    assert_eq!(wav.tracks.len(), 2 * l.channels.len(), "{id}");
    let labels = l.label_track_names();
    let names: Vec<_> = labels.iter().map(String::as_str).collect();
    assert_eq!(
        read_track_names(&file, &names, names.len()).unwrap(),
        Some(labels.clone()),
        "{id} ICHL"
    );
    let reference = peak(&hrir.tracks[0]) / tag("FL", 0);
    for (i, c) in l.channels.iter().enumerate() {
        for ear in 0..2 {
            let track = &wav.tracks[2 * i + ear];
            if c.layer == Layer::Lfe || !measured.contains(&c.slot) {
                assert!(track.iter().all(|x| *x == 0.0), "{id} {}", c.label);
                continue;
            }
            let side = ["left", "right"][ear];
            let index = HEXADECAGONAL_TRACK_ORDER
                .iter()
                .position(|n| *n == format!("{}-{side}", c.slot))
                .unwrap();
            assert_eq!(track, &hrir.tracks[index], "{id} {}-{side}", c.label);
            let ratio = peak(track) / reference;
            assert!(
                (ratio - tag(c.slot, ear)).abs() < 1e-4,
                "{id} {}-{side} carries {ratio}, not the {} tag {}",
                c.label,
                c.slot,
                tag(c.slot, ear)
            );
        }
    }
}

/// The highest-risk mapping: 22.2's FL is ±60° (WL), FLc ±30° (FL), LFE2
/// sits at channel 10, and the upper ring is the height layer. The same slots
/// cover Auro 13.1, which is written too.
#[test]
fn nhk_22_2_file_puts_every_slot_and_ear_on_its_smpte_channel() {
    let temp = Temp::demo();
    let l = layout("22.2").unwrap();
    let speakers: Vec<_> = l
        .channels
        .iter()
        .filter(|c| c.layer != Layer::Lfe)
        .map(|c| c.slot)
        .collect();
    measurements(&temp, &speakers);
    let poll = run(&temp, json!({}));
    assert_format_file(&temp.0, "22.2", &speakers);
    assert_format_file(&temp.0, "13.1", &speakers);
    let written = events(&poll, "cli_success_layout_file");
    assert_eq!(written.len(), 2, "{written:?}");
    assert!(written.iter().all(|e| e["level"] == "SUCCESS"));
    for other in &FORMAT_FILES[2..] {
        assert!(!temp.0.join(other).exists(), "{other}");
    }
}

/// The community set covers Auro 13.1 completely and the other formats only
/// in part: Auro is written automatically, the listed Atmos file is written
/// with silent missing channels, 22.2 and DTS are reported as near misses.
#[test]
fn complete_formats_are_written_and_listed_ones_are_filled_with_silence() {
    let temp = Temp::demo();
    measurements(&temp, &COMMUNITY_21);
    let poll = run(&temp, json!({"layout_files":"24.1.10"}));
    assert_format_file(&temp.0, "13.1", &COMMUNITY_21);
    assert_format_file(&temp.0, "24.1.10", &COMMUNITY_21);
    assert!(!temp.0.join("nhk_22.2.wav").exists());
    assert!(!temp.0.join("dtsx_30.2.wav").exists());
    assert_eq!(events(&poll, "cli_success_layout_file").len(), 2);
    let partial = events(&poll, "cli_warning_layout_partial");
    assert_eq!(partial.len(), 1, "{partial:?}");
    assert_eq!(partial[0]["level"], "WARNING");
    let message = partial[0]["message"].as_str().unwrap();
    assert!(message.contains("Dolby Atmos 24.1.10"), "{message}");
    assert!(message.contains("Lsc (SCL)"), "{message}");
    let near: Vec<_> = events(&poll, "cli_info_layout_missing")
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(near.len(), 2, "{near:?}");
    assert!(
        near.iter()
            .any(|m| m.contains("NHK 22.2") && m.contains("BtFC (DFC)"))
    );
    assert!(
        near.iter()
            .any(|m| m.contains("DTS:X Pro 30.2") && m.contains("Lc (FCL)"))
    );
    let readme = std::fs::read_to_string(temp.0.join("README.md")).unwrap();
    assert!(readme.contains("auro_13.1.wav"), "{readme}");
    assert!(readme.contains("| 25–26 | HLs | HBL |"), "{readme}");
    assert!(readme.contains("atmos_24.1.10.wav"), "{readme}");
}

#[test]
fn layout_files_none_writes_no_format_file() {
    let temp = Temp::demo();
    measurements(&temp, &COMMUNITY_21);
    let poll = run(&temp, json!({"layout_files":"none"}));
    for name in FORMAT_FILES {
        assert!(!temp.0.join(name).exists(), "{name}");
    }
    assert!(events(&poll, "cli_success_layout_file").is_empty());
    assert!(events(&poll, "cli_info_layout_missing").is_empty());
}

/// The 7-speaker demo (7.1 without LFE) is no immersive format and shares no
/// slot outside 7.1 with any: no file, no near-miss notice.
/// A measurement of 2.x speakers keeps the 2.x files: no format file, no
/// notice, and the intermediate responses keep their 32 tracks instead of
/// growing by the silent 3.x slots.
#[test]
fn the_demo_keeps_its_2x_outputs() {
    let temp = Temp::demo();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs.clone());
    let result = service.call("start_brir", vec![json!({"dir_path":temp.0})]);
    let poll = wait(&jobs, result["data"]["job"]["job_id"].as_str().unwrap());
    assert_eq!(poll.job.status, JobStatus::Succeeded);
    for name in FORMAT_FILES {
        assert!(!temp.0.join(name).exists(), "{name}");
    }
    for name in [
        "responses.wav",
        "headphone-responses.wav",
        "room-responses.wav",
    ] {
        let wav = read_wav(&temp.0.join(name)).unwrap();
        assert_eq!(
            wav.tracks.len(),
            HEXADECAGONAL_TRACK_ORDER_2X.len(),
            "{name}"
        );
    }
    for key in [
        "cli_success_layout_file",
        "cli_warning_layout_partial",
        "cli_info_layout_missing",
    ] {
        assert!(events(&poll, key).is_empty(), "{key}");
    }
}

#[test]
fn layout_files_is_a_validated_brir_option() {
    let temp = Temp::new();
    let jobs = JobRegistry::new();
    let service = service(&temp, jobs);
    let boot = service.call("bootstrap", vec![]);
    assert_eq!(boot["data"]["brir_defaults"]["layout_files"], "auto");
    for bad in [json!("7.1.4"), json!(3), json!("auto,22.2")] {
        let result = service.call(
            "start_brir",
            vec![json!({"dir_path":temp.0,"layout_files":bad})],
        );
        assert_eq!(result["ok"], false, "{bad}");
        assert_eq!(result["error"]["code"], "INVALID_REQUEST", "{bad}");
    }
    assert_eq!(IMMERSIVE_LAYOUTS.len(), FORMAT_FILES.len());
}
