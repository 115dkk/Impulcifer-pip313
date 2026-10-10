#![forbid(unsafe_code)]
//! ADR 0007 rule 6: output recovery reads and writes the format files by
//! slot, never by track index, and keeps every source in agreement.
use impulcifer_io::{brir_layout::read_track_names, read_wav, write_wav};
use impulcifer_service::recovery::{
    RecoveryErrorCode, RecoveryOptions, plan_brir_outputs, recover_brir_outputs,
};
use impulcifer_types::{
    constants::{HESUVI_TRACK_ORDER, HEXADECAGONAL_TRACK_ORDER, SPEAKER_NAMES},
    layouts::{ImmersiveLayout, Layer, layout},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "impulcifer-layout-recovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const COUNT: usize = 480;
/// A distinct constant per slot and ear (exact in PCM_32).
fn sentinel(slot: &str, ear: usize) -> Vec<f64> {
    let k = SPEAKER_NAMES.iter().position(|s| *s == slot).unwrap();
    vec![(2 * k + ear + 1) as f64 / 128.0; COUNT]
}
fn track(name: &str) -> Vec<f64> {
    let (slot, ear) = name.rsplit_once('-').unwrap();
    if slot.starts_with("LFE") {
        return vec![0.0; COUNT];
    }
    sentinel(slot, (ear == "right") as usize)
}
/// A format file as Impulcifer writes it, with only `measured` slots sounding.
fn write_format(dir: &Path, l: &ImmersiveLayout, measured: &[&str]) -> PathBuf {
    let path = dir.join(l.file_name);
    let tracks: Vec<_> = l
        .track_names()
        .iter()
        .map(|n| {
            let slot = n.rsplit_once('-').unwrap().0;
            if measured.contains(&slot) {
                track(n)
            } else {
                vec![0.0; COUNT]
            }
        })
        .collect();
    write_wav(&path, 48000, &tracks, 32).unwrap();
    impulcifer_io::brir_layout::append_track_names(
        &path,
        &l.label_track_names()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    path
}
fn slots(l: &ImmersiveLayout) -> Vec<&'static str> {
    l.channels
        .iter()
        .filter(|c| c.layer != Layer::Lfe)
        .map(|c| c.slot)
        .collect()
}
/// Every track of `file` (in `order`, read by name) holds its sentinel.
fn assert_tracks(file: &Path, order: &[String], measured: &[&str]) {
    let wav = read_wav(file).unwrap();
    assert_eq!(wav.tracks.len(), order.len(), "{}", file.display());
    for (name, data) in order.iter().zip(&wav.tracks) {
        let slot = name.rsplit_once('-').unwrap().0;
        let expected = if measured.contains(&slot) {
            track(name)
        } else {
            vec![0.0; COUNT]
        };
        assert_eq!(data, &expected, "{} {name}", file.display());
    }
}
fn strings(order: &[&str]) -> Vec<String> {
    order.iter().map(|s| (*s).to_owned()).collect()
}

/// Only nhk_22.2.wav survives: hrir.wav and hesuvi.wav come back with every
/// slot at its own tracks (WL is tracks 1–2 of the 22.2 file but 17–18 of
/// hrir.wav), and Auro 13.1, which the same slots cover, is written too.
#[test]
fn a_format_file_alone_restores_hrir_hesuvi_and_covered_formats() {
    let temp = Temp::new();
    let nhk = layout("22.2").unwrap();
    let measured = slots(nhk);
    write_format(&temp.0, nhk, &measured);
    let options = RecoveryOptions::default();
    let plan = plan_brir_outputs(&temp.0, &options).unwrap();
    assert_eq!(plan.source_kind, "layout");
    let expected: Vec<_> = SPEAKER_NAMES
        .iter()
        .filter(|s| measured.contains(s))
        .map(|s| s.to_string())
        .collect();
    assert_eq!(plan.speakers, expected);
    let names: Vec<_> = plan
        .planned_files
        .iter()
        .map(|f| {
            Path::new(&f.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["hrir.wav", "hesuvi.wav", "auro_13.1.wav"]);
    let kinds: Vec<_> = plan.planned_files.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(kinds, ["hrir", "hesuvi", "layout"]);
    let result = recover_brir_outputs(&temp.0, &options).unwrap();
    assert_eq!(result.created_files.len(), 3);
    assert_tracks(
        &temp.0.join("hrir.wav"),
        &strings(&HEXADECAGONAL_TRACK_ORDER),
        &measured,
    );
    assert_tracks(
        &temp.0.join("hesuvi.wav"),
        &strings(&HESUVI_TRACK_ORDER),
        &measured,
    );
    let auro = layout("13.1").unwrap();
    let file = temp.0.join(auro.file_name);
    assert_tracks(&file, &auro.track_names(), &measured);
    let labels = auro.label_track_names();
    let refs: Vec<_> = labels.iter().map(String::as_str).collect();
    assert_eq!(
        read_track_names(&file, &refs, refs.len()).unwrap(),
        Some(labels)
    );
}

/// hrir.wav with the community's 21 slots: the missing hesuvi.wav and the
/// complete Auro 13.1 file are planned, the partial formats are not, and the
/// format file is never compacted, even with remove_silent_channels.
#[test]
fn hrir_restores_the_complete_format_files_only() {
    let temp = Temp::new();
    let measured = [
        "FL", "FC", "FR", "SL", "SR", "BL", "BR", "HFL", "HFR", "HFC", "HBL", "HBR", "TFL", "TFR",
        "TSL", "TSR", "TBL", "TBR", "TC", "WL", "WR",
    ];
    let tracks: Vec<_> = HEXADECAGONAL_TRACK_ORDER
        .iter()
        .map(|n| {
            if measured.contains(&n.rsplit_once('-').unwrap().0) {
                track(n)
            } else {
                vec![0.0; COUNT]
            }
        })
        .collect();
    write_wav(&temp.0.join("hrir.wav"), 48000, &tracks, 32).unwrap();
    let options = RecoveryOptions {
        include_hangloose: false,
        remove_silent_channels: true,
    };
    let result = recover_brir_outputs(&temp.0, &options).unwrap();
    let names: Vec<_> = result
        .created_files
        .iter()
        .map(|f| {
            Path::new(f)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["hesuvi.wav", "auro_13.1.wav"]);
    let auro = layout("13.1").unwrap();
    assert_tracks(&temp.0.join(auro.file_name), &auro.track_names(), &measured);
}

/// A format file that disagrees with hrir.wav stops the recovery and names
/// the slot track that differs.
#[test]
fn a_disagreeing_format_file_is_a_source_mismatch_naming_the_slot() {
    let temp = Temp::new();
    let nhk = layout("22.2").unwrap();
    let measured = slots(nhk);
    let path = write_format(&temp.0, nhk, &measured);
    let mut wav = read_wav(&path).unwrap();
    // Track 33 (index 32) is TpBL-left = HBL-left.
    assert_eq!(nhk.track_names()[32], "HBL-left");
    wav.tracks[32][7] += 1.0 / 1024.0;
    let hrir: Vec<_> = HEXADECAGONAL_TRACK_ORDER
        .iter()
        .map(|n| {
            if measured.contains(&n.rsplit_once('-').unwrap().0) {
                track(n)
            } else {
                vec![0.0; COUNT]
            }
        })
        .collect();
    write_wav(&temp.0.join("hrir.wav"), 48000, &hrir, 32).unwrap();
    write_wav(&path, 48000, &wav.tracks, 32).unwrap();
    let error = recover_brir_outputs(&temp.0, &RecoveryOptions::default()).unwrap_err();
    assert_eq!(error.code, RecoveryErrorCode::SourceMismatch);
    assert_eq!(error.details["tracks"], serde_json::json!(["HBL-left"]));
    assert!(!temp.0.join("hesuvi.wav").exists());
}

/// A format file keeps all its tracks; LFE tracks must be silent.
#[test]
fn malformed_format_files_are_rejected() {
    let temp = Temp::new();
    let auro = layout("13.1").unwrap();
    let path = temp.0.join(auro.file_name);
    write_wav(&path, 48000, &vec![vec![0.5; COUNT]; 26], 32).unwrap();
    let error = recover_brir_outputs(&temp.0, &RecoveryOptions::default()).unwrap_err();
    assert_eq!(error.code, RecoveryErrorCode::InvalidChannelCount);
    let mut tracks: Vec<_> = auro.track_names().iter().map(|n| track(n)).collect();
    tracks[6] = vec![0.25; COUNT]; // LFE-left
    write_wav(&path, 48000, &tracks, 32).unwrap();
    let error = recover_brir_outputs(&temp.0, &RecoveryOptions::default()).unwrap_err();
    assert_eq!(error.code, RecoveryErrorCode::NonSilentLfe);
}

/// Hangloose files of the added slots (digits included) are found by slot.
#[test]
fn hangloose_files_of_added_slots_are_recovered() {
    let temp = Temp::new();
    let split = temp.0.join("Hangloose");
    fs::create_dir(&split).unwrap();
    for slot in ["FL", "FR", "SL1", "BL2", "HBC", "DFC", "BCL"] {
        write_wav(
            &split.join(format!("{slot}.wav")),
            48000,
            &[sentinel(slot, 0), sentinel(slot, 1)],
            32,
        )
        .unwrap();
    }
    let result = recover_brir_outputs(&temp.0, &RecoveryOptions::default()).unwrap();
    assert_eq!(result.source_kind, "hangloose");
    assert_eq!(
        result.speakers,
        ["FL", "FR", "SL1", "BL2", "BCL", "HBC", "DFC"]
    );
    let measured = ["FL", "FR", "SL1", "BL2", "HBC", "DFC", "BCL"];
    assert_tracks(
        &temp.0.join("hrir.wav"),
        &strings(&HEXADECAGONAL_TRACK_ORDER),
        &measured,
    );
}
