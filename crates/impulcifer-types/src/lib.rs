#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! Shared contracts for Impulcifer 3.x: processing configuration, speaker
//! constants, the pywebview-compatible IPC envelope, the job model and the
//! audio backend traits. This crate has no platform or DSP dependencies so
//! every other crate can depend on it.

pub mod audio;
pub mod config;
pub mod constants;
pub mod ipc;
pub mod job;
pub mod stages;
