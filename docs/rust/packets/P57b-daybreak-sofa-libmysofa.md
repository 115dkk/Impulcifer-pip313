# P57b: make the SOFA writer's files open in libmysofa

Repository: `/home/user/Impulcifer-pip313` (branch `ccr-083fe5e3-isoctv`). Work only inside it. Read first
`CLAUDE.md` (3.x workspace section), `docs/adr/0019-sofa-export.md`, `docs/rust/SOFA.md` ("HDF5 container") and the
packet you implemented, `docs/rust/packets/P57-daybreak-sofa-writer.md`. The writer is
`crates/impulcifer-io/src/sofa.rs` (committed as e976505), its tests `crates/impulcifer-io/tests/sofa.rs`.

Build with the shared target directory, one cargo command at a time:
`export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target CARGO_PROFILE_DEV_DEBUG=0`. Do not delete anything in
that directory and do not create other target directories.

## The problem

The real libmysofa refuses every file the writer produces. It is now installed from the distribution
(`libmysofa-utils` 1.3.2, the same package `brir-3x.yml` uses in CI):

```text
$ mysofa2json -c small.sofa
Error reading file small.sofa. Error code: 10000:invalid format      (exit 16)
$ mysofa2json -c crates/impulcifer-io/tests/fixtures/sofa_netcdf_c_reference.sofa
(exit 0)
```

pyfar `sofar` (`verify=True`) and the Rust `sofar` 0.4.0 port both open the same files, so the Rust port is more
lenient than libmysofa and is not enough as an oracle. The superblock is fine (version 2, end-of-file address equals
the file size); the refusal comes later, from `dataobjectRead` or something it calls. Every `MYSOFA_INVALID_FORMAT`
return in the reader is a candidate.

## Tools you have

- `/usr/bin/mysofa2json` (`mysofa2json <file>` dumps the content as JSON; `-c` also runs libmysofa's AES69 check).
  This is the oracle: `mysofa2json -c <file>` must exit 0.
- The libmysofa 1.3.2 source (BSD-3-Clause), for reading only:
  `/tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/libmysofa-1.3.2/src/`
  (`hdf/superblock.c`, `hdf/dataobject.c`, `hdf/fractalhead.c`, `hdf/btree.c`, `hdf/gcol.c`, `hrtf/reader.c`,
  `hrtf/check.c`). **Do not build, compile or run anything from that directory** and do not install packages: the
  environment refuses running downloaded code. Trace the reader by reading it against the bytes.
- `python3 -I` with the standard library only, for scripts that parse and compare the two files' bytes (superblock,
  object headers and their messages with versions and flags, link info, fractal heap header and direct block,
  B-tree header and leaf, attribute messages, datatypes, dataspaces, layouts, global heap). Put such scripts in
  `/tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/p57b/`, not in the
  repository.
- The pyfar check:
  `/tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/sofa-venv/bin/python -I -c "import sofar,sys; s=sofar.read_sofa(sys.argv[1], verify=True); print(s.Data_IR.shape)" <file>`

To get files to check, add one ignored test to `crates/impulcifer-io/tests/sofa.rs`:
`#[test] #[ignore] fn write_sample_files_for_external_readers()` that writes these four files into the directory
named by the environment variable `SOFA_SAMPLE_DIR` (panic with a clear message when it is unset):

| File | M | N | Notes |
| --- | --- | --- | --- |
| `small.sofa` | 7 | 300 | FL, FR, FC, BL, BR, SL, SR directions at 2 m, right ear shorter than left (padding) |
| `long.sofa` | 7 | 96000 | a realistic BRIR length (2 s at 48 kHz) |
| `forty.sofa` | 40 | 4096 | the most speakers the app can export |
| `one.sofa` | 1 | 1 | the smallest file |

Use distinct, non-trivial sample values per measurement and ear, and the real title and comment strings of
`docs/rust/SOFA.md` (the long brir.sofa comment). Run it with
`SOFA_SAMPLE_DIR=<scratch dir> cargo test -p impulcifer-io --test sofa -- --ignored write_sample_files_for_external_readers`.

## What to do

1. Find the exact check (file, line) in libmysofa that refuses the file and what in our bytes triggers it. Fix the
   writer so the file follows what netCDF-C writes there. Repeat until `mysofa2json -c` exits 0 for all four files.
   There may be more than one cause.
2. Check the content, not only the exit code: from `mysofa2json <file>` (without `-c`), `SourcePosition`,
   `Data.SamplingRate` and the first and last few samples of `Data.IR` for two measurements and both ears must equal
   the input (the JSON prints decimals; compare within 1e-9 relative). The attributes `Conventions`,
   `SOFAConventions`, `DataType`, `RoomType`, `Title` and `Comment` must read back exactly.
3. Keep pyfar's `verify=True` passing for all four files, keep every test in `tests/sofa.rs` green (they may need
   no change), keep the output deterministic, keep `written_file_has_no_compact_link_messages` meaningful.
4. If the file's structure now differs from what `docs/rust/SOFA.md` "HDF5 container" says, report the exact
   difference (I will update the document); do not edit docs yourself.

## Allowed files

`crates/impulcifer-io/src/sofa.rs` (and `src/sofa/**` if you split it), `crates/impulcifer-io/tests/sofa.rs`.
Nothing else in the repository.

## Verification (foreground; report the tail of each)

```bash
cd /home/user/Impulcifer-pip313
export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target CARGO_PROFILE_DEV_DEBUG=0
OUT=/tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/p57b/out
mkdir -p "$OUT"
cargo fmt --all -- --check
cargo clippy -p impulcifer-io --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-io --test sofa
SOFA_SAMPLE_DIR="$OUT" cargo test -p impulcifer-io --test sofa -- --ignored write_sample_files_for_external_readers
for f in "$OUT"/*.sofa; do mysofa2json -c "$f" > /dev/null; echo "$(basename "$f") libmysofa exit $?"; done
for f in "$OUT"/*.sofa; do /tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/sofa-venv/bin/python -I -c "import sofar,sys; s=sofar.read_sofa(sys.argv[1], verify=True); print(sys.argv[1].rsplit('/',1)[-1], s.Data_IR.shape)" "$f"; done
cargo test -p impulcifer-io
cargo test -p impulcifer-policy
```

Run every command in the foreground and end your turn only after reporting: the root cause(s) with the libmysofa
file and line and the offending bytes, what you changed, any difference from `docs/rust/SOFA.md`, the content check
of step 2, and the command outputs. Do not commit. Do not start other agents. Do not use an alias or any other trick
to get past a policy gate or the environment's refusal to run downloaded code.
