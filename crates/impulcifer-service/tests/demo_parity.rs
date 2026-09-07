#![forbid(unsafe_code)]
mod brir_support;
use brir_support::*;
use impulcifer_jobs::registry::JobRegistry;
use impulcifer_service::brir::{Catalog, run::run_brir};
use impulcifer_types::{
    config::ProcessingConfig,
    job::{JobKind, JobStatus},
};
use serde_json::{Value, json};
use std::time::Instant;

fn scenario(name: &str) {
    let dir = Temp::demo();
    let oracle: Value =
        serde_json::from_slice(&std::fs::read(golden(&format!("p11_{name}.json"))).unwrap())
            .unwrap();
    let jobs = JobRegistry::new();
    let config = ProcessingConfig {
        dir_path: Some(dir.0.to_string_lossy().into_owned()),
        vbass: name == "vbass",
        ..Default::default()
    };
    let mut catalog = Catalog::english();
    catalog.readme_date = Some(oracle["date"].as_str().unwrap().into());
    let start = Instant::now();
    let job = jobs
        .start(JobKind::Brir, true, move |ctx| {
            let run = run_brir(&config, &catalog, ctx)?;
            Ok(json!({"output_path":run.output_path}))
        })
        .unwrap();
    let poll = wait(&jobs, &job.job_id);
    assert_eq!(
        poll.job.status,
        JobStatus::Succeeded,
        "{:?}",
        poll.job.error
    );
    let seconds = start.elapsed().as_secs_f64();
    println!(
        "P11 {name}: Rust {seconds:.6}s Python {:.6}s ratio {:.6}",
        oracle["python_seconds"].as_f64().unwrap(),
        oracle["python_seconds"].as_f64().unwrap() / seconds
    );
    let keys: Vec<_> = poll
        .events
        .iter()
        .filter(|e| e.kind == impulcifer_types::job::JobEventKind::Progress)
        .map(|e| e.payload["key"].as_str().unwrap())
        .collect();
    println!("P11 {name} progress total={} keys={keys:?}", keys.len());
    let mut failures = Vec::new();
    for filename in ["hesuvi.wav", "hrir.wav"] {
        let product = &oracle["products"][filename];
        let wav = impulcifer_io::read_wav(&dir.0.join(filename)).unwrap();
        assert_eq!(wav.sample_rate, product["fs"].as_u64().unwrap() as u32);
        assert_eq!(
            wav.tracks.len(),
            product["tracks"].as_array().unwrap().len()
        );
        for (track, expected) in wav.tracks.iter().zip(product["tracks"].as_array().unwrap()) {
            let label = expected["name"].as_str().unwrap();
            assert_eq!(track.len(), product["frames"].as_u64().unwrap() as usize);
            let max = track.iter().map(|x| x.abs()).fold(0.0, f64::max);
            let rms = (track.iter().map(|x| x * x).sum::<f64>() / track.len() as f64).sqrt();
            let peak = track
                .iter()
                .enumerate()
                .max_by(|(i, a), (j, b)| a.abs().total_cmp(&b.abs()).then_with(|| j.cmp(i)))
                .unwrap()
                .0;
            let refmax = expected["max_abs"].as_f64().unwrap();
            let refrms = expected["rms"].as_f64().unwrap();
            let mut error = 0.0_f64;
            for (samples, key) in [
                (&track[..256], "first"),
                (&track[track.len() - 256..], "last"),
            ] {
                for (a, b) in samples.iter().zip(expected[key].as_array().unwrap()) {
                    error = error.max((a - b.as_f64().unwrap()).abs());
                }
            }
            if let Some(file) = expected["file"].as_str() {
                let bytes = std::fs::read(golden(file)).unwrap();
                assert_eq!(bytes.len(), track.len() * 8);
                for (a, b) in track.iter().zip(bytes.chunks_exact(8)) {
                    error = error.max((a - f64::from_le_bytes(b.try_into().unwrap())).abs());
                }
            }
            let ratio = |v: f64, r: f64| {
                if r == 0.0 {
                    if v == 0.0 { 1.0 } else { f64::INFINITY }
                } else {
                    v / r
                }
            };
            println!(
                "{name} {filename} {label}: max_error={error:.9e} max_ratio={:.9} rms_ratio={:.9} peak={peak}/{}",
                ratio(max, refmax),
                ratio(rms, refrms),
                expected["peak_index"]
            );
            if error > 1e-3 * refmax
                || (ratio(max, refmax) - 1.0).abs() > 1e-4
                || (ratio(rms, refrms) - 1.0).abs() > 1e-4
                || peak != expected["peak_index"].as_u64().unwrap() as usize
            {
                failures.push(format!("{filename}/{label}"));
            }
        }
    }
    let actual = std::fs::read_to_string(dir.0.join("README.md")).unwrap();
    let expected = std::fs::read_to_string(golden(&format!("p11_{name}_en.txt"))).unwrap();
    let (db_tol, ms_tol) = if name == "vbass" {
        (5.0, 20.0)
    } else {
        (0.1, 2.0)
    };
    let diffs = readme_differences(&actual, &expected, db_tol, ms_tol);
    if !diffs.is_empty() {
        println!("README en differences: {diffs:#?}");
        failures.push("README en".into());
    }
    assert!(failures.is_empty(), "P11 parity failures: {failures:?}");
}
#[test]
fn demo_brir_matches_python_within_budget() {
    scenario("default");
}
#[test]
fn demo_vbass_matches_python_within_budget() {
    scenario("vbass");
}
