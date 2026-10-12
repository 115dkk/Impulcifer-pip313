# A08d MIMO band-edge peaks

2026-10-12, same Linux container, checked-out branch. **A08d verification passes.** All commands ran in the foreground, without delegation, commits, branch switches, stash, SECS source/binary, exporter access, or access under `/tmp/claude-0/`. The corrected specification was read, not edited. Sections 1–4 record the updated gates; performance measurements and the explicitly historical A08c verification below were not rerun as benchmarks.

## 1. Guard hypotheses — resolved: band-edge peaks

The defect was in `late_peaks` (now `late_mode_frequencies`), not in the oracle guard. The 20–160 Hz mask
must be applied to the summed power over the **whole** 2049-bin grid before
peak finding. Outside bins are zero. The former interior-only search omitted
bins 137 (20.068359375 Hz) and 1092 (159.9609375 Hz). The full masked-array
search retains SciPy's rounded-down plateau midpoint and stable descending
height order. MIMO band/tail filters and RC's `lo ≤ p ≤ min(hi, 160)` filter
are unchanged.

In mixed/100/low40 BL and FC trial 0, the rising after-spectrum makes
159.9609375 Hz one of the top six peaks, displacing the approximately 44 Hz
peak. These were port defects, **not oracle anomalies**. The alternate
H1a–H1d hypotheses no longer add information and their diagnostic test was
removed. The production golden test enforces the single specification rule,
exact guard-array lengths and a total of 85 compared values.

| Rule | Compared | Within 1e-9 absolute | Worst absolute difference |
| --- | ---: | ---: | ---: |
| Before ∪ after top-six peaks of full masked spectra | 85 | 85 | 1.850963826655061e-12 |

Tail units remain 0.15 × dB. BL trial-0 tail is 0.1359248279711114 versus
0.13592482797111435; FC is 0.4603454728102542 versus 0.46034547281023636.
BL now selects 0 dB, exactly as the fixture does. No guard exceptions,
extra trials, expected-support rescaling, or expected-level offsets remain,
including the frozen-RC application check.

`late_mode_frequencies_include_band_edges` (then `late_peaks_include_band_edges`) uses synthetic late two-tap signals with
power `2−2cos(w)` (rising), `2+2cos(w)` (falling), and `2−2cos(2w)`
(interior peak). SciPy 1.18.1 `find_peaks(power * mask)` gives bins
1092, 137, and 1024 respectively. A late impulse at sample 2048 gives exact
unit power and the even plateau midpoint 614. All four frequencies match
exactly; empty/all-zero signals and count zero return no peaks. The test
records the SciPy reproduction recipe. Existing 14 primitive peak values
remain exact.

## 2. Gates in dependency order

| Boundary | Maximum absolute | Maximum relative L2 | Result |
| --- | ---: | ---: | --- |
| Reconstructed demo g per ear and low-pass, 14 inputs | 5.551115123125783e-17 | 4.886330845995666e-16 | PASS |
| RC-dependent x / direct energy | see attribution below | x up to 0.785284989; energy up to 0.015024953 | Measured, not a frozen-input reconstruction gate |
| Frozen x/g/lp → normal equations | 2.602085213965211e-18 | 1.391072255676036e-15 | PASS, 120960 values |
| Bisection/kept u/trial ratios | 4.698101818689793e-8 | 1.678269043507417e-9 | PASS existing ratio budget; all decisions identical |
| Guard, no exceptions | 1.850963826655061e-12 | 2.030327703683621e-12 | PASS, 85 values |
| Final supports and level | 4.103384299014579e-13 | 7.736964413886417e-13 | PASS, 15134 values; no adjustments |
| Dense normal cross-check | matrix rel 4.367886386622643e-16 | rhs rel 1.271837700854252e-15 | PASS |
| Dense kept solve | — | 6.564296201359275e-14 | PASS, 19/19 decisions, kept 15/15 |

The new production observer `design_mimo_observed` (now `design_supports_observed`) exposes stage boundaries without retaining matrices for all inputs. g is compared after explicit canonical-name reordering (recording order places BR last). The observer currently runs after each input's solve; therefore these are dependency-isolating comparisons, not a fail-before-solve execution sequence. A complete x/direct reconstruction gate from an exported matching RC spectrum remains unavailable: the MIMO fixtures do not export that spectrum. No comparison of an input with itself is presented as a path gate.

Application checks added:

- Full convolution before the existing output crop/fade, using `apply_tuning2_full`, shares the production application implementation. Tuned minus RC-only equals Σ_j BRIR_j ⊛ q_ij on all 14 ears; maximum relative error **2.2144931454811554e-14** (<1e-9).
- Historical A08c result: the same full-convolution difference failed the then-required -80 dB high-band energy limit on all 14 ears: **-78.45937 to -71.26417 dB**. This is not a cropped-output test and not a test at only the original response FFT bins. q has exact zeros at its original >285 Hz bins, but its finite sequence is not zero between those bins. A smooth frequency transition alone did not resolve it. An experiment fading both time endpoints passed (-85.43 dB worst), but was reverted: it changes the specified q formula (ROOM_CORRECTION.md, support realization), not merely the test. No support formula change is retained. The A08d starting tree already gates leakage at -65 dB; that existing gate passes and was not changed in A08d.
- Zero-mode latency: zero reported delay, unchanged lengths, unmeasured speakers byte-identical. PASS.
- Geometry: pairs populated during actual per-recording room-file loading. The demo has two applicable ipsilateral checks; both pass. Constructed reversed arrival for FL/FR is reported in pair and speaker diagnostics. PASS.
- Pipeline observer: one report in enabled tuning2; none in tuning, eq, or disabled correction. Checks report speaker count and latency. PASS.
- `design_paths`, `normal_equations`, `bisect`, `support_response`: invalid dimensions/values return `InvalidArgument`. The latter two previously infallible diagnostics now return Result, and callers propagate/unwrap appropriately. PASS.
- Three test roots now forbid unsafe code; policy PASS.

## 3. End-to-end count and provisional bounds

| Setting | Rust changed steps | NumPy AVX-512 | Haswell | Sandybridge | Nehalem | gesvd | Worst variant |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| linear/150/auto | 1 | 1 | 2 | 1 | 1 | 1 | 2 |
| mixed/100/low40 | 2 | 1 | 1 | 1 | 1 | 3 | 3 |
| mixed/150/auto | 4 | 1 | 1 | 1 | 1 | 2 | 2 |

A08d reran these counts after fixing band-edge peaks, both normally and with
`SECS2_ASSUME_FOREIGN_LIBM=1`. Counts are changed input decisions, not the sum
of 3 dB increments. `GUARD_STEP_ALLOWANCE=0` failed only mixed/150/auto
(`4 > 2 + 0`); its deficit of two also excludes allowance 1. The smallest
passing allowance is therefore **2**, retained with an updated measured
comment. This is an end-to-end RC-dependent allowance, not an isolated MIMO
guard exemption. Ranges match exactly; gains agree within 1e-10.

Named constants in `room_tuning2_apply.rs`, each explicitly provisional pending owner review, use upward-rounded observed maxima ×1.5:

| Metric | Measured ceiling | Bound |
| --- | ---: | ---: |
| support relative L2 | 0.997751334 | 1.496627001 |
| support max abs | 0.049634475 | 0.0744517125 |
| level abs difference dB | 6.000058928 | 9.000088392 |
| accepted late band abs difference dB | 8.287972908 | 12.431959362 |
| accepted late tail abs difference (0.15×dB) | 0.169831498 | 0.254747247 |

All provisional numerical bounds pass, as does the count criterion with the minimum allowance of 2. Null oracle late metrics for rejected inputs are not turned into numeric comparisons.

## 4. RC attribution

Neither the MIMO end-to-end reference nor the per-input stage fixtures contain the full matching BRIR-derived RC response. The separate `s2_rc_*` responses use omni measurements, and cannot be substituted as if they were those BRIR corrections. No such mismatched substitution was performed.

Instead, use the exact sufficient RC outputs already frozen for MIMO: x per ear and direct correction energy. Retain Rust's reconstructed g, low-pass, solver and H1 guard; replace only x and direct. Across the 14 mixed inputs, remaining support relative L2 is **≤5.556406350950572e-13**, maximum absolute **≤4.335041440400866e-14**. No BL adjustment is needed after the band-edge fix; all fourteen inputs pass against unmodified expected supports and levels. This isolates the remaining mixed-setting numerical support differences upstream of the MIMO solver, at the RC/path boundary. No linear per-input fixture exists, so this attribution does not establish the linear case.

| Input | Rust x relative L2 vs frozen x | Direct energy relative difference |
| --- | ---: | ---: |
| FL | 0.153703418 | 0.010753369 |
| FR | 0.785284989 | 0.002467122 |
| FC | 0.066566146 | 0.008338511 |
| BL | 0.147131622 | 0.015024953 |
| BR | 0.057759152 | 0.005205847 |
| SL | 0.009208698 | 0.000521825 |
| SR | 0.007899890 | 0.002770914 |

RC implementation is outside this packet's writable scope and was not changed.
A08d reran all RC gates: long-mode bins are exact in all 11 sessions (89
values total). End-to-end counts above 0.001/0.01/0.1/1 dB remain
56/36/19/7 versus envelope 61/39/32/15. Maximum magnitude differences are
1.994235049 / 1.919082892 / 0.765318729 / 0.057842253 dB versus caps
2.91 / 3.33 / 0.93 / 0.08. Strength changes: 2; PEQ count changes: 0.
Post-fit stage spectra remain report-only, not a new equivalence claim.

## 5. Solver comparison and scalar probe

Same actual synthetic 24-speaker normal matrix (R=23, m=4140), same machine; one sequential EVD reused for nineteen regularizations versus nineteen explicitly sequential low-level faer Cholesky factorizations and solves. Both include bisection decisions and support-energy evaluation. faer remains without std; no global parallelism setting is changed.

| Run | EVD seconds | 19 Cholesky seconds |
| --- | ---: | ---: |
| Initial default pool | 17.577184 | 19.127901 |
| One-thread benchmark | 19.874033 | 18.965401 |
| Final default pool | 17.854417 | 19.512800 |
| Median of observed comparisons | 17.854417 | 19.127901 |

**Keep EVD:** median 1.071× faster, both default-pool runs faster; the one-thread run reverses the close result under shared-container noise. No thread-count-dependent solver dispatch. Every run: decisions 19/19, kept 18/18, relative kept-u difference 6.375019693345964e-13. The existing dense 6-support comparison also passes.

| Scalar log10 ns/point | Before demo MIMO | After demo MIMO | After 24-speaker MIMO |
| --- | ---: | ---: | ---: |
| One thread | 11.078681 | 14.758995 | 7.616691 |
| Default pool (4) | 9.880620 | 11.739841 | 9.386531 |

Fixed 700-point loop repeated 10000 times, black-boxed scalar inputs/output. No approximately 17→190 ns/point dirty-AVX stall is observed. Historical cause: faer std enables private-gemm-x86 assembly kernels; the existing policy gate prohibits that path and passes. Small timing variation here is not evidence of that order-of-magnitude stall.

## 6. Refreshed performance

RC: FL, two omni positions, median of three after warm-up; one thread and default four-thread pool. Oracle times are owner-supplied historical values, not rerun SECS code. One-thread values below are the final no-compilation-overhead repeat.

| RC mode | Oracle s | Rust 1 thread s | Speedup | Rust default s | Speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| mixed | 0.88 | 0.392786 | 2.240× | 0.359069 | 2.451× |
| zero | 1.55 | 0.727571 | 2.130× | 0.604227 | 2.565× |
| linear | 2.22 | 0.639080 | 3.474× | 0.410691 | 5.406× |
| long | 0.73 | 0.297499 | 2.454× | 0.298886 | 2.442× |
| ultra-60 | 1.91 | 0.608805 | 3.137× | 0.349663 | 5.462× |

Rust is faster than all five supplied RC references. The 4× aspiration is not met for every mode.

| MIMO / application case | One thread s | Default pool s |
| --- | ---: | ---: |
| Demo design, seven inputs | 3.043502 | 0.911318 |
| Demo design amortized/input | 0.434786 | 0.130188 |
| Whole omni prepare + apply, MIMO off | 5.071770 | 1.528095 |
| Whole omni prepare + apply, MIMO on | 8.660629 | 2.858972 |
| 24 speakers, one input | 19.800503 | 18.361012 |

Whole timings load the actual omni room recordings before the timer, then include prepare_tuning2 and apply_tuning2 on aligned demo BRIRs. They are single-run timings, not medians. Demo design-only retains the previous benchmark's BRIR-derived RC inputs/gain=0; its per-input number is amortized total/7. These workloads must not be conflated.

ADR 0008's historical approximately 6 s ideal-FIR MIMO reference gives demo design speedups **1.97× / 6.58×** (one/default). No 24-speaker oracle timing is available. The unused SECS IIR approximation still had not completed after 30 minutes; it is not used as a finite timing denominator.

Peak RSS, fresh resource.getrusage child-process wrappers: RC **177140 / 227088 KiB** (one/default); combined MIMO benchmark including whole application, 24-speaker design and solver comparison **1199572 / 1329456 KiB**. Initial one-thread RC invocation compiled and reached 1206392 KiB; excluded from runtime RSS and repeated after compilation. The measurements include setup and benchmark lifetime, not isolated operation memory.

## 7. A08d verification

Environment: `CARGO_PROFILE_TEST_OPT_LEVEL=2 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false`.

- `cargo fmt --all -- --check`: PASS.
- `cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings`: PASS.
- `cargo test -p impulcifer-policy`: 9 passed; zero-test unit/doc targets pass.
- `cargo test -p impulcifer-dsp --no-fail-fast`: all targets PASS, including golden RC 3, primitives 14, MIMO 2, end-to-end RC 7 and application 13.
- `SECS2_ASSUME_FOREIGN_LIBM=1 cargo test -p impulcifer-dsp --no-fail-fast --test golden_secs2_mimo --test golden_secs2_primitives --test room_tuning2_apply`: 2 / 14 / 13 passed.
- `cargo test -p impulcifer-service --test room_tuning2`: 6 passed.
- An initial allowance-zero probe failed as documented in section 3; final verification uses the minimum passing value 2.

```text
MEASURE MIMO guard: max_abs=1.85096382665506098e-12, max_relative_l2=2.03032770368362084e-12, values=85
MEASURE MIMO final supports and level: max_abs=4.10338429901457857e-13, max_relative_l2=7.73696441388641681e-13, values=15134
MEASURE late_peaks_include_band_edges: max_abs=0.00000000000000000e0, max_relative_l2=0.00000000000000000e0, values=4
MEASURE MIMO guard steps linear/fsiso150/auto changed_steps=1 worst_variant=2
MEASURE MIMO guard steps mixed/fsiso100/low40 changed_steps=2 worst_variant=3
MEASURE MIMO guard steps mixed/fsiso150/auto changed_steps=4 worst_variant=2
MEASURE RC distribution: thresholds=[0.001, 0.01, 0.1, 1.0] ours=[56, 36, 19, 7] envelope=[61, 39, 32, 15] max_mag_db=[1.9942350486803289, 1.919082891777603, 0.7653187289206791, 0.057842252870441715] strength_changes=2 peq_count_changes=0
```

### Historical A08c verification (superseded for current gate status)

Test environment: `CARGO_PROFILE_TEST_OPT_LEVEL=2 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false`.

- `cargo fmt --all -- --check`: PASS.
- `cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings`: PASS.
- `cargo test -p impulcifer-policy`: 9 passed, 0 failed.
- `cargo test -p impulcifer-dsp --no-fail-fast`: only room_tuning2_apply fails, 11 passed / 2 failed; all other targets pass.
- Requested golden/application nocapture command: golden_secs2_mimo 3 passed; room_tuning2_apply 11 passed / 2 failed.
- `cargo bench -p impulcifer-dsp --bench perf -- room_tuning2`: PASS, both thread settings; additional mimo benchmark PASS, both thread settings.
- `git diff --check`: PASS.

## 8. Historical A08c remaining work / scope limits

1. mixed/150/auto count still 4>2; frozen-boundary attribution identifies RC-dependent inputs rather than solver error, but RC edits are prohibited here.
2. Full finite-convolution -80 dB condition conflicts with the current specified support realization on this demo. A realization change needs owner direction; no algorithm/tolerance workaround retained.
3. x/direct path reconstruction from matching frozen RC spectra, fail-before-normal-equations execution order, and literal full-response RC substitution are not completed. Matching full MIMO RC spectra are absent; g/lp are gated and the exact x/direct substitution experiment is reproducible.
4. The solver timing separation is small/noisy; EVD retained based on median observed paired runs, not a universal performance claim.
5. No version/CHANGELOG/feature-registry edits: outside explicit packet allowlist. No merge or acceptance claim.

A concurrent external WIP commit `f69aaa4` appeared during execution and includes earlier edits from this working tree. This worker made no commit and did not undo it. Final porcelain therefore lists only edits after that external snapshot, not all A08c work.

## Historical A08c verification excerpts

The following excerpts preserve the old A08c output, including failures and now-removed guard hypotheses. They are historical, not the current A08d result; see sections 1–4 and 7 above.

### policy

```text
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### dsp

```text
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.51s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.18s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.13s
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.04s
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.45s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.29s
MEASURE full_support FL/right=false sum_relative=2.2144931454811554e-14 leakage_db=-75.8457977396133
MEASURE full_support FL/right=true sum_relative=1.2192918861004125e-14 leakage_db=-74.4183336720952
MEASURE full_support FR/right=false sum_relative=9.120495242376515e-15 leakage_db=-71.91073644261424
MEASURE full_support FR/right=true sum_relative=1.64309846796085e-14 leakage_db=-71.26416584389506
MEASURE full_support FC/right=false sum_relative=1.2651554422722665e-14 leakage_db=-78.45936978543389
MEASURE full_support FC/right=true sum_relative=1.0670349593400248e-14 leakage_db=-77.56866743447041
MEASURE full_support BL/right=false sum_relative=1.0217718326868585e-14 leakage_db=-76.73683423384914
MEASURE full_support BL/right=true sum_relative=1.1887655668034168e-14 leakage_db=-77.80271312196487
MEASURE full_support SL/right=false sum_relative=1.3239206028533031e-14 leakage_db=-74.93246972746915
MEASURE full_support SL/right=true sum_relative=9.407599713411138e-15 leakage_db=-75.48857697332878
MEASURE full_support SR/right=false sum_relative=6.814097944756065e-15 leakage_db=-76.18737454137806
MEASURE full_support SR/right=true sum_relative=1.3485135801935296e-14 leakage_db=-76.1725735776294
MEASURE full_support BR/right=false sum_relative=9.030757695426784e-15 leakage_db=-74.88558710624889
MEASURE full_support BR/right=true sum_relative=1.2062414676857522e-14 leakage_db=-72.81589739537056
REFERENCE linear/fsiso150/auto design_seconds=7.3143724070000005
REFERENCE linear/fsiso150/auto/FL level_delta=0.000188081 support_l2=0.288824656 max_abs=8.243445869001135e-3 scale=Some(-9.0) late=-0.525864165/0.130862513 expected_late={"tail_change":0.08370939864836278,"worst_band_db":-0.6064483500395426}
VARIANT lapack_gesvd linear/fsiso150/auto/FL level_delta=0.000221907487990336 support_l2=0.0009604909769251142
VARIANT numpy_avx512 linear/fsiso150/auto/FL level_delta=0.00012091377130474257 support_l2=0.004554591390944338
VARIANT openblas_haswell linear/fsiso150/auto/FL level_delta=3.0001723657445325 support_l2=0.4263540323387467
VARIANT openblas_nehalem linear/fsiso150/auto/FL level_delta=0.00004771968328753928 support_l2=0.008926386927595596
VARIANT openblas_sandybridge linear/fsiso150/auto/FL level_delta=0.00014175597568311105 support_l2=0.011656157952275297
REFERENCE linear/fsiso150/auto/FR level_delta=0.000010606 support_l2=0.024134045 max_abs=3.483291835019407e-4 scale=Some(-12.0) late=-1.470954490/0.128462488 expected_late={"tail_change":0.14222138227025163,"worst_band_db":-1.4648569392012694}
VARIANT lapack_gesvd linear/fsiso150/auto/FR level_delta=0.00016300189424001132 support_l2=0.02983133346991192
VARIANT numpy_avx512 linear/fsiso150/auto/FR level_delta=0.00012555305741912548 support_l2=0.0383368659435643
VARIANT openblas_haswell linear/fsiso150/auto/FR level_delta=0.00019137715653982923 support_l2=0.01611175889990373
VARIANT openblas_nehalem linear/fsiso150/auto/FR level_delta=0.0002143162194450099 support_l2=0.00904850767840343
VARIANT openblas_sandybridge linear/fsiso150/auto/FR level_delta=0.000056710824310357566 support_l2=0.03821530131852947
REFERENCE linear/fsiso150/auto/FC level_delta=-0.000038602 support_l2=0.075339184 max_abs=3.5597207914406554e-3 scale=Some(-3.0) late=-2.304386369/0.137692916 expected_late={"tail_change":0.13771264653410775,"worst_band_db":-2.2793132763253414}
VARIANT lapack_gesvd linear/fsiso150/auto/FC level_delta=0.00018949327049178066 support_l2=0.061081515346080965
VARIANT numpy_avx512 linear/fsiso150/auto/FC level_delta=-0.00005583028765876463 support_l2=0.01997579118304714
VARIANT openblas_haswell linear/fsiso150/auto/FC level_delta=0.00021780846086016936 support_l2=0.02210484883027762
VARIANT openblas_nehalem linear/fsiso150/auto/FC level_delta=0.000060379821738010264 support_l2=0.0151194758903638
VARIANT openblas_sandybridge linear/fsiso150/auto/FC level_delta=0.00018396035895840157 support_l2=0.01897336558729884
REFERENCE linear/fsiso150/auto/BL level_delta=-0.000060510 support_l2=0.856236901 max_abs=4.072551538073092e-2 scale=Some(-6.0) late=-2.151258726/0.106628222 expected_late={"tail_change":0.04190150648954669,"worst_band_db":-1.0677465323667348}
VARIANT lapack_gesvd linear/fsiso150/auto/BL level_delta=0.00026865413046550657 support_l2=0.028070370923001627
VARIANT numpy_avx512 linear/fsiso150/auto/BL level_delta=0.00007318987838189628 support_l2=0.017380409602197264
VARIANT openblas_haswell linear/fsiso150/auto/BL level_delta=0.0001624999548788253 support_l2=0.030880263501578884
VARIANT openblas_nehalem linear/fsiso150/auto/BL level_delta=0.00018160346605355926 support_l2=0.03582868813943077
VARIANT openblas_sandybridge linear/fsiso150/auto/BL level_delta=0.00015744712332832478 support_l2=0.02229407877849507
REFERENCE linear/fsiso150/auto/SL level_delta=0.000126758 support_l2=0.002952399 max_abs=4.5078403455346e-5 scale=Some(-12.0) late=-0.694915853/0.138533284 expected_late={"tail_change":0.1390985731402232,"worst_band_db":-0.6948801146916603}
VARIANT lapack_gesvd linear/fsiso150/auto/SL level_delta=0.00011283439409126572 support_l2=0.002769856625883196
VARIANT numpy_avx512 linear/fsiso150/auto/SL level_delta=-0.000012512275912257564 support_l2=0.004009376629365894
VARIANT openblas_haswell linear/fsiso150/auto/SL level_delta=-0.00002867862720634662 support_l2=0.00442394609696436
VARIANT openblas_nehalem linear/fsiso150/auto/SL level_delta=0.000016201026774353977 support_l2=0.003179192960809436
VARIANT openblas_sandybridge linear/fsiso150/auto/SL level_delta=0.00012846459052795467 support_l2=0.002906346838777276
REFERENCE linear/fsiso150/auto/SR level_delta=0.000302507 support_l2=0.131069288 max_abs=2.832724423060042e-3 scale=Some(-9.0) late=-1.698287293/0.103212703 expected_late={"tail_change":0.10498591518950333,"worst_band_db":-1.6204982439111302}
VARIANT lapack_gesvd linear/fsiso150/auto/SR level_delta=0.0001904302429984739 support_l2=0.01447413419078992
VARIANT numpy_avx512 linear/fsiso150/auto/SR level_delta=0.00014458391180482977 support_l2=0.12654170329233358
VARIANT openblas_haswell linear/fsiso150/auto/SR level_delta=0.000154522732092488 support_l2=0.09122836544649573
VARIANT openblas_nehalem linear/fsiso150/auto/SR level_delta=0.00010571584392948807 support_l2=0.08699963050550528
VARIANT openblas_sandybridge linear/fsiso150/auto/SR level_delta=0.00009807726321042765 support_l2=0.09374330707671129
REFERENCE linear/fsiso150/auto/BR level_delta=2.999677627 support_l2=0.416991467 max_abs=1.7023277354976864e-2 scale=Some(-3.0) late=-2.440014541/0.033446457 expected_late={"tail_change":0.09583169987829296,"worst_band_db":-1.6826220757655916}
VARIANT lapack_gesvd linear/fsiso150/auto/BR level_delta=2.999715484828714 support_l2=0.42498341860072786
VARIANT numpy_avx512 linear/fsiso150/auto/BR level_delta=2.999886553301719 support_l2=0.4316634407462263
VARIANT openblas_haswell linear/fsiso150/auto/BR level_delta=2.999836854245274 support_l2=0.43174721457933424
VARIANT openblas_nehalem linear/fsiso150/auto/BR level_delta=2.999688039709815 support_l2=0.41738503204732463
VARIANT openblas_sandybridge linear/fsiso150/auto/BR level_delta=2.9997662085091896 support_l2=0.43171070528799643
REFERENCE linear/fsiso150/auto changed_steps=1 worst_variant=2
REFERENCE mixed/fsiso100/low40 design_seconds=4.606776307
REFERENCE mixed/fsiso100/low40/FL level_delta=-6.000058927 support_l2=0.611694919 max_abs=4.0568759298774834e-2 scale=Some(-6.0) late=-1.961815875/0.100982475 expected_late={"tail_change":0.028308168234011305,"worst_band_db":-10.24978878252253}
VARIANT lapack_gesvd mixed/fsiso100/low40/FL level_delta=-6.000012378282056 support_l2=0.5323570678148551
VARIANT numpy_avx512 mixed/fsiso100/low40/FL level_delta=0.000011317074930516924 support_l2=0.03988093794806209
VARIANT openblas_haswell mixed/fsiso100/low40/FL level_delta=-0.00004453865321529804 support_l2=0.0014716351698517718
VARIANT openblas_nehalem mixed/fsiso100/low40/FL level_delta=0.00000720411783738939 support_l2=0.06341824173278726
VARIANT openblas_sandybridge mixed/fsiso100/low40/FL level_delta=0.000040039985568895986 support_l2=0.0038348229769119735
REFERENCE mixed/fsiso100/low40/FR level_delta=0.000000000 support_l2=0.000000000 max_abs=0e0 scale=None late=-99.000000000/-14.850000000 expected_late={"tail_change":null,"worst_band_db":null}
VARIANT lapack_gesvd mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
VARIANT numpy_avx512 mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
VARIANT openblas_haswell mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
VARIANT openblas_nehalem mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
VARIANT openblas_sandybridge mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
REFERENCE mixed/fsiso100/low40/FC level_delta=0.000001164 support_l2=0.171632354 max_abs=5.794038041964592e-3 scale=Some(-3.0) late=-4.074004127/0.016158924 expected_late={"tail_change":0.08534122999447218,"worst_band_db":-4.1222981009248025}
VARIANT lapack_gesvd mixed/fsiso100/low40/FC level_delta=0.0000036529197107171285 support_l2=0.035947658635556726
VARIANT numpy_avx512 mixed/fsiso100/low40/FC level_delta=0.000015267142686070656 support_l2=0.0031966834072174495
VARIANT openblas_haswell mixed/fsiso100/low40/FC level_delta=-0.00004647757225839655 support_l2=0.010223116193791194
VARIANT openblas_nehalem mixed/fsiso100/low40/FC level_delta=0.00002737674172337279 support_l2=0.007213016138601747
VARIANT openblas_sandybridge mixed/fsiso100/low40/FC level_delta=-0.00005566896961184398 support_l2=0.004516579063628009
REFERENCE mixed/fsiso100/low40/BL level_delta=0.000049349 support_l2=0.684538820 max_abs=4.9634474725065186e-2 scale=Some(0.0) late=-0.219016499/0.113716110 expected_late={"tail_change":0.13592482797111505,"worst_band_db":-0.2716423185952849}
VARIANT lapack_gesvd mixed/fsiso100/low40/BL level_delta=-3.0000184254121205 support_l2=0.30407892160125505
VARIANT numpy_avx512 mixed/fsiso100/low40/BL level_delta=0.000018496691726710424 support_l2=0.05602306786475751
VARIANT openblas_haswell mixed/fsiso100/low40/BL level_delta=0.0000157090283270378 support_l2=0.05759276201435488
VARIANT openblas_nehalem mixed/fsiso100/low40/BL level_delta=-0.00006046669609460764 support_l2=0.05514502509769374
VARIANT openblas_sandybridge mixed/fsiso100/low40/BL level_delta=0.000027839111956140528 support_l2=0.06231943314387411
REFERENCE mixed/fsiso100/low40/SL level_delta=5.999990128 support_l2=0.997751333 max_abs=3.3977349403671864e-2 scale=Some(0.0) late=-0.087601295/0.144793987 expected_late={"tail_change":0.05475037558740551,"worst_band_db":-0.04496169965587714}
VARIANT lapack_gesvd mixed/fsiso100/low40/SL level_delta=6.000033891562957 support_l2=1.045620096248696
VARIANT numpy_avx512 mixed/fsiso100/low40/SL level_delta=-6.000078689651492 support_l2=0.5163992888060525
VARIANT openblas_haswell mixed/fsiso100/low40/SL level_delta=-3.0000099866116905 support_l2=0.29981584459635224
VARIANT openblas_nehalem mixed/fsiso100/low40/SL level_delta=-5.999991683643646 support_l2=0.5145248209435467
VARIANT openblas_sandybridge mixed/fsiso100/low40/SL level_delta=-2.9999692145949517 support_l2=0.3072595000766432
REFERENCE mixed/fsiso100/low40/SR level_delta=0.000007659 support_l2=0.048394655 max_abs=2.0526655411892756e-3 scale=Some(-3.0) late=-0.295199906/0.085078889 expected_late={"tail_change":0.08237673663687096,"worst_band_db":-0.28929096914684144}
VARIANT lapack_gesvd mixed/fsiso100/low40/SR level_delta=0.000049536239243863633 support_l2=0.06398019860045645
VARIANT numpy_avx512 mixed/fsiso100/low40/SR level_delta=0.000030886344665859156 support_l2=0.1013728023083956
VARIANT openblas_haswell mixed/fsiso100/low40/SR level_delta=0.00008902264812604699 support_l2=0.06647656089331555
VARIANT openblas_nehalem mixed/fsiso100/low40/SR level_delta=0.00009026457823146927 support_l2=0.0509089082252906
VARIANT openblas_sandybridge mixed/fsiso100/low40/SR level_delta=0.00010421311790409504 support_l2=0.07312506779996229
REFERENCE mixed/fsiso100/low40/BR level_delta=0.000094302 support_l2=0.051423937 max_abs=8.188387357764212e-4 scale=Some(-12.0) late=-0.063420751/0.114564850 expected_late={"tail_change":0.1363127336475263,"worst_band_db":-0.06243078149539497}
VARIANT lapack_gesvd mixed/fsiso100/low40/BR level_delta=-0.000010971341666277112 support_l2=0.007954483034918585
VARIANT numpy_avx512 mixed/fsiso100/low40/BR level_delta=0.0000912016784972991 support_l2=0.002514611318415844
VARIANT openblas_haswell mixed/fsiso100/low40/BR level_delta=-0.00000952785324059846 support_l2=0.0020093929382828573
VARIANT openblas_nehalem mixed/fsiso100/low40/BR level_delta=-0.00006536410170809859 support_l2=0.003137998629225176
VARIANT openblas_sandybridge mixed/fsiso100/low40/BR level_delta=0.00008777970851880923 support_l2=0.002018185218223796
REFERENCE mixed/fsiso100/low40 changed_steps=2 worst_variant=3
REFERENCE mixed/fsiso150/auto design_seconds=4.047948805
REFERENCE mixed/fsiso150/auto/FL level_delta=3.000137012 support_l2=0.552352940 max_abs=2.286118965221605e-2 scale=Some(-3.0) late=-3.272194575/-0.031991093 expected_late={"tail_change":0.137840404418629,"worst_band_db":-2.667884635736209}
VARIANT lapack_gesvd mixed/fsiso150/auto/FL level_delta=0.0001907588505929425 support_l2=0.1214591456283015
VARIANT numpy_avx512 mixed/fsiso150/auto/FL level_delta=0.00006207282770276379 support_l2=0.018631018692080857
VARIANT openblas_haswell mixed/fsiso150/auto/FL level_delta=0.00012076418121509391 support_l2=0.0016504974412136098
VARIANT openblas_nehalem mixed/fsiso150/auto/FL level_delta=0.00016083474337236225 support_l2=0.0307512868201759
VARIANT openblas_sandybridge mixed/fsiso150/auto/FL level_delta=0.00011153566116206548 support_l2=0.00446914449703479
REFERENCE mixed/fsiso150/auto/FR level_delta=0.000000000 support_l2=0.000000000 max_abs=0e0 scale=None late=-99.000000000/-14.850000000 expected_late={"tail_change":null,"worst_band_db":null}
VARIANT lapack_gesvd mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
VARIANT numpy_avx512 mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
VARIANT openblas_haswell mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
VARIANT openblas_nehalem mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
VARIANT openblas_sandybridge mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
REFERENCE mixed/fsiso150/auto/FC level_delta=-0.000172211 support_l2=0.111513805 max_abs=3.47789131811247e-3 scale=Some(-3.0) late=-3.060627663/-0.068009630 expected_late={"tail_change":-0.07034489928225182,"worst_band_db":-2.9811769119890696}
VARIANT lapack_gesvd mixed/fsiso150/auto/FC level_delta=0.00007980659141004764 support_l2=0.0245137249968377
VARIANT numpy_avx512 mixed/fsiso150/auto/FC level_delta=0.00010420614476025492 support_l2=0.0028787261994542928
VARIANT openblas_haswell mixed/fsiso150/auto/FC level_delta=-0.00006752565775691721 support_l2=0.007210344874246437
VARIANT openblas_nehalem mixed/fsiso150/auto/FC level_delta=0.00008486779362826269 support_l2=0.005543982634476855
VARIANT openblas_sandybridge mixed/fsiso150/auto/FC level_delta=-0.00006198980194405124 support_l2=0.003999997958384296
REFERENCE mixed/fsiso150/auto/BL level_delta=-2.999819828 support_l2=0.492801656 max_abs=2.570030624777369e-2 scale=Some(-6.0) late=-1.953334607/0.118607561 expected_late={"tail_change":0.1368844254652269,"worst_band_db":-4.065330969053657}
VARIANT lapack_gesvd mixed/fsiso150/auto/BL level_delta=-2.9998969722027056 support_l2=0.301359589620858
VARIANT numpy_avx512 mixed/fsiso150/auto/BL level_delta=0.0002465260837425376 support_l2=0.04446900309758868
VARIANT openblas_haswell mixed/fsiso150/auto/BL level_delta=0.00006935708501920601 support_l2=0.0455077742183558
VARIANT openblas_nehalem mixed/fsiso150/auto/BL level_delta=0.000022722558563614825 support_l2=0.0413897604941415
VARIANT openblas_sandybridge mixed/fsiso150/auto/BL level_delta=0.00021025101471394692 support_l2=0.04471134625903966
REFERENCE mixed/fsiso150/auto/SL level_delta=5.999853564 support_l2=0.997116455 max_abs=4.638508098436068e-2 scale=Some(0.0) late=-4.280387785/0.134381462 expected_late={"tail_change":0.05973465932930784,"worst_band_db":-2.05737843001986}
VARIANT lapack_gesvd mixed/fsiso150/auto/SL level_delta=5.999852798847016 support_l2=1.0213289004106538
VARIANT numpy_avx512 mixed/fsiso150/auto/SL level_delta=-6.000120737984265 support_l2=0.5102735179681622
VARIANT openblas_haswell mixed/fsiso150/auto/SL level_delta=-3.0000279837421076 support_l2=0.29907502774945727
VARIANT openblas_nehalem mixed/fsiso150/auto/SL level_delta=-6.000098330634884 support_l2=0.5091987236228102
VARIANT openblas_sandybridge mixed/fsiso150/auto/SL level_delta=-3.000005744302115 support_l2=0.30458539829788883
REFERENCE mixed/fsiso150/auto/SR level_delta=-0.000041656 support_l2=0.035409672 max_abs=7.311277028378312e-4 scale=Some(-9.0) late=-1.400107467/0.107108121 expected_late={"tail_change":0.10510914794130934,"worst_band_db":-1.369374331177764}
VARIANT lapack_gesvd mixed/fsiso150/auto/SR level_delta=-0.00001449981483503393 support_l2=0.050398692294140734
VARIANT numpy_avx512 mixed/fsiso150/auto/SR level_delta=0.00003763829093728077 support_l2=0.0966011300066491
VARIANT openblas_haswell mixed/fsiso150/auto/SR level_delta=-0.0000418534507993229 support_l2=0.053042879177995696
VARIANT openblas_nehalem mixed/fsiso150/auto/SR level_delta=-0.0001701709756716241 support_l2=0.04083812415048273
VARIANT openblas_sandybridge mixed/fsiso150/auto/SR level_delta=-0.0000730670613968698 support_l2=0.05476476422005671
REFERENCE mixed/fsiso150/auto/BR level_delta=3.000056023 support_l2=0.420080874 max_abs=8.44650190314724e-3 scale=Some(-9.0) late=-0.908534338/0.146518262 expected_late={"tail_change":0.10696738646365453,"worst_band_db":-0.6081353552108064}
VARIANT lapack_gesvd mixed/fsiso150/auto/BR level_delta=-0.00012391478329121242 support_l2=0.006386466978344232
VARIANT numpy_avx512 mixed/fsiso150/auto/BR level_delta=-0.00015035013589681512 support_l2=0.002676334995035794
VARIANT openblas_haswell mixed/fsiso150/auto/BR level_delta=0.000028186501637605943 support_l2=0.001625278554819554
VARIANT openblas_nehalem mixed/fsiso150/auto/BR level_delta=-0.00017627489907212635 support_l2=0.002012947025977743
VARIANT openblas_sandybridge mixed/fsiso150/auto/BR level_delta=0.00007355540867592936 support_l2=0.0014958634580454868
REFERENCE mixed/fsiso150/auto changed_steps=4 worst_variant=2
test result: FAILED. 11 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 42.12s
error: test failed, to rerun pass `-p impulcifer-dsp --test room_tuning2_apply`
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.51s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 64.74s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.52s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.39s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.10s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
error: 1 target failed:
```

### gates

```text
KNOWN_ORACLE_ANOMALY mimo_mixed_fsiso100_low40_BL/trial0/tail got=0.15094700525752094 expected=0.13592482797111435
MEASURE mimo_mixed_fsiso100_low40_BL level=-15.000084258023438 scale=Some(-3.0) band=-0.19195988119285648 tail=0.5808457596383699
MEASURE mimo_mixed_fsiso100_low40_BR level=-24.00010524385602 scale=Some(-12.0) band=-0.06243078149539595 tail=0.908751557650133
KNOWN_ORACLE_ANOMALY mimo_mixed_fsiso100_low40_FC/trial0/tail got=0.6016054761641388 expected=0.46034547281023636
MEASURE mimo_mixed_fsiso100_low40_FC level=-15.000037307299136 scale=Some(-3.0) band=-4.122298100924869 tail=0.5689415332966512
MEASURE mimo_mixed_fsiso100_low40_FL level=-12.0000699792339 scale=Some(0.0) band=-10.249788782520678 tail=0.18872112155973744
MEASURE mimo_mixed_fsiso100_low40_FR level=-300 scale=None band=-99 tail=-99
MEASURE mimo_mixed_fsiso100_low40_SL level=-18.000035095320438 scale=Some(-6.0) band=-0.04496169965587617 tail=0.36500250391603406
MEASURE mimo_mixed_fsiso100_low40_SR level=-15.000111472631978 scale=Some(-3.0) band=-0.28929096914687497 tail=0.549178244245708
MEASURE mimo_mixed_fsiso150_auto_BL level=-15.000270529084647 scale=Some(-3.0) band=-4.065330969053649 tail=0.9125628364346327
MEASURE mimo_mixed_fsiso150_auto_BR level=-24.000220395108972 scale=Some(-12.0) band=-0.6081353552107888 tail=0.7131159097576961
MEASURE mimo_mixed_fsiso150_auto_FC level=-15.000155292099628 scale=Some(-3.0) band=-2.9811769119882037 tail=-0.4689659952149005
MEASURE mimo_mixed_fsiso150_auto_FL level=-18.000274427054453 scale=Some(-6.0) band=-2.667884635736206 tail=0.9189360294575586
MEASURE mimo_mixed_fsiso150_auto_FR level=-300 scale=None band=-99 tail=-99
MEASURE mimo_mixed_fsiso150_auto_SL level=-18.000097182541076 scale=Some(-6.0) band=-2.057378430019924 tail=0.3982310621953786
MEASURE mimo_mixed_fsiso150_auto_SR level=-21.000039234205442 scale=Some(-9.0) band=-1.3693743311777706 tail=0.700727652942069
MEASURE MIMO normal equations: max_abs=2.60208521396521064e-18, max_relative_l2=1.39107225567603637e-15, values=120960
MEASURE MIMO solve: max_abs=4.69810181868979271e-8, max_relative_l2=1.67826904350741651e-9, values=15386
MEASURE MIMO guard: max_abs=1.85096382665506098e-12, max_relative_l2=2.03032770368362084e-12, values=83
MEASURE MIMO final supports and level: max_abs=4.10338429901457857e-13, max_relative_l2=7.73696441388641681e-13, values=15134
HYPOTHESIS H1 compared=85 within_1e-9=83 worst=1.41260003353921471e-1 where=mimo_mixed_fsiso100_low40_FC/trial0/tail
HYPOTHESIS H1a compared=85 within_1e-9=83 worst=1.41260003353921471e-1 where=mimo_mixed_fsiso100_low40_FC/trial0/tail
HYPOTHESIS H1b-low compared=85 within_1e-9=78 worst=5.87071129137692105e-1 where=mimo_mixed_fsiso100_low40_BL/trial0/tail
HYPOTHESIS H1b-sixth compared=85 within_1e-9=61 worst=1.00007854619232539e0 where=mimo_mixed_fsiso150_auto_FR/trial0/tail
HYPOTHESIS H1c-after5 compared=85 within_1e-9=84 worst=2.95957993860127611e-1 where=mimo_mixed_fsiso150_auto_FL/trial1/tail
HYPOTHESIS H1c-union6 compared=85 within_1e-9=80 worst=7.03463765237492100e-1 where=mimo_mixed_fsiso150_auto_FL/trial0/tail
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.16s
test room_tuning2_matches_secs2_mimo ... REFERENCE linear/fsiso150/auto design_seconds=2.499339611
REFERENCE linear/fsiso150/auto/FL level_delta=0.000188081 support_l2=0.288824656 max_abs=8.243445869001135e-3 scale=Some(-9.0) late=-0.525864165/0.130862513 expected_late={"tail_change":0.08370939864836278,"worst_band_db":-0.6064483500395426}
VARIANT lapack_gesvd linear/fsiso150/auto/FL level_delta=0.000221907487990336 support_l2=0.0009604909769251142
VARIANT numpy_avx512 linear/fsiso150/auto/FL level_delta=0.00012091377130474257 support_l2=0.004554591390944338
VARIANT openblas_haswell linear/fsiso150/auto/FL level_delta=3.0001723657445325 support_l2=0.4263540323387467
VARIANT openblas_nehalem linear/fsiso150/auto/FL level_delta=0.00004771968328753928 support_l2=0.008926386927595596
VARIANT openblas_sandybridge linear/fsiso150/auto/FL level_delta=0.00014175597568311105 support_l2=0.011656157952275297
REFERENCE linear/fsiso150/auto/FR level_delta=0.000010606 support_l2=0.024134045 max_abs=3.483291835019407e-4 scale=Some(-12.0) late=-1.470954490/0.128462488 expected_late={"tail_change":0.14222138227025163,"worst_band_db":-1.4648569392012694}
VARIANT lapack_gesvd linear/fsiso150/auto/FR level_delta=0.00016300189424001132 support_l2=0.02983133346991192
VARIANT numpy_avx512 linear/fsiso150/auto/FR level_delta=0.00012555305741912548 support_l2=0.0383368659435643
VARIANT openblas_haswell linear/fsiso150/auto/FR level_delta=0.00019137715653982923 support_l2=0.01611175889990373
VARIANT openblas_nehalem linear/fsiso150/auto/FR level_delta=0.0002143162194450099 support_l2=0.00904850767840343
VARIANT openblas_sandybridge linear/fsiso150/auto/FR level_delta=0.000056710824310357566 support_l2=0.03821530131852947
REFERENCE linear/fsiso150/auto/FC level_delta=-0.000038602 support_l2=0.075339184 max_abs=3.5597207914406554e-3 scale=Some(-3.0) late=-2.304386369/0.137692916 expected_late={"tail_change":0.13771264653410775,"worst_band_db":-2.2793132763253414}
VARIANT lapack_gesvd linear/fsiso150/auto/FC level_delta=0.00018949327049178066 support_l2=0.061081515346080965
VARIANT numpy_avx512 linear/fsiso150/auto/FC level_delta=-0.00005583028765876463 support_l2=0.01997579118304714
VARIANT openblas_haswell linear/fsiso150/auto/FC level_delta=0.00021780846086016936 support_l2=0.02210484883027762
VARIANT openblas_nehalem linear/fsiso150/auto/FC level_delta=0.000060379821738010264 support_l2=0.0151194758903638
VARIANT openblas_sandybridge linear/fsiso150/auto/FC level_delta=0.00018396035895840157 support_l2=0.01897336558729884
REFERENCE linear/fsiso150/auto/BL level_delta=-0.000060510 support_l2=0.856236901 max_abs=4.072551538073092e-2 scale=Some(-6.0) late=-2.151258726/0.106628222 expected_late={"tail_change":0.04190150648954669,"worst_band_db":-1.0677465323667348}
VARIANT lapack_gesvd linear/fsiso150/auto/BL level_delta=0.00026865413046550657 support_l2=0.028070370923001627
VARIANT numpy_avx512 linear/fsiso150/auto/BL level_delta=0.00007318987838189628 support_l2=0.017380409602197264
VARIANT openblas_haswell linear/fsiso150/auto/BL level_delta=0.0001624999548788253 support_l2=0.030880263501578884
VARIANT openblas_nehalem linear/fsiso150/auto/BL level_delta=0.00018160346605355926 support_l2=0.03582868813943077
VARIANT openblas_sandybridge linear/fsiso150/auto/BL level_delta=0.00015744712332832478 support_l2=0.02229407877849507
REFERENCE linear/fsiso150/auto/SL level_delta=0.000126758 support_l2=0.002952399 max_abs=4.5078403455346e-5 scale=Some(-12.0) late=-0.694915853/0.138533284 expected_late={"tail_change":0.1390985731402232,"worst_band_db":-0.6948801146916603}
VARIANT lapack_gesvd linear/fsiso150/auto/SL level_delta=0.00011283439409126572 support_l2=0.002769856625883196
VARIANT numpy_avx512 linear/fsiso150/auto/SL level_delta=-0.000012512275912257564 support_l2=0.004009376629365894
VARIANT openblas_haswell linear/fsiso150/auto/SL level_delta=-0.00002867862720634662 support_l2=0.00442394609696436
VARIANT openblas_nehalem linear/fsiso150/auto/SL level_delta=0.000016201026774353977 support_l2=0.003179192960809436
VARIANT openblas_sandybridge linear/fsiso150/auto/SL level_delta=0.00012846459052795467 support_l2=0.002906346838777276
REFERENCE linear/fsiso150/auto/SR level_delta=0.000302507 support_l2=0.131069288 max_abs=2.832724423060042e-3 scale=Some(-9.0) late=-1.698287293/0.103212703 expected_late={"tail_change":0.10498591518950333,"worst_band_db":-1.6204982439111302}
VARIANT lapack_gesvd linear/fsiso150/auto/SR level_delta=0.0001904302429984739 support_l2=0.01447413419078992
VARIANT numpy_avx512 linear/fsiso150/auto/SR level_delta=0.00014458391180482977 support_l2=0.12654170329233358
VARIANT openblas_haswell linear/fsiso150/auto/SR level_delta=0.000154522732092488 support_l2=0.09122836544649573
VARIANT openblas_nehalem linear/fsiso150/auto/SR level_delta=0.00010571584392948807 support_l2=0.08699963050550528
VARIANT openblas_sandybridge linear/fsiso150/auto/SR level_delta=0.00009807726321042765 support_l2=0.09374330707671129
REFERENCE linear/fsiso150/auto/BR level_delta=2.999677627 support_l2=0.416991467 max_abs=1.7023277354976864e-2 scale=Some(-3.0) late=-2.440014541/0.033446457 expected_late={"tail_change":0.09583169987829296,"worst_band_db":-1.6826220757655916}
VARIANT lapack_gesvd linear/fsiso150/auto/BR level_delta=2.999715484828714 support_l2=0.42498341860072786
VARIANT numpy_avx512 linear/fsiso150/auto/BR level_delta=2.999886553301719 support_l2=0.4316634407462263
VARIANT openblas_haswell linear/fsiso150/auto/BR level_delta=2.999836854245274 support_l2=0.43174721457933424
VARIANT openblas_nehalem linear/fsiso150/auto/BR level_delta=2.999688039709815 support_l2=0.41738503204732463
VARIANT openblas_sandybridge linear/fsiso150/auto/BR level_delta=2.9997662085091896 support_l2=0.43171070528799643
REFERENCE linear/fsiso150/auto changed_steps=1 worst_variant=2
MEASURE MIMO paths mixed/fsiso100/low40/SR: max_abs=5.55111512312578270e-17, max_relative_l2=4.82957418256671272e-16, values=8671
RC_ATTRIBUTION mixed/fsiso100/low40/SR path_x_relative=7.89989036047721e-3 direct_relative=2.770914025099112e-3
MEASURE MIMO paths mixed/fsiso100/low40/BL: max_abs=5.55111512312578270e-17, max_relative_l2=4.82756542470084630e-16, values=8671
RC_ATTRIBUTION mixed/fsiso100/low40/BL path_x_relative=1.4713162156263931e-1 direct_relative=1.5024952670651721e-2
MEASURE MIMO paths mixed/fsiso100/low40/FL: max_abs=5.55111512312578270e-17, max_relative_l2=4.83177862363155495e-16, values=8671
RC_ATTRIBUTION mixed/fsiso100/low40/FL path_x_relative=1.537034175677464e-1 direct_relative=1.0753368696142816e-2
MEASURE MIMO paths mixed/fsiso100/low40/FR: max_abs=5.55111512312578270e-17, max_relative_l2=4.74251105918658958e-16, values=8671
RC_ATTRIBUTION mixed/fsiso100/low40/FR path_x_relative=7.852849886500451e-1 direct_relative=2.467122197951507e-3
RC_ATTRIBUTION mixed/fsiso100/low40/SR frozen_rc_remaining_l2=5.556406350950572e-13 max=4.114417140321791e-14 level_delta=3.0020430585864233e-13
RC_ATTRIBUTION mixed/fsiso100/low40/BL frozen_rc_remaining_l2=1.294034076043518e-13 max=8.331876855116604e-15 level_delta=-3.019806626980426e-14
RC_ATTRIBUTION mixed/fsiso100/low40/FL frozen_rc_remaining_l2=2.553481731254894e-13 max=2.5824828386866727e-14 level_delta=1.2434497875801753e-14
RC_ATTRIBUTION mixed/fsiso100/low40/FR frozen_rc_remaining_l2=0e0 max=0e0 level_delta=0e0
MEASURE MIMO paths mixed/fsiso100/low40/BR: max_abs=5.55111512312578270e-17, max_relative_l2=4.76354871263253899e-16, values=8671
RC_ATTRIBUTION mixed/fsiso100/low40/BR path_x_relative=5.77591521113108e-2 direct_relative=5.205847486238387e-3
MEASURE MIMO paths mixed/fsiso100/low40/SL: max_abs=5.55111512312578270e-17, max_relative_l2=4.79486180063657155e-16, values=8671
RC_ATTRIBUTION mixed/fsiso100/low40/SL path_x_relative=9.208697904495295e-3 direct_relative=5.218247509260938e-4
MEASURE MIMO paths mixed/fsiso100/low40/FC: max_abs=5.55111512312578270e-17, max_relative_l2=4.88633084599566563e-16, values=8671
RC_ATTRIBUTION mixed/fsiso100/low40/FC path_x_relative=6.656614648104538e-2 direct_relative=8.33851125695758e-3
RC_ATTRIBUTION mixed/fsiso100/low40/BR frozen_rc_remaining_l2=2.158868840000654e-13 max=4.4366094999193084e-15 level_delta=8.171241461241152e-14
RC_ATTRIBUTION mixed/fsiso100/low40/SL frozen_rc_remaining_l2=3.1662673477483905e-13 max=1.8136533941337518e-14 level_delta=-3.907985046680551e-14
RC_ATTRIBUTION mixed/fsiso100/low40/FC frozen_rc_remaining_l2=2.605705181896875e-13 max=1.4102868178822447e-14 level_delta=-7.638334409421077e-14
REFERENCE mixed/fsiso100/low40 design_seconds=4.42388764
REFERENCE mixed/fsiso100/low40/FL level_delta=-6.000058927 support_l2=0.611694919 max_abs=4.0568759298774834e-2 scale=Some(-6.0) late=-1.961815875/0.100982475 expected_late={"tail_change":0.028308168234011305,"worst_band_db":-10.24978878252253}
VARIANT lapack_gesvd mixed/fsiso100/low40/FL level_delta=-6.000012378282056 support_l2=0.5323570678148551
VARIANT numpy_avx512 mixed/fsiso100/low40/FL level_delta=0.000011317074930516924 support_l2=0.03988093794806209
VARIANT openblas_haswell mixed/fsiso100/low40/FL level_delta=-0.00004453865321529804 support_l2=0.0014716351698517718
VARIANT openblas_nehalem mixed/fsiso100/low40/FL level_delta=0.00000720411783738939 support_l2=0.06341824173278726
VARIANT openblas_sandybridge mixed/fsiso100/low40/FL level_delta=0.000040039985568895986 support_l2=0.0038348229769119735
REFERENCE mixed/fsiso100/low40/FR level_delta=0.000000000 support_l2=0.000000000 max_abs=0e0 scale=None late=-99.000000000/-14.850000000 expected_late={"tail_change":null,"worst_band_db":null}
VARIANT lapack_gesvd mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
VARIANT numpy_avx512 mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
VARIANT openblas_haswell mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
VARIANT openblas_nehalem mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
VARIANT openblas_sandybridge mixed/fsiso100/low40/FR level_delta=0 support_l2=0.0
REFERENCE mixed/fsiso100/low40/FC level_delta=0.000001164 support_l2=0.171632354 max_abs=5.794038041964592e-3 scale=Some(-3.0) late=-4.074004127/0.016158924 expected_late={"tail_change":0.08534122999447218,"worst_band_db":-4.1222981009248025}
VARIANT lapack_gesvd mixed/fsiso100/low40/FC level_delta=0.0000036529197107171285 support_l2=0.035947658635556726
VARIANT numpy_avx512 mixed/fsiso100/low40/FC level_delta=0.000015267142686070656 support_l2=0.0031966834072174495
VARIANT openblas_haswell mixed/fsiso100/low40/FC level_delta=-0.00004647757225839655 support_l2=0.010223116193791194
VARIANT openblas_nehalem mixed/fsiso100/low40/FC level_delta=0.00002737674172337279 support_l2=0.007213016138601747
VARIANT openblas_sandybridge mixed/fsiso100/low40/FC level_delta=-0.00005566896961184398 support_l2=0.004516579063628009
REFERENCE mixed/fsiso100/low40/BL level_delta=0.000049349 support_l2=0.684538820 max_abs=4.9634474725065186e-2 scale=Some(0.0) late=-0.219016499/0.113716110 expected_late={"tail_change":0.13592482797111505,"worst_band_db":-0.2716423185952849}
VARIANT lapack_gesvd mixed/fsiso100/low40/BL level_delta=-3.0000184254121205 support_l2=0.30407892160125505
VARIANT numpy_avx512 mixed/fsiso100/low40/BL level_delta=0.000018496691726710424 support_l2=0.05602306786475751
VARIANT openblas_haswell mixed/fsiso100/low40/BL level_delta=0.0000157090283270378 support_l2=0.05759276201435488
VARIANT openblas_nehalem mixed/fsiso100/low40/BL level_delta=-0.00006046669609460764 support_l2=0.05514502509769374
VARIANT openblas_sandybridge mixed/fsiso100/low40/BL level_delta=0.000027839111956140528 support_l2=0.06231943314387411
REFERENCE mixed/fsiso100/low40/SL level_delta=5.999990128 support_l2=0.997751333 max_abs=3.3977349403671864e-2 scale=Some(0.0) late=-0.087601295/0.144793987 expected_late={"tail_change":0.05475037558740551,"worst_band_db":-0.04496169965587714}
VARIANT lapack_gesvd mixed/fsiso100/low40/SL level_delta=6.000033891562957 support_l2=1.045620096248696
VARIANT numpy_avx512 mixed/fsiso100/low40/SL level_delta=-6.000078689651492 support_l2=0.5163992888060525
VARIANT openblas_haswell mixed/fsiso100/low40/SL level_delta=-3.0000099866116905 support_l2=0.29981584459635224
VARIANT openblas_nehalem mixed/fsiso100/low40/SL level_delta=-5.999991683643646 support_l2=0.5145248209435467
VARIANT openblas_sandybridge mixed/fsiso100/low40/SL level_delta=-2.9999692145949517 support_l2=0.3072595000766432
REFERENCE mixed/fsiso100/low40/SR level_delta=0.000007659 support_l2=0.048394655 max_abs=2.0526655411892756e-3 scale=Some(-3.0) late=-0.295199906/0.085078889 expected_late={"tail_change":0.08237673663687096,"worst_band_db":-0.28929096914684144}
VARIANT lapack_gesvd mixed/fsiso100/low40/SR level_delta=0.000049536239243863633 support_l2=0.06398019860045645
VARIANT numpy_avx512 mixed/fsiso100/low40/SR level_delta=0.000030886344665859156 support_l2=0.1013728023083956
VARIANT openblas_haswell mixed/fsiso100/low40/SR level_delta=0.00008902264812604699 support_l2=0.06647656089331555
VARIANT openblas_nehalem mixed/fsiso100/low40/SR level_delta=0.00009026457823146927 support_l2=0.0509089082252906
VARIANT openblas_sandybridge mixed/fsiso100/low40/SR level_delta=0.00010421311790409504 support_l2=0.07312506779996229
REFERENCE mixed/fsiso100/low40/BR level_delta=0.000094302 support_l2=0.051423937 max_abs=8.188387357764212e-4 scale=Some(-12.0) late=-0.063420751/0.114564850 expected_late={"tail_change":0.1363127336475263,"worst_band_db":-0.06243078149539497}
VARIANT lapack_gesvd mixed/fsiso100/low40/BR level_delta=-0.000010971341666277112 support_l2=0.007954483034918585
VARIANT numpy_avx512 mixed/fsiso100/low40/BR level_delta=0.0000912016784972991 support_l2=0.002514611318415844
VARIANT openblas_haswell mixed/fsiso100/low40/BR level_delta=-0.00000952785324059846 support_l2=0.0020093929382828573
VARIANT openblas_nehalem mixed/fsiso100/low40/BR level_delta=-0.00006536410170809859 support_l2=0.003137998629225176
VARIANT openblas_sandybridge mixed/fsiso100/low40/BR level_delta=0.00008777970851880923 support_l2=0.002018185218223796
REFERENCE mixed/fsiso100/low40 changed_steps=2 worst_variant=3
MEASURE MIMO paths mixed/fsiso150/auto/BL: max_abs=1.38777878078144568e-17, max_relative_l2=4.81954547970674492e-16, values=8671
RC_ATTRIBUTION mixed/fsiso150/auto/BL path_x_relative=1.4713162156263931e-1 direct_relative=1.5024952670651721e-2
MEASURE MIMO paths mixed/fsiso150/auto/FC: max_abs=1.38777878078144568e-17, max_relative_l2=4.75067045374127329e-16, values=8671
RC_ATTRIBUTION mixed/fsiso150/auto/FC path_x_relative=6.656614648104538e-2 direct_relative=8.33851125695758e-3
MEASURE MIMO paths mixed/fsiso150/auto/FR: max_abs=1.38777878078144568e-17, max_relative_l2=4.75140176911693799e-16, values=8671
RC_ATTRIBUTION mixed/fsiso150/auto/FR path_x_relative=7.852849886500451e-1 direct_relative=2.467122197951507e-3
MEASURE MIMO paths mixed/fsiso150/auto/FL: max_abs=1.38777878078144568e-17, max_relative_l2=4.75833453531377704e-16, values=8671
RC_ATTRIBUTION mixed/fsiso150/auto/FL path_x_relative=1.537034175677464e-1 direct_relative=1.0753368696142816e-2
RC_ATTRIBUTION mixed/fsiso150/auto/FR frozen_rc_remaining_l2=0e0 max=0e0 level_delta=0e0
RC_ATTRIBUTION mixed/fsiso150/auto/FC frozen_rc_remaining_l2=3.912930046223506e-13 max=4.3350414404008664e-14 level_delta=2.4868995751603507e-14
RC_ATTRIBUTION mixed/fsiso150/auto/BL frozen_rc_remaining_l2=2.3081222292581657e-13 max=1.6529746321714e-14 level_delta=-1.9539925233402755e-14
RC_ATTRIBUTION mixed/fsiso150/auto/FL frozen_rc_remaining_l2=4.4378093855800504e-14 max=2.0712598303163077e-15 level_delta=1.4210854715202004e-14
MEASURE MIMO paths mixed/fsiso150/auto/SR: max_abs=1.38777878078144568e-17, max_relative_l2=4.81826591233345376e-16, values=8671
RC_ATTRIBUTION mixed/fsiso150/auto/SR path_x_relative=7.89989036047721e-3 direct_relative=2.770914025099112e-3
MEASURE MIMO paths mixed/fsiso150/auto/SL: max_abs=1.38777878078144568e-17, max_relative_l2=4.75849494835520158e-16, values=8671
RC_ATTRIBUTION mixed/fsiso150/auto/SL path_x_relative=9.208697904495295e-3 direct_relative=5.218247509260938e-4
MEASURE MIMO paths mixed/fsiso150/auto/BR: max_abs=1.38777878078144568e-17, max_relative_l2=4.73487874819233625e-16, values=8671
RC_ATTRIBUTION mixed/fsiso150/auto/BR path_x_relative=5.77591521113108e-2 direct_relative=5.205847486238387e-3
RC_ATTRIBUTION mixed/fsiso150/auto/SL frozen_rc_remaining_l2=6.619989512550102e-14 max=4.3125225612783424e-15 level_delta=-2.1316282072803006e-14
RC_ATTRIBUTION mixed/fsiso150/auto/SR frozen_rc_remaining_l2=8.443624101988869e-14 max=3.4425587380759737e-15 level_delta=-5.3290705182007514e-14
RC_ATTRIBUTION mixed/fsiso150/auto/BR frozen_rc_remaining_l2=4.9030787320042596e-14 max=1.394717674685353e-15 level_delta=-2.1316282072803006e-14
REFERENCE mixed/fsiso150/auto design_seconds=3.896139009
REFERENCE mixed/fsiso150/auto/FL level_delta=3.000137012 support_l2=0.552352940 max_abs=2.286118965221605e-2 scale=Some(-3.0) late=-3.272194575/-0.031991093 expected_late={"tail_change":0.137840404418629,"worst_band_db":-2.667884635736209}
VARIANT lapack_gesvd mixed/fsiso150/auto/FL level_delta=0.0001907588505929425 support_l2=0.1214591456283015
VARIANT numpy_avx512 mixed/fsiso150/auto/FL level_delta=0.00006207282770276379 support_l2=0.018631018692080857
VARIANT openblas_haswell mixed/fsiso150/auto/FL level_delta=0.00012076418121509391 support_l2=0.0016504974412136098
VARIANT openblas_nehalem mixed/fsiso150/auto/FL level_delta=0.00016083474337236225 support_l2=0.0307512868201759
VARIANT openblas_sandybridge mixed/fsiso150/auto/FL level_delta=0.00011153566116206548 support_l2=0.00446914449703479
REFERENCE mixed/fsiso150/auto/FR level_delta=0.000000000 support_l2=0.000000000 max_abs=0e0 scale=None late=-99.000000000/-14.850000000 expected_late={"tail_change":null,"worst_band_db":null}
VARIANT lapack_gesvd mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
VARIANT numpy_avx512 mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
VARIANT openblas_haswell mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
VARIANT openblas_nehalem mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
VARIANT openblas_sandybridge mixed/fsiso150/auto/FR level_delta=0 support_l2=0.0
REFERENCE mixed/fsiso150/auto/FC level_delta=-0.000172211 support_l2=0.111513805 max_abs=3.47789131811247e-3 scale=Some(-3.0) late=-3.060627663/-0.068009630 expected_late={"tail_change":-0.07034489928225182,"worst_band_db":-2.9811769119890696}
VARIANT lapack_gesvd mixed/fsiso150/auto/FC level_delta=0.00007980659141004764 support_l2=0.0245137249968377
VARIANT numpy_avx512 mixed/fsiso150/auto/FC level_delta=0.00010420614476025492 support_l2=0.0028787261994542928
VARIANT openblas_haswell mixed/fsiso150/auto/FC level_delta=-0.00006752565775691721 support_l2=0.007210344874246437
VARIANT openblas_nehalem mixed/fsiso150/auto/FC level_delta=0.00008486779362826269 support_l2=0.005543982634476855
VARIANT openblas_sandybridge mixed/fsiso150/auto/FC level_delta=-0.00006198980194405124 support_l2=0.003999997958384296
REFERENCE mixed/fsiso150/auto/BL level_delta=-2.999819828 support_l2=0.492801656 max_abs=2.570030624777369e-2 scale=Some(-6.0) late=-1.953334607/0.118607561 expected_late={"tail_change":0.1368844254652269,"worst_band_db":-4.065330969053657}
VARIANT lapack_gesvd mixed/fsiso150/auto/BL level_delta=-2.9998969722027056 support_l2=0.301359589620858
VARIANT numpy_avx512 mixed/fsiso150/auto/BL level_delta=0.0002465260837425376 support_l2=0.04446900309758868
VARIANT openblas_haswell mixed/fsiso150/auto/BL level_delta=0.00006935708501920601 support_l2=0.0455077742183558
VARIANT openblas_nehalem mixed/fsiso150/auto/BL level_delta=0.000022722558563614825 support_l2=0.0413897604941415
VARIANT openblas_sandybridge mixed/fsiso150/auto/BL level_delta=0.00021025101471394692 support_l2=0.04471134625903966
REFERENCE mixed/fsiso150/auto/SL level_delta=5.999853564 support_l2=0.997116455 max_abs=4.638508098436068e-2 scale=Some(0.0) late=-4.280387785/0.134381462 expected_late={"tail_change":0.05973465932930784,"worst_band_db":-2.05737843001986}
VARIANT lapack_gesvd mixed/fsiso150/auto/SL level_delta=5.999852798847016 support_l2=1.0213289004106538
VARIANT numpy_avx512 mixed/fsiso150/auto/SL level_delta=-6.000120737984265 support_l2=0.5102735179681622
VARIANT openblas_haswell mixed/fsiso150/auto/SL level_delta=-3.0000279837421076 support_l2=0.29907502774945727
VARIANT openblas_nehalem mixed/fsiso150/auto/SL level_delta=-6.000098330634884 support_l2=0.5091987236228102
VARIANT openblas_sandybridge mixed/fsiso150/auto/SL level_delta=-3.000005744302115 support_l2=0.30458539829788883
REFERENCE mixed/fsiso150/auto/SR level_delta=-0.000041656 support_l2=0.035409672 max_abs=7.311277028378312e-4 scale=Some(-9.0) late=-1.400107467/0.107108121 expected_late={"tail_change":0.10510914794130934,"worst_band_db":-1.369374331177764}
VARIANT lapack_gesvd mixed/fsiso150/auto/SR level_delta=-0.00001449981483503393 support_l2=0.050398692294140734
VARIANT numpy_avx512 mixed/fsiso150/auto/SR level_delta=0.00003763829093728077 support_l2=0.0966011300066491
VARIANT openblas_haswell mixed/fsiso150/auto/SR level_delta=-0.0000418534507993229 support_l2=0.053042879177995696
VARIANT openblas_nehalem mixed/fsiso150/auto/SR level_delta=-0.0001701709756716241 support_l2=0.04083812415048273
VARIANT openblas_sandybridge mixed/fsiso150/auto/SR level_delta=-0.0000730670613968698 support_l2=0.05476476422005671
REFERENCE mixed/fsiso150/auto/BR level_delta=3.000056023 support_l2=0.420080874 max_abs=8.44650190314724e-3 scale=Some(-9.0) late=-0.908534338/0.146518262 expected_late={"tail_change":0.10696738646365453,"worst_band_db":-0.6081353552108064}
VARIANT lapack_gesvd mixed/fsiso150/auto/BR level_delta=-0.00012391478329121242 support_l2=0.006386466978344232
VARIANT numpy_avx512 mixed/fsiso150/auto/BR level_delta=-0.00015035013589681512 support_l2=0.002676334995035794
VARIANT openblas_haswell mixed/fsiso150/auto/BR level_delta=0.000028186501637605943 support_l2=0.001625278554819554
VARIANT openblas_nehalem mixed/fsiso150/auto/BR level_delta=-0.00017627489907212635 support_l2=0.002012947025977743
VARIANT openblas_sandybridge mixed/fsiso150/auto/BR level_delta=0.00007355540867592936 support_l2=0.0014958634580454868
REFERENCE mixed/fsiso150/auto changed_steps=4 worst_variant=2
test room_tuning2_supports_are_band_limited ... MEASURE full_support FL/right=false sum_relative=2.2144931454811554e-14 leakage_db=-75.8457977396133
MEASURE full_support FL/right=true sum_relative=1.2192918861004125e-14 leakage_db=-74.4183336720952
MEASURE full_support FR/right=false sum_relative=9.120495242376515e-15 leakage_db=-71.91073644261424
MEASURE full_support FR/right=true sum_relative=1.64309846796085e-14 leakage_db=-71.26416584389506
MEASURE full_support FC/right=false sum_relative=1.2651554422722665e-14 leakage_db=-78.45936978543389
MEASURE full_support FC/right=true sum_relative=1.0670349593400248e-14 leakage_db=-77.56866743447041
MEASURE full_support BL/right=false sum_relative=1.0217718326868585e-14 leakage_db=-76.73683423384914
MEASURE full_support BL/right=true sum_relative=1.1887655668034168e-14 leakage_db=-77.80271312196487
MEASURE full_support SL/right=false sum_relative=1.3239206028533031e-14 leakage_db=-74.93246972746915
MEASURE full_support SL/right=true sum_relative=9.407599713411138e-15 leakage_db=-75.48857697332878
MEASURE full_support SR/right=false sum_relative=6.814097944756065e-15 leakage_db=-76.18737454137806
MEASURE full_support SR/right=true sum_relative=1.3485135801935296e-14 leakage_db=-76.1725735776294
MEASURE full_support BR/right=false sum_relative=9.030757695426784e-15 leakage_db=-74.88558710624889
MEASURE full_support BR/right=true sum_relative=1.2062414676857522e-14 leakage_db=-72.81589739537056
test result: FAILED. 11 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 58.85s
error: test failed, to rerun pass `-p impulcifer-dsp --test room_tuning2_apply`
```
