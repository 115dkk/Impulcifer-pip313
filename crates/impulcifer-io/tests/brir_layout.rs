#![forbid(unsafe_code)]
use impulcifer_io::{
    IoError,
    brir_layout::{append_track_names, read_track_names},
    write_wav,
};
use std::path::PathBuf;
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self(std::env::temp_dir().join(format!(
            "p11-map-{}-{}.wav",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
const NAMES: [&str; 4] = ["FL-left", "FL-right", "FR-left", "FR-right"];
#[test]
fn brir_layout_four_track_round_trip() {
    let t = Temp::new();
    write_wav(&t.0, 48000, &vec![vec![0.0; 8]; 4], 32).unwrap();
    assert_eq!(read_track_names(&t.0, &NAMES, 4).unwrap(), None);
    append_track_names(&t.0, &NAMES).unwrap();
    assert_eq!(read_track_names(&t.0, &NAMES, 4).unwrap().unwrap(), NAMES);
    assert!(matches!(
        read_track_names(&t.0, &NAMES, 3),
        Err(IoError::Format(_))
    ));
    assert!(matches!(
        read_track_names(&t.0, &["UNKNOWN"], 4),
        Err(IoError::Format(_))
    ));
    append_track_names(&t.0, &NAMES).unwrap();
    assert!(matches!(
        read_track_names(&t.0, &NAMES, 4),
        Err(IoError::Format(_))
    ));
}
#[test]
fn brir_layout_odd_json_padding_and_riff_size() {
    let t = Temp::new();
    write_wav(&t.0, 48000, &[vec![0.0; 8]], 32).unwrap();
    append_track_names(&t.0, &["FL-right"]).unwrap();
    let b = std::fs::read(&t.0).unwrap();
    let i = b.windows(4).position(|b| b == b"ICHL").unwrap();
    let size = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
    assert_eq!(size % 2, 1);
    assert_eq!(*b.last().unwrap(), 0);
    assert_eq!(
        u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize,
        b.len() - 8
    );
    assert_eq!(
        read_track_names(&t.0, &NAMES, 1).unwrap().unwrap(),
        ["FL-right"]
    );
}
#[test]
fn brir_layout_reads_python_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/migration/goldens/p11_python_ichl.wav");
    assert_eq!(read_track_names(&path, &NAMES, 4).unwrap().unwrap(), NAMES);
}
#[test]
fn brir_layout_rejects_invalid_json_version_and_duplicate_names() {
    for payload in [
        "{",
        r#"{"version":true,"tracks":[]}"#,
        r#"{"version":2,"tracks":[]}"#,
        r#"{"version":1,"tracks":["FL-left","FL-left"]}"#,
    ] {
        let t = Temp::new();
        let mut b = b"RIFF".to_vec();
        let length = 4 + 8 + payload.len() + payload.len() % 2;
        b.extend((length as u32).to_le_bytes());
        b.extend(b"WAVEICHL");
        b.extend((payload.len() as u32).to_le_bytes());
        b.extend(payload.as_bytes());
        if payload.len() % 2 != 0 {
            b.push(0);
        }
        std::fs::write(&t.0, b).unwrap();
        assert!(matches!(
            read_track_names(&t.0, &NAMES, 2),
            Err(IoError::Format(_))
        ));
    }
}
