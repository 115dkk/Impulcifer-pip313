# P28b — Windows fixed-size sample decoding

Companion to P28, limited to `impulcifer-sys-win`. Rust 1.99 Clippy's
`chunks_exact_to_as_chunks` requires fixed-size iteration to use typed chunks.
Convert four-byte sample decoding to `as_chunks::<4>()` and array references.
Keep length validation, little-endian decoding, signed-zero detection, packet
ordering and channel-dependent frame iteration identical. Do not change WASAPI
lifecycle, thread ownership or the zero-unsafe budget.

Reuse the existing packet, conversion and zero-run tests. Linux does not compile
the Windows backend; validate it with the Windows toolchain/CI as well.
