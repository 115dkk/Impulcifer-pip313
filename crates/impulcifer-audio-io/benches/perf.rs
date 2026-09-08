#![forbid(unsafe_code)]

#[path = "../tests/bench_support/mod.rs"]
mod bench_support;

fn main() {
    if let Err(error) = bench_support::run(impulcifer_audio_io::default_backend().as_ref()) {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
