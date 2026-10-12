Implemented the A09 wiring in `wt-a09`, without committing, switching branches, stashing, delegating, or editing A08b’s DSP files. **Verification is not fully green:** there are DSP acceptance failures and a missing system dependency, detailed below.

## 1. Fields, options, and defaults

All seven fields are registered in `EXTENSION_FIELD_NAMES`, CLI `EXTENSION_OPTIONS`, policy canonical names, and `features.toml`.

| Field / CLI option | Default | Validation |
|---|---:|---|
| `room_tuning2_mode` | `"mixed"` | `zero`, `mixed`, `linear`, `long`, or 30–200 |
| `room_tuning2_max_boost` | `10` | 0–10 |
| `room_tuning2_low` | `20` | 10–200 |
| `room_tuning2_high` | `"auto"` | `auto` or 100–20000, above low |
| `room_tuning2_mimo` | `false` | Boolean; CLI `true` / `false` |
| `room_tuning2_fsiso` | `150` | 50–150 |
| `room_tuning2_mimo_low` | `"auto"` | `auto` or 15–150 |

- `mode`, `high`, and `mimo_low` accept numeric or string JSON values.
- `room_mode` accepts `eq`, `tuning`, and `tuning2`; `tuning2` accepts `room_range=legacy`.
- CLI extension defaults remain suppressed.
- `Tuning2Mode` and its parameter calculation moved unchanged into `impulcifer-types`; the DSP module re-exports both `Tuning2Mode` and `ModeParams`.
- Bootstrap/default publication and the Python filter already derive from the configuration and extension list, so no separate Python edit was needed.
- Frozen 2.x field/option lists and `oracle_defaults()` were not changed.
- Registry additions include all requested config, room, reference, and performance entries. Both canonical-registration and existing-test-name gates pass. The performance entry follows the existing DSP benchmark-smoke pattern; no new oracle performance audit is claimed.

## 2. Every tuning check extended

In `crates/impulcifer-service/src/brir/inputs.rs`:

1. **Virtual-bass room target:** tuning2 receives the room target even with `room_range=legacy`.
2. **Recording-pair collection:** tuning2 retains per-recording geometry pairs.
3. **Generic room recording:** tuning2 ignores `room.wav` and emits `cli_room_tuning_room_wav_ignored`.
4. **Room-plan dispatch:** tuning2 calls `room_correction_tuning2` on uncropped split IRs, with the prepared target, microphone calibration, symmetric policy, configured level matching, and optional MIMO.
5. **Room-EQ plots:** tuning2 is excluded alongside v1 tuning.

The existing DSP pipeline already has separate tuning2 application/observer wiring. Its virtual-bass room-gain filter is explicitly `room_mode == "eq"`, so it already excludes both tuning modes and required no edit.

## 3. Demo results

### Timing and normalization

Three complete release-CLI runs per configuration:

| Configuration | Wall times | Median | Normalization |
|---|---|---:|---:|
| `eq` | 2.066, 1.956, 1.954 s | **1.956 s** | +1.45 dB |
| tuning2, mixed | 4.011, 3.987, 3.873 s | **3.987 s** | +0.65 dB |
| tuning2, mixed + MIMO | 4.983, 4.682, 4.770 s | **4.770 s** | +1.31 dB |

The mixed normalization difference from eq is 0.80 dB, within the requested 6 dB bound.

Latency tests passed for mixed **8 ms**, zero **0 ms**, linear **28 ms**, and numeric `60` **58 ms**, using the −18 dB direct peak within ±5 samples.

### Thread-count equality

With MIMO enabled and `fsiso=120`, both `RAYON_NUM_THREADS=1` and `4` produced:

```text
efe37960f4656bbec0db86ffbd650dae6e38a5bcc7cdef240677af9c1a8c54db  data/demo/hesuvi.wav
```

### Requested CLI log lines

```text
Virtual room tuning 2 from 20 to 500 Hz; every channel starts 8 ms later
⚠ BL: pre-echo reaches -29.1 dB before the direct sound; a filter with less latency reduces it
BL: 6 other speakers cancel its late bass (MIMO), at -24.0 dB
SL: 6 other speakers cancel its late bass (MIMO), at -15.0 dB
FC: 6 other speakers cancel its late bass (MIMO), at -18.0 dB
FL: 6 other speakers cancel its late bass (MIMO), at -18.0 dB
FR: MIMO would make a bass band worse, so this speaker gets no MIMO
SR: 6 other speakers cancel its late bass (MIMO), at -12.0 dB
BR: 6 other speakers cancel its late bass (MIMO), at -21.0 dB
```

No v1-only log keys appeared. The eq run’s no-tuning-log check passed.

### Generated README section

```markdown
## Virtual room tuning v2

Mode: mixed. Latency: 8.00 ms. Band: 20–500 Hz (default).

| Speaker | Points | Level (dB) | Trim (dB) | Source | PEQ bands | Phase strength | Phase resolution | Long modes (Hz) | Representation RMS (dB) | LF EDT before / after (ms, information) | Median excess GD before / after (ms, information) | Weak |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| BL | 2 | -40.46 | -0.38 | omni | 16 | 0.7 | 1/6 oct | 31.9, 32.4, 47.6, 52.1, 52.7 | 2.71 | 457.63 / 500.25 | 3.14 / 2.16 | false |
| SL | 2 | -40.70 | -0.14 | omni | 16 | 0.7 | 1/6 oct | 35.6, 47.6, 48.0, 64.0 | 2.52 | 479.73 / 377.57 | 4.11 / 2.38 | false |
| FC | 2 | -40.28 | -0.56 | omni | 16 | 0.7 | 1/6 oct | 25.5, 32.1, 47.3, 52.3, 52.4 | 2.71 | 452.47 / 364.40 | 3.49 / 1.67 | false |
| FL | 2 | -40.45 | -0.39 | omni | 16 | 0.7 | 1/6 oct | 31.6 | 2.51 | 461.26 / 379.80 | 3.85 / 1.84 | false |
| FR | 2 | -40.59 | -0.25 | omni | 16 | 0.7 | 1/6 oct | 36.0, 47.6, 47.8, 51.9, 64.0, 64.2 | 2.35 | 492.82 / 505.09 | 3.42 / 1.94 | false |
| SR | 2 | -40.10 | -0.74 | omni | 16 | 0.7 | 1/6 oct | 32.2, 52.7 | 2.20 | 463.65 / 296.84 | 3.76 / 1.62 | false |
| BR | 2 | -40.84 | 0.00 | omni | 16 | 0.7 | 1/6 oct | 47.6, 47.9, 51.4, 51.9, 64.0, 64.2 | 2.71 | 457.11 / 474.62 | 5.09 / 4.88 | false |

Pre-echo uses the first −18 dB direct peak and a 2 ms guard, against the louder of the speaker's two direct sounds. Warn only above −30 dB and at least 6 dB above the same untuned ear. Weak means mean omni/in-ear magnitude RMS exceeds 4 dB (30–300 Hz, level matched at 100–300 Hz); EDT and excess GD are information only.

| Speaker | Ear | Pre-echo untuned / tuned (dB) | Warning |
| --- | --- | --- | --- |
| BL | left | -600.00 / -29.14 | true |
| BL | right | -600.00 / -31.41 | false |
| SL | left | -600.00 / -36.64 | false |
| SL | right | -600.00 / -39.87 | false |
| FC | left | -600.00 / -31.14 | false |
| FC | right | -600.00 / -32.66 | false |
| FL | left | -600.00 / -30.50 | false |
| FL | right | -600.00 / -35.81 | false |
| FR | left | -600.00 / -37.45 | false |
| FR | right | -600.00 / -35.45 | false |
| SR | left | -600.00 / -40.54 | false |
| SR | right | -600.00 / -40.40 | false |
| BR | left | -600.00 / -43.88 | false |
| BR | right | -600.00 / -41.13 | false |

### MIMO supports

| Input | Supports | Support level (dB) | Guard | Band change (dB) | Tail change (dB) |
| --- | --- | --- | --- | --- | --- |
| BL | SL, FC, FL, FR, SR, BR | -24.0 | −12 dB | -0.71 | 0.76 |
| SL | BL, FC, FL, FR, SR, BR | -15.0 | −3 dB | -3.69 | 0.87 |
| FC | BL, SL, FL, FR, SR, BR | -18.0 | −6 dB | -1.75 | 0.77 |
| FL | BL, SL, FC, FR, SR, BR | -18.0 | −6 dB | -2.11 | 0.83 |
| FR | — | -300.0 | off | -99.00 | -99.00 |
| SR | BL, SL, FC, FL, FR, BR | -12.0 | 0 dB | -3.65 | 0.85 |
| BR | BL, SL, FC, FL, FR, SR | -21.0 | −9 dB | -1.39 | 0.94 |

Virtual room tuning v2 reproduces the correction method of SECS 2 by the Speaker Gallery (DCinside), released under GPL-3.0. Impulcifer's implementation is independent and contains no SECS code; SECS 2 was used only as a black box to produce the reference numbers.
```

### Measured late-LF energy change

Service integration test, MIMO versus RC-only, with final normalization undone:

```text
LATE BL -1.417685 dB
LATE SL -4.962912 dB
LATE FC -3.591328 dB
LATE FL -4.008441 dB
LATE SR -8.340699 dB
LATE BR -2.483006 dB
```

Every input with supports improved; FR had no supports.

## 4. Verification results

All requested commands completed in the foreground with the specified test-profile environment.

### Formatting and lint

- `cargo fmt --all -- --check`: **pass**
- `git diff --check`: **pass**
- Workspace clippy: **blocked by missing `gdk-3.0 >= 3.22`**, required by the app’s `gdk-sys`.
- Clippy across types, DSP, CLI, service, and policy, all targets: **pass**

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.52s
```

### Key `test result:` lines

| Suite | Result |
|---|---|
| Types new config test | `test result: ok. 1 passed; 0 failed` |
| CLI existing integration tests | `test result: ok. 12 passed; 0 failed` |
| CLI new tuning2 test | `test result: ok. 1 passed; 0 failed` |
| DSP tuning2 RC | `test result: ok. 7 passed; 0 failed` |
| DSP tuning2 application | `test result: FAILED. 8 passed; 1 failed` |
| DSP MIMO golden | `test result: FAILED. 1 passed; 1 failed` |
| DSP tuning2 MIMO | `test result: ok. 2 passed; 0 failed` |
| DSP TRF | `test result: ok. 1 passed; 0 failed` |
| Policy full suite | `test result: FAILED. 8 passed; 1 failed` |
| Service new tuning2 suite | `test result: FAILED. 5 passed; 1 failed` |
| Service existing room-v2/v1 suite | `test result: ok. 12 passed; 0 failed` |
| Service output suite | `test result: FAILED. 2 passed; 1 failed` |
| Service demo parity | `test result: ok. 2 passed; 0 failed` |
| Service config parity | `test result: ok. 19 passed; 0 failed` |
| Settings | `test result: ok. 4 passed; 0 failed; … 20 filtered out` |
| UI catalog, separately unfiltered | `test result: ok. 4 passed; 0 failed` |
| Canonical registry, targeted | `test result: ok. 1 passed; 0 failed` |
| Existing registry test names, targeted | `test result: ok. 1 passed; 0 failed` |

Other test targets in the requested suites passed, with the existing ignored hardware/network tests unchanged.

The exact settings/catalog command filters the catalog tests out; I additionally ran `--test ui_catalog` without that filter, and all four passed.

### Frontend tail

```text
added 1 package, and audited 2 packages in 2s
found 0 vulnerabilities

> typecheck
> tsc -p tsconfig.json

bridge contract: loading/interactive/complete, positional IPC, envelope identity, once and missing invoke OK
observer contract: frozen Tauri namespace, real envelope identity, startup console/rejection OK
driver contract: boot timeout preserves diagnostics, checkpoint and done; no substitute IPC OK
```

## 5. Final `git status --porcelain`

```text
 M crates/impulcifer-cli/src/lib.rs
 M crates/impulcifer-cli/src/options.rs
 M crates/impulcifer-dsp/src/stages/room_tuning2/mod.rs
 M crates/impulcifer-policy/tests/gates.rs
 M crates/impulcifer-service/src/brir/inputs.rs
 M crates/impulcifer-service/src/brir/outputs.rs
 M crates/impulcifer-service/src/brir/run.rs
 M crates/impulcifer-types/src/config.rs
 M features.toml
?? crates/impulcifer-cli/tests/tuning2.rs
?? crates/impulcifer-service/tests/room_tuning2.rs
?? crates/impulcifer-types/tests/tuning2_config.rs
```

Tracked `data/demo` files were restored after the CLI runs.

## 6. Remaining blockers

These were left intact rather than weakening assertions or editing files owned by A08b:

1. **Default mixed pre-echo acceptance:** BL left reports **−29.144205877 dB**, against the required −30 dB warning threshold. `room_tuning2_runs_on_the_demo` fails on that condition. The remaining demo contracts are checked before that assertion and pass.

2. **Existing DSP MIMO golden failure:**
   ```text
   mimo_mixed_fsiso100_low40_BL/tail[0]:
     got=0.1509470053 expected=0.1359248280
   mimo_mixed_fsiso100_low40_FC/tail[0]:
     got=0.6016054762 expected=0.4603454728
   ```

3. **Existing DSP MIMO reference failure:**
   ```text
   mixed/fsiso150/auto: 4>2
   ```
   Four changed guard steps versus the allowed two.

4. **Existing policy failure:** missing `#![forbid(unsafe_code)]` in:
   - `crates/impulcifer-dsp/tests/room_tuning2_mimo.rs`
   - `crates/impulcifer-dsp/tests/golden_secs2_mimo.rs`
   - `crates/impulcifer-dsp/tests/room_tuning2_apply.rs`

5. **Environment:** full workspace clippy needs the missing GDK development dependency.

6. **Expected root-only failure:** `output_readonly_file_failure_preserves_existing_bytes` fails as anticipated, but it is **not the only failure** in this checkout.

Versions, root README/CHANGELOG, docs, catalogs, apps, workflows, and the 2.x tree were not edited.
