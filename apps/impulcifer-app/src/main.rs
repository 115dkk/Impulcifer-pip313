#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]
// Release builds are GUI-subsystem executables on Windows: no console window
// is attached, so closing a stray terminal cannot take the app down with it.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The desktop executable. The shell itself is the library (`lib.rs`), which
//! Android loads as a shared library instead (ADR 0020).

fn main() {
    impulcifer_app_lib::run();
}
