# P28c — Isolate parallel updater test directories

Rust 1.99 Windows CI failed `local_feed_sdk_download_drains_body_before_close`
at `packaging.rs`'s `TemporaryRoot::new`: `create_dir` returned `AlreadyExists`.
The two parallel updater tests combine only PID and wall-clock nanoseconds;
the Windows clock can return the same value to both calls.

Fix the app and service copies of this test helper with a process-local atomic
sequence in addition to PID and time. Preserve exclusive directory creation and
per-test cleanup; never share a colliding directory with `create_dir_all`.
Add concurrent regression coverage that keeps allocated directories alive while
checking uniqueness and cleanup. Change test infrastructure only, with no new
dependencies or updater/runtime semantics changes. Verify on Windows CI.
