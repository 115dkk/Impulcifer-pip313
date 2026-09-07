#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

//! WAV read/write (PCM_32 output like 2.x, multi-track), ffmpeg/ffprobe
//! process management, CSV/TXT helpers. No memory mapping: files are read
//! with bounded std reads.

pub mod ffmpeg;
pub mod wav;
