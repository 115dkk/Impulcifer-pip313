# P19 plot oracle and rendering contracts

Run `py -3.14 E:/Impulcifer/tests/migration/export_goldens_plots.py` in the foreground.
The exporter copies canonical inputs to system temporary directories and runs the
actual 2.x pipeline with defaults, then with `plot=True`. It never writes outputs
under `data/`. Temporary image directories are printed and retained for inspection.

## Fixture format

`goldens/p19_plots.json` records every relative PNG path with `size` (encoded,
tight-bbox-cropped width/height) and `canvas` (`fig.get_size_inches() * fig.dpi`,
rounded to integer pixels before saving). Canvas observation also wraps the
spawned panel workers; temporary `.canvas.json` sidecars transfer those values
without replacing Python's process-pool rendering. `Figure.savefig` observation captures the actual
matplotlib `Line2D` frequency/value arrays, legends, titles and axis limits for
results, headphones and EQ. No separate FR reconstruction is used by the exporter.
All grids are native, with `stride=1`. EQ has additional common and split CSV cases
because the canonical demo has no EQ input. The headphone legend stores the
formatted gains; raw curves retain enough precision to recompute exact band means.

`p19_results_left.f64` and `p19_results_right.f64` contain the complete little-endian
f64 summed IR snapshots immediately before the default result plot. These isolate
plot data preparation from the previously measured upstream FIR oracle noise.
`panel_cases` contains two deterministic synthetic IR/recording inputs and the six
actual matplotlib axis limits before the per-axis union. This checks two-pass
synchronization without storing large image fixtures.

## Tests and budgets

- `impulcifer-plots/tests/render.rs` checks exact requested uncropped figure sizes,
  decoded pixel variance >100, at least three colors, all five result-series
  colors, per-axis union, malformed FR input and output-write failure.
- `impulcifer-service/tests/brir_plots.rs` exercises real service writes, PNG sets,
  headphone gains/difference/limits, both EQ variants, result-series preparation,
  byte-identical HeSuVi/README with and without `plot`, and pre-resample snapshots
  plus hook failure propagation.
- Native frequency grids must match exactly. Same-input smoothing retains
  `1e-9 + 1e-11 * abs(reference)` from README-fr.md, both from Python's summed IR
  (`results_from_python_ir_meet_strict_fr_budget`) and directly from Python raw FR
  (`smoothing_from_python_raw_meets_fr_budget`). Pipeline-to-pipeline raw and
  smoothed results, headphones and EQ use 0.05 dB and the left minus right
  smoothed difference 0.1 dB (two 0.05 dB curves; the CI Windows runner measured
  0.052 dB where this machine measured 0.008), per the packet's
  second-run downstream-budget decision and README-stages.md oracle-noise policy.
  The pipeline raw, smoothed and left-minus-right series use 0.05 dB for bins
  within 50 dB of the reference maximum (the difference takes the depth of the
  quieter ear at each bin); deeper bins keep the same linear tolerance as the bin
  at the floor, so their dB budget grows by 10^((depth - 50) / 20). Only the last
  grid bin (23950 Hz, 67.6 dB below the right ear's maximum) is deeper than 50 dB.
  Cross-platform evidence (2026-09-08): Rust versus the Windows golden is at most
  0.0007 dB within 40 dB of the maximum and 0.003 dB within 50 dB on Windows,
  Debian 13 (glibc) and the Ubuntu/Windows runners; at the last bin Debian and
  Ubuntu measured 0.081 dB, the Windows runner 0.091 dB, this machine 0.001 dB,
  macOS passed. The 2.x Python pipeline itself, re-exported on Debian with the same
  NumPy 2.5.3/SciPy 1.18.1, differs from the Windows goldens by 4.2e-4 of the peak
  end to end (`p11_default` products, `p10_default_normalize` onwards; the
  measurement stages agree to 1e-14), so a flat dB budget at a bin 67 dB down would
  test platform rounding, not the port. The Python raw series moves 0.002 dB at
  that bin under a 1e-8 relative perturbation of the summed IR
  (`oracle_noise_plots_raw.py`). Re-exporting the P19 plot golden on Linux is not
  usable as a reference: the exporter's Linux capture is offset by 0.254 dB on
  every bin (a harness artefact of the forked plot worker, not a pipeline
  difference).
  The isolated checks do not inherit the downstream budget. Synthetic synchronized
  limits use 1e-7 in axis units.
- Both PNG set tests compare Rust's encoded dimensions to the captured Python
  canvas exactly. Cropped dimensions remain documentation, not acceptance limits.

Measured maxima on 2026-09-08, in dB unless stated otherwise:

| comparison | left | right | budget |
|---|---:|---:|---|
| Same Python summed IR, raw | 3.623767952377e-13 | 7.247535904753e-13 | 0.05 |
| Same Python summed IR, smoothed | 3.943512183469e-13 | 4.511946372077e-13 | strict P09 |
| Same Python raw FR, smoothed | 3.730349362741e-13 | 4.440892098501e-13 | strict P09 |
| Pipeline raw | 1.690253775324e-2 | 9.932502761174e-4 | 0.05 |
| Pipeline smoothed | 7.480052478478e-3 | 5.692192181925e-4 | 0.05 |
| Headphones raw | 4.277467269276e-12 | 1.290345608140e-11 | 0.05 |

Pipeline smoothed difference: `8.049271696670e-3`; headphone difference:
`8.625988812128e-12`; headphone limits: `7.105427357601e-13`;
EQ common/split raw and error: `4.440892098501e-16`. All use 0.05 dB.
Frequency grids are exact. Synchronized panel limits differ by at most
`7.105427357601e-15` in axis units.

| PNG group | Python cropped | Python canvas = Rust |
|---|---|---|
| `plots/results.png`, default | 1005x785 | 1200x900 |
| `plots/results.png`, plot | 1005x782 | 1200x900 |
| `plots/headphones.png`, both | 1780x938 | 2200x1000 |
| `plots/pre/{speaker}-{side}.png`, 14 files | 1799x938 | 2200x1000 |
| `plots/post/{speaker}-{side}.png`, 14 files | 1799x938 | 2200x1000 |
| `plots/room/{speaker}-{side}.png`, 14 files | 1811x938 | 2200x1000 |
| Overlay BL, FC, SR | 1025x628 | 1200x700 |
| Overlay BR, FL, FR, SL | 1017x628 | 1200x700 |
| Common EQ | 997x782 | 1200x900 |
| Split EQ | 1772x782 | 2200x900 |
| Generic-only `plots/room/room.png` | 1237x782 | 1500x900 |

Default and plot PNG sets contain 2 and 51 files respectively. The JSON is
1,669,150 bytes; the two 268,792-byte IR snapshots bring all P19 fixtures to
2,206,734 bytes, below both fixture budgets.

## Rendering choices

The renderer has no system font lookup. DejaVu Sans 2.37 is embedded with the
unmodified release license. `plotters` uses only bitmap_backend, ab_glyph and
line_series; RGB PNG encoding uses `png`. The workspace already includes
`crates/*`, so no root manifest edit is needed.

Python tight bounding boxes and palette quantization are not reproduced. Text
rasterization, tick placement, subplot margins and integer-pixel line widths
therefore differ from matplotlib. The waterfall uses the permitted 2D heat map,
with the same magnitude samples, log-frequency interpolation, normalization and
3x3 smoothing, the magma palette and a -100..0 dB color scale. Its time axis uses
milliseconds once rather than reproducing Python's extra multiplication in the
3D x limit. Spectrogram uses Python's analytic gnuplot2 palette and a color scale.

The five fixed result colors are preserved even though the optional palette
validator rejects the light-blue chroma and warns about light-curve contrast.
All multi-series plots retain legends.

Specific room plots are also generated when `plot=True`, since the actual demo
oracle includes fourteen `plots/room/*.png` files. Generic measurements now write
`plots/room/room.png`, including the target, individual centered/smoothed gray
measurement curves, aggregate smoothed raw/error, and Python's band-limited y range.
These are plot-only copies; no correction objects are mutated.

The demo does **not** contain the packet's assumed `room.wav`. Both oracle and
`generic_only_room_plot_matches_python_canvas` copy only `FL,FR.wav`,
`headphones.wav`, and `room-FL,FR-left.wav` (renamed to `room.wav`) into a temporary
directory. This uses an unchanged measured room recording and exercises the
actual generic-only branch, without adding a file under `data/`. The substitution
is recorded in `generic_room.source` in the fixture.

Bokeh interactive/additional HTML and microphone-deviation debug figures remain
outside this PNG implementation. Only explicit interactive/debug requests warn;
`plot=True` alone no longer incorrectly warns about microphone debug figures.

## Dependency and font provenance

Resolved renderer dependencies are plotters/plotters-backend/plotters-bitmap 0.3.7
(MIT), png 0.18.1 (MIT OR Apache-2.0), ab_glyph 0.2.32 and owned_ttf_parser 0.25.1
(Apache-2.0), ttf-parser 0.25.1, thiserror 2.0.20 and service rayon 1.12.0
(MIT OR Apache-2.0). No dependency was added during this continuation.
The existing magma RGB8 palette is CC0, with its source notice in the assets.

DejaVu's [official downloads](https://dejavu-fonts.github.io/Download.html) and
[license](https://dejavu-fonts.github.io/License.html) were verified with WebFetch.
The GitHub release mirror archive `dejavu-fonts-ttf-2.37.zip` matches the official
SHA256 `7576310b219e04159d35ff61dd4a4ec4cdba4f35c00e002a136f00e96a908b0a`.
Both embedded files were byte-compared against that archive:

- `DejaVuSans.ttf`: 757,076 bytes, SHA256
  `7da195a74c55bef988d0d48f9508bd5d849425c1770dba5d7bfc6ce9ed848954`.
- `DejaVu-LICENSE.txt`: 8,816 bytes, SHA256
  `7a083b136e64d064794c3419751e5c7dd10d2f64c108fe5ba161eae5e5958a93`.

The font is unmodified and below 800 KB. Bitstream/Arev redistribution notices
are retained; DejaVu changes are public domain. There is no system font lookup.

## Verification and remaining integration

All commands ran in the foreground. Exporter, workspace fmt (after correcting one
new call's formatting), requested clippy, plots tests (4 passed), service tests
(83 passed, 1 hardware test ignored), Ruff, diff check, PA03 orchestrator and
standalone cargo benchmark completed successfully. The full service suite includes
11 P19 tests and both unchanged demo parity tests. Visual inspection compared real
Python/Rust results, headphones, post panels, generic room and overlay output;
labels, legends and curves are present. The waterfall and font/margin differences
above remain intentional. The optional palette validator was not rerun; its
previous fixed-palette caveats are retained above.

Policy completed with exit 101: 5 passed, 1 failed. The renamed test now is
`optional_dsp_stages_and_png_plots_complete_with_only_unsupported_plot_warnings`.
Five existing registry references still use the old name, under `output.hrir_wav`,
`output.hesuvi_wav`, `output.hangloose_wavs`, `output.jamesdsp_wav`, and
`output.truehd_layout_wavs`. Parent integration must replace those references in
`features.toml`, which this packet prohibits editing. No compatibility dummy test
was added to conceal the stale registry references. Thus overall acceptance is
still pending that registry update and an independent policy rerun.

Full stdout/stderr logs are in
`C:/Users/32170336/AppData/Local/Temp/impulcifer-p19-final-_jdz_tot/`:
`export.log`, `fmt.log` (initial failure), `fmt-final.log`, `clippy.log`,
`plots-tests.log`, `service-tests.log`, `policy-tests.log`, `ruff.log`,
`diff-check.log`, `plots-service-focused.log`, `perf-oracle.log`, `perf-rust.log`.
The metadata summary's first read failed on Windows' default cp949 decoding;
reading the unchanged metadata as UTF-8 succeeded. No build/test gate depended
on that failed display command.
Python PNG evidence: `C:/Users/32170336/AppData/Local/Temp/impulcifer-p19-oracle-hcwuzent/`.
Rust inspected evidence: `C:/Users/32170336/AppData/Local/Temp/impulcifer-service-test-325184-2/`
and generic room `C:/Users/32170336/AppData/Local/Temp/impulcifer-service-test-325184-0/`.
Performance results are appended to `docs/rust/perf/pipeline-demo.md`.
