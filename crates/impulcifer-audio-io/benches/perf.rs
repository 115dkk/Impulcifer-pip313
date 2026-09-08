#![forbid(unsafe_code)]

#[path = "../tests/bench_support/mod.rs"]
mod bench_support;

fn main() {
    // `cargo test --all-targets` runs benches even with `test = false`; CI runners
    // have neither WASAPI nor the CABLE-A virtual cable, so the bench is a no-op
    // there (the bench code itself is covered by the bench_smoke test).
    if std::env::var_os("CI").is_some() {
        eprintln!("audio bench skipped: CI is set (needs Windows WASAPI and CABLE-A)");
        return;
    }
    if let Err(error) = bench_support::run(impulcifer_audio_io::default_backend().as_ref()) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
