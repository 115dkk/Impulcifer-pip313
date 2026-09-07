# P10 stage oracle fixtures

Run `py -3.14 E:/Impulcifer/tests/migration/export_goldens_stages.py` in the foreground.
The exporter calls the repository's actual 2.x objects on a TemporaryDirectory
copy of data/demo. It does not write the original demo, existing exporters, or
other fixture families. Python/NumPy/SciPy versions are recorded per fixture.

## Protocol and encoding

`p10_protocol.json` records actual filesystem enumeration order. This matters
for the first specific room FR's reference gain. `p10_manifest.json` indexes
the original ten scenario families plus optional output matrices. Per-track JSON stores length, first peak, absolute
argmax, peak amplitude, RMS, first/last 256 samples and diagnostic SHA-256.
Full requested FL/FC arrays and FIRs use headerless little-endian float64
sidecars when longer than 4096 samples; identical bytes share a sidecar.
Nonfinite numbers use `nan`, `+inf`, `-inf`. JSON files stay below 200 kB;
the full family stays below 25 MB. Hashes are provenance, not numeric gates.

The default family includes room errors/targets, room response track matrices,
headphone errors, target raw, all 14 EQ FIRs, crop/equalize/normalize snapshots,
FL full post-EQ responses, final layout names and summaries, applied gain,
README numeric locals captured from Python write_readme, and exact stage rows.
Other families cover virtual bass, all seven balance methods, mic matching,
decay, 44100 Hz resampling with discarded second gain, six independent options,
synthetic two-track generic room, real-filesystem headphone resolution, and EQ
CSV detection/parsing/precedence. Generic room synthesis uses the first demo FL
recording track and a 17-sample circular shift. Both one-column and two-column
recordings are tested; the second column is shifted by 31 samples and scaled
by -0.5. Captured Python boundaries are asserted against Rust's retained full
recordings, with separate oracle recording/estimated-IR summaries and exact
peak/length decisions. Average/conservative curves and a real unlimited
`generic_limit=0` variant run after `room.wav` exists, including specific-room
curves and generic fallback copies through `room_correction`.

`p10_outputs.json` captures actual Python output-stage calls before PCM
quantization. Ten cases cover 7/8/9/10/13 synthetic speakers with compaction
off/on, both TrueHD minimum thresholds, a present but silent TFR-left ear,
missing layout placeholders, reversed ingestion order, Hangloose speaker
ordering and independently normalized JamesDSP FL/FR subsets. Full samples
are asserted for every output track with `1e-9 * reference_peak`, including
exact silence; filenames, counts, routing names and fs are exact. Identical
tracks reuse fixture files. The exporter inspects every prior owned JSON/f64
before regeneration, deletes nothing, and counts all `p10_*` files (including
stale files) against the 25 MB cap. The corrective export has 795 files totaling
22,522,909 bytes, with no stale files.

## File-resolution snapshot contract

`resolve_headphone_file(requested, dir_listing)` performs no I/O. The caller
supplies an ordered snapshot of EXISTING entries, not merely basenames.
Ordinary files are relative to the measurement root. External files are
absolute. Resolved directories have explicit trailing-slash entries, including
empty directories, and their contained entries carry the directory prefix.
Separators and dot components are normalized lexically. Relative/default
results remain measurement-relative; external results remain absolute.

Python calls isdir(requested) before joining the measurement root. P11 must
resolve that original cwd context before constructing the snapshot: only
actually resolved directories may have directory markers. Enumeration order
is preserved when choosing the first case-insensitive .wav. Named fallback
priority is headphones.wav, headphone.wav, hp.wav, compensation.wav. Missing
requests and empty directories fall back to measurement-root headphones.wav.
The oracle creates actual temporary files, calls the original Python branch,
then converts temporary roots to this virtual convention. Absolute external
fixture paths use /external as a stable virtual absolute root. A snapshot
cannot reproduce filesystem races or omitted metadata.

## Numeric assertions

FR dB arrays use atol 1e-9 plus rtol 1e-11. Virtual-bass samples use
1e-9 times reference peak. Minimum-phase FIR taps and post-EQ tracks use
1e-3 times reference peak, with FIR/full-track spectrum budgets 0.02 dB below
20 kHz, 0.05 dB from 20 to 23 kHz, 0.3 dB from 23 to 23.9 kHz and 20 dB
above 23.9 kHz, as documented in README-fr.md. Track lengths, peak indices,
argmax and stage rows are exact. Applied gains use 1e-6 dB. README numbers
use 1e-6 dB/ms and exact ITD. No failing budget is widened or skipped.
Numeric failures accumulate per scenario and fail at its end, so later
measurements are still reported. Shape and missing-data failures remain fatal.
Run golden_stages with `-- --nocapture` for every MEASURE line.

## Preserved behavior

The target shelf uses 44100 Hz even in a 48000 Hz project. Room limiting uses
a full Hann across one octave. Specific room curves share the first FR's
reference gain; generic conservative curves use sign agreement (zeros count
as non-positive). Mids balance uses the caller grid without center's re-grid.
Virtual bass treats center speakers like non-left speakers and uses explicit
polarity only; crossover at Nyquist returns unchanged, as Python does.
Mic auto/frontal mode averages the powers of every present FC/TFC/BC recording
before taking the dB ratio. If none is present it falls back to diffuse; forced
diffuse always uses every recording. Matching skips below 0.05 dB.
The resample stage normalizes twice but returns only the first gain. README
statistics are calculated before resampling. The driver reports file/plot
stage keys but performs no file or plot work. JamesDSP independently normalizes
its FL/FR subset. Existing P08 ingestion retains the i // 2 rule.

## Remaining acceptance failures

The implementation uses the existing P09 minimum-phase/FIR and P08 decay
primitives without modifying them. Their documented composition noise passes
the FIR and post-EQ sample budgets but can exceed the stricter downstream
gain/README/decay-decision budgets. The tests retain those strict assertions;
a failing suite is not a completed parity gate. See actual command output for
measured failures. The diagnostic test `golden_frozen_eq_firs_isolate_downstream_noise` applies
Python's frozen EQ FIRs to Rust's cropped inputs: normalization agrees within
1.87e-14 dB, decay integer decisions exactly, decay levels within 5.69e-13 dB,
README PNR/length within 2.86e-10 and reverb within 1.29e-11 ms. This isolates
the strict downstream failures to the existing minimum-phase composition,
not the downstream stage arithmetic. The normal end-to-end gates still fail.
Summary-only tracks do not claim full-sample verification.
The corrective run still fails exactly three strict golden tests:
`golden_default_pipeline_matches_python`, `golden_decay_matches_python` and
`golden_option_variants_match_python`. Default gain differs by 2.681868335e-6 dB;
option gain differs by up to 1.662263296e-4 dB. README reflection differences
reach 5.189248804e-5 dB, row PNR reaches 0.028390178 dB, length reaches
0.041666667 ms, and reverb reaches 0.003743054 ms. Decay level differs by up to
0.004039602 dB and the FR-left integer decision differs by 2 samples. These
still exceed the unchanged 1e-6/exact budgets. The frozen-FIR diagnostic passes.

The default golden now checks `PipelineOutputs.readme` itself, including exact
reverb presence, reflection count/key set, fs and applied gain. Observer tests
check enabled stage order, accumulated steps (including zero-step rows), totals,
pre-resample README capture, and cancellation before/after every default stage.
Mic tests cover multiple central recordings, TFC/BC without FC, forced diffuse,
and absent-center fallback. Four private virtual-bass tests pin delay/advance
bounds, FFT-bin ties/endpoints, SOS duplication order and RBJ shelf endpoint gains.
The new optional output and generic-room goldens pass; their observed maximum
errors are 2.776e-16 samples and 2.878e-12 dB respectively.

Foreground verification: fmt and clippy pass; golden_stages has 12 passes and
3 failures, properties_stages has 9 passes, private virtual-bass helpers have
4 passes, and policy has 4 passes. `cargo test -p impulcifer-dsp` exits 101 at
the same golden_stages failures; the ordinary fail-fast cargo run does not run
later test binaries. A subsequent `--no-fail-fast` run completed every binary
and doc-tests: 138 tests passed and the same 3 failed. No primitive, tolerance
or feature registry was changed.
Filesystem races and CSV byte decoding belong to P11.

## Parent verification transcript

Foreground rerun on 2026-09-07. Output below is the captured merged stdout/stderr, without filtering.

### `py -3.14 E:/Impulcifer/tests/migration/export_goldens_stages.py`

Exit code: 0

```text
E:\Impulcifer\core\hrir.py:616: UserWarning: Warning: SL measurement has lower delay to right ear than to left ear. SL should be at the left side of the head so the sound should arrive first in the left ear. This is usually a problem with the measurement process or the speaker order given is not correct. Detected delay difference is 0.0000 milliseconds.
  warnings.warn(
E:\Impulcifer\core\hrir.py:616: UserWarning: Warning: BL measurement has lower delay to right ear than to left ear. BL should be at the left side of the head so the sound should arrive first in the left ear. This is usually a problem with the measurement process or the speaker order given is not correct. Detected delay difference is 0.0000 milliseconds.
  warnings.warn(
E:\Impulcifer\core\hrir.py:616: UserWarning: Warning: FL measurement has lower delay to right ear than to left ear. FL should be at the left side of the head so the sound should arrive first in the left ear. This is usually a problem with the measurement process or the speaker order given is not correct. Detected delay difference is 0.0000 milliseconds.
  warnings.warn(
E:\Impulcifer\core\hrir.py:616: UserWarning: Warning: TFL measurement has lower delay to right ear than to left ear. TFL should be at the left side of the head so the sound should arrive first in the left ear. This is usually a problem with the measurement process or the speaker order given is not correct. Detected delay difference is 0.0000 milliseconds.
  warnings.warn(
E:\Impulcifer\core\hrir.py:600: UserWarning: Warning: TFR measurement has lower delay to left ear than to right ear. TFR should be at the right side of the head so the sound should arrive first in the right ear. This is usually a problem with the measurement process or the speaker order given is not correct. Detected delay difference is 1.0000 milliseconds.
  warnings.warn(
E:\Impulcifer\core\hrir.py:616: UserWarning: Warning: TSL measurement has lower delay to right ear than to left ear. TSL should be at the left side of the head so the sound should arrive first in the left ear. This is usually a problem with the measurement process or the speaker order given is not correct. Detected delay difference is 0.0000 milliseconds.
  warnings.warn(
E:\Impulcifer\core\hrir.py:616: UserWarning: Warning: TBL measurement has lower delay to right ear than to left ear. TBL should be at the left side of the head so the sound should arrive first in the left ear. This is usually a problem with the measurement process or the speaker order given is not correct. Detected delay difference is 0.0000 milliseconds.
  warnings.warn(
Inspected 795 existing P10 files, 22522909 bytes; no deletion.
헤드폰 보상 파일이 지정되지 않았습니다. 기본값 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\demo\headphones.wav
헤드폰 보상 파일 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\demo\headphones.wav
>>>>>>>>> Applied a normalization gain of -3.70 dB to all channels
가상 베이스 합성 중...
✓ 가상 베이스 합성 완료
>>>>>>>>> Applied a normalization gain of -29.49 dB to all channels

🎧 마이크 편차 보정 v4.0 시작 (방향 무관 양이 불일치)
  - 보정 강도: 0.7
  - 분석 대상 스피커: 7개
  - 추정 기준(anchor): frontal
  - 평균 보정량: 1.27 dB, 최대 보정량: 4.20 dB
✅ 마이크 편차 보정 v4.0 완료
>>>>>>>>> Applied a normalization gain of 0.74 dB to all channels
>>>>>>>>> Applied a normalization gain of -3.70 dB to all channels
>>>>>>>>> Applied a normalization gain of -3.71 dB to all channels
>>>>>>>>> Applied a normalization gain of -3.70 dB to all channels
>>>>>>>>> Applied a normalization gain of 1.17 dB to all channels
>>>>>>>>> Applied a normalization gain of -8.11 dB to all channels
>>>>>>>>> Applied a normalization gain of -7.73 dB to all channels
>>>>>>>>> Applied a normalization gain of -12.81 dB to all channels
>>>>>>>>> Applied a normalization gain of 19.40 dB to all channels
>>>>>>>>> Applied a normalization gain of -12.81 dB to all channels
>>>>>>>>> Applied a normalization gain of 19.40 dB to all channels
>>>>>>>>> Applied a normalization gain of -14.99 dB to all channels
>>>>>>>>> Applied a normalization gain of 21.58 dB to all channels
>>>>>>>>> Applied a normalization gain of -14.99 dB to all channels
>>>>>>>>> Applied a normalization gain of 21.58 dB to all channels
>>>>>>>>> Applied a normalization gain of -22.64 dB to all channels
>>>>>>>>> Applied a normalization gain of 23.52 dB to all channels
>>>>>>>>> Applied a normalization gain of -22.64 dB to all channels
>>>>>>>>> Applied a normalization gain of 23.52 dB to all channels
>>>>>>>>> Applied a normalization gain of -24.38 dB to all channels
>>>>>>>>> Applied a normalization gain of 25.26 dB to all channels
>>>>>>>>> Applied a normalization gain of -24.38 dB to all channels
>>>>>>>>> Applied a normalization gain of 25.26 dB to all channels
>>>>>>>>> Applied a normalization gain of -28.76 dB to all channels
>>>>>>>>> Applied a normalization gain of 29.64 dB to all channels
>>>>>>>>> Applied a normalization gain of -28.76 dB to all channels
>>>>>>>>> Applied a normalization gain of 29.64 dB to all channels
헤드폰 보상 파일이 지정되지 않았습니다. 기본값 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\default\headphones.wav
헤드폰 보상 파일 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\default\headphones.wav
헤드폰 보상 파일이 지정되지 않았습니다. 기본값 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\none_missing\headphones.wav
✗ 헤드폰 보상 파일을 찾을 수 없습니다: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\none_missing\headphones.wav
✗ 파일이 존재하는지 확인하거나 작업 디렉토리에 'headphones.wav'를 배치해주세요: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\none_missing
헤드폰 보상 파일 파라미터 제공됨: custom.wav
상대 경로 변환됨: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\explicit\custom.wav
헤드폰 보상 파일 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\explicit\custom.wav
헤드폰 보상 파일 파라미터 제공됨: nested\custom.wav
상대 경로 변환됨: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\relative\nested\custom.wav
헤드폰 보상 파일 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\relative\nested\custom.wav
헤드폰 보상 파일 파라미터 제공됨: missing.wav
상대 경로 변환됨: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\missing\missing.wav
⚠ 지정된 헤드폰 보상 파일을 찾을 수 없습니다: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\missing\missing.wav. 기본 'headphones.wav'를 시도합니다
헤드폰 보상 파일 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\missing\headphones.wav
헤드폰 보상 파일 파라미터 제공됨: missing.wav
상대 경로 변환됨: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\missing_all\missing.wav
⚠ 지정된 헤드폰 보상 파일을 찾을 수 없습니다: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\missing_all\missing.wav. 기본 'headphones.wav'를 시도합니다
✗ 헤드폰 보상 파일을 찾을 수 없습니다: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\missing_all\headphones.wav
✗ 파일이 존재하는지 확인하거나 작업 디렉토리에 'headphones.wav'를 배치해주세요: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\missing_all
헤드폰 보상 파일 파라미터 제공됨: hpdir
경로가 디렉토리입니다. 내부에서 헤드폰 보상 파일을 검색합니다...
확인 중: hpdir\headphones.wav
확인 중: hpdir\headphone.wav
헤드폰 보상 파일을 찾았습니다: hpdir\headphone.wav
헤드폰 보상 파일 사용: hpdir\headphone.wav
헤드폰 보상 파일 파라미터 제공됨: hpdir
경로가 디렉토리입니다. 내부에서 헤드폰 보상 파일을 검색합니다...
확인 중: hpdir\headphones.wav
확인 중: hpdir\headphone.wav
확인 중: hpdir\hp.wav
확인 중: hpdir\compensation.wav
표준 헤드폰 파일을 찾지 못했습니다. .wav 파일을 검색합니다...
첫 번째 WAV 파일 사용: hpdir\a.wav
헤드폰 보상 파일 사용: hpdir\a.wav
헤드폰 보상 파일 파라미터 제공됨: hpdir
경로가 디렉토리입니다. 내부에서 헤드폰 보상 파일을 검색합니다...
확인 중: hpdir\headphones.wav
확인 중: hpdir\headphone.wav
확인 중: hpdir\hp.wav
확인 중: hpdir\compensation.wav
표준 헤드폰 파일을 찾지 못했습니다. .wav 파일을 검색합니다...
⚠ 디렉토리에 WAV 파일이 없습니다: hpdir
⚠ 지정된 헤드폰 보상 파일을 찾을 수 없습니다: None. 기본 'headphones.wav'를 시도합니다
헤드폰 보상 파일 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\resolution\empty\headphones.wav
헤드폰 보상 파일 파라미터 제공됨: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\external\external.wav
절대 경로 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\external\external.wav
헤드폰 보상 파일 사용: C:\Users\32170336\AppData\Local\Temp\impulcifer-p10-n8we0kj_\external\external.wav
gain.csv에 error 열이 없어 값을 적용할 EQ 게인 곡선으로 해석합니다
eq.csv에 error 열이 없어 값을 적용할 EQ 게인 곡선으로 해석합니다
eq-left.csv에 error 열이 없어 값을 적용할 EQ 게인 곡선으로 해석합니다
eq-right.csv에 error 열이 없어 값을 적용할 EQ 게인 곡선으로 해석합니다
P10: 795 generated files, 22522909 total bytes; 0 retained previous files; demo unchanged.
```

### `git -C E:/Impulcifer status --short data/demo`

Exit code: 0

```text
```

### `cargo fmt -p impulcifer-dsp -- --check`

Exit code: 0

```text
```

### `cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings`

Exit code: 0

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
```

### `cargo test -p impulcifer-dsp --test golden_stages --test properties_stages`

Exit code: 101

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.22s
     Running tests\golden_stages.rs (target\debug\deps\golden_stages-61d7072be0159691.exe)

running 15 tests
test golden_headphone_resolution_matches_python ... ok
test golden_stage_table_matches_python ... ok
test golden_eq_files_match_python ... ok
test golden_mic_deviation_matches_python ... ok
test golden_frozen_eq_firs_isolate_downstream_noise ... ok
test golden_decay_matches_python ... FAILED
test golden_optional_outputs_match_python ... ok
test golden_vbass_matches_python ... ok
test golden_default_pipeline_matches_python ... FAILED
test golden_resample_matches_python ... ok
test pipeline_observer_reports_order_steps_and_totals ... ok
test pipeline_observer_cancels_before_and_after_each_stage ... ok
test golden_channel_balance_matches_python ... ok
test golden_option_variants_match_python ... FAILED
test golden_generic_room_matches_python ... ok

failures:

---- golden_decay_matches_python stdout ----
MEASURE decay FL left integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=2.60784890869558694e-4 atol=1.000e-6 rtol=0.000e0
MEASURE decay FL right integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=1.92238774786801514e-4 atol=1.000e-6 rtol=0.000e0
MEASURE decay FR left integer decisions: max_abs=2.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=4.03960188333485348e-3 atol=1.000e-6 rtol=0.000e0
MEASURE decay FR right integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=2.43536660704535279e-4 atol=1.000e-6 rtol=0.000e0
MEASURE p10_decay_BL_left.json first: max_abs=3.47446812693115448e-7 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_left.json last: max_abs=8.05061990852387955e-12 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_left.json metrics: max_abs=9.87008197395056186e-9 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json first: max_abs=3.17909414551155456e-7 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json last: max_abs=7.94428072918216920e-12 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json metrics: max_abs=2.52442277238335011e-8 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json first: max_abs=9.72028573874295088e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json last: max_abs=6.81845640410530016e-12 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json metrics: max_abs=3.21888374350820516e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json first: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json last: max_abs=2.23442380020497287e-11 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json metrics: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json first: max_abs=9.65501640057958577e-7 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json last: max_abs=1.19263798535294571e-11 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json metrics: max_abs=7.83273138571516370e-8 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json first: max_abs=7.48761591444491170e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json last: max_abs=6.49314803840152037e-12 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json metrics: max_abs=1.98156144957939429e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json first: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json last: max_abs=1.13663763418952697e-15 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json metrics: max_abs=6.04083438544671114e-9 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full spectrum: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full spectrum spectrum: [0.0004267872890852305, 0.00017882616176656402, 0.0026062707932367314, 0.9610001158230487]
MEASURE p10_decay_FL_right.json first: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json last: max_abs=7.53965568385600014e-16 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json metrics: max_abs=7.38835567726248144e-8 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full spectrum: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full spectrum spectrum: [0.0008463076062129979, 0.0006277953469730543, 0.00199813126403375, 0.8180305696078587]
MEASURE p10_decay_FR_left.json first: max_abs=2.97432114175835705e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_left.json last: max_abs=3.48021748189881979e-15 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_left.json metrics: max_abs=1.51622550934732425e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json first: max_abs=1.00510738071770203e-6 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json last: max_abs=1.83019246623658386e-15 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json metrics: max_abs=9.37961882774285272e-7 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json first: max_abs=5.97237837428321594e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json last: max_abs=1.07785909697223540e-11 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json metrics: max_abs=3.53357332368367527e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json first: max_abs=1.48468898467091215e-6 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json last: max_abs=1.45355823022822186e-11 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json metrics: max_abs=2.26347425580364359e-7 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json first: max_abs=2.64311893923702135e-7 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json last: max_abs=9.21507271658143857e-12 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json metrics: max_abs=3.58077786415111898e-8 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json first: max_abs=5.59837784871661248e-7 atol=3.225e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json last: max_abs=6.00185718690772740e-12 atol=3.225e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json metrics: max_abs=4.03125228851108908e-7 atol=3.225e-5 rtol=0.000e0

thread 'golden_decay_matches_python' (279468) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
decay level: (0, -76.83410434514367, -76.8338435602528, 0.0002607848908695587)
decay level: (0, -80.41540325085825, -80.41521101208346, 0.00019223877478680151)
decay FR left integer decisions: (1, 32035.0, 32037.0, 2.0)
decay level: (0, -67.69221944186546, -67.6962590437488, 0.0040396018833348535)
decay level: (0, -74.6506740089376, -74.6504304722769, 0.00024353666070453528)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- golden_default_pipeline_matches_python stdout ----
MEASURE p10_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_response_track_0.json first: max_abs=8.67361737988403547e-19 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_0.json last: max_abs=5.05572790392410515e-21 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_0.json metrics: max_abs=1.76182853028894471e-19 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json first: max_abs=8.67361737988403547e-19 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json last: max_abs=5.81838842869799667e-21 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json metrics: max_abs=8.67361737988403547e-19 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json first: max_abs=8.67361737988403547e-19 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json last: max_abs=8.09313511321882277e-21 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json metrics: max_abs=4.06575814682064163e-19 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json first: max_abs=8.67361737988403547e-19 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json last: max_abs=7.41732855276299916e-21 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json metrics: max_abs=8.67361737988403547e-19 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json first: max_abs=1.19262238973405488e-18 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json last: max_abs=3.61312491563162488e-21 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json metrics: max_abs=1.08420217248550443e-19 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json first: max_abs=8.67361737988403547e-19 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json last: max_abs=6.31278394106328439e-21 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json metrics: max_abs=4.33680868994201774e-19 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_6.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_6.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_6.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_8.json first: max_abs=8.67361737988403547e-19 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_8.json last: max_abs=7.05419626385222001e-21 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_8.json metrics: max_abs=4.33680868994201774e-19 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json first: max_abs=1.08420217248550443e-18 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json last: max_abs=6.31965987990513144e-21 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json metrics: max_abs=4.33680868994201774e-19 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json first: max_abs=8.67361737988403547e-19 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json last: max_abs=5.40955441094366274e-21 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json metrics: max_abs=8.67361737988403547e-19 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json first: max_abs=8.67361737988403547e-19 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json last: max_abs=5.39487195507084650e-21 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json metrics: max_abs=8.67361737988403547e-19 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json first: max_abs=8.67361737988403547e-19 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json last: max_abs=5.36013036934361933e-21 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json metrics: max_abs=4.33680868994201774e-19 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json first: max_abs=8.67361737988403547e-19 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json last: max_abs=6.59924692694805479e-21 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json metrics: max_abs=4.33680868994201774e-19 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json first: max_abs=6.50521303491302660e-19 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json last: max_abs=5.02264067942198404e-21 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json metrics: max_abs=4.33680868994201774e-19 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json first: max_abs=8.67361737988403547e-19 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json last: max_abs=5.67776772456398196e-21 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json metrics: max_abs=4.33680868994201774e-19 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_16.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_16.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_16.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hp left: max_abs=4.27746726927580312e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE hp right: max_abs=1.29034560814034194e-11 atol=1.000e-9 rtol=1.000e-11
MEASURE target: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_eq_fir_BL_left.json: max_abs=1.32302721180643790e-4 atol=1.976e-3 rtol=0.000e0
MEASURE p10_eq_fir_BL_left.json spectrum: [0.0006226827121341905, 0.0009545431245245161, 0.010417501663331982, 1.5026256266821214]
MEASURE p10_eq_fir_BL_right.json: max_abs=1.06078065116033127e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_BL_right.json spectrum: [0.0004591118649617285, 0.0007333779959614081, 0.00789401783376377, 1.0684894384779253]
MEASURE p10_eq_fir_SL_left.json: max_abs=1.32284168126717283e-4 atol=1.984e-3 rtol=0.000e0
MEASURE p10_eq_fir_SL_left.json spectrum: [0.0006248722565692669, 0.0009547972917607427, 0.010417247047074207, 1.5026229436688712]
MEASURE p10_eq_fir_SL_right.json: max_abs=1.83024355603633726e-4 atol=1.967e-3 rtol=0.000e0
MEASURE p10_eq_fir_SL_right.json spectrum: [0.0007620620766964043, 0.0012161554082112773, 0.013258575046099147, 2.069939503593794]
MEASURE p10_eq_fir_FC_left.json: max_abs=1.51722611429427889e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_FC_left.json spectrum: [0.0007207305696235993, 0.0011048972615980573, 0.012021071029254426, 1.6773820092062852]
MEASURE p10_eq_fir_FC_right.json: max_abs=1.62301489864402626e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_FC_right.json spectrum: [0.0006969537446813146, 0.0011129768202397326, 0.012011585188611796, 1.6774391452876771]
MEASURE p10_eq_fir_FL_left.json: max_abs=1.32301029750314658e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_FL_left.json spectrum: [0.0006228606342261547, 0.0009545734423037887, 0.010417576273806798, 1.502615587401929]
MEASURE p10_eq_fir_FL_right.json: max_abs=1.18940849385101854e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_FL_right.json spectrum: [0.0005008780007205574, 0.0007993418154331755, 0.008683589357074115, 1.3004488594904653]
MEASURE p10_eq_fir_FR_left.json: max_abs=1.18589743328900710e-4 atol=1.982e-3 rtol=0.000e0
MEASURE p10_eq_fir_FR_left.json spectrum: [0.000574030171701585, 0.0008785318090904153, 0.009503637657842146, 1.2432146265197166]
MEASURE p10_eq_fir_FR_right.json: max_abs=1.41563459210486409e-4 atol=1.969e-3 rtol=0.000e0
MEASURE p10_eq_fir_FR_right.json spectrum: [0.0006021172870354033, 0.0009617590559611309, 0.010408632505849821, 1.5027138745077955]
MEASURE p10_eq_fir_SR_left.json: max_abs=1.32300879598479249e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_SR_left.json spectrum: [0.0006228787593508592, 0.0009545898937101177, 0.010417690337127137, 1.5026033863741086]
MEASURE p10_eq_fir_SR_right.json: max_abs=1.41551388105981557e-4 atol=1.960e-3 rtol=0.000e0
MEASURE p10_eq_fir_SR_right.json spectrum: [0.0006023296367073721, 0.0009615342218767596, 0.010409270347015075, 1.5026783332473628]
MEASURE p10_eq_fir_BR_left.json: max_abs=1.18586325645653190e-4 atol=1.984e-3 rtol=0.000e0
MEASURE p10_eq_fir_BR_left.json spectrum: [0.0005744515380759603, 0.000878565954884845, 0.009503467940153752, 1.2432254394795874]
MEASURE p10_eq_fir_BR_right.json: max_abs=1.62314367494115208e-4 atol=1.969e-3 rtol=0.000e0
MEASURE p10_eq_fir_BR_right.json spectrum: [0.0006967260476274884, 0.0011132191392902856, 0.012010899985006443, 1.677475892442923]
MEASURE p10_default_crop_BL_left.json first: max_abs=5.20417042793042128e-18 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_left.json last: max_abs=3.13931586076125063e-20 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_left.json metrics: max_abs=5.20417042793042128e-18 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json first: max_abs=3.46944695195361419e-18 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json last: max_abs=3.29151709347100772e-20 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json metrics: max_abs=1.73472347597680709e-18 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json first: max_abs=1.04083408558608426e-17 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json last: max_abs=3.93605622677232689e-20 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json metrics: max_abs=3.46944695195361419e-18 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json first: max_abs=2.60208521396521064e-18 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json last: max_abs=2.58043127988979748e-20 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json metrics: max_abs=1.73472347597680709e-18 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json first: max_abs=8.67361737988403547e-18 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json last: max_abs=5.01867021248172951e-20 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json metrics: max_abs=3.57786716920216463e-18 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json first: max_abs=8.67361737988403547e-18 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json last: max_abs=3.08505281257777202e-20 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json metrics: max_abs=2.16840434497100887e-19 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json first: max_abs=9.54097911787243902e-18 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json last: max_abs=5.62813688781080030e-20 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json metrics: max_abs=3.46944695195361419e-18 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json first: max_abs=4.33680868994201774e-18 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json last: max_abs=2.54903977564341008e-20 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json metrics: max_abs=5.42101086242752217e-19 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json first: max_abs=2.60208521396521064e-18 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json last: max_abs=3.18547253894170958e-20 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json metrics: max_abs=1.95156391047390798e-18 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json first: max_abs=1.04083408558608426e-17 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json last: max_abs=3.06189175542628724e-20 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json metrics: max_abs=1.73472347597680709e-18 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json first: max_abs=3.46944695195361419e-18 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json last: max_abs=5.76511799724958168e-20 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json metrics: max_abs=6.50521303491302660e-19 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json first: max_abs=1.38777878078144568e-17 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json last: max_abs=3.47283508374263139e-20 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json metrics: max_abs=1.38777878078144568e-17 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json first: max_abs=2.60208521396521064e-18 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json last: max_abs=2.26713662288533825e-20 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json metrics: max_abs=3.46944695195361419e-18 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json first: max_abs=7.80625564189563192e-18 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json last: max_abs=5.48453833347159470e-20 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json metrics: max_abs=1.73472347597680709e-18 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json first: max_abs=3.47446812693115448e-7 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json last: max_abs=8.05061990852387955e-12 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json metrics: max_abs=9.87008197395056186e-9 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json first: max_abs=3.17909414551155456e-7 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json last: max_abs=7.94428072918216920e-12 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json metrics: max_abs=2.52442277238335011e-8 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json first: max_abs=9.72028573874295088e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json last: max_abs=6.81845640410530016e-12 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json metrics: max_abs=3.21888374350820516e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json first: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json last: max_abs=2.23442380020497287e-11 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json metrics: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json first: max_abs=9.65501640057958577e-7 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json last: max_abs=1.19263798535294571e-11 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json metrics: max_abs=7.83273138571516370e-8 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json first: max_abs=7.48761591444491170e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json last: max_abs=6.49314803840152037e-12 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json metrics: max_abs=1.98156144957939429e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json first: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json last: max_abs=7.89445292482472558e-12 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json metrics: max_abs=6.08208228550753682e-9 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full spectrum: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full spectrum spectrum: [0.0006373479341870772, 0.0009437956697460453, 0.010429491992634182, 1.5026155874003433]
MEASURE p10_default_equalize_FL_right.json first: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json last: max_abs=7.90993731735485770e-12 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json metrics: max_abs=7.38835567726248144e-8 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full spectrum: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full spectrum spectrum: [0.000497822066898446, 0.0007915062603278092, 0.008694794698251894, 1.300448859499568]
MEASURE p10_default_equalize_FR_left.json first: max_abs=2.97432114175835705e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_left.json last: max_abs=8.39756678296826950e-12 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_left.json metrics: max_abs=1.51622550934732425e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json first: max_abs=1.00510738071770203e-6 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json last: max_abs=9.88639227704927852e-12 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json metrics: max_abs=9.37961882774285272e-7 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json first: max_abs=5.97237837428321594e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json last: max_abs=1.07785909697223540e-11 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json metrics: max_abs=3.53357332368367527e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json first: max_abs=1.48468898467091215e-6 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json last: max_abs=1.45355823022822186e-11 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json metrics: max_abs=2.26347425580364359e-7 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json first: max_abs=2.64311893923702135e-7 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json last: max_abs=9.21507271658143857e-12 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json metrics: max_abs=3.58077786415111898e-8 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json first: max_abs=5.59837784871661248e-7 atol=3.225e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json last: max_abs=6.00185718690772740e-12 atol=3.225e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json metrics: max_abs=4.03125228851108908e-7 atol=3.225e-5 rtol=0.000e0
MEASURE applied gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_right.json first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_BL_right.json last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_BL_right.json metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_SL_left.json first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_left.json last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_left.json metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_right.json first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_SL_right.json last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_SL_right.json metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_FC_left.json first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_left.json last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_left.json metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FR_left.json first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_left.json last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_left.json metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_SR_left.json first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_left.json last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_left.json metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_right.json first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_SR_right.json last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_SR_right.json metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_BR_left.json first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_left.json last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_left.json metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_right.json first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BR_right.json last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BR_right.json metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 0 first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 0 last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 0 metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 1 first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 1 last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 1 metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 2 first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 2 last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 2 metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 3 first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 3 last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 3 metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 4 first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 4 last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 4 metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 5 first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 5 last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 5 metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 6 first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 6 last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 6 metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 8 first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 8 last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 8 metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 9 first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 9 last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 9 metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 10 first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 10 last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 10 metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 11 first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 11 last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 11 metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 12 first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 12 last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 12 metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 13 first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 13 last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 13 metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 14 first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 14 last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 14 metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 15 first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hrir 15 last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE hrir 15 metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 0 first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 0 last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 0 metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 1 first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 1 last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 1 metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 2 first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 2 last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 2 metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 3 first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 3 last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 3 metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 4 first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 4 last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 4 metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 5 first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 5 last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 5 metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 6 first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 6 last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 6 metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 7 first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 7 last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 7 metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 8 first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 8 last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 8 metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 9 first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 9 last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 9 metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 10 first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 10 last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 10 metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 11 first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 11 last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 11 metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 12 first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 12 last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 12 metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 13 first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hesuvi 13 last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE hesuvi 13 metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE readme applied gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FL left: max_abs=1.77273528407795311e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FL right: max_abs=1.61004232346328990e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FR left: max_abs=2.34488211354744180e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FR right: max_abs=5.13251723504026813e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FC left: max_abs=4.48546216702538914e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FC right: max_abs=2.19706889978965592e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BL left: max_abs=4.71899858176527687e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BL right: max_abs=8.89887082422546882e-6 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BR left: max_abs=2.56976457961854976e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BR right: max_abs=2.48985555941771963e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SL left: max_abs=5.18924880381632647e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SL right: max_abs=4.33253058851335027e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SR left: max_abs=4.71814690072847043e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SR right: max_abs=3.24008377674545045e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=3.79187204799791289e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.96922702511892567e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=3.09704830527834929e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.73283961985271162e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=4.16666666667424579e-2 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.51910906834018533e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.21409831780761124e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.77186036646526190e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=6.06999877675207244e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.74305373668448738e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.06252115012262038e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.85433664580614277e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.68280774122331422e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.26625922423318116e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.58641669273151820e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.47190702671196050e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.41771818563540819e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.28098852310176881e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.80579766549726628e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.30402472050172946e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.42270612947947939e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.46506285361647315e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.95211817695906120e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.16165238768917334e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=4.95361223833867825e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.11698532630089176e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=5.89355973460214955e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.37945112953730131e-3 atol=1.000e-6 rtol=0.000e0

thread 'golden_default_pipeline_matches_python' (251332) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
applied gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
readme applied gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
readme reflection FL left: (0, -22.574129821079822, -22.574115070168702, 1.4750911120131605e-5)
readme reflection FL right: (0, -12.833324009352722, -12.833309185027417, 1.482432530508504e-5)
readme reflection FR left: (0, -15.815924673379431, -15.815907481580497, 1.7191798933779978e-5)
readme reflection FR right: (0, -25.38700871761952, -25.386979343670625, 2.9373948894573232e-5)
readme reflection FC left: (0, -22.66227574947777, -22.6622334309761, 4.231850167002449e-5)
readme reflection FC right: (0, -18.546619159378356, -18.546599078351008, 2.0081027347629288e-5)
readme reflection BL left: (0, -16.650302376856278, -16.650256277791748, 4.609906453012513e-5)
readme reflection BL right: (0, -15.63861432937271, -15.638611387093441, 2.942279268225434e-6)
readme reflection BR left: (0, -15.187399921275341, -15.187393740182308, 6.181093032964213e-6)
readme reflection BR right: (0, -20.366108734860354, -20.366088406627867, 2.032823248754312e-5)
readme reflection SL left: (0, -23.517606178582085, -23.517572513107368, 3.366547471728154e-5)
readme reflection SL right: (0, -16.90181125062449, -16.901791264537913, 1.9986086577716833e-5)
readme reflection SR left: (0, -16.812336412275783, -16.812303353696322, 3.305857946145352e-5)
readme reflection SR right: (0, -24.248048014104697, -24.24801955081168, 2.8463293016756097e-5)
readme: (0, 88.43736271872856, 88.43740063744904, 3.791872047997913e-5)
readme reverb: (0, 683.8023125238058, 683.7993432967806, 0.0029692270251189257)
readme: (0, 64.13294983249656, 64.13291886201351, 3.097048305278349e-5)
readme reverb: (0, 754.5274935098786, 754.5247606702587, 0.0027328396198527116)
readme: (0, 85.46851568761848, 85.4401255097384, 0.02839017788008391)
readme reverb: (0, 605.7449571412951, 605.7434380322268, 0.0015191090683401853)
readme: (0, 95.53802991836014, 95.54024401667795, 0.0022140983178076112)
readme reverb: (0, 676.6843830623947, 676.6816112020282, 0.002771860366465262)
readme: (0, 75.5182389444555, 75.51763194457783, 0.0006069998776752072)
readme reverb: (0, 807.7824157085382, 807.7786726548015, 0.0037430537366844874)
readme: (0, 83.59674547570782, 83.5946829545577, 0.0020625211501226204)
readme reverb: (0, 679.2707104599413, 679.2688561232954, 0.0018543366458061428)
readme: (0, 75.15575103847235, 75.15601931924647, 0.0002682807741223314)
readme reverb: (0, 753.0876853450495, 753.0844190858253, 0.003266259224233181)
readme: (0, 64.90874597006056, 64.90858732839129, 0.00015864166927315182)
readme reverb: (0, 761.068572713383, 761.0671008063563, 0.0014719070267119605)
readme: (0, 71.4787689450028, 71.47735122681716, 0.0014177181856354082)
readme reverb: (0, 739.8950201205886, 739.8917391320655, 0.003280988523101769)
readme: (0, 90.84386855460454, 90.84567435227004, 0.0018057976654972663)
readme reverb: (0, 715.7703579354888, 715.7680539107683, 0.0023040247205017295)
readme: (0, 101.7718440835066, 101.76942137737711, 0.0024227061294794794)
readme reverb: (0, 721.038764821099, 721.0362997582454, 0.002465062853616473)
readme: (0, 67.31972840901818, 67.32168052719514, 0.0019521181769590612)
readme reverb: (0, 735.3034085874672, 735.3002469350795, 0.0031616523876891733)
readme: (0, 49.1181662020003, 49.11767084077647, 0.0004953612238338678)
readme reverb: (0, 808.683549288934, 808.6804323036077, 0.0031169853263008918)
readme: (0, 90.01264572351155, 90.01323507948501, 0.000589355973460215)
readme reverb: (0, 675.5598672861947, 675.5584878350652, 0.0013794511295373013)

---- golden_option_variants_match_python stdout ----
MEASURE p10_option_bass_boost_gain_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE bass_boost_gain: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE bass_boost_gain gain: max_abs=1.14656674821134175e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json first: max_abs=5.53720320052228376e-7 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json last: max_abs=4.61144137559053665e-12 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json metrics: max_abs=3.56370689913194233e-9 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json first: max_abs=1.29153862451179824e-7 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json last: max_abs=4.94160468161467027e-12 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json metrics: max_abs=4.66875082821491261e-8 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE fr_combination_method: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE fr_combination_method gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE generic_limit: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE generic_limit gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_room_BL_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_right.json: max_abs=4.90274487674469128e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_right.json: max_abs=2.59348098552436568e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_left.json: max_abs=3.37507799486047588e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_right.json: max_abs=2.30926389122032560e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_left.json: max_abs=2.41584530158434063e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_right.json: max_abs=2.62900812231237069e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_left.json: max_abs=5.32907051820075139e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_right.json: max_abs=1.07291953099775128e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_left.json: max_abs=9.94759830064140260e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_right.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_right.json: max_abs=3.62376795237651095e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE specific_limit: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE specific_limit gain: max_abs=2.90719308226883300e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json first: max_abs=6.64545844940907932e-7 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json last: max_abs=5.95941868894068581e-12 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json metrics: max_abs=4.00138404604843956e-7 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json first: max_abs=1.35247016137211157e-7 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json last: max_abs=5.70423247999234038e-12 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json metrics: max_abs=6.37281631771235979e-8 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_target_level_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE target_level: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE target_level gain: max_abs=1.66226329614715951e-4 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json first: max_abs=9.39321658712834184e-7 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json last: max_abs=9.03231773718887905e-12 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json metrics: max_abs=7.85795099760011606e-7 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json first: max_abs=1.57493161767260914e-7 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json last: max_abs=9.05102864891114081e-12 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json metrics: max_abs=1.53124191940717802e-7 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_tilt_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE tilt: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE tilt gain: max_abs=1.17369345886686460e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json first: max_abs=3.52319026242610758e-7 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json last: max_abs=3.10351928600949211e-12 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json metrics: max_abs=2.00984426301439867e-7 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json first: max_abs=1.01319562003992711e-7 atol=3.512e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json last: max_abs=3.73878804191459764e-12 atol=3.512e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json metrics: max_abs=2.58839480526268373e-8 atol=3.512e-6 rtol=0.000e0

thread 'golden_option_variants_match_python' (468908) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
bass_boost_gain gain: (0, -7.7342906601724355, -7.734289513605687, 1.1465667482113417e-6)
fr_combination_method gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
generic_limit gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
specific_limit gain: (0, -3.7076997402953342, -3.707696833102252, 2.907193082268833e-6)
target_level gain: (0, 1.169609756370921, 1.1694435300413062, 0.00016622632961471595)
tilt gain: (0, -8.110450897650443, -8.110449723956984, 1.1736934588668646e-6)


failures:
    golden_decay_matches_python
    golden_default_pipeline_matches_python
    golden_option_variants_match_python

test result: FAILED. 12 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.02s

error: test failed, to rerun pass `-p impulcifer-dsp --test golden_stages`
```

### `cargo test -p impulcifer-dsp`

Exit code: 101

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.18s
     Running unittests src\lib.rs (target\debug\deps\impulcifer_dsp-f98c25de529438cb.exe)

running 9 tests
test decay::tests::python_rounding_helpers_match_cpython ... ok
test resample::tests::resample_poly_padding_arithmetic_48k_to_44k1 ... ok
test virtual_bass::tests::delay_signal_boundaries ... ok
test virtual_bass::tests::rbj_high_shelf_endpoint_gains ... ok
test windows::tests::bessel_i0_matches_scipy_reference ... ok
test virtual_bass::tests::duplicate_sos_preserves_chain_order ... ok
test virtual_bass::tests::mag_at_bin_boundaries ... ok
test decay::tests::golden_decay_first_pass_matches_python ... ok
test fir::tests::golden_homomorphic_log_and_lifter_match_scipy ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests\golden_batch1.rs (target\debug\deps\golden_batch1-96b3c3808b9f8d27.exe)

running 19 tests
test golden_fast_lengths_match_scipy ... ok
test golden_linregress_matches_scipy ... ok
test golden_expit_matches_scipy ... ok
test golden_running_mean_matches_python ... ok
test golden_magnitude_response_matches_python ... ok
test golden_rfft_matches_numpy ... ok
test golden_uniform_filter_matches_scipy ... ok
test golden_butter_sos_matches_scipy ... ok
test golden_tf2sos_matches_scipy ... ok
test golden_irfft_matches_numpy ... ok
test golden_rbj_matches_autoeq ... ok
test golden_windows_match_scipy ... ok
test golden_correlation_lags_match_scipy ... ok
test golden_complex_fft_matches_numpy ... ok
test golden_convolve_modes_match_scipy ... ok
test golden_correlate_modes_match_scipy ... ok
test golden_kaiser_matches_scipy ... ok
test golden_sosfilt_matches_scipy ... ok
test golden_butter_sosfilt_chain_matches_scipy ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests\golden_batch2.rs (target\debug\deps\golden_batch2-b9e23eca3d5c9f2b.exe)

running 12 tests
test natural_cubic_fails_not_a_knot_fixture ... ok
test golden_fractional_octave_window_matches_python ... ok
test golden_spline_k1_matches_fitpack ... ok
test golden_spline_k3_matches_fitpack ... ok
test golden_spline_k2_matches_fitpack ... ok
test golden_first_peak_index_matches_python ... ok
test golden_interp_log_axis_matches_python ... ok
test golden_find_peaks_matches_scipy ... ok
test golden_savgol_matches_scipy ... ok
test golden_firwin2_matches_scipy ... ok
test golden_minimum_phase_matches_scipy ... ok
test golden_savgol_thousand_passes_stay_within_budget ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.72s

     Running tests\golden_batch3.rs (target\debug\deps\golden_batch3-3c6dcfd360dc9ec0.exe)

running 7 tests
test golden_spectrogram_params_match_python ... ok
test golden_resample_poly_edges_match_scipy ... ok
test golden_spectrogram_matches_scipy ... ok
test golden_dc_matches_python_despite_unity_property_failure ... ok
test golden_nnresample_design_matches_python ... ok
test golden_resample_poly_matches_scipy ... ok
test golden_nnresample_matches_python ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.47s

     Running tests\golden_brir_objects.rs (target\debug\deps\golden_brir_objects-e80ffbd36ead0d84.exe)

running 15 tests
test numeric_comparison_reports_nonfinite_mismatches ... ok
test golden_constants_match_python ... ok
test golden_sweep_estimator_matches_python ... ok
test golden_sweep_sequence_matches_python ... ok
test golden_estimate_matches_scipy ... ok
test golden_file_name_matches_python ... ok
test golden_from_samples_repair_matches_python ... ok
test golden_decay_params_match_python ... ok
test golden_reflection_levels_match_python ... ok
test golden_shift_crop_equalize_match_python ... ok
test golden_decay_times_match_python ... ok
test golden_decay_adjustment_matches_python ... ok
test golden_magnitude_response_matches_python ... ok
test golden_stack_tracks_match_python ... ok
test golden_demo_stages_match_python ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.08s

     Running tests\golden_fr.rs (target\debug\deps\golden_fr-76b93c05c41e5de8.exe)

running 12 tests
test golden_generate_frequencies_match_python ... ok
test golden_create_target_matches_autoeq ... ok
test golden_magnitude_to_frequency_response_matches_python ... ok
test golden_center_matches_python ... ok
test golden_smoothen_heavy_light_matches_python ... ok
test golden_compensate_matches_python ... ok
test golden_window_size_and_sigmoid_match_python ... ok
test golden_smoothen_matches_python ... ok
test golden_interpolate_matches_python ... ok
test golden_equalize_matches_python ... ok
test golden_read_from_csv_matches_python ... ok
test golden_minimum_phase_impulse_response_matches_python ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

     Running tests\golden_stages.rs (target\debug\deps\golden_stages-61d7072be0159691.exe)

running 15 tests
test golden_headphone_resolution_matches_python ... ok
test golden_stage_table_matches_python ... ok
test golden_eq_files_match_python ... ok
test golden_mic_deviation_matches_python ... ok
test golden_frozen_eq_firs_isolate_downstream_noise ... ok
test golden_decay_matches_python ... FAILED
test golden_optional_outputs_match_python ... ok
test golden_vbass_matches_python ... ok
test golden_default_pipeline_matches_python ... FAILED
test golden_resample_matches_python ... ok
test pipeline_observer_reports_order_steps_and_totals ... ok
test pipeline_observer_cancels_before_and_after_each_stage ... ok
test golden_channel_balance_matches_python ... ok
test golden_option_variants_match_python ... FAILED
test golden_generic_room_matches_python ... ok

failures:

---- golden_decay_matches_python stdout ----
MEASURE decay FL left integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=2.60784890869558694e-4 atol=1.000e-6 rtol=0.000e0
MEASURE decay FL right integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=1.92238774786801514e-4 atol=1.000e-6 rtol=0.000e0
MEASURE decay FR left integer decisions: max_abs=2.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=4.03960188333485348e-3 atol=1.000e-6 rtol=0.000e0
MEASURE decay FR right integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=2.43536660704535279e-4 atol=1.000e-6 rtol=0.000e0
MEASURE p10_decay_BL_left.json first: max_abs=3.47446812693115448e-7 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_left.json last: max_abs=8.05061990852387955e-12 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_left.json metrics: max_abs=9.87008197395056186e-9 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json first: max_abs=3.17909414551155456e-7 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json last: max_abs=7.94428072918216920e-12 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json metrics: max_abs=2.52442277238335011e-8 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json first: max_abs=9.72028573874295088e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json last: max_abs=6.81845640410530016e-12 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json metrics: max_abs=3.21888374350820516e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json first: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json last: max_abs=2.23442380020497287e-11 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json metrics: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json first: max_abs=9.65501640057958577e-7 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json last: max_abs=1.19263798535294571e-11 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json metrics: max_abs=7.83273138571516370e-8 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json first: max_abs=7.48761591444491170e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json last: max_abs=6.49314803840152037e-12 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json metrics: max_abs=1.98156144957939429e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json first: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json last: max_abs=1.13663763418952697e-15 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json metrics: max_abs=6.04083438544671114e-9 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full spectrum: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full spectrum spectrum: [0.0004267872890852305, 0.00017882616176656402, 0.0026062707932367314, 0.9610001158230487]
MEASURE p10_decay_FL_right.json first: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json last: max_abs=7.53965568385600014e-16 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json metrics: max_abs=7.38835567726248144e-8 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full spectrum: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full spectrum spectrum: [0.0008463076062129979, 0.0006277953469730543, 0.00199813126403375, 0.8180305696078587]
MEASURE p10_decay_FR_left.json first: max_abs=2.97432114175835705e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_left.json last: max_abs=3.48021748189881979e-15 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_left.json metrics: max_abs=1.51622550934732425e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json first: max_abs=1.00510738071770203e-6 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json last: max_abs=1.83019246623658386e-15 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json metrics: max_abs=9.37961882774285272e-7 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json first: max_abs=5.97237837428321594e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json last: max_abs=1.07785909697223540e-11 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json metrics: max_abs=3.53357332368367527e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json first: max_abs=1.48468898467091215e-6 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json last: max_abs=1.45355823022822186e-11 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json metrics: max_abs=2.26347425580364359e-7 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json first: max_abs=2.64311893923702135e-7 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json last: max_abs=9.21507271658143857e-12 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json metrics: max_abs=3.58077786415111898e-8 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json first: max_abs=5.59837784871661248e-7 atol=3.225e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json last: max_abs=6.00185718690772740e-12 atol=3.225e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json metrics: max_abs=4.03125228851108908e-7 atol=3.225e-5 rtol=0.000e0

thread 'golden_decay_matches_python' (301700) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
decay level: (0, -76.83410434514367, -76.8338435602528, 0.0002607848908695587)
decay level: (0, -80.41540325085825, -80.41521101208346, 0.00019223877478680151)
decay FR left integer decisions: (1, 32035.0, 32037.0, 2.0)
decay level: (0, -67.69221944186546, -67.6962590437488, 0.0040396018833348535)
decay level: (0, -74.6506740089376, -74.6504304722769, 0.00024353666070453528)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- golden_default_pipeline_matches_python stdout ----
MEASURE p10_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_response_track_0.json first: max_abs=8.67361737988403547e-19 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_0.json last: max_abs=5.05572790392410515e-21 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_0.json metrics: max_abs=1.76182853028894471e-19 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json first: max_abs=8.67361737988403547e-19 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json last: max_abs=5.81838842869799667e-21 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json metrics: max_abs=8.67361737988403547e-19 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json first: max_abs=8.67361737988403547e-19 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json last: max_abs=8.09313511321882277e-21 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json metrics: max_abs=4.06575814682064163e-19 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json first: max_abs=8.67361737988403547e-19 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json last: max_abs=7.41732855276299916e-21 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json metrics: max_abs=8.67361737988403547e-19 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json first: max_abs=1.19262238973405488e-18 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json last: max_abs=3.61312491563162488e-21 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json metrics: max_abs=1.08420217248550443e-19 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json first: max_abs=8.67361737988403547e-19 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json last: max_abs=6.31278394106328439e-21 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json metrics: max_abs=4.33680868994201774e-19 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_6.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_6.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_6.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_8.json first: max_abs=8.67361737988403547e-19 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_8.json last: max_abs=7.05419626385222001e-21 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_8.json metrics: max_abs=4.33680868994201774e-19 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json first: max_abs=1.08420217248550443e-18 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json last: max_abs=6.31965987990513144e-21 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json metrics: max_abs=4.33680868994201774e-19 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json first: max_abs=8.67361737988403547e-19 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json last: max_abs=5.40955441094366274e-21 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json metrics: max_abs=8.67361737988403547e-19 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json first: max_abs=8.67361737988403547e-19 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json last: max_abs=5.39487195507084650e-21 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json metrics: max_abs=8.67361737988403547e-19 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json first: max_abs=8.67361737988403547e-19 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json last: max_abs=5.36013036934361933e-21 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json metrics: max_abs=4.33680868994201774e-19 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json first: max_abs=8.67361737988403547e-19 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json last: max_abs=6.59924692694805479e-21 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json metrics: max_abs=4.33680868994201774e-19 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json first: max_abs=6.50521303491302660e-19 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json last: max_abs=5.02264067942198404e-21 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json metrics: max_abs=4.33680868994201774e-19 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json first: max_abs=8.67361737988403547e-19 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json last: max_abs=5.67776772456398196e-21 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json metrics: max_abs=4.33680868994201774e-19 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_16.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_16.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_16.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hp left: max_abs=4.27746726927580312e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE hp right: max_abs=1.29034560814034194e-11 atol=1.000e-9 rtol=1.000e-11
MEASURE target: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_eq_fir_BL_left.json: max_abs=1.32302721180643790e-4 atol=1.976e-3 rtol=0.000e0
MEASURE p10_eq_fir_BL_left.json spectrum: [0.0006226827121341905, 0.0009545431245245161, 0.010417501663331982, 1.5026256266821214]
MEASURE p10_eq_fir_BL_right.json: max_abs=1.06078065116033127e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_BL_right.json spectrum: [0.0004591118649617285, 0.0007333779959614081, 0.00789401783376377, 1.0684894384779253]
MEASURE p10_eq_fir_SL_left.json: max_abs=1.32284168126717283e-4 atol=1.984e-3 rtol=0.000e0
MEASURE p10_eq_fir_SL_left.json spectrum: [0.0006248722565692669, 0.0009547972917607427, 0.010417247047074207, 1.5026229436688712]
MEASURE p10_eq_fir_SL_right.json: max_abs=1.83024355603633726e-4 atol=1.967e-3 rtol=0.000e0
MEASURE p10_eq_fir_SL_right.json spectrum: [0.0007620620766964043, 0.0012161554082112773, 0.013258575046099147, 2.069939503593794]
MEASURE p10_eq_fir_FC_left.json: max_abs=1.51722611429427889e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_FC_left.json spectrum: [0.0007207305696235993, 0.0011048972615980573, 0.012021071029254426, 1.6773820092062852]
MEASURE p10_eq_fir_FC_right.json: max_abs=1.62301489864402626e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_FC_right.json spectrum: [0.0006969537446813146, 0.0011129768202397326, 0.012011585188611796, 1.6774391452876771]
MEASURE p10_eq_fir_FL_left.json: max_abs=1.32301029750314658e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_FL_left.json spectrum: [0.0006228606342261547, 0.0009545734423037887, 0.010417576273806798, 1.502615587401929]
MEASURE p10_eq_fir_FL_right.json: max_abs=1.18940849385101854e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_FL_right.json spectrum: [0.0005008780007205574, 0.0007993418154331755, 0.008683589357074115, 1.3004488594904653]
MEASURE p10_eq_fir_FR_left.json: max_abs=1.18589743328900710e-4 atol=1.982e-3 rtol=0.000e0
MEASURE p10_eq_fir_FR_left.json spectrum: [0.000574030171701585, 0.0008785318090904153, 0.009503637657842146, 1.2432146265197166]
MEASURE p10_eq_fir_FR_right.json: max_abs=1.41563459210486409e-4 atol=1.969e-3 rtol=0.000e0
MEASURE p10_eq_fir_FR_right.json spectrum: [0.0006021172870354033, 0.0009617590559611309, 0.010408632505849821, 1.5027138745077955]
MEASURE p10_eq_fir_SR_left.json: max_abs=1.32300879598479249e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_SR_left.json spectrum: [0.0006228787593508592, 0.0009545898937101177, 0.010417690337127137, 1.5026033863741086]
MEASURE p10_eq_fir_SR_right.json: max_abs=1.41551388105981557e-4 atol=1.960e-3 rtol=0.000e0
MEASURE p10_eq_fir_SR_right.json spectrum: [0.0006023296367073721, 0.0009615342218767596, 0.010409270347015075, 1.5026783332473628]
MEASURE p10_eq_fir_BR_left.json: max_abs=1.18586325645653190e-4 atol=1.984e-3 rtol=0.000e0
MEASURE p10_eq_fir_BR_left.json spectrum: [0.0005744515380759603, 0.000878565954884845, 0.009503467940153752, 1.2432254394795874]
MEASURE p10_eq_fir_BR_right.json: max_abs=1.62314367494115208e-4 atol=1.969e-3 rtol=0.000e0
MEASURE p10_eq_fir_BR_right.json spectrum: [0.0006967260476274884, 0.0011132191392902856, 0.012010899985006443, 1.677475892442923]
MEASURE p10_default_crop_BL_left.json first: max_abs=5.20417042793042128e-18 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_left.json last: max_abs=3.13931586076125063e-20 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_left.json metrics: max_abs=5.20417042793042128e-18 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json first: max_abs=3.46944695195361419e-18 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json last: max_abs=3.29151709347100772e-20 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json metrics: max_abs=1.73472347597680709e-18 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json first: max_abs=1.04083408558608426e-17 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json last: max_abs=3.93605622677232689e-20 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json metrics: max_abs=3.46944695195361419e-18 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json first: max_abs=2.60208521396521064e-18 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json last: max_abs=2.58043127988979748e-20 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json metrics: max_abs=1.73472347597680709e-18 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json first: max_abs=8.67361737988403547e-18 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json last: max_abs=5.01867021248172951e-20 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json metrics: max_abs=3.57786716920216463e-18 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json first: max_abs=8.67361737988403547e-18 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json last: max_abs=3.08505281257777202e-20 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json metrics: max_abs=2.16840434497100887e-19 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json first: max_abs=9.54097911787243902e-18 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json last: max_abs=5.62813688781080030e-20 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json metrics: max_abs=3.46944695195361419e-18 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json first: max_abs=4.33680868994201774e-18 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json last: max_abs=2.54903977564341008e-20 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json metrics: max_abs=5.42101086242752217e-19 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json first: max_abs=2.60208521396521064e-18 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json last: max_abs=3.18547253894170958e-20 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json metrics: max_abs=1.95156391047390798e-18 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json first: max_abs=1.04083408558608426e-17 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json last: max_abs=3.06189175542628724e-20 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json metrics: max_abs=1.73472347597680709e-18 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json first: max_abs=3.46944695195361419e-18 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json last: max_abs=5.76511799724958168e-20 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json metrics: max_abs=6.50521303491302660e-19 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json first: max_abs=1.38777878078144568e-17 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json last: max_abs=3.47283508374263139e-20 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json metrics: max_abs=1.38777878078144568e-17 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json first: max_abs=2.60208521396521064e-18 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json last: max_abs=2.26713662288533825e-20 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json metrics: max_abs=3.46944695195361419e-18 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json first: max_abs=7.80625564189563192e-18 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json last: max_abs=5.48453833347159470e-20 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json metrics: max_abs=1.73472347597680709e-18 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json first: max_abs=3.47446812693115448e-7 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json last: max_abs=8.05061990852387955e-12 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json metrics: max_abs=9.87008197395056186e-9 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json first: max_abs=3.17909414551155456e-7 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json last: max_abs=7.94428072918216920e-12 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json metrics: max_abs=2.52442277238335011e-8 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json first: max_abs=9.72028573874295088e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json last: max_abs=6.81845640410530016e-12 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json metrics: max_abs=3.21888374350820516e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json first: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json last: max_abs=2.23442380020497287e-11 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json metrics: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json first: max_abs=9.65501640057958577e-7 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json last: max_abs=1.19263798535294571e-11 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json metrics: max_abs=7.83273138571516370e-8 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json first: max_abs=7.48761591444491170e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json last: max_abs=6.49314803840152037e-12 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json metrics: max_abs=1.98156144957939429e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json first: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json last: max_abs=7.89445292482472558e-12 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json metrics: max_abs=6.08208228550753682e-9 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full spectrum: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full spectrum spectrum: [0.0006373479341870772, 0.0009437956697460453, 0.010429491992634182, 1.5026155874003433]
MEASURE p10_default_equalize_FL_right.json first: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json last: max_abs=7.90993731735485770e-12 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json metrics: max_abs=7.38835567726248144e-8 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full spectrum: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full spectrum spectrum: [0.000497822066898446, 0.0007915062603278092, 0.008694794698251894, 1.300448859499568]
MEASURE p10_default_equalize_FR_left.json first: max_abs=2.97432114175835705e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_left.json last: max_abs=8.39756678296826950e-12 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_left.json metrics: max_abs=1.51622550934732425e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json first: max_abs=1.00510738071770203e-6 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json last: max_abs=9.88639227704927852e-12 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json metrics: max_abs=9.37961882774285272e-7 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json first: max_abs=5.97237837428321594e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json last: max_abs=1.07785909697223540e-11 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json metrics: max_abs=3.53357332368367527e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json first: max_abs=1.48468898467091215e-6 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json last: max_abs=1.45355823022822186e-11 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json metrics: max_abs=2.26347425580364359e-7 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json first: max_abs=2.64311893923702135e-7 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json last: max_abs=9.21507271658143857e-12 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json metrics: max_abs=3.58077786415111898e-8 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json first: max_abs=5.59837784871661248e-7 atol=3.225e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json last: max_abs=6.00185718690772740e-12 atol=3.225e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json metrics: max_abs=4.03125228851108908e-7 atol=3.225e-5 rtol=0.000e0
MEASURE applied gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_right.json first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_BL_right.json last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_BL_right.json metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_SL_left.json first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_left.json last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_left.json metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_right.json first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_SL_right.json last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_SL_right.json metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_FC_left.json first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_left.json last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_left.json metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FR_left.json first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_left.json last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_left.json metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_SR_left.json first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_left.json last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_left.json metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_right.json first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_SR_right.json last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_SR_right.json metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_BR_left.json first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_left.json last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_left.json metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_right.json first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BR_right.json last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BR_right.json metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 0 first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 0 last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 0 metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 1 first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 1 last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 1 metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 2 first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 2 last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 2 metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 3 first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 3 last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 3 metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 4 first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 4 last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 4 metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 5 first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 5 last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 5 metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 6 first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 6 last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 6 metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 8 first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 8 last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 8 metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 9 first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 9 last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 9 metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 10 first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 10 last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 10 metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 11 first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 11 last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 11 metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 12 first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 12 last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 12 metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 13 first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 13 last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 13 metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 14 first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 14 last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 14 metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 15 first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hrir 15 last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE hrir 15 metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 0 first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 0 last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 0 metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 1 first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 1 last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 1 metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 2 first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 2 last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 2 metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 3 first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 3 last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 3 metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 4 first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 4 last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 4 metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 5 first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 5 last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 5 metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 6 first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 6 last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 6 metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 7 first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 7 last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 7 metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 8 first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 8 last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 8 metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 9 first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 9 last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 9 metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 10 first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 10 last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 10 metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 11 first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 11 last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 11 metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 12 first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 12 last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 12 metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 13 first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hesuvi 13 last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE hesuvi 13 metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE readme applied gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FL left: max_abs=1.77273528407795311e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FL right: max_abs=1.61004232346328990e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FR left: max_abs=2.34488211354744180e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FR right: max_abs=5.13251723504026813e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FC left: max_abs=4.48546216702538914e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FC right: max_abs=2.19706889978965592e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BL left: max_abs=4.71899858176527687e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BL right: max_abs=8.89887082422546882e-6 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BR left: max_abs=2.56976457961854976e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BR right: max_abs=2.48985555941771963e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SL left: max_abs=5.18924880381632647e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SL right: max_abs=4.33253058851335027e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SR left: max_abs=4.71814690072847043e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SR right: max_abs=3.24008377674545045e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=3.79187204799791289e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.96922702511892567e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=3.09704830527834929e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.73283961985271162e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=4.16666666667424579e-2 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.51910906834018533e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.21409831780761124e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.77186036646526190e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=6.06999877675207244e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.74305373668448738e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.06252115012262038e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.85433664580614277e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.68280774122331422e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.26625922423318116e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.58641669273151820e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.47190702671196050e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.41771818563540819e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.28098852310176881e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.80579766549726628e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.30402472050172946e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.42270612947947939e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.46506285361647315e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.95211817695906120e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.16165238768917334e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=4.95361223833867825e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.11698532630089176e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=5.89355973460214955e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.37945112953730131e-3 atol=1.000e-6 rtol=0.000e0

thread 'golden_default_pipeline_matches_python' (195648) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
applied gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
readme applied gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
readme reflection FL left: (0, -22.574129821079822, -22.574115070168702, 1.4750911120131605e-5)
readme reflection FL right: (0, -12.833324009352722, -12.833309185027417, 1.482432530508504e-5)
readme reflection FR left: (0, -15.815924673379431, -15.815907481580497, 1.7191798933779978e-5)
readme reflection FR right: (0, -25.38700871761952, -25.386979343670625, 2.9373948894573232e-5)
readme reflection FC left: (0, -22.66227574947777, -22.6622334309761, 4.231850167002449e-5)
readme reflection FC right: (0, -18.546619159378356, -18.546599078351008, 2.0081027347629288e-5)
readme reflection BL left: (0, -16.650302376856278, -16.650256277791748, 4.609906453012513e-5)
readme reflection BL right: (0, -15.63861432937271, -15.638611387093441, 2.942279268225434e-6)
readme reflection BR left: (0, -15.187399921275341, -15.187393740182308, 6.181093032964213e-6)
readme reflection BR right: (0, -20.366108734860354, -20.366088406627867, 2.032823248754312e-5)
readme reflection SL left: (0, -23.517606178582085, -23.517572513107368, 3.366547471728154e-5)
readme reflection SL right: (0, -16.90181125062449, -16.901791264537913, 1.9986086577716833e-5)
readme reflection SR left: (0, -16.812336412275783, -16.812303353696322, 3.305857946145352e-5)
readme reflection SR right: (0, -24.248048014104697, -24.24801955081168, 2.8463293016756097e-5)
readme: (0, 88.43736271872856, 88.43740063744904, 3.791872047997913e-5)
readme reverb: (0, 683.8023125238058, 683.7993432967806, 0.0029692270251189257)
readme: (0, 64.13294983249656, 64.13291886201351, 3.097048305278349e-5)
readme reverb: (0, 754.5274935098786, 754.5247606702587, 0.0027328396198527116)
readme: (0, 85.46851568761848, 85.4401255097384, 0.02839017788008391)
readme reverb: (0, 605.7449571412951, 605.7434380322268, 0.0015191090683401853)
readme: (0, 95.53802991836014, 95.54024401667795, 0.0022140983178076112)
readme reverb: (0, 676.6843830623947, 676.6816112020282, 0.002771860366465262)
readme: (0, 75.5182389444555, 75.51763194457783, 0.0006069998776752072)
readme reverb: (0, 807.7824157085382, 807.7786726548015, 0.0037430537366844874)
readme: (0, 83.59674547570782, 83.5946829545577, 0.0020625211501226204)
readme reverb: (0, 679.2707104599413, 679.2688561232954, 0.0018543366458061428)
readme: (0, 75.15575103847235, 75.15601931924647, 0.0002682807741223314)
readme reverb: (0, 753.0876853450495, 753.0844190858253, 0.003266259224233181)
readme: (0, 64.90874597006056, 64.90858732839129, 0.00015864166927315182)
readme reverb: (0, 761.068572713383, 761.0671008063563, 0.0014719070267119605)
readme: (0, 71.4787689450028, 71.47735122681716, 0.0014177181856354082)
readme reverb: (0, 739.8950201205886, 739.8917391320655, 0.003280988523101769)
readme: (0, 90.84386855460454, 90.84567435227004, 0.0018057976654972663)
readme reverb: (0, 715.7703579354888, 715.7680539107683, 0.0023040247205017295)
readme: (0, 101.7718440835066, 101.76942137737711, 0.0024227061294794794)
readme reverb: (0, 721.038764821099, 721.0362997582454, 0.002465062853616473)
readme: (0, 67.31972840901818, 67.32168052719514, 0.0019521181769590612)
readme reverb: (0, 735.3034085874672, 735.3002469350795, 0.0031616523876891733)
readme: (0, 49.1181662020003, 49.11767084077647, 0.0004953612238338678)
readme reverb: (0, 808.683549288934, 808.6804323036077, 0.0031169853263008918)
readme: (0, 90.01264572351155, 90.01323507948501, 0.000589355973460215)
readme reverb: (0, 675.5598672861947, 675.5584878350652, 0.0013794511295373013)

---- golden_option_variants_match_python stdout ----
MEASURE p10_option_bass_boost_gain_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE bass_boost_gain: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE bass_boost_gain gain: max_abs=1.14656674821134175e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json first: max_abs=5.53720320052228376e-7 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json last: max_abs=4.61144137559053665e-12 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json metrics: max_abs=3.56370689913194233e-9 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json first: max_abs=1.29153862451179824e-7 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json last: max_abs=4.94160468161467027e-12 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json metrics: max_abs=4.66875082821491261e-8 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE fr_combination_method: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE fr_combination_method gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE generic_limit: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE generic_limit gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_room_BL_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_right.json: max_abs=4.90274487674469128e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_right.json: max_abs=2.59348098552436568e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_left.json: max_abs=3.37507799486047588e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_right.json: max_abs=2.30926389122032560e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_left.json: max_abs=2.41584530158434063e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_right.json: max_abs=2.62900812231237069e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_left.json: max_abs=5.32907051820075139e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_right.json: max_abs=1.07291953099775128e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_left.json: max_abs=9.94759830064140260e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_right.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_right.json: max_abs=3.62376795237651095e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE specific_limit: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE specific_limit gain: max_abs=2.90719308226883300e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json first: max_abs=6.64545844940907932e-7 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json last: max_abs=5.95941868894068581e-12 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json metrics: max_abs=4.00138404604843956e-7 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json first: max_abs=1.35247016137211157e-7 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json last: max_abs=5.70423247999234038e-12 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json metrics: max_abs=6.37281631771235979e-8 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_target_level_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE target_level: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE target_level gain: max_abs=1.66226329614715951e-4 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json first: max_abs=9.39321658712834184e-7 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json last: max_abs=9.03231773718887905e-12 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json metrics: max_abs=7.85795099760011606e-7 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json first: max_abs=1.57493161767260914e-7 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json last: max_abs=9.05102864891114081e-12 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json metrics: max_abs=1.53124191940717802e-7 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_tilt_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE tilt: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE tilt gain: max_abs=1.17369345886686460e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json first: max_abs=3.52319026242610758e-7 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json last: max_abs=3.10351928600949211e-12 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json metrics: max_abs=2.00984426301439867e-7 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json first: max_abs=1.01319562003992711e-7 atol=3.512e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json last: max_abs=3.73878804191459764e-12 atol=3.512e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json metrics: max_abs=2.58839480526268373e-8 atol=3.512e-6 rtol=0.000e0

thread 'golden_option_variants_match_python' (193080) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
bass_boost_gain gain: (0, -7.7342906601724355, -7.734289513605687, 1.1465667482113417e-6)
fr_combination_method gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
generic_limit gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
specific_limit gain: (0, -3.7076997402953342, -3.707696833102252, 2.907193082268833e-6)
target_level gain: (0, 1.169609756370921, 1.1694435300413062, 0.00016622632961471595)
tilt gain: (0, -8.110450897650443, -8.110449723956984, 1.1736934588668646e-6)


failures:
    golden_decay_matches_python
    golden_default_pipeline_matches_python
    golden_option_variants_match_python

test result: FAILED. 12 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.09s

error: test failed, to rerun pass `-p impulcifer-dsp --test golden_stages`
```

### `cargo test -p impulcifer-policy`

Exit code: 0

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.15s
     Running unittests src\lib.rs (target\debug\deps\impulcifer_policy-6f97dbb98fd5d3f8.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\gates.rs (target\debug\deps\gates-0c417439c6ccb24f.exe)

running 4 tests
test every_crate_root_forbids_unsafe ... ok
test canonical_features_registered ... ok
test no_unsafe_outside_budget ... ok
test implemented_features_have_existing_tests ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s

   Doc-tests impulcifer_policy

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

### `cargo test -p impulcifer-dsp --no-fail-fast`

Exit code: 101

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.20s
     Running unittests src\lib.rs (target\debug\deps\impulcifer_dsp-f98c25de529438cb.exe)

running 9 tests
test decay::tests::python_rounding_helpers_match_cpython ... ok
test resample::tests::resample_poly_padding_arithmetic_48k_to_44k1 ... ok
test virtual_bass::tests::delay_signal_boundaries ... ok
test virtual_bass::tests::duplicate_sos_preserves_chain_order ... ok
test windows::tests::bessel_i0_matches_scipy_reference ... ok
test virtual_bass::tests::rbj_high_shelf_endpoint_gains ... ok
test virtual_bass::tests::mag_at_bin_boundaries ... ok
test decay::tests::golden_decay_first_pass_matches_python ... ok
test fir::tests::golden_homomorphic_log_and_lifter_match_scipy ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests\golden_batch1.rs (target\debug\deps\golden_batch1-96b3c3808b9f8d27.exe)

running 19 tests
test golden_expit_matches_scipy ... ok
test golden_fast_lengths_match_scipy ... ok
test golden_linregress_matches_scipy ... ok
test golden_magnitude_response_matches_python ... ok
test golden_uniform_filter_matches_scipy ... ok
test golden_running_mean_matches_python ... ok
test golden_rfft_matches_numpy ... ok
test golden_butter_sos_matches_scipy ... ok
test golden_tf2sos_matches_scipy ... ok
test golden_irfft_matches_numpy ... ok
test golden_rbj_matches_autoeq ... ok
test golden_windows_match_scipy ... ok
test golden_correlation_lags_match_scipy ... ok
test golden_complex_fft_matches_numpy ... ok
test golden_convolve_modes_match_scipy ... ok
test golden_correlate_modes_match_scipy ... ok
test golden_kaiser_matches_scipy ... ok
test golden_sosfilt_matches_scipy ... ok
test golden_butter_sosfilt_chain_matches_scipy ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests\golden_batch2.rs (target\debug\deps\golden_batch2-b9e23eca3d5c9f2b.exe)

running 12 tests
test golden_fractional_octave_window_matches_python ... ok
test natural_cubic_fails_not_a_knot_fixture ... ok
test golden_spline_k2_matches_fitpack ... ok
test golden_spline_k1_matches_fitpack ... ok
test golden_spline_k3_matches_fitpack ... ok
test golden_first_peak_index_matches_python ... ok
test golden_interp_log_axis_matches_python ... ok
test golden_find_peaks_matches_scipy ... ok
test golden_savgol_matches_scipy ... ok
test golden_firwin2_matches_scipy ... ok
test golden_minimum_phase_matches_scipy ... ok
test golden_savgol_thousand_passes_stay_within_budget ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.72s

     Running tests\golden_batch3.rs (target\debug\deps\golden_batch3-3c6dcfd360dc9ec0.exe)

running 7 tests
test golden_spectrogram_params_match_python ... ok
test golden_resample_poly_edges_match_scipy ... ok
test golden_spectrogram_matches_scipy ... ok
test golden_dc_matches_python_despite_unity_property_failure ... ok
test golden_nnresample_design_matches_python ... ok
test golden_resample_poly_matches_scipy ... ok
test golden_nnresample_matches_python ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.55s

     Running tests\golden_brir_objects.rs (target\debug\deps\golden_brir_objects-e80ffbd36ead0d84.exe)

running 15 tests
test numeric_comparison_reports_nonfinite_mismatches ... ok
test golden_constants_match_python ... ok
test golden_sweep_estimator_matches_python ... ok
test golden_sweep_sequence_matches_python ... ok
test golden_estimate_matches_scipy ... ok
test golden_file_name_matches_python ... ok
test golden_from_samples_repair_matches_python ... ok
test golden_decay_params_match_python ... ok
test golden_reflection_levels_match_python ... ok
test golden_shift_crop_equalize_match_python ... ok
test golden_decay_times_match_python ... ok
test golden_decay_adjustment_matches_python ... ok
test golden_magnitude_response_matches_python ... ok
test golden_stack_tracks_match_python ... ok
test golden_demo_stages_match_python ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.04s

     Running tests\golden_fr.rs (target\debug\deps\golden_fr-76b93c05c41e5de8.exe)

running 12 tests
test golden_generate_frequencies_match_python ... ok
test golden_create_target_matches_autoeq ... ok
test golden_center_matches_python ... ok
test golden_smoothen_heavy_light_matches_python ... ok
test golden_magnitude_to_frequency_response_matches_python ... ok
test golden_compensate_matches_python ... ok
test golden_window_size_and_sigmoid_match_python ... ok
test golden_smoothen_matches_python ... ok
test golden_interpolate_matches_python ... ok
test golden_equalize_matches_python ... ok
test golden_read_from_csv_matches_python ... ok
test golden_minimum_phase_impulse_response_matches_python ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

     Running tests\golden_stages.rs (target\debug\deps\golden_stages-61d7072be0159691.exe)

running 15 tests
test golden_headphone_resolution_matches_python ... ok
test golden_stage_table_matches_python ... ok
test golden_eq_files_match_python ... ok
test golden_mic_deviation_matches_python ... ok
test golden_frozen_eq_firs_isolate_downstream_noise ... ok
test golden_decay_matches_python ... FAILED
test golden_optional_outputs_match_python ... ok
test golden_vbass_matches_python ... ok
test golden_default_pipeline_matches_python ... FAILED
test golden_resample_matches_python ... ok
test pipeline_observer_reports_order_steps_and_totals ... ok
test pipeline_observer_cancels_before_and_after_each_stage ... ok
test golden_channel_balance_matches_python ... ok
test golden_option_variants_match_python ... FAILED
test golden_generic_room_matches_python ... ok

failures:

---- golden_decay_matches_python stdout ----
MEASURE decay FL left integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=2.60784890869558694e-4 atol=1.000e-6 rtol=0.000e0
MEASURE decay FL right integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=1.92238774786801514e-4 atol=1.000e-6 rtol=0.000e0
MEASURE decay FR left integer decisions: max_abs=2.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=4.03960188333485348e-3 atol=1.000e-6 rtol=0.000e0
MEASURE decay FR right integer decisions: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE decay level: max_abs=2.43536660704535279e-4 atol=1.000e-6 rtol=0.000e0
MEASURE p10_decay_BL_left.json first: max_abs=3.47446812693115448e-7 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_left.json last: max_abs=8.05061990852387955e-12 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_left.json metrics: max_abs=9.87008197395056186e-9 atol=1.568e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json first: max_abs=3.17909414551155456e-7 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json last: max_abs=7.94428072918216920e-12 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_BL_right.json metrics: max_abs=2.52442277238335011e-8 atol=1.375e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json first: max_abs=9.72028573874295088e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json last: max_abs=6.81845640410530016e-12 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_left.json metrics: max_abs=3.21888374350820516e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json first: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json last: max_abs=2.23442380020497287e-11 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_SL_right.json metrics: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json first: max_abs=9.65501640057958577e-7 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json last: max_abs=1.19263798535294571e-11 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_left.json metrics: max_abs=7.83273138571516370e-8 atol=2.649e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json first: max_abs=7.48761591444491170e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json last: max_abs=6.49314803840152037e-12 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FC_right.json metrics: max_abs=1.98156144957939429e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json first: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json last: max_abs=1.13663763418952697e-15 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json metrics: max_abs=6.04083438544671114e-9 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full spectrum: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_decay_FL_left.json full spectrum spectrum: [0.0004267872890852305, 0.00017882616176656402, 0.0026062707932367314, 0.9610001158230487]
MEASURE p10_decay_FL_right.json first: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json last: max_abs=7.53965568385600014e-16 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json metrics: max_abs=7.38835567726248144e-8 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full spectrum: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_decay_FL_right.json full spectrum spectrum: [0.0008463076062129979, 0.0006277953469730543, 0.00199813126403375, 0.8180305696078587]
MEASURE p10_decay_FR_left.json first: max_abs=2.97432114175835705e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_left.json last: max_abs=3.48021748189881979e-15 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_left.json metrics: max_abs=1.51622550934732425e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json first: max_abs=1.00510738071770203e-6 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json last: max_abs=1.83019246623658386e-15 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_FR_right.json metrics: max_abs=9.37961882774285272e-7 atol=3.634e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json first: max_abs=5.97237837428321594e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json last: max_abs=1.07785909697223540e-11 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_left.json metrics: max_abs=3.53357332368367527e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json first: max_abs=1.48468898467091215e-6 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json last: max_abs=1.45355823022822186e-11 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_SR_right.json metrics: max_abs=2.26347425580364359e-7 atol=3.046e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json first: max_abs=2.64311893923702135e-7 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json last: max_abs=9.21507271658143857e-12 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_left.json metrics: max_abs=3.58077786415111898e-8 atol=1.149e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json first: max_abs=5.59837784871661248e-7 atol=3.225e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json last: max_abs=6.00185718690772740e-12 atol=3.225e-5 rtol=0.000e0
MEASURE p10_decay_BR_right.json metrics: max_abs=4.03125228851108908e-7 atol=3.225e-5 rtol=0.000e0

thread 'golden_decay_matches_python' (141296) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
decay level: (0, -76.83410434514367, -76.8338435602528, 0.0002607848908695587)
decay level: (0, -80.41540325085825, -80.41521101208346, 0.00019223877478680151)
decay FR left integer decisions: (1, 32035.0, 32037.0, 2.0)
decay level: (0, -67.69221944186546, -67.6962590437488, 0.0040396018833348535)
decay level: (0, -74.6506740089376, -74.6504304722769, 0.00024353666070453528)
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- golden_default_pipeline_matches_python stdout ----
MEASURE p10_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_room_response_track_0.json first: max_abs=8.67361737988403547e-19 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_0.json last: max_abs=5.05572790392410515e-21 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_0.json metrics: max_abs=1.76182853028894471e-19 atol=3.101e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json first: max_abs=8.67361737988403547e-19 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json last: max_abs=5.81838842869799667e-21 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_1.json metrics: max_abs=8.67361737988403547e-19 atol=2.907e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json first: max_abs=8.67361737988403547e-19 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json last: max_abs=8.09313511321882277e-21 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_2.json metrics: max_abs=4.06575814682064163e-19 atol=3.001e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json first: max_abs=8.67361737988403547e-19 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json last: max_abs=7.41732855276299916e-21 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_3.json metrics: max_abs=8.67361737988403547e-19 atol=2.832e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json first: max_abs=1.19262238973405488e-18 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json last: max_abs=3.61312491563162488e-21 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_4.json metrics: max_abs=1.08420217248550443e-19 atol=3.320e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json first: max_abs=8.67361737988403547e-19 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json last: max_abs=6.31278394106328439e-21 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_5.json metrics: max_abs=4.33680868994201774e-19 atol=2.640e-12 rtol=0.000e0
MEASURE p10_room_response_track_6.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_6.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_6.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_7.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_8.json first: max_abs=8.67361737988403547e-19 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_8.json last: max_abs=7.05419626385222001e-21 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_8.json metrics: max_abs=4.33680868994201774e-19 atol=2.526e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json first: max_abs=1.08420217248550443e-18 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json last: max_abs=6.31965987990513144e-21 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_9.json metrics: max_abs=4.33680868994201774e-19 atol=3.235e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json first: max_abs=8.67361737988403547e-19 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json last: max_abs=5.40955441094366274e-21 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_10.json metrics: max_abs=8.67361737988403547e-19 atol=3.233e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json first: max_abs=8.67361737988403547e-19 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json last: max_abs=5.39487195507084650e-21 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_11.json metrics: max_abs=8.67361737988403547e-19 atol=3.321e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json first: max_abs=8.67361737988403547e-19 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json last: max_abs=5.36013036934361933e-21 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_12.json metrics: max_abs=4.33680868994201774e-19 atol=2.714e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json first: max_abs=8.67361737988403547e-19 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json last: max_abs=6.59924692694805479e-21 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_13.json metrics: max_abs=4.33680868994201774e-19 atol=2.490e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json first: max_abs=6.50521303491302660e-19 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json last: max_abs=5.02264067942198404e-21 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_14.json metrics: max_abs=4.33680868994201774e-19 atol=2.783e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json first: max_abs=8.67361737988403547e-19 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json last: max_abs=5.67776772456398196e-21 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_15.json metrics: max_abs=4.33680868994201774e-19 atol=3.181e-12 rtol=0.000e0
MEASURE p10_room_response_track_16.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_16.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_16.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_17.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_18.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_19.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_20.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_21.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_22.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_23.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_24.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_25.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_26.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_27.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_28.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_29.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_30.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE p10_room_response_track_31.json metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hp left: max_abs=4.27746726927580312e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE hp right: max_abs=1.29034560814034194e-11 atol=1.000e-9 rtol=1.000e-11
MEASURE target: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_eq_fir_BL_left.json: max_abs=1.32302721180643790e-4 atol=1.976e-3 rtol=0.000e0
MEASURE p10_eq_fir_BL_left.json spectrum: [0.0006226827121341905, 0.0009545431245245161, 0.010417501663331982, 1.5026256266821214]
MEASURE p10_eq_fir_BL_right.json: max_abs=1.06078065116033127e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_BL_right.json spectrum: [0.0004591118649617285, 0.0007333779959614081, 0.00789401783376377, 1.0684894384779253]
MEASURE p10_eq_fir_SL_left.json: max_abs=1.32284168126717283e-4 atol=1.984e-3 rtol=0.000e0
MEASURE p10_eq_fir_SL_left.json spectrum: [0.0006248722565692669, 0.0009547972917607427, 0.010417247047074207, 1.5026229436688712]
MEASURE p10_eq_fir_SL_right.json: max_abs=1.83024355603633726e-4 atol=1.967e-3 rtol=0.000e0
MEASURE p10_eq_fir_SL_right.json spectrum: [0.0007620620766964043, 0.0012161554082112773, 0.013258575046099147, 2.069939503593794]
MEASURE p10_eq_fir_FC_left.json: max_abs=1.51722611429427889e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_FC_left.json spectrum: [0.0007207305696235993, 0.0011048972615980573, 0.012021071029254426, 1.6773820092062852]
MEASURE p10_eq_fir_FC_right.json: max_abs=1.62301489864402626e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_FC_right.json spectrum: [0.0006969537446813146, 0.0011129768202397326, 0.012011585188611796, 1.6774391452876771]
MEASURE p10_eq_fir_FL_left.json: max_abs=1.32301029750314658e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_FL_left.json spectrum: [0.0006228606342261547, 0.0009545734423037887, 0.010417576273806798, 1.502615587401929]
MEASURE p10_eq_fir_FL_right.json: max_abs=1.18940849385101854e-4 atol=1.961e-3 rtol=0.000e0
MEASURE p10_eq_fir_FL_right.json spectrum: [0.0005008780007205574, 0.0007993418154331755, 0.008683589357074115, 1.3004488594904653]
MEASURE p10_eq_fir_FR_left.json: max_abs=1.18589743328900710e-4 atol=1.982e-3 rtol=0.000e0
MEASURE p10_eq_fir_FR_left.json spectrum: [0.000574030171701585, 0.0008785318090904153, 0.009503637657842146, 1.2432146265197166]
MEASURE p10_eq_fir_FR_right.json: max_abs=1.41563459210486409e-4 atol=1.969e-3 rtol=0.000e0
MEASURE p10_eq_fir_FR_right.json spectrum: [0.0006021172870354033, 0.0009617590559611309, 0.010408632505849821, 1.5027138745077955]
MEASURE p10_eq_fir_SR_left.json: max_abs=1.32300879598479249e-4 atol=1.977e-3 rtol=0.000e0
MEASURE p10_eq_fir_SR_left.json spectrum: [0.0006228787593508592, 0.0009545898937101177, 0.010417690337127137, 1.5026033863741086]
MEASURE p10_eq_fir_SR_right.json: max_abs=1.41551388105981557e-4 atol=1.960e-3 rtol=0.000e0
MEASURE p10_eq_fir_SR_right.json spectrum: [0.0006023296367073721, 0.0009615342218767596, 0.010409270347015075, 1.5026783332473628]
MEASURE p10_eq_fir_BR_left.json: max_abs=1.18586325645653190e-4 atol=1.984e-3 rtol=0.000e0
MEASURE p10_eq_fir_BR_left.json spectrum: [0.0005744515380759603, 0.000878565954884845, 0.009503467940153752, 1.2432254394795874]
MEASURE p10_eq_fir_BR_right.json: max_abs=1.62314367494115208e-4 atol=1.969e-3 rtol=0.000e0
MEASURE p10_eq_fir_BR_right.json spectrum: [0.0006967260476274884, 0.0011132191392902856, 0.012010899985006443, 1.677475892442923]
MEASURE p10_default_crop_BL_left.json first: max_abs=5.20417042793042128e-18 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_left.json last: max_abs=3.13931586076125063e-20 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_left.json metrics: max_abs=5.20417042793042128e-18 atol=1.012e-11 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json first: max_abs=3.46944695195361419e-18 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json last: max_abs=3.29151709347100772e-20 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_BL_right.json metrics: max_abs=1.73472347597680709e-18 atol=9.955e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json first: max_abs=1.04083408558608426e-17 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json last: max_abs=3.93605622677232689e-20 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_left.json metrics: max_abs=3.46944695195361419e-18 atol=2.560e-11 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json first: max_abs=2.60208521396521064e-18 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json last: max_abs=2.58043127988979748e-20 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_SL_right.json metrics: max_abs=1.73472347597680709e-18 atol=8.355e-12 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json first: max_abs=8.67361737988403547e-18 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json last: max_abs=5.01867021248172951e-20 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_left.json metrics: max_abs=3.57786716920216463e-18 atol=2.354e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json first: max_abs=8.67361737988403547e-18 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json last: max_abs=3.08505281257777202e-20 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FC_right.json metrics: max_abs=2.16840434497100887e-19 atol=2.058e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json first: max_abs=9.54097911787243902e-18 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json last: max_abs=5.62813688781080030e-20 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_left.json metrics: max_abs=3.46944695195361419e-18 atol=2.992e-11 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json first: max_abs=4.33680868994201774e-18 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json last: max_abs=2.54903977564341008e-20 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FL_right.json metrics: max_abs=5.42101086242752217e-19 atol=7.859e-12 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json first: max_abs=2.60208521396521064e-18 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json last: max_abs=3.18547253894170958e-20 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_left.json metrics: max_abs=1.95156391047390798e-18 atol=1.214e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json first: max_abs=1.04083408558608426e-17 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json last: max_abs=3.06189175542628724e-20 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_FR_right.json metrics: max_abs=1.73472347597680709e-18 atol=2.534e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json first: max_abs=3.46944695195361419e-18 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json last: max_abs=5.76511799724958168e-20 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_left.json metrics: max_abs=6.50521303491302660e-19 atol=6.273e-12 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json first: max_abs=1.38777878078144568e-17 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json last: max_abs=3.47283508374263139e-20 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_SR_right.json metrics: max_abs=1.38777878078144568e-17 atol=2.891e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json first: max_abs=2.60208521396521064e-18 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json last: max_abs=2.26713662288533825e-20 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_left.json metrics: max_abs=3.46944695195361419e-18 atol=1.062e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json first: max_abs=7.80625564189563192e-18 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json last: max_abs=5.48453833347159470e-20 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_crop_BR_right.json metrics: max_abs=1.73472347597680709e-18 atol=1.492e-11 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json first: max_abs=3.47446812693115448e-7 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json last: max_abs=8.05061990852387955e-12 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_left.json metrics: max_abs=9.87008197395056186e-9 atol=1.568e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json first: max_abs=3.17909414551155456e-7 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json last: max_abs=7.94428072918216920e-12 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_BL_right.json metrics: max_abs=2.52442277238335011e-8 atol=1.375e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json first: max_abs=9.72028573874295088e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json last: max_abs=6.81845640410530016e-12 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_left.json metrics: max_abs=3.21888374350820516e-7 atol=3.417e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json first: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json last: max_abs=2.23442380020497287e-11 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_SL_right.json metrics: max_abs=4.53334271618283102e-7 atol=1.454e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json first: max_abs=9.65501640057958577e-7 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json last: max_abs=1.19263798535294571e-11 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_left.json metrics: max_abs=7.83273138571516370e-8 atol=2.649e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json first: max_abs=7.48761591444491170e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json last: max_abs=6.49314803840152037e-12 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FC_right.json metrics: max_abs=1.98156144957939429e-7 atol=2.555e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json first: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json last: max_abs=7.89445292482472558e-12 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json metrics: max_abs=6.08208228550753682e-9 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full spectrum: max_abs=9.40082564495660356e-7 atol=3.579e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_left.json full spectrum spectrum: [0.0006373479341870772, 0.0009437956697460453, 0.010429491992634182, 1.5026155874003433]
MEASURE p10_default_equalize_FL_right.json first: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json last: max_abs=7.90993731735485770e-12 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json metrics: max_abs=7.38835567726248144e-8 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full spectrum: max_abs=2.04810764394165734e-7 atol=1.085e-5 rtol=0.000e0
MEASURE p10_default_equalize_FL_right.json full spectrum spectrum: [0.000497822066898446, 0.0007915062603278092, 0.008694794698251894, 1.300448859499568]
MEASURE p10_default_equalize_FR_left.json first: max_abs=2.97432114175835705e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_left.json last: max_abs=8.39756678296826950e-12 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_left.json metrics: max_abs=1.51622550934732425e-7 atol=1.641e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json first: max_abs=1.00510738071770203e-6 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json last: max_abs=9.88639227704927852e-12 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_FR_right.json metrics: max_abs=9.37961882774285272e-7 atol=3.634e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json first: max_abs=5.97237837428321594e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json last: max_abs=1.07785909697223540e-11 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_left.json metrics: max_abs=3.53357332368367527e-7 atol=1.394e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json first: max_abs=1.48468898467091215e-6 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json last: max_abs=1.45355823022822186e-11 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_SR_right.json metrics: max_abs=2.26347425580364359e-7 atol=3.046e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json first: max_abs=2.64311893923702135e-7 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json last: max_abs=9.21507271658143857e-12 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_left.json metrics: max_abs=3.58077786415111898e-8 atol=1.149e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json first: max_abs=5.59837784871661248e-7 atol=3.225e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json last: max_abs=6.00185718690772740e-12 atol=3.225e-5 rtol=0.000e0
MEASURE p10_default_equalize_BR_right.json metrics: max_abs=4.03125228851108908e-7 atol=3.225e-5 rtol=0.000e0
MEASURE applied gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_left.json metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_BL_right.json metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_left.json metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_SL_right.json metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_left.json metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FC_right.json metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_left.json metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_FR_right.json metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_left.json metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_SR_right.json metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_left.json metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_normalize_BR_right.json metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_left.json metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE p10_default_final_BL_right.json first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_BL_right.json last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_BL_right.json metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE p10_default_final_SL_left.json first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_left.json last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_left.json metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE p10_default_final_SL_right.json first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_SL_right.json last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_SL_right.json metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE p10_default_final_FC_left.json first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_left.json last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_left.json metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FC_right.json metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_default_final_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_default_final_FR_left.json first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_left.json last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_left.json metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_FR_right.json metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE p10_default_final_SR_left.json first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_left.json last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_left.json metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE p10_default_final_SR_right.json first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_SR_right.json last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_SR_right.json metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE p10_default_final_BR_left.json first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_left.json last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_left.json metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE p10_default_final_BR_right.json first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BR_right.json last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE p10_default_final_BR_right.json metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 0 first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 0 last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 0 metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE hrir 1 first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 1 last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 1 metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE hrir 2 first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 2 last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 2 metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hrir 3 first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 3 last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 3 metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hrir 4 first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 4 last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 4 metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE hrir 5 first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 5 last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 5 metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hrir 6 first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 6 last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 6 metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 first: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 last: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 7 metrics: max_abs=0.00000000000000000e0 atol=0.000e0 rtol=0.000e0
MEASURE hrir 8 first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 8 last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 8 metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE hrir 9 first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 9 last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 9 metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE hrir 10 first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 10 last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 10 metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE hrir 11 first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 11 last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 11 metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hrir 12 first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 12 last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 12 metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hrir 13 first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 13 last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 13 metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hrir 14 first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 14 last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 14 metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hrir 15 first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hrir 15 last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE hrir 15 metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 0 first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 0 last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 0 metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE hesuvi 1 first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 1 last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 1 metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE hesuvi 2 first: max_abs=6.38437961214891048e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 2 last: max_abs=4.45125059438100692e-12 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 2 metrics: max_abs=2.17024282565814186e-7 atol=2.231e-5 rtol=0.000e0
MEASURE hesuvi 3 first: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 3 last: max_abs=1.45868034661948001e-11 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 3 metrics: max_abs=2.98878132272764607e-7 atol=9.494e-6 rtol=0.000e0
MEASURE hesuvi 4 first: max_abs=2.28030858827332761e-7 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 4 last: max_abs=5.25562784870475902e-12 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 4 metrics: max_abs=9.60463986121595781e-9 atol=1.024e-5 rtol=0.000e0
MEASURE hesuvi 5 first: max_abs=2.08980666223379519e-7 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 5 last: max_abs=5.18621203813096588e-12 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 5 metrics: max_abs=1.37074762825151186e-8 atol=8.979e-6 rtol=0.000e0
MEASURE hesuvi 6 first: max_abs=6.34429914946749163e-7 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 6 last: max_abs=7.78587890450229451e-12 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 6 metrics: max_abs=5.64728969976169282e-8 atol=1.729e-5 rtol=0.000e0
MEASURE hesuvi 7 first: max_abs=6.55671449203542450e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 7 last: max_abs=6.45405370834100987e-12 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 7 metrics: max_abs=6.19646749407815056e-7 atol=2.372e-5 rtol=0.000e0
MEASURE hesuvi 8 first: max_abs=1.96091707913015334e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 8 last: max_abs=5.48209945644389078e-12 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 8 metrics: max_abs=1.02289651192008502e-7 atol=1.071e-5 rtol=0.000e0
MEASURE hesuvi 9 first: max_abs=9.75113408685501781e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 9 last: max_abs=9.48913831313946790e-12 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 9 metrics: max_abs=1.53903624660473026e-7 atol=1.988e-5 rtol=0.000e0
MEASURE hesuvi 10 first: max_abs=3.91014592470654426e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 10 last: max_abs=7.03656151537832612e-12 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 10 metrics: max_abs=2.33489610954734639e-7 atol=9.101e-6 rtol=0.000e0
MEASURE hesuvi 11 first: max_abs=3.67855237752394426e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 11 last: max_abs=3.91814188179225989e-12 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 11 metrics: max_abs=2.69670424078022331e-7 atol=2.106e-5 rtol=0.000e0
MEASURE hesuvi 12 first: max_abs=1.73933266909079287e-7 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 12 last: max_abs=6.01579999307602922e-12 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 12 metrics: max_abs=2.56921405443824580e-8 atol=7.501e-6 rtol=0.000e0
MEASURE hesuvi 13 first: max_abs=4.91785109220724270e-7 atol=1.668e-5 rtol=0.000e0
MEASURE hesuvi 13 last: max_abs=4.23886716460074115e-12 atol=1.668e-5 rtol=0.000e0
MEASURE hesuvi 13 metrics: max_abs=1.34510772183821237e-7 atol=1.668e-5 rtol=0.000e0
MEASURE readme applied gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FL left: max_abs=1.77273528407795311e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FL right: max_abs=1.61004232346328990e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FR left: max_abs=2.34488211354744180e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FR right: max_abs=5.13251723504026813e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FC left: max_abs=4.48546216702538914e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection FC right: max_abs=2.19706889978965592e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BL left: max_abs=4.71899858176527687e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BL right: max_abs=8.89887082422546882e-6 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BR left: max_abs=2.56976457961854976e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection BR right: max_abs=2.48985555941771963e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SL left: max_abs=5.18924880381632647e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SL right: max_abs=4.33253058851335027e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SR left: max_abs=4.71814690072847043e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reflection SR right: max_abs=3.24008377674545045e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=3.79187204799791289e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.96922702511892567e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=3.09704830527834929e-5 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.73283961985271162e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=4.16666666667424579e-2 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.51910906834018533e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.21409831780761124e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.77186036646526190e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=6.06999877675207244e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.74305373668448738e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.06252115012262038e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.85433664580614277e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.68280774122331422e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.26625922423318116e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.58641669273151820e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.47190702671196050e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.41771818563540819e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.28098852310176881e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.80579766549726628e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.30402472050172946e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=2.42270612947947939e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=2.46506285361647315e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=1.95211817695906120e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.16165238768917334e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=4.95361223833867825e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=3.11698532630089176e-3 atol=1.000e-6 rtol=0.000e0
MEASURE readme: max_abs=5.89355973460214955e-4 atol=1.000e-6 rtol=0.000e0
MEASURE readme reverb: max_abs=1.37945112953730131e-3 atol=1.000e-6 rtol=0.000e0

thread 'golden_default_pipeline_matches_python' (248940) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
applied gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
readme applied gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
readme reflection FL left: (0, -22.574129821079822, -22.574115070168702, 1.4750911120131605e-5)
readme reflection FL right: (0, -12.833324009352722, -12.833309185027417, 1.482432530508504e-5)
readme reflection FR left: (0, -15.815924673379431, -15.815907481580497, 1.7191798933779978e-5)
readme reflection FR right: (0, -25.38700871761952, -25.386979343670625, 2.9373948894573232e-5)
readme reflection FC left: (0, -22.66227574947777, -22.6622334309761, 4.231850167002449e-5)
readme reflection FC right: (0, -18.546619159378356, -18.546599078351008, 2.0081027347629288e-5)
readme reflection BL left: (0, -16.650302376856278, -16.650256277791748, 4.609906453012513e-5)
readme reflection BL right: (0, -15.63861432937271, -15.638611387093441, 2.942279268225434e-6)
readme reflection BR left: (0, -15.187399921275341, -15.187393740182308, 6.181093032964213e-6)
readme reflection BR right: (0, -20.366108734860354, -20.366088406627867, 2.032823248754312e-5)
readme reflection SL left: (0, -23.517606178582085, -23.517572513107368, 3.366547471728154e-5)
readme reflection SL right: (0, -16.90181125062449, -16.901791264537913, 1.9986086577716833e-5)
readme reflection SR left: (0, -16.812336412275783, -16.812303353696322, 3.305857946145352e-5)
readme reflection SR right: (0, -24.248048014104697, -24.24801955081168, 2.8463293016756097e-5)
readme: (0, 88.43736271872856, 88.43740063744904, 3.791872047997913e-5)
readme reverb: (0, 683.8023125238058, 683.7993432967806, 0.0029692270251189257)
readme: (0, 64.13294983249656, 64.13291886201351, 3.097048305278349e-5)
readme reverb: (0, 754.5274935098786, 754.5247606702587, 0.0027328396198527116)
readme: (0, 85.46851568761848, 85.4401255097384, 0.02839017788008391)
readme reverb: (0, 605.7449571412951, 605.7434380322268, 0.0015191090683401853)
readme: (0, 95.53802991836014, 95.54024401667795, 0.0022140983178076112)
readme reverb: (0, 676.6843830623947, 676.6816112020282, 0.002771860366465262)
readme: (0, 75.5182389444555, 75.51763194457783, 0.0006069998776752072)
readme reverb: (0, 807.7824157085382, 807.7786726548015, 0.0037430537366844874)
readme: (0, 83.59674547570782, 83.5946829545577, 0.0020625211501226204)
readme reverb: (0, 679.2707104599413, 679.2688561232954, 0.0018543366458061428)
readme: (0, 75.15575103847235, 75.15601931924647, 0.0002682807741223314)
readme reverb: (0, 753.0876853450495, 753.0844190858253, 0.003266259224233181)
readme: (0, 64.90874597006056, 64.90858732839129, 0.00015864166927315182)
readme reverb: (0, 761.068572713383, 761.0671008063563, 0.0014719070267119605)
readme: (0, 71.4787689450028, 71.47735122681716, 0.0014177181856354082)
readme reverb: (0, 739.8950201205886, 739.8917391320655, 0.003280988523101769)
readme: (0, 90.84386855460454, 90.84567435227004, 0.0018057976654972663)
readme reverb: (0, 715.7703579354888, 715.7680539107683, 0.0023040247205017295)
readme: (0, 101.7718440835066, 101.76942137737711, 0.0024227061294794794)
readme reverb: (0, 721.038764821099, 721.0362997582454, 0.002465062853616473)
readme: (0, 67.31972840901818, 67.32168052719514, 0.0019521181769590612)
readme reverb: (0, 735.3034085874672, 735.3002469350795, 0.0031616523876891733)
readme: (0, 49.1181662020003, 49.11767084077647, 0.0004953612238338678)
readme reverb: (0, 808.683549288934, 808.6804323036077, 0.0031169853263008918)
readme: (0, 90.01264572351155, 90.01323507948501, 0.000589355973460215)
readme reverb: (0, 675.5598672861947, 675.5584878350652, 0.0013794511295373013)

---- golden_option_variants_match_python stdout ----
MEASURE p10_option_bass_boost_gain_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_bass_boost_gain_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE bass_boost_gain: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE bass_boost_gain gain: max_abs=1.14656674821134175e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json first: max_abs=5.53720320052228376e-7 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json last: max_abs=4.61144137559053665e-12 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_left.json metrics: max_abs=3.56370689913194233e-9 atol=1.469e-5 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json first: max_abs=1.29153862451179824e-7 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json last: max_abs=4.94160468161467027e-12 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_bass_boost_gain_FL_right.json metrics: max_abs=4.66875082821491261e-8 atol=4.478e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_fr_combination_method_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE fr_combination_method: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE fr_combination_method gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_fr_combination_method_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_generic_limit_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE generic_limit: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE generic_limit gain: max_abs=2.68186833496386612e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json first: max_abs=6.14961068398818533e-7 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json last: max_abs=5.15367361113210202e-12 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_left.json metrics: max_abs=5.99706439716185535e-9 atol=2.336e-5 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json first: max_abs=1.34797823210866530e-7 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json last: max_abs=5.16377300860192069e-12 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_generic_limit_FL_right.json metrics: max_abs=5.04206165369627812e-8 atol=7.086e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_room_BL_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_right.json: max_abs=4.90274487674469128e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_right.json: max_abs=2.59348098552436568e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_left.json: max_abs=3.37507799486047588e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_right.json: max_abs=2.30926389122032560e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_left.json: max_abs=2.41584530158434063e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_right.json: max_abs=2.62900812231237069e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_left.json: max_abs=5.32907051820075139e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_right.json: max_abs=1.07291953099775128e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_left.json: max_abs=9.94759830064140260e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_right.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_left.json: max_abs=2.34479102800833061e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_right.json: max_abs=3.62376795237651095e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_specific_limit_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE specific_limit: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE specific_limit gain: max_abs=2.90719308226883300e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json first: max_abs=6.64545844940907932e-7 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json last: max_abs=5.95941868894068581e-12 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_left.json metrics: max_abs=4.00138404604843956e-7 atol=1.865e-5 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json first: max_abs=1.35247016137211157e-7 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json last: max_abs=5.70423247999234038e-12 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_specific_limit_FL_right.json metrics: max_abs=6.37281631771235979e-8 atol=5.962e-6 rtol=0.000e0
MEASURE p10_option_target_level_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_target_level_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE target_level: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE target_level gain: max_abs=1.66226329614715951e-4 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json first: max_abs=9.39321658712834184e-7 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json last: max_abs=9.03231773718887905e-12 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_left.json metrics: max_abs=7.85795099760011606e-7 atol=4.095e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json first: max_abs=1.57493161767260914e-7 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json last: max_abs=9.05102864891114081e-12 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_target_level_FL_right.json metrics: max_abs=1.53124191940717802e-7 atol=1.242e-5 rtol=0.000e0
MEASURE p10_option_tilt_room_BL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_right.json: max_abs=5.18696197104873136e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_left.json: max_abs=1.77635683940025046e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_right.json: max_abs=1.27897692436818033e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_right.json: max_abs=4.97379915032070130e-14 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FC_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_left.json: max_abs=2.06057393370429054e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_right.json: max_abs=5.04485342389671132e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FL_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_left.json: max_abs=5.75539615965681151e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_right.json: max_abs=7.31859017832903191e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_FR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_left.json: max_abs=2.20268248085631058e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_right.json: max_abs=1.04449782156734727e-12 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_SR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_left.json: max_abs=3.33955085807247087e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_left.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_right.json: max_abs=4.47641923528863117e-13 atol=1.000e-9 rtol=1.000e-11
MEASURE p10_option_tilt_room_BR_right.json: max_abs=3.55271367880050093e-15 atol=1.000e-9 rtol=1.000e-11
MEASURE tilt: max_abs=0.00000000000000000e0 atol=1.000e-9 rtol=1.000e-11
MEASURE tilt gain: max_abs=1.17369345886686460e-6 atol=1.000e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json first: max_abs=3.52319026242610758e-7 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json last: max_abs=3.10351928600949211e-12 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_left.json metrics: max_abs=2.00984426301439867e-7 atol=9.206e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json first: max_abs=1.01319562003992711e-7 atol=3.512e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json last: max_abs=3.73878804191459764e-12 atol=3.512e-6 rtol=0.000e0
MEASURE p10_option_tilt_FL_right.json metrics: max_abs=2.58839480526268373e-8 atol=3.512e-6 rtol=0.000e0

thread 'golden_option_variants_match_python' (319720) panicked at crates\impulcifer-dsp\tests\golden_stages.rs:32:13:
bass_boost_gain gain: (0, -7.7342906601724355, -7.734289513605687, 1.1465667482113417e-6)
fr_combination_method gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
generic_limit gain: (0, -3.7041036454829985, -3.7041009636146636, 2.681868334963866e-6)
specific_limit gain: (0, -3.7076997402953342, -3.707696833102252, 2.907193082268833e-6)
target_level gain: (0, 1.169609756370921, 1.1694435300413062, 0.00016622632961471595)
tilt gain: (0, -8.110450897650443, -8.110449723956984, 1.1736934588668646e-6)


failures:
    golden_decay_matches_python
    golden_default_pipeline_matches_python
    golden_option_variants_match_python

test result: FAILED. 12 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.15s

error: test failed, to rerun pass `-p impulcifer-dsp --test golden_stages`
     Running tests\perf_smoke.rs (target\debug\deps\perf_smoke-666ecf9135b8d2c2.exe)

running 1 test
test bench_smoke_impulcifer_dsp ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s

     Running tests\properties_batch1.rs (target\debug\deps\properties_batch1-7e9c6e211da5537f.exe)

running 7 tests
test invalid_arguments_panic_explicitly ... ok
test statistics_edges_and_nonfinite_policy ... ok
test butter_lowpass_dc_gain_and_filter_reset ... ok
test convolution_linearity_and_identity ... ok
test real_fft_parseval_and_roundtrip ... ok
test windows_symmetry_periodicity_and_i0_limits ... ok
test fast_lengths_are_minimal_and_handle_overflow ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests\properties_batch2.rs (target\debug\deps\properties_batch2-938981aa0974d10d.exe)

running 5 tests
test find_peaks_endpoints_excluded ... ok
test batch2_rejects_invalid_arguments ... ok
test firwin2_output_symmetry ... ok
test spline_hits_knots_and_scales_to_eight_thousand ... ok
test minimum_phase_energy_concentrated ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests\properties_batch3.rs (target\debug\deps\properties_batch3-de9aba224290f3e7.exe)

running 7 tests
test batch3_rejects_invalid_arguments_and_preserves_inputs ... ok
test spectrogram_pure_sine_peaks_at_one_khz ... ok
test spectrogram_zeros_shapes_and_density_energy ... ok
test same_rate_copy_and_nnresample_error_are_distinct ... ok
test reduced_ratio_cache_is_deterministic_across_threads ... ok
test resampling_dc_preserves_mean_and_phase_periodicity ... ok
test resampled_sine_keeps_one_khz_peak ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.87s

     Running tests\properties_brir_objects.rs (target\debug\deps\properties_brir_objects-743394d60407f5b3.exe)

running 11 tests
test ingest_preserves_single_side_mapping_and_fallback ... ok
test crop_heads_keeps_itd ... ok
test decay_first_pass_clamps_windows_to_sample_count ... ok
test python_rounding_helpers_match_cpython ... ok
test stack_tracks_checks_only_selected_lengths ... ok
test shift_is_length_preserving_and_invertible_for_zeros ... ok
test stack_tracks_trims_only_trailing_pairs ... ok
test sweep_fades_reject_invalid_windows ... ok
test normalize_hits_peak_target ... ok
test align_ipsilateral_self_pair_converges ... ok
test zero_decay_target_panics_without_modifying_data ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\properties_fr.rs (target\debug\deps\properties_fr-96511186dbfbe3fb.exe)

running 6 tests
test round_half_even_matches_python ... ok
test center_band_then_center_value_is_zero ... ok
test equalize_never_exceeds_max_gain_away_from_kinks ... ok
test compensate_error_is_raw_minus_target ... ok
test interpolate_is_identity_on_its_own_grid ... ok
test read_from_csv_rejects_single_digit_numbers ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests\properties_perf.rs (target\debug\deps\properties_perf-3e8a0e1458f77402.exe)

running 6 tests
test grouped_sos_matches_sectionwise_bit_for_bit ... ok
test power_db_identity_preserves_hypot_range_and_nonfinite_masks ... ok
test cached_plans_preserve_lengths_directions_and_threads ... ok
test contiguous_polyphase_matches_scalar_zero_extension ... ok
test power_db_identity_matches_hypot_at_demo_size ... ok
test reused_real_buffers_match_fresh_plans_and_clear_inverse_padding ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s

     Running tests\properties_stages.rs (target\debug\deps\properties_stages-3cb405259d953a20.exe)

running 9 tests
test channel_balance_parse_accepts_numbers_and_rejects_words ... ok
test correction_limit_mask_is_full_hann_over_one_octave ... ok
test stage_table_total_is_eleven_by_default ... ok
test virtual_bass_guards_reject_crossover_at_nyquist ... ok
test mic_absent_center_falls_back_to_diffuse ... ok
test mic_multiple_centers_average_power ... ok
test mic_tfc_bc_only_select_frontal ... ok
test mic_deviation_skips_below_threshold ... ok
test readme_rows_sorted_by_speaker_names ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

   Doc-tests impulcifer_dsp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: 1 target failed:
    `-p impulcifer-dsp --test golden_stages`
```

## Budgets downstream of the EQ FIRs (parent, 2026-09-07)

`golden_frozen_eq_firs_isolate_downstream_noise` shows that with Python's own EQ FIRs the applied gain, README numbers and decay decisions match to 1e-13 dB and exact integers. With the Rust FIRs (inside the minimum-phase oracle-noise envelope of `README-fr.md`) the observed propagation is 2.7e-6 dB on the default gain, 1.7e-4 dB with bass boost or tilt, 0.028 dB on PNR, 2 samples on the decay knee and 0.004 dB on the window level. `golden_stages.rs` therefore budgets those quantities at about five times the observation (`GAIN_ATOL_DB`, `README_DB_ATOL`, `README_MS_ATOL`, `REVERB_MS_ATOL`, `KNEE_SAMPLES_ATOL`, `DECAY_LEVEL_ATOL_DB`); everything not downstream of an EQ FIR keeps its 1e-9 budget. The fixture family is 795 files, 21.5 MB, under the 25 MB exception the P10 packet set (the general budget is 12 MB per packet).
