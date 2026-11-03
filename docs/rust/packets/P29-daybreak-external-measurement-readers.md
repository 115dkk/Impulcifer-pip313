# P29: readers for REW `.mdat` and Dirac Live `.liveproject` room measurements

Repository: `/home/user/Impulcifer-pip313` (Rust workspace at the root). Read first:

- `/home/user/Impulcifer-pip313/CLAUDE.md` (the 3.x section: `#![forbid(unsafe_code)]`, no new dependencies, features gate)
- `/home/user/Impulcifer-pip313/docs/adr/0010-room-measurements-from-rew-and-dirac.md`
- `/home/user/Impulcifer-pip313/docs/rust/EXTERNAL_MEASUREMENTS.md`: the specification. Implement exactly what it says.

Clean-room rule: implement from the specification and the fixture only. Do not search for, download or read
REW or Dirac program code, decompilations, protobuf schemas extracted from their binaries, or third-party
readers of these formats. The specification is complete for this packet; if something seems missing, report it
instead of looking it up elsewhere.

## Scope of this packet

Only the `impulcifer-io` crate. The service, CLI, IPC and UI come in later packets; do not touch them.

Create `crates/impulcifer-io/src/external/` with:

- `mod.rs`: the shared types and `read_external(path: &Path) -> Result<ExternalFile, ExternalError>` (format
  detection by content), plus `read_external_bytes(bytes: &[u8]) -> Result<ExternalFile, ExternalError>` used by
  tests. Expose it from `lib.rs` as `pub mod external;`.
- `java_stream.rs`: a generic Java object serialization stream reader (specification section "Stream"),
  returning the top-level contents as an owned value tree (objects keep their class name and, per class of the
  hierarchy, field name to value; primitive arrays are typed vectors). Back-references resolve to the same value
  (store values in an arena indexed by handle and refer by index; do not clone large arrays per reference).
- `rew.rs`: measurements from the stream (section "Measurements").
- `dirac.rs`: the `QDataStream` header, the protobuf wire format reader, the measurement block search and the
  response matrix decode (section "Dirac Live `.liveproject`").
- `assign.rs`: speaker-name aliases, subwoofer names, the automatic assignment for both formats, and the map
  syntax: `parse_map(file: &ExternalFile, map: &str) -> Result<Vec<Assignment>, ExternalError>` (empty or `auto`
  returns the automatic assignment), `auto_assign(file: &ExternalFile) -> Vec<Assignment>`,
  `format_map(&[Assignment]) -> String`, and `slot_for_label(label: &str) -> Option<&'static str>`.

```rust
pub enum EarSide { Left, Right }
pub struct Assignment { pub slot: &'static str, pub side: EarSide, pub id: String }
```

Assignments are always returned in canonical order (40-slot order from `impulcifer_types::layouts::SLOTS`, left
before right). Use the slot codes from `SLOTS`; do not hard-code a second list of codes.

`ExternalError` is a `thiserror` type with a message; it must never panic on any input. Apply every limit of the
section "Limits for untrusted input".

Numeric details that tests will check: Dirac samples are `f64`, decoded with the formula in the specification
(`256f64.powf(y.abs())`); REW samples are the stored `f32` values widened to `f64` exactly.

## Fixture

`crates/impulcifer-io/tests/fixtures/rew_two_irs_and_fr.mdat` was written by REW 5.40 beta 135 itself. The JSON
next to it holds the 4800 input samples and start times of its two impulse responses (REW stores them padded to
8192 samples); the third measurement, `FR only`, has no impulse response.

There is no Dirac fixture file. Write a small builder in the Dirac tests that produces a project in the layout
the specification describes (version `u32` 10, a few `QString`s of varying length, then `[u32 0][u8 1]`, the
`u32` block length and the protobuf message; append 64 pseudo-random bytes after the block to stand for the
next block).

## Tests

All must exist and pass. Put unit tests in `#[cfg(test)]` modules of the files above and the fixture test in
`crates/impulcifer-io/tests/external_rew.rs`.

1. `rew_fixture_reads_two_impulse_responses_and_skips_the_frequency_response_only_one`: ids `1 2 3`, names
   `FL left`, `FL 오른쪽 🎧`, `FR only`; sample rate 48000; the first two have 8192 samples whose first 4800 equal
   the JSON values exactly and the rest are 0; start times -0.01 and -0.0098 exactly; the third is skipped with
   no samples.
2. Java stream unit tests built from hand-written bytes: a surrogate pair decodes to one character and a
   malformed sequence to U+FFFD; `C0 80` decodes to NUL; a `TC_REFERENCE` to an earlier string and to an earlier
   object resolves; `TC_RESET` clears handles (a reference after it is an error); an `SC_WRITE_METHOD` class's
   annotation (block data and an object) is consumed so the next top-level content parses; an array whose
   length exceeds the remaining bytes is an error without allocating it; nesting beyond the depth limit is an
   error; a truncated stream is an error; a stream whose first content is not the REW header string is
   `not a REW measurement file`.
3. Dirac unit tests with the builder: the block is found after headers of different lengths; the title decodes;
   speakers keep column order and the kind-4 column and a column named `Sub2` are subwoofers; with rows 1 and 3
   measured out of 9, `positions` is `[1, 3]` and the ids are `1@1 2@1 3@1 1@3 2@3 3@3`; codes 32767, -32767,
   0 and 16384 decode to `scale`, `-scale`, `0` and `scale * (256^(16384/32767) - 1) / 255`; -32768 clamps to
   `-scale`; the calibration drops its 0 Hz point; a matrix whose element count is not columns times rows, a
   speaker count different from the columns, and a file without a block are errors.
4. Assignment unit tests: every REW example of the specification; `FR only` (skipped) and a `Sub` title are not
   assigned; a both-ears measurement after a left-only one for the same slot fills only the right side; Dirac
   `Left Right LeftSurround RightSurround Subwoofer Subwoofer` gives FL FR SL SR at position 1, and at position
   2 when position 1 is not measured; two Dirac columns named `Left` assign FL once; `parse_map` accepts
   `fl = 1, FR.right=2` and rejects an unknown id, a skipped id, an unknown slot and a repeated slot and side;
   `format_map` writes `FL=1@1,FR=2@1` for both-sides pairs and `FL.left=1,FL.right=2` otherwise, and
   `parse_map(file, &format_map(&x)) == x`.

## Verification (run in the foreground, from the repository root)

```bash
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt --all -- --check
cargo clippy -p impulcifer-io --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io
```

Disk space is limited: do not run `cargo clean`, and do not build other crates' test binaries. Do not commit.
End your turn only after reporting.

## Report

The files you created, the public API (signatures), the names of all tests, and the final lines of the three
commands' output. Mention anything in the specification you found ambiguous and how you resolved it.
