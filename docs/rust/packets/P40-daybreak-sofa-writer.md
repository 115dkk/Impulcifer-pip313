# P40: a pure-Rust SOFA (SimpleFreeFieldHRIR) writer that libmysofa can open

Repository: `/home/user/Impulcifer-pip313` (branch `ccr-083fe5e3-isoctv`). Work only inside it. Read first:
`CLAUDE.md` (3.x workspace section: `#![forbid(unsafe_code)]`, `features.toml`), `docs/adr/0019-sofa-export.md`,
and `docs/rust/SOFA.md` (the specification; sections "Convention" and "HDF5 container" are what you build).

Build with the shared target directory so the disk does not fill up, one cargo command at a time:
`export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target CARGO_PROFILE_DEV_DEBUG=0`. Do not delete anything in
that directory. The disk is small (about 6 GB free): do not create other target directories.

## Why this is hand-written

SOFA is netCDF-4, i.e. HDF5. libmysofa (used by most SOFA players) reads a group's children only from dense link
storage (Link Info message -> fractal heap of link messages, plus a v2 B-tree name index) and rejects a file that
contains a Link message (type 6). The pure-Rust `netcdf-writer`/`hdf5-writer` 0.13.0 write compact links (Link
messages in the object header), so their files do not open in libmysofa. netCDF-C writes dense links. Do not add
`netcdf-writer`, `hdf5-writer`, `netcdf`, `hdf5*` or any other HDF5/netCDF crate as a normal dependency.

## What to build

A new module `crates/impulcifer-io/src/sofa.rs` (submodules under `src/sofa/` if you want), exported from `lib.rs`:

```rust
pub struct SofaMeasurement {
    /// Degrees, SOFA/ITU convention (azimuth positive to the left), distance in metres.
    pub azimuth: f64,
    pub elevation: f64,
    pub distance: f64,
    pub left: Vec<f64>,
    pub right: Vec<f64>,
}

pub struct SofaMetadata {
    pub title: String,
    pub comment: String,
    /// The application/API version string, e.g. "3.21.0".
    pub version: String,
    /// "YYYY-MM-DD HH:MM:SS" (UTC); used for DateCreated and DateModified.
    pub date: String,
}

/// Bytes of a SimpleFreeFieldHRIR 1.0 file (SOFA 2.1) with the attributes and variables of docs/rust/SOFA.md.
/// Every measurement is padded with zeros to the longest one. Errors: no measurements, fs == 0, a non-finite value.
pub fn encode_simple_free_field_hrir(
    fs: u32,
    measurements: &[SofaMeasurement],
    meta: &SofaMetadata,
) -> Result<Vec<u8>, IoError>;

pub fn write_simple_free_field_hrir(
    path: &Path,
    fs: u32,
    measurements: &[SofaMeasurement],
    meta: &SofaMetadata,
) -> Result<(), IoError>;

/// "YYYY-MM-DD HH:MM:SS" for the given Unix seconds (UTC, proleptic Gregorian; no new dependency).
pub fn sofa_date(unix_seconds: u64) -> String;
```

The fixed global attributes (Conventions, Version, SOFAConventions, ... ListenerShortName) and the fixed variables
(ListenerPosition, ListenerUp, ListenerView, ReceiverPosition, EmitterPosition, Data.Delay) are written by the
encoder with the values in `docs/rust/SOFA.md`; `title`, `comment`, `version`, `date` fill the rest.

HDF5 layout: follow `docs/rust/SOFA.md` "HDF5 container". The fixture
`crates/impulcifer-io/tests/fixtures/sofa_netcdf_c_reference.sofa` was written by netCDF-C (via pyfar `sofar`) and
opens in every reader; study its bytes (superblock version, root group header, fractal heap and B-tree parameters,
dimension datasets, DIMENSION_LIST/REFERENCE_LIST, global heap, attribute datatypes). References you may read:
- HDF5 file format specification (public): https://support.hdfgroup.org/documentation/hdf5/latest/_f_m_t3.html
- libmysofa's reader, BSD-3-Clause: https://github.com/hoene/libmysofa/tree/main/src/hdf (what it accepts is the
  constraint; e.g. `dataobject.c` lists the object header message types it reads and fails on any other).
- The Rust port `sofar` 0.4.0 (MIT/Apache) in `~/.cargo/registry` after you add it as a dev-dependency.
You may write the Jenkins lookup3 checksum yourself (a few dozen lines; it is public domain). Keep the module
`#![forbid(unsafe_code)]`-clean (the crate root already forbids unsafe). No new normal dependencies.

Output must be deterministic: same input, same bytes (fixed object order, no timestamps, no random values).

## Tests (`crates/impulcifer-io/tests/sofa.rs`)

Add `sofar = { version = "=0.4.0", default-features = false }` under `[dev-dependencies]` of `impulcifer-io`
(enable a feature only if the reader needs it; say which). Use `OpenOptions::new().normalized(false)` and the file's
own sample rate so the reader neither normalizes nor resamples.

- `netcdf_c_reference_fixture_opens_in_sofar`: the fixture opens (sanity check of the oracle).
- `written_file_opens_in_sofar_with_the_right_shape`: 7 measurements (FL, FR, FC, BL, BR, SL, SR directions, 2 m),
  N = 300, distinct values per measurement and ear; `sofar` opens it, `num_measurements() == 7`, `filter_len() == 300`,
  `sample_rate() == 48000`; the source positions and the samples it reports equal the input (f32 tolerance if the
  reader converts to f32; find where it exposes positions and raw IRs).
- `written_file_is_deterministic`: encoding twice gives identical bytes.
- `measurements_are_padded_and_inputs_validated`: unequal lengths are zero-padded; empty input, fs 0 and NaN are errors.
- `sofa_date_formats_utc`: e.g. 0 -> "1970-01-01 00:00:00", 1763078400 -> "2025-11-14 00:00:00", a leap day.
- `written_file_has_no_compact_link_messages`: walk the root group's object header bytes you wrote and assert there is
  no message of type 6 (guards the libmysofa constraint even if the reader changes).

Optional local check (strict convention verification through netCDF-C/libhdf5): a Python venv with pyfar `sofar`
1.3.0 already exists. Write a file with a tiny test or example, then run
`/tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/sofa-venv/bin/python -I -c "import sofar,sys; s=sofar.read_sofa(sys.argv[1], verify=True); print(s.Data_IR.shape)" <file>`
and report the output. Do not install anything else.

## features.toml

Add `io.sofa_writer` (`kind = "io"`, `status = "implemented"`, tests = the sofar-open test and the determinism test).
Check how other `io.*` entries name tests (`impulcifer-io::test_name`).

## Allowed files

`crates/impulcifer-io/src/lib.rs` (module declaration and re-exports only), `crates/impulcifer-io/src/sofa.rs` and
`crates/impulcifer-io/src/sofa/**`, `crates/impulcifer-io/tests/sofa.rs`, `crates/impulcifer-io/Cargo.toml`
(the dev-dependency only), `Cargo.lock` (by cargo only), `features.toml` (the one entry). Do not touch other crates,
`apps/`, `.github/`, docs, or the fixture.

## Verification (foreground; report the tail of each)

```bash
cd /home/user/Impulcifer-pip313
export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all -- --check
cargo clippy -p impulcifer-io --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io --test sofa
cargo test -p impulcifer-io
cargo test -p impulcifer-policy
```

Run every command in the foreground and end your turn only after reporting: the files changed, the HDF5 structures
you write (superblock version, how links/attributes/dimension scales are stored, message versions), the test names,
the pyfar check output if you ran it, and the command outputs. Do not commit. Do not start other agents. Do not use
an alias or any other trick to get past a policy gate. If libmysofa's constraints force a choice this packet does not
cover, make the smallest one that keeps every test above meaningful and say what it was.
