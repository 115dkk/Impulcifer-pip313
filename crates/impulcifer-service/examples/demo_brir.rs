#![forbid(unsafe_code)]
//! PA03 shared driver. All work completes before this process returns.
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::{ImpulciferService, NoopHost};
use impulcifer_types::job::{JobEventKind, JobStatus};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

type Error = Box<dyn std::error::Error>;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub struct DemoCopy(pub PathBuf);
impl DemoCopy {
    pub fn new(source: &Path, fast: bool) -> Result<Self, Error> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let temp = loop {
            let path = std::env::temp_dir().join(format!(
                "impulcifer-pa03-{}-{}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => break Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e.into()),
            }
        };
        // Explicit demo input manifest excludes every generated output. FL,FR is
        // the first canonical speaker file and supplies the required FL reference.
        for name in [
            "FL,FR.wav",
            "FC.wav",
            "BL,SL.wav",
            "SR,BR.wav",
            "headphones.wav",
            "room.wav",
            "room-BL,SL-left.wav",
            "room-BL,SL-right.wav",
            "room-FC-left.wav",
            "room-FC-right.wav",
            "room-FL,FR-left.wav",
            "room-FL,FR-right.wav",
            "room-SR,BR-left.wav",
            "room-SR,BR-right.wav",
            "room-target.csv",
            "room-mic-calibration.csv",
            "room-mic-calibration.txt",
            "eq.csv",
            "eq.txt",
            "eq-left.csv",
            "eq-left.txt",
            "eq-right.csv",
            "eq-right.txt",
        ] {
            if fast && ["FC.wav", "BL,SL.wav", "SR,BR.wav"].contains(&name) {
                continue;
            }
            let path = source.join(name);
            if path.is_file() {
                std::fs::copy(path, temp.0.join(name))?;
            }
        }
        Ok(temp)
    }
}
impl Drop for DemoCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn run(dir: &Path, vbass: bool) -> Result<Value, Error> {
    let jobs = JobRegistry::new();
    let settings = dir.join("pa03-settings.json");
    std::fs::write(&settings, r#"{"language":"en","language_selected":true}"#)?;
    let service = ImpulciferService::with_dependencies(
        Box::new(NoopHost),
        settings,
        impulcifer_audio_io::default_backend(),
        jobs.clone(),
        root().join("data"),
    );
    let signal = root()
        .join("data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
        .canonicalize()?;
    let start = Instant::now();
    let response = service.call(
        "start_brir",
        vec![json!({
            "dir_path":dir, "test_signal":signal, "vbass":vbass, "vbass_freq":250,
        })],
    );
    let id = response["data"]["job"]["job_id"]
        .as_str()
        .ok_or_else(|| format!("start failed: {response}"))?;
    let mut seq = 0;
    let mut boundaries = Vec::new();
    let end_timestamp;
    loop {
        let poll = jobs.poll(id, seq).map_err(|e| format!("poll: {e:?}"))?;
        seq = poll.next_seq;
        for event in &poll.events {
            if event.kind == JobEventKind::Progress {
                boundaries.push((
                    event.payload["key"].as_str().unwrap().to_owned(),
                    poll.timestamps_ms[&event.seq],
                ));
            }
        }
        if poll.job.status.is_terminal() {
            if poll.job.status != JobStatus::Succeeded {
                return Err(format!("job failed: {:?}", poll.job.error).into());
            }
            end_timestamp = poll
                .events
                .last()
                .map(|e| poll.timestamps_ms[&e.seq])
                .unwrap();
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let seconds = start.elapsed().as_secs_f64();
    for name in [
        "hesuvi.wav",
        "hrir.wav",
        "responses.wav",
        "headphone-responses.wav",
        "README.md",
    ] {
        if !dir.join(name).is_file() {
            return Err(format!("missing output: {name}").into());
        }
    }
    let stages: Vec<_> = boundaries
        .iter()
        .enumerate()
        .map(|(i, (key, timestamp))| {
            let end = boundaries.get(i + 1).map_or(end_timestamp, |(_, t)| *t);
            json!({"key":key,"ms":end.saturating_sub(*timestamp)})
        })
        .collect();
    Ok(json!({"seconds":seconds,"stages":stages}))
}

#[allow(dead_code)]
fn main() -> Result<(), Error> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let source = args
        .first()
        .ok_or("usage: demo_brir <dir> [--vbass] [--fast]")?;
    if args.iter().skip(1).any(|a| a != "--vbass" && a != "--fast") {
        return Err("unknown argument".into());
    }
    let dir = DemoCopy::new(Path::new(source), args.iter().any(|a| a == "--fast"))?;
    let result = run(&dir.0, args.iter().any(|a| a == "--vbass"))?;
    println!("PA03_RESULT {result}");
    Ok(())
}
