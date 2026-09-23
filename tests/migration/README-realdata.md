# Real-measurement parity: 3.x vs 2.x vs LionLion123/Impulcifer

The demo goldens certify 3.x against 2.x on one measurement. This run applies
the same acceptance rule to every recording of a private measurement set
(19 measurers from the Korean BRIR community), comparing three
implementations on identical inputs:

| Label | Implementation | Environment |
|---|---|---|
| `rust` | 3.0.0-alpha.2 CLI (`target/release/impulcifer`, master `3929392`) | Rust 1.97.0, `RAYON_NUM_THREADS=1` |
| `v2` | impulcifer-py313 2.14.2 from PyPI | CPython 3.13.12, NumPy 2.5.3, SciPy 1.18.1 |
| `lion` | LionLion123/Impulcifer (GitHub HEAD) | CPython 3.8.20, NumPy 1.19.5, SciPy 1.5.4, matplotlib 3.3.4, autoeq-pkg 1.2.5 (its pinned requirements) |

The measurement set is never committed and never uploaded as a CI artifact.
Only the aggregate numbers below are recorded; measurers are M01–M19.

## Cases

Every room-recording folder of a measurer is combined with every headphone
recording (`headphones.wav`) of the same measurer: 64 room folders, 92
headphone recordings, **244 cases**. Each implementation runs in its own
directory holding hard links to the same inputs, with the same explicit sweep
(`--test_signal=data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`). Every
recording was checked to use that sweep (envelope inspection of the files the
auto-detection misread, see below). Two scenarios run for every case:

- `default`: room correction (no room measurements are present, so it is a
  no-op), headphone compensation, EQ; all defaults.
- `vbass`: default plus virtual bass at 250 Hz with explicit normal polarity
  (`--vbass --vbass_freq=250 --vbass_polarity=normal` for 3.x and 2.x,
  `--vbass 250` for LionLion, which has no automatic polarity).

## Acceptance rule

`crates/impulcifer-service/tests/demo_parity.rs` (README-service.md,
"Numerical contracts"), applied per named track of `hrir.wav` and
`hesuvi.wav` after mapping each file's track order to names (LionLion writes
all 32 hexadecagonal tracks, 2.x and 3.x trim silent extensions):

- identical sample rate and length;
- `max |a - b| <= 1e-3 * max |ref|`;
- max-abs and RMS ratios within `1e-4`;
- the same absolute-maximum sample index;
- a silent reference track stays silent.

The reference is 2.x for (3.x, 2.x) and (LionLion, 2.x), and LionLion for
(3.x, LionLion).

## Results

### default (244 cases)

| pair | within budget | worst max\|a-b\|/max\|ref\| | worst max-abs ratio dev | worst RMS ratio dev | peak index mismatches |
|---|---|---|---|---|---|
| rust~v2 | 234/244 | 7.10e-05 | 6.35e-05 | 1.86e-05 | 0 |
| lion~v2 | 233/244 | 1.69e-05 | 1.69e-05 | 1.58e-05 | 0 |
| rust~lion | 233/244 | 7.36e-05 | 6.20e-05 | 2.43e-05 | 0 |

Worst values are over the passing cases. The cases outside the budget:

- **M14 `reverb impulse` × 10 headphones**: a folder holding only
  `SL,FC,SR.wav`. All three implementations stop with the same error (no FL
  left-ear reference for onset alignment), so they agree; there is no output
  to compare.
- **M04 `room impulse/7.1` × 1 headphone**: the centre was recorded as
  `FC,X.wav` (the centre sweep, then a skipped one). LionLion reads `X` as a
  skipped sweep like the original Impulcifer and keeps FC; 2.14.2 and
  3.0.0-alpha.2 narrowed the speaker-name pattern to two or three capitals, so
  the file matched nothing and FC disappeared (12 instead of 14 tracks, and the
  normalization gain moved with it). 3.x and 2.x still agree with each other.
  Fixed in 2.14.3 and in 3.x (see the CHANGELOG); the fixed builds are compared
  below.

### vbass

Pending: filled in when the run finishes.

## Found along the way: sweep auto-detection

With `--test_signal` left at `auto`, 2.14.2 and 3.0.0-alpha.2 detected a
21.53 s and an 18.45 s sweep for M18's stereo and Auro-3D folders and a
3.08 s sweep for M02's first folder, all with high confidence, although every
file holds the standard 6.15 s sweep. The envelope threshold (peak − 40 dB)
sat under M18's noise floor, so the whole file read as one sweep, and a 1.2 s
noise burst at the start of M02's capture counted as the first onset. 2.14.3
and 3.x raise the threshold to 10 dB over the noise floor and drop regions
shorter than half the longest; all 64 room folders then resolve to the 6.15 s
sweep, except M18 stereo, whose sweeps are 10.15 s apart instead of 8.15 s and
now come back with low confidence, which falls back to the bundled 6.15 s
sweep.

## Running it

```text
python tests/migration/realdata_parity.py --data <set> --work <scratch> \
    --rust target/release/impulcifer --v2 <venv-2.14.2>/bin/impulcifer \
    --lion-python <py38>/bin/python --lion-repo <LionLion123/impulcifer> \
    --sweep data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav \
    --scenario default --scenario vbass --jobs 3
```

Each case keeps only its logs, `run.json` and `result.json`; the audio is
deleted after comparison (keeping it needs tens of gigabytes). With
`--identity-rust <other build>` the run instead checks that two 3.x builds
write byte-identical `hrir.wav`/`hesuvi.wav` on every case; `--match` limits
the cases to labels containing a string.
