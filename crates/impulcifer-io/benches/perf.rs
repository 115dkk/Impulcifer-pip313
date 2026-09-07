//! PA01: whole-call timings, shared temporary fixtures, no benchmark dependencies.
#![forbid(unsafe_code)]

use impulcifer_io::{sweep_files::parse_sweep_file_name, wav};
use std::{
    hint::black_box,
    path::{Path, PathBuf},
    time::Instant,
};

const SWEEP: &str = "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav";
const NAME: &str = "sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav";

pub struct TempDir(pub PathBuf);
impl TempDir {
    pub(crate) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "impulcifer-perf-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn input(channels: usize, frames: usize) -> Vec<Vec<f64>> {
    let mut state = 7u32;
    (0..channels)
        .map(|_| {
            (0..frames)
                .map(|_| {
                    state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                    f64::from(state) / 2147483648.0 - 1.0
                })
                .collect()
        })
        .collect()
}

fn verify(path: &Path, tracks: &[Vec<f64>]) {
    use std::io::Read;
    let mut file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
    let mut checksum = 0u64;
    for track in tracks {
        let mut bytes = vec![0; track.len() * 8];
        file.read_exact(&mut bytes).unwrap();
        for (sample, bytes) in track.iter().zip(bytes.chunks_exact(8)) {
            let expected = f64::from_le_bytes(bytes.try_into().unwrap());
            assert_eq!(sample.to_bits(), expected.to_bits(), "{}", path.display());
            checksum = checksum.wrapping_add(sample.to_bits());
        }
    }
    assert_eq!(file.read(&mut [0]).unwrap(), 0);
    println!(
        "verified {}: bit-exact, checksum={checksum:016x}",
        path.file_name().unwrap().to_string_lossy()
    );
}

pub fn float_fixture(path: &Path, channels: usize, frames: usize) {
    // Tiny smoke only: IEEE binary32 +/-0.5 encoded using integer bits, no Python.
    let size = (channels * frames * 4) as u32;
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend((size + 36).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(3u16.to_le_bytes());
    bytes.extend((channels as u16).to_le_bytes());
    bytes.extend(48000u32.to_le_bytes());
    bytes.extend((48000 * channels as u32 * 4).to_le_bytes());
    bytes.extend((channels as u16 * 4).to_le_bytes());
    bytes.extend(32u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(size.to_le_bytes());
    for i in 0..channels * frames {
        bytes.extend(
            (if i % 2 == 0 {
                0x3f000000u32
            } else {
                0xbf000000u32
            })
            .to_le_bytes(),
        );
    }
    std::fs::write(path, bytes).unwrap();
}

fn measure<T>(op: &str, size: &str, smoke: bool, mut f: impl FnMut() -> T) {
    if smoke {
        black_box(f());
        return;
    }
    for _ in 0..3 {
        black_box(f());
    }
    let mut times = Vec::new();
    for _ in 0..11 {
        let start = Instant::now();
        let result = black_box(f());
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        drop(result); // Allocation included; destruction excluded, in both languages.
    }
    times.sort_by(f64::total_cmp);
    println!("| {op} | {size} | {:.6} | {:.6} |", times[5], times[0]);
}

pub fn run(dir: &Path, smoke: bool) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let n = if smoke { 17 } else { 96000 };
    let short = if smoke { 19 } else { 295000 };
    let a = input(32, n);
    let b = input(30, n);
    let c = input(2, short);
    let pcm = dir.join("pcm32.wav");
    let output = dir.join("rust-output.wav");
    let float = dir.join("float32.wav");
    if smoke {
        wav::write_wav(&pcm, 48000, &a, 32).unwrap();
        float_fixture(&float, 8, 23);
    } else {
        for (name, tracks) in [("input32", &a), ("input30", &b), ("input2", &c)] {
            verify(&dir.join(format!("{name}.f64")), tracks);
        }
    }
    let sweep = if smoke {
        pcm.clone()
    } else {
        root.join("data").join(SWEEP)
    };
    let demo = if smoke {
        pcm.clone()
    } else {
        root.join("data/demo/FL,FR.wav")
    };
    if !smoke {
        for (name, path) in [
            ("pcm32", &pcm),
            ("sweep", &sweep),
            ("demo", &demo),
            ("float32", &float),
        ] {
            let decoded = wav::read_wav(path).unwrap();
            assert_eq!(decoded.sample_rate, 48000);
            verify(&dir.join(format!("{name}-expected.f64")), &decoded.tracks);
        }
        verify(
            &dir.join("roundtrip-expected.f64"),
            &wav::pcm32_round_trip(&b),
        );
    }
    println!("| op | size | rust median ms | rust min ms |");
    for (op, tracks, bits) in [
        ("write_pcm32_32tracks", &a, 32),
        ("write_pcm32_30tracks", &b, 32),
        ("write_pcm16_2tracks", &c, 16),
    ] {
        measure(
            op,
            &format!("{}x{}", tracks.len(), tracks[0].len()),
            smoke,
            || wav::write_wav(&output, 48000, black_box(tracks), bits).unwrap(),
        );
    }
    for (op, path) in [
        ("read_pcm32_32tracks", &pcm),
        ("read_bundled_sweep", &sweep),
        ("read_demo_recording", &demo),
        ("read_float32_8tracks", &float),
    ] {
        let shape = wav::read_wav(path).unwrap();
        let size = format!("{}x{}", shape.tracks.len(), shape.tracks[0].len());
        drop(shape);
        measure(op, &size, smoke, || wav::read_wav(black_box(path)).unwrap());
    }
    measure("pcm32_round_trip", &format!("30x{n}"), smoke, || {
        wav::pcm32_round_trip(black_box(&b))
    });
    let names: Vec<_> = (0..if smoke { 3 } else { 10000 })
        .map(|_| NAME.to_owned())
        .collect();
    measure(
        "sweep_file_name_parse",
        &names.len().to_string(),
        smoke,
        || {
            names
                .iter()
                .map(|name| parse_sweep_file_name(black_box(name)).unwrap())
                .collect::<Vec<_>>()
        },
    );
}

#[allow(dead_code)]
fn main() {
    if let Some(dir) = std::env::var_os("IMPULCIFER_PERF_DIR") {
        run(Path::new(&dir), false);
    } else {
        let dir = TempDir::new();
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/migration/bench_oracle_impulcifer_io.py");
        // Paired runs share the soundfile-written float32 fixture. Without a
        // usable Python (CI runners, plain `cargo bench` elsewhere) fall back to
        // the Rust-written fixture so the bench still runs standalone.
        let python_ok = std::process::Command::new("python")
            .arg(script)
            .arg("--fixture-only")
            .arg(&dir.0)
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !python_ok {
            eprintln!("python fixture unavailable; using the Rust-written float32 fixture");
            float_fixture(&dir.0.join("float32.wav"), 8, 480_000);
        }
        run(&dir.0, false);
    }
}
