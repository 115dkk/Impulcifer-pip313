# P08 BRIR object golden format

`export_goldens_brir.py` imports and executes the repository's 2.x Python
objects. It writes only `goldens/p08_*`. Run it with the existing Python
requirements installed; the reference run used Python 3.13.3, NumPy 2.4.6,
SciPy 1.18.0 and nnresample 0.2.4.1. Python here denotes the interpreter;
2.x denotes the Impulcifer application being used as the oracle.

Each JSON document contains `inputs`, `outputs`, and `meta`, following the
read-only shared exporter conventions. Nonfinite numbers are strings
`-inf`, `+inf`, or `nan`. JSON files are below 200,000 bytes. Arrays are
headerless, little-endian IEEE float64 `.f64` files. Descriptors contain
`file`, `length`, `sha256`, first/last 256 values, max absolute value,
absolute argmax, and RMS. Identical full arrays share a binary file. Hashes
are diagnostics: Rust compares numerical samples, not rounded hashes.

Sequence fixtures store the actual Python-generated shape, per-track
endpoints/hash, and active interval observed from the Python output. Every
active interval is asserted identical to a slice of the full Python sweep
before export. This lossless representation avoids materializing repeated
sweeps and long silent regions; tests reconstruct the full reference from
those oracle-recorded intervals, independently of Rust sequence routing.

## Demo-stage protocol

The exporter copies `data/demo` into a `TemporaryDirectory` before opening
any demo recording. It selects speaker-list WAV names and sorts filenames.
The recorded order is `BL,SL.wav`, `FC.wav`, `FL,FR.wav`, `SR,BR.wav`.
It constructs the estimator through the Python `from_wav` method using the
bundled `sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`.

The stages are open, `crop_heads(1)`, ipsilateral alignment with canonical
pairs and 30 ms segments, default FL-left onset alignment, `crop_tails`, and
`normalize(peak_target=-0.1)`. Snapshots immediately follow every stage.
Every track records length, first peak, max absolute value, absolute argmax,
RMS, first/last 256 samples, and full-byte SHA-256. FL left/right also retain
full arrays after open and tail crop. Stage returns store the tail length
and normalization gain. Stack fixtures preserve both canonical layout
orders, trimmed track hashes and compacted names.

IR and decay fixtures use the head-cropped state before either alignment.
Shift, crop, equalization, magnitude, and decay-window outputs retain full
arrays. Decay includes FL/FR/FC both ears, the short-input guard and seeded
exponential noise with nominal RT60 0.5 seconds. `sys.settrace` captures
first-pass `windows`, `t_windows`, and `noise_floor` directly from the Python
function's locals rather than from another formula implementation.
Forced length/value repair fixtures call Python `from_wav` on temporary
DOUBLE WAV files. No bundled 44.1 kHz sweep exists in this checkout.

## Known packet conflicts

The mandatory raw full-array inventory cannot fit below 12 MB. Just the
48 kHz sweep (2,362,160 bytes), inverse (2,362,160), synthetic estimate
(2,386,160), and two post-open FL IRs (6,260,320) total **13,370,800 bytes**,
before any other mandatory fixture. Export reports this conflict explicitly.
The complete full-array fixture set is larger; no precision or tolerance
is reduced to satisfy the storage limit.

The fixed `Hrir` type has no estimator field, so `crop_heads` cannot check
whether the original estimator rate differs after resampling. Callers must
maintain that precondition. `open_recording_samples` and `crop_tails` can
and do enforce rate equality because they receive an estimator.

The fixed infallible `compact_tracks` tuple signature returns empty vectors
for all-silent input; Python raises ValueError. Decay adjustment returns
None when no usable decay time exists or the calculated window length is
negative; Python raises in those cases. These are explicit differences,
not claimed parity. Other invalid input guards map to InvalidArgument or
existing primitive assertions as appropriate.

Python's adjustment loop selects the last consecutively available decay
time, stopping at the first missing/zero time; the survey's first-available
wording is inaccurate. Duplicate sequence names are accepted and the final
assignment replaces an earlier duplicate's entire row. Single-side ingest
retains the `i // 2` mapping and overwrite behavior. Both fallback splits
operate on the already silence-cropped recording. From-samples value repair
preserves constructor duration. Lags retain Python's first-segment-length
origin even when segment lengths differ. None of these quirks is corrected.

## Verification

Run the seven commands in P08 in the foreground. Rust golden tests read
recording files via the dev-only `impulcifer-io` dependency. Use the additional
`-- --nocapture --test-threads=1` arguments to print per-comparison maximum
errors and the PCM32 mismatch count. Quantization is inline ties-to-even
rounding of `x * 2^31`, saturated to i32. The sweep contains 295,270 samples
(the packet's 295,000 figure is approximate). No tolerance is widened.
