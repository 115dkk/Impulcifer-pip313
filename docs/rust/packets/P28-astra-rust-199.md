# P28 — Rust 1.99 / edition 2024 modernization

Read `CLAUDE.md` first. Preserve all public behavior, error text, IPC contracts,
floating-point operation order and BRIR bytes. Do not read SECS sources or its
golden-export script. Keep unsafe forbidden.

Implement meaningful modernization in first-party Rust crates (except
`impulcifer-sys-win`). Inspect the official Rust 1.98/1.99 release notes and
the installed 1.99 compiler before choosing APIs. Prioritize owned lossy UTF-8
conversion (`String::from_utf8_lossy_owned`, 1.99), fixed-size slice windows
(`array_windows`) and edition-2024 let chains where they simplify real code.
Avoid cosmetic churn, speculative fast math, unnecessary dependencies and
changes to borrowed-only formatting paths. Add focused regression coverage for
changed parsing/decoding edge cases; reuse numerical golden tests.

The coordinating agent owns workspace manifests, toolchain, version/changelog,
documentation, dependency resolution, full Clippy/tests and old/new CLI SHA-256
comparison. Report modified files, rationale, verification and remaining risks.

Reference: [Rust 1.99 release notes (including 1.98)](https://github.com/rust-lang/rust/blob/1.99.0/RELEASES.md)
and [edition 2024 resolver 3](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html).
