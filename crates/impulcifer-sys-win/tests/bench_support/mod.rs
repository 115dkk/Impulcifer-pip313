//! PA05 backend-only enumeration; does not open or start streams.
use impulcifer_sys_win::WasapiBackend;
use impulcifer_types::audio::{AudioBackend, AudioError, Endpoint};

pub fn enumerate_devices() -> Result<Vec<Endpoint>, AudioError> {
    WasapiBackend::new().enumerate()
}

#[allow(dead_code)] // Hardware entry point is compiled, never executed by the smoke test.
pub fn run() -> Result<(), String> {
    if !cfg!(windows) {
        return Err("PA05 requires Windows WASAPI".into());
    }
    println!("| op | size | rust median ms | rust min ms |");
    for _ in 0..3 {
        for _ in 0..20 {
            std::hint::black_box(enumerate_devices().map_err(|e| e.to_string())?);
        }
    }
    let mut elapsed = Vec::with_capacity(5);
    for batch in 0..5 {
        let start = std::time::Instant::now();
        for _ in 0..20 {
            std::hint::black_box(enumerate_devices().map_err(|e| e.to_string())?);
        }
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        println!("raw op=enumerate_backend batch={batch} calls=20 wall_ms={ms:.6}");
        elapsed.push(ms);
    }
    elapsed.sort_by(f64::total_cmp);
    println!(
        "| enumerate_backend | 20 calls/batch | {:.6} | {:.6} |",
        elapsed[2], elapsed[0]
    );
    Ok(())
}
