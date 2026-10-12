# A08 room tuning 2 measurements

2026-10-12, Linux container, release profile. **Partial implementation, not an acceptance report:** the deterministic guard fixture and end-to-end MIMO variance gate fail. The existing A07 fit-prefix test also fails in this worktree; its source is excluded from A08.

## Timing

`cargo bench -p impulcifer-dsp --bench perf --no-run`, then the resulting binary with `mimo` or `room_tuning2`. RC timings are medians of three runs from the unchanged existing benchmark. MIMO timings are single runs, not statistically stable medians. The MIMO benchmark prepares RC outside the measured section; the application timing includes MIMO and diagnostics but not RC preparation.

| Case | Rust seconds | Reference / interpretation |
| --- | ---: | --- |
| RC zero, FL / two positions | 0.857882 | Regenerated golden has no `secs_seconds` |
| RC mixed | 0.407558 | Regenerated golden has no `secs_seconds` |
| RC linear | 0.936585 | Regenerated golden has no `secs_seconds` |
| RC long | 0.528135 | Regenerated golden has no `secs_seconds` |
| RC ultra-60 | 0.841699 | Regenerated golden has no `secs_seconds` |
| MIMO demo, 7 inputs | 1.080584 | ADR 0008 records about 6 s for SECS ideal-FIR design: roughly 5.6× faster; historical, not a fresh matched run |
| MIMO demo, amortized/input | 0.154369 | Total / 7, parallel inputs; not isolated input latency |
| Whole apply, demo | 1.364873 | RC + supports convolution and diagnostics; preparation excluded |
| Synthetic 24 speakers, one input, R=23, m=4140 | 23.479461 | No matched SECS timing; scaling still needs optimization |

Peak RSS over the combined benchmark process: **1,162,252 KiB** (Python `resource.getrusage(RUSAGE_CHILDREN)`). `/usr/bin/time` is unavailable. Peak includes preparation, the seven-input run and the large synthetic run; it is not an isolated solver-memory measure.

The benchmark's RC uses aligned BRIRs, symmetric positions, and zero trims in the design-only timing. The synthetic set repeats the seven BRIRs with 11-sample increments in delay. All 23 other channels are candidates. SECS's downstream IIR realization, unnecessary for BRIR FIR convolution, reportedly did not finish after 30 minutes / about 1.9 CPU-hours with two workers on this demo. That is not a directly comparable completed runtime and is not included in a speedup claim.

## Numerical method

Build each convolution Gram block with prefix sums along diagonals, starting at the short end, combining the full interval and nine times the early interval. Never subtract two long accumulated sums. Build the right-hand side directly over the late interval. Use a sequential `faer` symmetric eigendecomposition once, then 19 shifted solves, with two residual-refinement passes against the original matrix. Matrix-vector multiplication walks contiguous columns while maintaining ascending accumulation order per output. No dependency features changed; no hand-written SIMD assembly added.

Dense construction / Cholesky cross-check on the oracle FL demo input:

- normal matrix relative L2: 4.367886386622643e-16;
- right-hand side relative L2: 1.2718377008542517e-15;
- kept solution relative L2: 6.564296201359275e-14;
- bisection decisions: 19/19; kept index 15/15.

Across 14 MIMO path fixtures, diagonal/block-row/right-side maximum absolute error was 2.6020852139652106e-18 and relative L2 1.3910722556760364e-15. One ill-conditioned discarded trial's ratio differs by 1.6782690435074165e-9 relative; its decision agrees. Kept-u comparisons pass 1e-9 relative plus 1e-12 component floor. Solver alternatives have not yet been benchmarked head-to-head.

## Acceptance blockers

The specified tail guard unions the before/after late peaks. In `mixed_fsiso100_low40_BL`, at scale 0, the new after peak 44.8 Hz yields 1.006313368383 dB. Independent SciPy `butter`/`sosfilt` reproduces it. The fixture instead records 0.906165519807 dB (scaled by .15), the maximum from the before-only peaks, and accepts scale 0. The implementation follows the written union rule and accepts −3 dB. `FC` in this setting also has a different first tail value (4.010703 vs 3.068970 dB), but both accept −3 dB. Other tested guard values agree to the 1e-9 gate. These two differences remain explicit failures, not relaxed budgets.

End-to-end, ranges and channel gains match; mixed/150 changes four guard steps whereas the worst oracle variant changes two. The test reports all variants alongside Rust for every input. The current A07 RC dependency is not being changed by this packet. No claim is made that its error explains the entire support difference: the requested perturbation study is still missing.

## End-to-end reference table

Ranges match exactly in all settings. Channel trims differ by less than 1e-10 dB. FR is off in both mixed settings, as in the oracle; no linear input is off. Below, tail is the oracle-scaled quantity (0.15 × dB), and Δlevel is Rust minus oracle. These are observations, not accepted error budgets.

| Setting | Input | Δlevel dB | Support relative L2 | Support max absolute | Band dB | Scaled tail |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| linear/150 | FL | 0.000006 | .287468 | .008287 | -.524368 | .136594 |
| linear/150 | FR | 0.000132 | .027359 | .000412 | -1.471571 | .127671 |
| linear/150 | FC | 0.000184 | .062972 | .002853 | -2.299518 | .146009 |
| linear/150 | BL | 0.000185 | .864185 | .040442 | -2.110440 | .098243 |
| linear/150 | SL | 0.000015 | .002836 | .000040 | -.694834 | .138547 |
| linear/150 | SR | 0.000061 | .096630 | .001925 | -1.649012 | .103477 |
| linear/150 | BR | 5.999637 | .999595 | .041566 | -3.627970 | .091802 |
| mixed/100 | FL | -6.000115 | .613369 | .040656 | -1.959224 | .101959 |
| mixed/100 | FR | 0 | 0 | 0 | off | off |
| mixed/100 | FC | -.000051 | .170091 | .005632 | -4.101202 | .073013 |
| mixed/100 | BL | -3.000004 | .649112 | .048744 | -.210916 | .104618 |
| mixed/100 | SL | -3.000014 | .295495 | .009573 | -.032959 | .121658 |
| mixed/100 | SR | -.000036 | .046852 | .002130 | -.295400 | .085050 |
| mixed/100 | BR | .000048 | .047002 | .000813 | -.063088 | .114781 |
| mixed/150 | FL | 3.000158 | .556373 | .023009 | -3.280105 | -.037133 |
| mixed/150 | FR | 0 | 0 | 0 | off | off |
| mixed/150 | FC | .000111 | .110751 | .003372 | -3.069238 | -.070906 |
| mixed/150 | BL | -2.999989 | .481085 | .025302 | -1.842558 | .106686 |
| mixed/150 | SL | -3.000020 | .294028 | .013693 | -1.435263 | .110139 |
| mixed/150 | SR | -.000040 | .035031 | .000730 | -1.399293 | .107097 |
| mixed/150 | BR | 3.000023 | .417810 | .008533 | -.919626 | .134981 |

Changed guard steps / worst variant: linear/150 **1/2**, mixed/100 **3/3**, mixed/150 **4/2 (FAIL)**. Large support differences, notably linear BL even without a step change, require investigation; do not infer equivalence from the step-count gate alone.

## Application demo

Prepared from uncropped omni measurements; applied to aligned, tail-cropped BRIRs. All channels have 16 PEQ bands, no weak/geometry flags. Geometry pairs were not populated in this test, so the absence of mismatch is not a positive geometry validation. Both thread pools (1 and 4 threads) produce identical samples. Every input has six supports in this omni-prepared demo (different RC source from the reference tests).

| Speaker | Level dB | Trim dB | Strength | EDT before / RC / MIMO ms | Excess GD before / RC / MIMO ms | MIMO scale dB | Late energy vs RC dB |
| --- | ---: | ---: | ---: | --- | --- | ---: | ---: |
| FL | -40.889121 | -0.418035 | .7 | 461.256 / 463.549 / 372.769 | 3.854 / 1.991 / 1.798 | -6 | -5.272899 |
| FR | -40.882314 | -0.424842 | .9 | 492.821 / 511.481 / 349.200 | 3.421 / 1.953 / 1.793 | -3 | -6.117236 |
| FC | -40.680217 | -0.626939 | .7 | 452.473 / 438.778 / 300.764 | 3.486 / 1.783 / 1.627 | -3 | -7.919117 |
| BL | -40.811125 | -0.496030 | .7 | 457.634 / 417.386 / 283.639 | 3.140 / 2.196 / 1.943 | -3 | -8.056308 |
| SL | -40.999350 | -0.307806 | .9 | 479.726 / 479.124 / 341.403 | 4.112 / 2.624 / 2.518 | -3 | -6.807418 |
| SR | -40.492690 | -0.814466 | .3 | 463.646 / 448.772 / 314.377 | 3.757 / 2.241 / 1.640 | -3 | -8.196525 |
| BR | -41.307156 | 0 | .9 | 457.112 / 469.150 / 398.477 | 5.091 / 5.236 / 5.289 | -3 | -6.440456 |

Pre-echo per ear is printed by `room_tuning2_runs_on_the_demo_dsp`; worst after levels range from about −29.71 to −39.02 dB. The output support band-limit test currently checks the design response only, not the finite cropped BRIR difference; the requested stronger assertion is unfinished. Zero-mode latency and nonempty geometry-pair cases also need explicit coverage.
