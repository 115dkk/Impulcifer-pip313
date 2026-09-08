#![forbid(unsafe_code)]

#[path = "../tests/bench_support/mod.rs"]
mod bench_support;

fn main() {
    if let Err(error) = bench_support::run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
