#![forbid(unsafe_code)]
//! Writes the demo inputs of the SECS 2 oracle fixtures (`tests/migration/README-secs2.md`)
//! as headerless little-endian float64 files, exactly as the Rust tests rebuild them:
//! the uncropped split omni ear-position IRs, and the aligned in-ear BRIRs at the point
//! where virtual room tuning runs (after `CropAndAlign`'s alignment, before tuning).
//!
//! cargo run --release -p impulcifer-dsp --example secs2_inputs -- <out dir>
use impulcifer_dsp::{estimator::SweepEstimator, hrir::Hrir, stages::room};
use impulcifer_io::read_wav;
use impulcifer_types::constants::IPSILATERAL_PAIRS;
use std::{fs, path::PathBuf};

const OMNI: [&str; 8] = [
    "room-FL,FR-left.wav",
    "room-FL,FR-right.wav",
    "room-FC-left.wav",
    "room-FC-right.wav",
    "room-BL,SL-left.wav",
    "room-BL,SL-right.wav",
    "room-SR,BR-left.wav",
    "room-SR,BR-right.wav",
];
const BRIR: [(&str, &[&str]); 4] = [
    ("FL,FR.wav", &["FL", "FR"]),
    ("FC.wav", &["FC"]),
    ("BL,SL.wav", &["BL", "SL"]),
    ("SR,BR.wav", &["SR", "BR"]),
];

fn write(path: PathBuf, data: &[f64]) {
    fs::write(
        path,
        data.iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap();
}

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = PathBuf::from(std::env::args().nth(1).expect("output directory"));
    fs::create_dir_all(out.join("omni")).unwrap();
    fs::create_dir_all(out.join("brir")).unwrap();
    let sweep = read_wav(&root.join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")).unwrap();
    let estimator = SweepEstimator::from_samples(sweep.sample_rate, &sweep.tracks[0]).unwrap();
    for file in OMNI {
        let names = room::parse_room_measurement_name(file).unwrap();
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        let mut hrir = Hrir {
            fs: 48000,
            speakers: vec![],
        };
        let speakers: Vec<_> = names.speakers.iter().map(String::as_str).collect();
        hrir.open_recording_samples(
            &estimator,
            wav.sample_rate,
            &wav.tracks,
            &speakers,
            names.side,
            2.0,
        )
        .unwrap();
        for s in &hrir.speakers {
            for (side, ir) in [("left", &s.left), ("right", &s.right)] {
                if let Some(ir) = ir {
                    let stem = file.trim_end_matches(".wav");
                    write(
                        out.join("omni")
                            .join(format!("{stem}__{}__{side}.f64", s.speaker)),
                        &ir.data,
                    );
                }
            }
        }
    }
    let mut hrir = Hrir {
        fs: 48000,
        speakers: vec![],
    };
    for (file, speakers) in BRIR {
        let wav = read_wav(&root.join("data/demo").join(file)).unwrap();
        hrir.open_recording_samples(
            &estimator,
            wav.sample_rate,
            &wav.tracks,
            speakers,
            None,
            2.0,
        )
        .unwrap();
    }
    hrir.crop_heads(1.0).unwrap();
    hrir.align_ipsilateral_all(&IPSILATERAL_PAIRS, 30.0);
    hrir.align_onset_groups_peak_leftref(None).unwrap();
    hrir.crop_tails(&estimator).unwrap();
    for s in &hrir.speakers {
        for (side, ir) in [("left", &s.left), ("right", &s.right)] {
            write(
                out.join("brir").join(format!("{}__{side}.f64", s.speaker)),
                &ir.as_ref().unwrap().data,
            );
        }
    }
}
