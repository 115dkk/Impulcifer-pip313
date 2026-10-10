#![forbid(unsafe_code)]
//! ADR 0007 rule 6: the recorder plays each slot's sweep on the device
//! channel the format's official order gives it, and slot codes with digits
//! (SL1, SL2, BL1, BL2) name recordings like any other code.
use impulcifer_dsp::stages::room::parse_room_measurement_name;
use impulcifer_service::{
    brir::discovery::recording_speakers,
    recording::{
        naming::derive_record_filename,
        sweep::{SweepSpec, build_sweep_playback, validate_sweep_spec},
    },
};
use impulcifer_types::{
    constants::{SPEAKER_NAMES, SWEEP_TRACK_LAYOUTS, Side},
    layouts::{IMMERSIVE_LAYOUTS, Layer},
};

#[test]
fn every_immersive_format_is_a_recorder_layout() {
    for l in IMMERSIVE_LAYOUTS {
        assert!(SWEEP_TRACK_LAYOUTS.contains(&l.id), "{}", l.id);
    }
}

/// Device channel i plays the sweep of the slot that channel i of the format
/// reads, inside that speaker's segment and nowhere else; LFE stays silent.
/// 22.2: FL (±30°) plays on channel 7 (FLc), WL on channel 1 (FL).
#[test]
fn each_slot_sweeps_on_its_official_device_channel() {
    for l in IMMERSIVE_LAYOUTS {
        let speakers: Vec<String> = l
            .channels
            .iter()
            .filter(|c| c.layer != Layer::Lfe)
            .map(|c| c.slot.to_owned())
            .collect();
        let spec = SweepSpec {
            fs: 8000,
            duration: 0.2,
            speakers: speakers.clone(),
            tracks: l.id.to_owned(),
        };
        let playback = build_sweep_playback(&spec).unwrap();
        assert_eq!(playback.tracks.len(), l.channels.len(), "{}", l.id);
        assert_eq!(
            playback.record_filename,
            format!("{}.wav", speakers.join(","))
        );
        let fs = f64::from(playback.estimator.fs);
        for (i, c) in l.channels.iter().enumerate() {
            let track = &playback.tracks[i];
            if c.layer == Layer::Lfe {
                assert!(track.iter().all(|x| *x == 0.0), "{} {}", l.id, c.label);
                continue;
            }
            let segment = playback
                .segments
                .iter()
                .find(|s| s.speaker == c.slot)
                .unwrap();
            let first = track.iter().position(|x| *x != 0.0).unwrap();
            let last = track.iter().rposition(|x| *x != 0.0).unwrap();
            assert!(
                first as f64 >= (segment.start * fs).floor() - 1.0
                    && last as f64 <= (segment.end * fs).ceil() + 1.0,
                "{} {} sweeps outside {}'s segment",
                l.id,
                c.label,
                c.slot
            );
        }
    }
    let spec = SweepSpec {
        fs: 8000,
        duration: 0.2,
        speakers: vec!["FL".into(), "WL".into()],
        tracks: "22.2".into(),
    };
    let tracks = build_sweep_playback(&spec).unwrap().tracks;
    let active: Vec<_> = tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.iter().any(|x| *x != 0.0))
        .map(|(i, _)| i + 1)
        .collect();
    assert_eq!(active, [1, 7]);
}

#[test]
fn a_slot_outside_the_layout_is_refused() {
    let spec = SweepSpec {
        fs: 48000,
        duration: 5.0,
        speakers: vec!["TFL".into()],
        tracks: "22.2".into(),
    };
    let error = validate_sweep_spec(spec).unwrap_err();
    assert!(error.contains("\"22.2\""), "{error}");
}

/// The three file-name parsers (recordings, room recordings, the recorder's
/// channel check) accept the digit codes and still refuse unknown ones.
#[test]
fn digit_slot_codes_name_recordings() {
    let names = |n: &str| recording_speakers(n).map(|v| v.join(","));
    assert_eq!(names("SL1,SR1.wav").as_deref(), Some("SL1,SR1"));
    assert_eq!(names("BL2,BR2.wav").as_deref(), Some("BL2,BR2"));
    assert_eq!(names("HFL,X.wav").as_deref(), Some("HFL,X"));
    for rejected in ["SL3.wav", "FC,XY1.wav", "S1.wav", "sl1.wav"] {
        assert_eq!(names(rejected), None, "{rejected}");
    }
    let room = parse_room_measurement_name("room-SL1,SR1-left.wav").unwrap();
    assert_eq!(room.speakers, ["SL1", "SR1"]);
    assert_eq!(room.side, Some(Side::Left));
    assert!(parse_room_measurement_name("room-SL3-left.wav").is_none());
    assert_eq!(
        derive_record_filename("sweep-seg-SL1,SR1-24.1.10-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"),
        "SL1,SR1.wav"
    );
    for code in SPEAKER_NAMES {
        assert_eq!(
            names(&format!("{code}.wav")).as_deref(),
            Some(code),
            "{code}"
        );
    }
}
