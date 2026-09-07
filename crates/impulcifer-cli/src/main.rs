#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Process entry point; the implementation is also callable by the Python wheel.
fn main() {
    let argv = std::env::args().collect::<Vec<_>>();
    let code = impulcifer_cli::run(
        &argv,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    );
    std::process::exit(code);
}
