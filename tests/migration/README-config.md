# P25: every ProcessingConfig field against 2.x

The 33 `config.*` entries of `features.toml` are the fields of
`core/pipeline.py::ProcessingConfig`. Their names, types, defaults, help text
and argparse parsing were already checked against 2.x (`p13_*` goldens:
`golden_cli_options_match_python`, and `golden_cli_parsing_matches_python`,
whose every-flag argv sets all 33). What a value does was covered only for
some of them: the p10/p11 demo goldens run the defaults, virtual bass at its
defaults, six room/target variants (`golden_option_variants_match_python`),
the stage table and the optional outputs. This packet runs every other field
that changes the written BRIR with a non-default value through the 3.x
service and compares the result with the real 2.x pipeline.

## Oracle

`export_goldens_config.py` (2.x environment: CPython 3.13.12, NumPy 2.5.3,
SciPy 1.18.1) copies `data/demo` to a temporary folder per scenario, applies
the scenario's setup, calls `impulcifer.main(dir_path=..., test_signal=<the
bundled 6.15 s sweep>, **config)` in a child process and records `hesuvi.wav`
and `hrir.wav`: per track the SHA-256, max-abs, RMS and absolute-peak index,
and for `hesuvi.wav` the first 512 and last 256 samples
(`p25_config_<scenario>.f64`). `p25_config_scenarios.json` holds the scenario
list with its setup, so `crates/impulcifer-service/tests/config_parity.rs`
runs exactly the same thing: files removed from the folder, moved out of it
or written next to it (`{outside}`), and the generic `room.wav` built like
the p10 generic-room fixture (first track of `FL,FR.wav` plus a copy rolled by
17 samples, PCM_32).

| scenario | config | what it exercises |
|---|---|---|
| `head_ms` | `head_ms` 2.5 | crop head length (CLI `--c`) |
| `bass_boost_shelf` | gain 6, fc 150, q 0.69 | the full bass shelf, not only the gain |
| `vbass_crossover` | vbass, `vbass_freq` 200, `vbass_hp` 20 | crossover and high-pass |
| `vbass_normal`, `vbass_invert` | vbass, `vbass_polarity` | both forced polarities (auto is p11 `vbass`) |
| `mic_deviation` | mic correction, no headphone compensation | the stage end to end |
| `mic_deviation_strength` | the same with strength 0.3 | strength reaches the filters |
| `no_room_correction`, `no_headphone_compensation`, `no_equalization` | one `do_*` off each | the three store_false flags on real data |
| `decay` | `{"FL": 0.3, "FR": 0.25}` | per-channel decay map end to end |
| `channel_balance` | `trend` | balance end to end (all seven methods are p10 stage goldens) |
| `room_files` | `room_target`, `room_mic_calibration` outside the folder | explicit files win over the demo's own |
| `room_target_missing` | `room_target` that does not exist | 2.x uses a flat target (`_open_room_target`) |
| `headphone_file` | `headphone_compensation_file` outside the folder, folder has none | the requested path is the only way to succeed |
| `specific_unlimited` | `specific_limit` 0 | specific room correction without the 400 Hz limit |
| `generic_conservative` | `fr_combination_method` conservative, generic `room.wav` only | generic combination |
| `generic_unlimited` | `generic_limit` 0, generic `room.wav` only | generic limit |

Every pair of scenarios that should differ does so by far more than the
budget (for example `vbass_normal` against `vbass_invert` 0.32 of the peak,
`mic_deviation` against `mic_deviation_strength` 0.28, the flat target against
the demo's target 0.84), so a field that 3.x ignored would fail its scenario.

## Directory order

2.x takes speakers and room measurements in `os.listdir` order, and that
order reaches the output. The normalization gain is the peak of the summed
magnitude response, which sits where the minimum-phase EQ FIRs are
noise-limited (README-fr.md), so the default demo run on Linux (directory
hash order) comes out 0.8 % (0.07 dB) louder than on Windows (sorted order),
in 2.14.2 from PyPI and in this tree alike. The p10/p11 goldens were exported
on Windows and 3.x sorts its listing, so the exporter pins `os.listdir` to the
sorted order. With the Linux order the same exporter reproduces the 0.8 %
difference; that is a 2.x platform dependence, not a 3.x one.

## Budget

`config_parity.rs` uses the demo_parity rule: per track max |a - b| at most
1e-3 of the reference peak over the stored samples, max-abs and RMS ratios
within 1e-4, the same absolute-peak index; `hrir.wav` the same without the
sample windows.

Without headphone compensation the EQ FIR carries only the room correction,
and there 1e-4 is inside the oracle's own noise. `oracle_noise_config.py`
reruns a scenario in 2.x with every linear-phase FIR perturbed by 1e-12 of its
peak before `minimum_phase` (the size of the Rust/Python `firwin2` difference,
README-fr.md), ten trials each:

| scenario | 2.x against itself, max-abs ratio | RMS ratio | 3.x against 2.x, max-abs ratio | RMS ratio |
|---|---|---|---|---|
| `no_headphone_compensation` | up to 2.2e-4 | up to 1.0e-4 | 2.7e-4 | 1.1e-4 |
| `mic_deviation` | up to 5.4e-4 | up to 1.6e-4 | 2.7e-4 | 9.4e-5 |
| `mic_deviation_strength` | up to 5.6e-4 | up to 1.6e-4 | 2.6e-4 | 9.5e-5 |

The equalization curves themselves agree to 1.6e-10 dB in these runs; the
difference is born in the minimum-phase step, where a 1.6e-10 dB change of
the curve already moves the 2.x FIR by 2.2e-4 dB. Scenarios with
`do_headphone_compensation` false therefore use 5e-4 for both ratios. With
headphone compensation the 2.x noise is of the same size (up to 2.5e-4 for
`head_ms` in three trials) but 3.x stays within 1e-4, so every other scenario
keeps 1e-4.

## Results (3.x release build, this branch)

| scenario | largest sample error / track peak | max-abs ratio | RMS ratio |
|---|---|---|---|
| `head_ms` | 4.5e-5 | 2.9e-5 | 1.4e-5 |
| `bass_boost_shelf` | 5.7e-5 | 2.8e-5 | 1.3e-5 |
| `vbass_crossover` | 4.1e-5 | 2.8e-5 | 8.6e-6 |
| `vbass_normal` | 3.9e-5 | 2.7e-5 | 6.3e-6 |
| `vbass_invert` | 4.0e-5 | 2.7e-5 | 6.4e-6 |
| `mic_deviation` | 4.0e-4 | 2.7e-4 | 9.4e-5 |
| `mic_deviation_strength` | 4.6e-4 | 2.6e-4 | 9.5e-5 |
| `no_room_correction` | 1.9e-5 | 9.8e-6 | 1.7e-6 |
| `no_headphone_compensation` | 5.3e-4 | 2.7e-4 | 1.1e-4 |
| `no_equalization` | 4.4e-5 | 2.8e-5 | 1.4e-5 |
| `decay` | 4.4e-5 | 2.8e-5 | 1.4e-5 |
| `channel_balance` | 4.2e-5 | 2.7e-5 | 1.2e-5 |
| `room_files` | 3.9e-5 | 1.4e-5 | 4.5e-6 |
| `room_target_missing` | 4.5e-5 | 2.0e-5 | 4.0e-6 |
| `headphone_file` | 4.4e-5 | 2.8e-5 | 1.4e-5 |
| `specific_unlimited` | 5.4e-5 | 3.6e-5 | 1.6e-5 |
| `generic_conservative` | 6.3e-5 | 3.3e-5 | 1.5e-5 |
| `generic_unlimited` | 5.8e-5 | 2.3e-5 | 1.1e-5 |

Every absolute-peak index matches. `room_target_missing` failed on 3.0.0: the
service read the requested path unconditionally and stopped with
FILE_NOT_FOUND where 2.x falls back to a flat target. 3.0.1 reads the target
only when the path is a file, like `_open_room_target`.

## What is not covered

`mic_deviation_debug_plots` stays `planned`: 3.x accepts it on every surface
but writes no plots and logs `cli_plots_not_available_yet`, while 2.x saves
the mic-deviation analysis plots under `plots/`.

## Running

```text
python tests/migration/export_goldens_config.py
python tests/migration/oracle_noise_config.py no_headphone_compensation --trials 10
cargo test -p impulcifer-service --test config_parity -- --nocapture
```

The exporter rewrites only `p25_config_*` files, never `data/demo`, and stays
under 2 MB (1.6 MB now). In a debug build the 18 scenarios take about four
minutes on four cores; in release about eight seconds.
