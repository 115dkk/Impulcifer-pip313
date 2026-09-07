#![allow(dead_code)]
use impulcifer_jobs::registry::{JobRegistry, PollResult};
use impulcifer_service::{ImpulciferService, NoopHost};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// README parity with numeric tolerance. Every non-numeric token must match
/// byte for byte (labels, layout, i18n text); a numeric token followed by a
/// unit is compared with the budget of tests/migration/README-stages.md
/// (dB 0.1, ms 2.0, us exact) because those numbers sit downstream of the
/// minimum-phase EQ FIRs, whose oracle noise moves the PNR by about 0.03 dB;
/// other numbers (formatted with two decimals) get 0.05.
/// `db_tol` / `ms_tol` are per scenario: 0.1 dB / 2 ms when the IR tails are
/// real room noise (default demo), 5 dB / 20 ms for scenarios whose tails are
/// filter roll-off at the 1e-8 level (virtual bass), where the Lundeby noise
/// floor is itself numerical noise while the written WAVs still agree to 5e-8.
pub fn readme_differences(actual: &str, expected: &str, db_tol: f64, ms_tol: f64) -> Vec<String> {
    let a = actual.replace("\r\n", "\n");
    let b = expected.replace("\r\n", "\n");
    let mut out = Vec::new();
    let (al, bl): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    if al.len() != bl.len() {
        out.push(format!("line count {} != {}", al.len(), bl.len()));
    }
    for (n, (x, y)) in al.iter().zip(&bl).enumerate() {
        if x == y {
            continue;
        }
        let xs: Vec<&str> = x.split_whitespace().collect();
        let ys: Vec<&str> = y.split_whitespace().collect();
        if xs.len() != ys.len() {
            out.push(format!("line {}: {x:?} != {y:?}", n + 1));
            continue;
        }
        for (i, (p, q)) in xs.iter().zip(&ys).enumerate() {
            if p == q {
                continue;
            }
            let (Ok(pv), Ok(qv)) = (p.parse::<f64>(), q.parse::<f64>()) else {
                out.push(format!("line {} token {}: {p:?} != {q:?}", n + 1, i + 1));
                continue;
            };
            let unit = xs.get(i + 1).copied().unwrap_or("");
            let tol = match unit {
                "dB" => db_tol,
                "ms" => ms_tol,
                "us" => 0.0,
                _ => 0.05,
            };
            if (pv - qv).abs() > tol {
                out.push(format!(
                    "line {} token {}: {p} vs {q} ({unit}, tol {tol})",
                    n + 1,
                    i + 1
                ));
            }
        }
    }
    out
}

pub struct Temp(pub PathBuf);
impl Temp {
    pub fn new() -> Self {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let p = std::env::temp_dir().join(format!(
                "impulcifer-service-test-{}-{}",
                std::process::id(),
                N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            match std::fs::create_dir(&p) {
                Ok(()) => return Self(p),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => panic!("{e}"),
            }
        }
    }
    pub fn demo() -> Self {
        let t = Self::new();
        for entry in std::fs::read_dir(root().join("data/demo")).unwrap() {
            let p = entry.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy();
            if [
                "hrir.wav",
                "hesuvi.wav",
                "responses.wav",
                "headphone-responses.wav",
                "room-responses.wav",
                "jamesdsp.wav",
            ]
            .contains(&name.as_ref())
                || name.starts_with("truehd_")
            {
                continue;
            }
            if p.is_file()
                && p.extension().is_some_and(|e| {
                    ["wav", "csv", "txt"]
                        .iter()
                        .any(|x| e.eq_ignore_ascii_case(x))
                })
            {
                std::fs::copy(&p, t.0.join(p.file_name().unwrap())).unwrap();
            }
        }
        t
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
pub fn golden(name: &str) -> PathBuf {
    root().join("tests/migration/goldens").join(name)
}
pub fn service(temp: &Temp, jobs: JobRegistry) -> ImpulciferService {
    std::fs::write(
        temp.0.join("settings.json"),
        r#"{"language":"en","language_selected":true}"#,
    )
    .unwrap();
    ImpulciferService::with_dependencies(
        Box::new(NoopHost),
        temp.0.join("settings.json"),
        impulcifer_audio_io::default_backend(),
        jobs,
        root().join("data"),
    )
}
pub fn wait(jobs: &JobRegistry, id: &str) -> PollResult {
    let start = Instant::now();
    loop {
        let p = jobs.poll(id, 0).unwrap();
        if p.job.status.is_terminal() {
            return p;
        }
        assert!(start.elapsed() < Duration::from_secs(240), "job timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}
