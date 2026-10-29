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
    constants::{
        SPEAKER_NAMES, SWEEP_TRACK_LAYOUTS, Side, sweep_layout_speakers, sweep_track_order,
    },
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

    let speakers = ["SL", "BL", "WL", "TSL"];
    let spec = SweepSpec {
        fs: 8000,
        duration: 0.2,
        speakers: speakers.iter().map(|s| (*s).to_owned()).collect(),
        tracks: "9.1.6".into(),
    };
    let tracks = build_sweep_playback(&spec).unwrap().tracks;
    let active: Vec<_> = tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.iter().any(|x| *x != 0.0))
        .map(|(i, _)| i + 1)
        .collect();
    assert_eq!(active, [5, 7, 9, 13]);
}

/// Choosing a layout fills in every speaker it sweeps, without LFE. Recorded as
/// filled in, each speaker sweeps on a device channel of its own, the file is
/// named after all of them, and processing reads the same speakers back.
#[test]
fn a_layout_fills_in_its_speakers_and_processing_reads_the_recording_name() {
    for layout in SWEEP_TRACK_LAYOUTS {
        let speakers: Vec<String> = sweep_layout_speakers(layout)
            .unwrap()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let spec = SweepSpec {
            fs: 8000,
            duration: 0.2,
            speakers: speakers.clone(),
            tracks: layout.to_owned(),
        };
        // Every device channel but LFE is in the list: none is left unmeasured.
        if let Some(order) = sweep_track_order(layout) {
            let lfe = order.iter().filter(|slot| slot.starts_with("LFE")).count();
            assert_eq!(speakers.len(), order.len() - lfe, "{layout}");
        }
        let playback = build_sweep_playback(&spec).unwrap();
        let swept: Vec<_> = playback.segments.iter().map(|s| &s.speaker).collect();
        assert_eq!(swept, speakers.iter().collect::<Vec<_>>(), "{layout}");
        let active = playback
            .tracks
            .iter()
            .filter(|t| t.iter().any(|x| *x != 0.0))
            .count();
        assert_eq!(active, speakers.len(), "{layout}");
        assert_eq!(
            recording_speakers(&playback.record_filename),
            Some(speakers),
            "{layout}"
        );
    }
    // The speaker count each format's name gives (13.1: 13, 22.2: 22, 24.1.10: 24 + 10).
    let counts: Vec<_> = SWEEP_TRACK_LAYOUTS
        .iter()
        .map(|layout| (*layout, sweep_layout_speakers(layout).unwrap().len()))
        .collect();
    assert_eq!(
        counts,
        [
            ("mono", 1),
            ("stereo", 2),
            ("5.1", 5),
            ("7.1", 7),
            ("7.1.4", 11),
            ("7.1.6", 13),
            ("9.1", 9),
            ("9.1.4", 13),
            ("9.1.6", 15),
            ("13.1", 13),
            ("22.2", 22),
            ("24.1.10", 34),
            ("30.2", 30)
        ]
    );
    let names = |layout| sweep_layout_speakers(layout).unwrap().join(",");
    assert_eq!(names("mono"), "FL");
    assert_eq!(names("stereo"), "FL,FR");
    assert_eq!(names("5.1"), "FL,FR,FC,BL,BR");
    assert_eq!(names("7.1"), "FL,FR,FC,BL,BR,SL,SR");
    assert_eq!(names("7.1.4"), "FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR");
    // 9.1 family: the Dolby order puts the side pair before the rear pair.
    assert_eq!(
        names("9.1.6"),
        "FL,FR,FC,SL,SR,BL,BR,WL,WR,TFL,TFR,TSL,TSR,TBL,TBR"
    );
    assert_eq!(
        names("7.1.6"),
        "FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TSL,TSR,TBL,TBR"
    );
    assert_eq!(sweep_layout_speakers("5.1.2"), None);
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
