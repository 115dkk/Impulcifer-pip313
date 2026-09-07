use crate::test_support::*;
use crate::{pcm32_round_trip, read_wav, write_wav};

fn payload(bytes: &[u8]) -> &[u8] {
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        if &bytes[pos..pos + 4] == b"data" {
            return &bytes[pos + 8..pos + 8 + size];
        }
        pos += 8 + size + (size & 1);
    }
    panic!("missing data")
}
#[test]
fn golden_write_wav_bytes_match_soundfile() {
    let tmp = TempDir::new();
    for bits in [16, 24, 32] {
        let golden = fixture(&format!("write_{bits}"));
        let input = field(&golden, "inputs");
        let output = field(&golden, "outputs");
        let path = tmp.0.join("test.wav");
        write_wav(&path, 48000, &matrix(field(input, "tracks")), bits).unwrap();
        let actual = std::fs::read(&path).unwrap();
        // Python write_wav defaults to tag=PCM even for >2 tracks. The packet
        // mandates WAVEX DIRECTOUT, so compare its unmodified supplemental oracle.
        assert_eq!(
            actual,
            base64(field(output, "wavex_base64")),
            "WAVEX PCM{bits}"
        );
        assert_eq!(
            payload(&actual),
            payload(&base64(field(output, "wav_base64"))),
            "payload PCM{bits}"
        );
        assert_eq!(
            read_wav(&path).unwrap().tracks,
            matrix(field(output, "tracks"))
        );
        for channels in [1, 2] {
            let golden = fixture(&format!("small_{bits}_{channels}"));
            write_wav(
                &path,
                48000,
                &matrix(field(field(&golden, "inputs"), "tracks")),
                bits,
            )
            .unwrap();
            assert_eq!(
                std::fs::read(&path).unwrap(),
                base64(field(field(&golden, "outputs"), "wav_base64"))
            );
        }
    }
}
#[test]
fn golden_read_wav_matches_python() {
    let golden = fixture("read_bundled");
    let input = field(&golden, "inputs");
    let output = field(&golden, "outputs");
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(field(input, "file").as_str().unwrap());
    let wav = read_wav(&path).unwrap();
    assert_eq!(wav.sample_rate, number(field(output, "sample_rate")) as u32);
    assert_eq!(wav.tracks.len(), 1);
    let track = &wav.tracks[0];
    assert_eq!(track.len(), number(field(output, "frames")) as usize);
    assert_eq!(&track[..64], vector(field(output, "first")));
    assert_eq!(&track[track.len() - 64..], vector(field(output, "last")));
    let bytes: Vec<_> = track.iter().flat_map(|n| n.to_le_bytes()).collect();
    assert_eq!(
        sha256(&bytes),
        field(output, "sha256_le_f64").as_str().unwrap()
    );
}
#[test]
fn golden_pcm32_round_trip_matches_python() {
    let golden = fixture("pcm32_round_trip");
    let input = matrix(field(field(&golden, "inputs"), "tracks"));
    assert_eq!(
        pcm32_round_trip(&input),
        matrix(field(field(&golden, "outputs"), "tracks"))
    );
}
#[test]
fn golden_read_riff_rf64_extensible_pcm_and_float() {
    let tmp = TempDir::new();
    let path = tmp.0.join("read.wav");
    for container in ["WAV", "WAVEX", "RF64"] {
        for subtype in ["PCM_16", "PCM_24", "PCM_32", "FLOAT", "DOUBLE"] {
            let golden = fixture(&format!("read_{container}_{subtype}"));
            let raw = base64(field(field(&golden, "inputs"), "wav_base64"));
            std::fs::write(&path, &raw).unwrap();
            let wav = read_wav(&path).unwrap_or_else(|e| panic!("{container}/{subtype}: {e}"));
            let output = field(&golden, "outputs");
            assert_eq!(wav.tracks, matrix(field(output, "tracks")));
            assert_eq!(wav.sample_rate, number(field(output, "sample_rate")) as u32);
            // Every physical truncation must fail; includes truncated post-data chunks.
            for length in 0..raw.len() {
                std::fs::write(&path, &raw[..length]).unwrap();
                assert!(
                    read_wav(&path).is_err(),
                    "{container}/{subtype} length {length}"
                );
            }
        }
    }
}
#[test]
fn read_write_round_trip_32_tracks_extensible_directout_mask() {
    let tmp = TempDir::new();
    let path = tmp.0.join("nested/output.wav");
    for channels in [3, 30, 32, 40] {
        let tracks: Vec<_> = (0..channels)
            .map(|i| vec![i as f64 / 64.0, -0.5, 0.25])
            .collect();
        write_wav(&path, 48000, &tracks, 32).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[20..22], &0xfffeu16.to_le_bytes());
        assert_eq!(&bytes[40..44], &[0; 4]);
        assert_eq!(read_wav(&path).unwrap().tracks, tracks);
        bytes[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(read_wav(&path).unwrap().tracks, tracks);
    }
}
#[test]
fn read_rejects_truncated_header() {
    let tmp = TempDir::new();
    let path = tmp.0.join("bad.wav");
    for bytes in [
        b"RIFF".as_slice(),
        b"RIFF\xff\xff\xff\xffWAVE",
        b"RIFF\x04\0\0\0WAVE",
    ] {
        std::fs::write(&path, bytes).unwrap();
        assert!(read_wav(&path).is_err());
    }
    write_wav(&path, 48000, &[vec![0.0; 3]], 16).unwrap();
    let original = std::fs::read(&path).unwrap();
    for (range, patch) in [
        (40..44, u32::MAX.to_le_bytes().to_vec()),
        (4..8, 4u32.to_le_bytes().to_vec()),
        (32..34, 3u16.to_le_bytes().to_vec()),
        (22..24, 0u16.to_le_bytes().to_vec()),
        (20..22, 42u16.to_le_bytes().to_vec()),
    ] {
        let mut bytes = original.clone();
        bytes[range].copy_from_slice(&patch);
        std::fs::write(&path, bytes).unwrap();
        assert!(read_wav(&path).is_err());
    }
}
fn chunk(id: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(data);
    if !data.len().is_multiple_of(2) {
        bytes.push(0);
    }
    bytes
}
fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(
        &(4u32 + chunks.iter().map(|c| c.len() as u32).sum::<u32>()).to_le_bytes(),
    );
    bytes.extend_from_slice(b"WAVE");
    for c in chunks {
        bytes.extend_from_slice(c);
    }
    bytes
}
#[test]
fn read_handles_padding_reordered_chunks_and_rf64_size_table() {
    let tmp = TempDir::new();
    let path = tmp.0.join("chunks.wav");
    write_wav(&path, 48000, &[vec![0.5]], 24).unwrap();
    let raw = std::fs::read(&path).unwrap();
    let fmt = chunk(b"fmt ", &raw[20..36]);
    let data = chunk(b"data", payload(&raw));
    let bytes = riff(&[
        chunk(b"JUNK", &[1]),
        data.clone(),
        fmt.clone(),
        chunk(b"LIST", &[1, 2, 3]),
    ]);
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(read_wav(&path).unwrap().tracks, [vec![0.5]]);
    // RF64: a non-data sentinel chunk is resolved via the ds64 table, including odd padding.
    let mut ds = Vec::new();
    ds.extend_from_slice(&0u64.to_le_bytes());
    ds.extend_from_slice(&3u64.to_le_bytes());
    ds.extend_from_slice(&1u64.to_le_bytes());
    ds.extend_from_slice(&1u32.to_le_bytes());
    ds.extend_from_slice(b"JUNK");
    ds.extend_from_slice(&1u64.to_le_bytes());
    let mut junk = chunk(b"JUNK", &[1]);
    junk[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut data = data;
    data[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut bytes = riff(&[chunk(b"ds64", &ds), fmt, junk, data]);
    let length = bytes.len() as u64 - 8;
    bytes[..4].copy_from_slice(b"RF64");
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    bytes[20..28].copy_from_slice(&length.to_le_bytes());
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(read_wav(&path).unwrap().tracks, [vec![0.5]]);
    bytes[44..48].copy_from_slice(&u32::MAX.to_le_bytes());
    std::fs::write(&path, bytes).unwrap();
    assert!(read_wav(&path).is_err());
}
#[test]
fn write_validates_before_truncating_and_handles_empty_frames() {
    let tmp = TempDir::new();
    let path = tmp.0.join("output.wav");
    std::fs::write(&path, b"keep").unwrap();
    for (rate, tracks, bits) in [
        (0, vec![vec![0.]], 32),
        (48000, vec![], 32),
        (48000, vec![vec![0.]], 8),
        (48000, vec![vec![f64::NAN]], 32),
        (48000, vec![vec![0.], vec![]], 32),
    ] {
        assert!(write_wav(&path, rate, &tracks, bits).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep");
    }
    write_wav(&path, 48000, &[vec![], vec![], vec![]], 24).unwrap();
    assert_eq!(
        read_wav(&path).unwrap().tracks,
        [Vec::<f64>::new(), vec![], vec![]]
    );
}
#[test]
fn segments_match_exported_python() {
    let golden = fixture("segments");
    let cases = field(field(&golden, "inputs"), "cases").as_array().unwrap();
    let expected = field(field(&golden, "outputs"), "segments")
        .as_array()
        .unwrap();
    for (case, expected) in cases.iter().zip(expected) {
        let case = case.as_array().unwrap();
        let actual =
            crate::sweep_files::infer_sweep_segments(case[0].as_str().unwrap(), number(&case[1]));
        let expected = expected.as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.speaker, field(expected, "speaker").as_str().unwrap());
            assert_eq!(actual.index, number(field(expected, "index")) as usize);
            assert_eq!(actual.total, number(field(expected, "total")) as usize);
            assert_eq!(actual.start, number(field(expected, "start")));
            assert_eq!(actual.end, number(field(expected, "end")));
        }
    }
}
