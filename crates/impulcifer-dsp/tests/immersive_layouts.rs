#![forbid(unsafe_code)]
//! ADR 0007 behaviour of the BRIR objects: every slot of the immersive
//! formats is opened, aligned and written like the 2.x speakers.
use impulcifer_dsp::{
    estimator::SweepEstimator,
    hrir::{Hrir, SpeakerIrs, ingest_recording},
    ir::ImpulseResponse,
};
use impulcifer_types::constants::{IPSILATERAL_PAIRS, SPEAKER_NAMES};

fn impulse(n: usize, peak: usize) -> ImpulseResponse {
    let mut data = vec![0.0; n];
    data[peak] = 1.0;
    ImpulseResponse {
        data,
        fs: 48000,
        recording: None,
    }
}
fn peak(ir: &ImpulseResponse) -> usize {
    ir.peak_index(0, None, 0.12589)
}

/// A recording named after any slot is opened, not skipped as unknown.
#[test]
fn recordings_of_every_slot_are_opened() {
    let e = SweepEstimator {
        fs: 10,
        low: 1.0,
        high: 5.0,
        n_octaves: 1.0,
        test_signal: vec![1.0; 10],
        duration: 1.0,
        inverse_filter: vec![1.0],
    };
    for speaker in SPEAKER_NAMES {
        let tracks = vec![vec![1.0; 10], vec![2.0; 10]];
        let out = ingest_recording(&e, 10, 10, &tracks, &[speaker], None, 0.0).unwrap();
        assert_eq!(out.len(), 1, "{speaker}");
        assert_eq!(out[0].speaker, speaker);
        assert_eq!(out[0].left.as_ref().unwrap().data, vec![1.0; 10]);
        assert_eq!(out[0].right.as_ref().unwrap().data, vec![2.0; 10]);
    }
}

/// Onset alignment moves every speaker to the FL reference, as an AVR's
/// distance setting would. A slot missing from the default groups would keep
/// its own onset (2.x lists its 15 speakers explicitly).
#[test]
fn onset_alignment_moves_every_slot_to_the_reference() {
    let onset = |speaker: &str| {
        let pair = IPSILATERAL_PAIRS
            .iter()
            .position(|(a, b)| *a == speaker || *b == speaker)
            .unwrap();
        200 + 7 * pair
    };
    let mut h = Hrir {
        fs: 48000,
        speakers: SPEAKER_NAMES
            .iter()
            .map(|s| SpeakerIrs {
                speaker: (*s).into(),
                left: Some(impulse(1024, onset(s))),
                right: Some(impulse(1024, onset(s) + 3)),
            })
            .collect(),
    };
    h.align_onset_groups_peak_leftref(None).unwrap();
    let reference = peak(h.get("FL").unwrap().left.as_ref().unwrap());
    for s in &h.speakers {
        assert_eq!(peak(s.left.as_ref().unwrap()), reference, "{}", s.speaker);
        assert_eq!(
            peak(s.right.as_ref().unwrap()),
            reference + 3,
            "{}",
            s.speaker
        );
    }
}

/// Sentinel per slot and ear: track i of a format file must hold exactly the
/// sentinel of its channel's slot and ear, LFE must be silent.
#[test]
fn layout_tracks_read_each_slot_and_ear_from_the_table() {
    use impulcifer_dsp::pipeline::layout_tracks;
    use impulcifer_types::layouts::{IMMERSIVE_LAYOUTS, Layer};
    let sentinel = |k: usize, ear: usize| (2 * k + ear + 1) as f64;
    let h = Hrir {
        fs: 48000,
        speakers: SPEAKER_NAMES
            .iter()
            .enumerate()
            .map(|(k, s)| SpeakerIrs {
                speaker: (*s).into(),
                left: Some(ImpulseResponse {
                    data: vec![sentinel(k, 0); 8],
                    fs: 48000,
                    recording: None,
                }),
                right: Some(ImpulseResponse {
                    data: vec![sentinel(k, 1); 8],
                    fs: 48000,
                    recording: None,
                }),
            })
            .collect(),
    };
    for l in IMMERSIVE_LAYOUTS {
        let tracks = layout_tracks(&h, &l).unwrap();
        assert_eq!(tracks.len(), 2 * l.channels.len(), "{}", l.id);
        for (i, c) in l.channels.iter().enumerate() {
            for ear in 0..2 {
                let expected = if c.layer == Layer::Lfe {
                    0.0
                } else {
                    sentinel(
                        SPEAKER_NAMES.iter().position(|s| *s == c.slot).unwrap(),
                        ear,
                    )
                };
                assert_eq!(
                    tracks[2 * i + ear],
                    vec![expected; 8],
                    "{} {}",
                    l.id,
                    c.label
                );
            }
        }
    }
}
