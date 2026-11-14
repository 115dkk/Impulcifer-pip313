# SOFA export (ADR 0019)

The decision and its reasons are in `docs/adr/0019-sofa-export.md`. This file is the specification the code follows.

## Files

| setting | files saved in the result folder |
| --- | --- |
| `sofa = false` (default) | none (every other output byte-identical to before) |
| `sofa = true` | `brir.sofa` |
| `sofa = true`, `sofa_split_headphone_eq = true`, headphone compensation used | `brir.sofa`, `brir-no-headphone-eq.sofa`, `headphone-eq.wav` |
| `sofa = true`, `sofa_split_headphone_eq = true`, headphone compensation not used | `brir.sofa`; the job log says why the split files were not saved |

`sofa_split_headphone_eq` without `sofa` does nothing. "Headphone compensation used" means `do_headphone_compensation` is on and a headphone compensation exists (from `headphones.wav` or a headphone measurement file, ADR 0011).

## Content

- **Speakers:** exactly the speakers of `hrir.wav`: the HRIR that `fill_output` returns for the `hrir.wav` targets (measured speakers, plus the ones filled when `missing_speakers = "neighbor"`), every speaker that has both ears, in `SLOTS` order. `LFE` and `LFE2` are never included.
- **Samples:** the same `f64` samples that `hrir.wav` is written from (after resampling, before WAV quantization), at the output sample rate. All measurements of one file have the same length N; when lengths differ the shorter ones are padded with zeros at the end.
- **Positions:** `SourcePosition` row = `[azimuth, elevation, 2.0]` from the slot's `SLOTS` entry (degrees, ITU convention, positive azimuth to the left; this is SOFA's spherical convention).
- **Split files:** `brir-no-headphone-eq.sofa` holds the BRIR without the headphone term, same speakers, positions, length and sample rate as `brir.sofa`. `headphone-eq.wav` holds two tracks (left ear, right ear) as 32-bit IEEE float at the output sample rate (`write_wav_float32`). The other WAVs are 32-bit integer PCM, but a boosting headphone EQ FIR has samples above full scale (about 1.96 on the demo), and integer PCM clipped them: the split agreement below then missed by 14.8 dB. Float keeps the FIR exact without a gain change.

## Headphone split

The equalization curve of one speaker and ear is `room + headphone + user EQ - target`, smoothed and limited (`stages/equalize.rs::equalization_curve`), turned into one minimum-phase FIR. The split does not try to factor that FIR:

1. **BRIR without the headphone term:** a copy of the HRIR taken just before the Equalize stage is equalized with the same inputs except `hp = None`.
2. **Headphone EQ:** for each ear, a curve built like `equalization_curve` from the headphone term alone (no room, no user EQ, target 0 dB), with the same smoothing and the same `equalize` parameters, turned into a minimum-phase FIR with the same call (`minimum_phase_impulse_response(fs, 5.0, false)`).
3. **Later stages on the copy:**
   - Decay: the same targets as the final BRIR (`adjust_decay`).
   - Channel balance: the correction FIRs computed on the final BRIR, applied unchanged to the copy. `correct_channel_balance` is split into a function that computes the per-group FIRs and one that applies them; the final BRIR's bytes do not change.
   - Normalize: the final BRIR's gain (`applied_gain_db`), applied unchanged.
   - Resample: the copy is resampled like the final BRIR and then receives the final BRIR's second normalization gain unchanged. The headphone EQ FIR is resampled to the output rate with the same resampler and keeps its level.
4. **Agreement:** with the demo data and default settings, convolving each ear of `brir-no-headphone-eq.sofa` with the matching `headphone-eq.wav` track gives the `brir.sofa` response within ±1.0 dB on a 1/3-octave smoothed magnitude between 100 Hz and 12 kHz, for every speaker and ear. A test checks this. If the demo does not meet it, report the measured deviation; do not widen the tolerance without the maintainer.

## Convention: SimpleFreeFieldHRIR 1.0 (SOFA 2.1)

Global attributes (all character strings):

| attribute | value |
| --- | --- |
| `Conventions` | `SOFA` |
| `Version` | `2.1` |
| `SOFAConventions` | `SimpleFreeFieldHRIR` |
| `SOFAConventionsVersion` | `1.0` |
| `APIName` | `Impulcifer` |
| `APIVersion` | the workspace version (e.g. `3.21.0`) |
| `ApplicationName` | `Impulcifer` |
| `ApplicationVersion` | the workspace version |
| `AuthorContact` | `unknown` |
| `Organization` | `unknown` |
| `License` | `No license provided, ask the author for permission` |
| `DataType` | `FIR` |
| `RoomType` | `free field` |
| `Title` | `Impulcifer BRIR` (split file: `Impulcifer BRIR without headphone compensation`) |
| `DatabaseName` | `Impulcifer` |
| `ListenerShortName` | `listener` |
| `Comment` | `Binaural room impulse responses measured in a room with Impulcifer, stored as SimpleFreeFieldHRIR for player compatibility. Source distance is nominal. Headphone compensation is included.` (split file: `... Headphone compensation is not included; see headphone-eq.wav.`) |
| `DateCreated`, `DateModified` | `YYYY-MM-DD HH:MM:SS` in UTC: `SOURCE_DATE_EPOCH` seconds when that environment variable is set, otherwise now |

The user's name is never written.

Dimensions: `I = 1`, `C = 3`, `R = 2`, `E = 1`, `N` = samples, `M` = measurements. No string dimension.

Variables (all IEEE `f64`):

| variable | dimensions | value | attributes |
| --- | --- | --- | --- |
| `ListenerPosition` | I, C | `[0, 0, 0]` | `Type = cartesian`, `Units = metre` |
| `ListenerUp` | I, C | `[0, 0, 1]` | |
| `ListenerView` | I, C | `[1, 0, 0]` | `Type = cartesian`, `Units = metre` |
| `ReceiverPosition` | R, C, I | `[0, 0.09, 0]`, `[0, -0.09, 0]` | `Type = cartesian`, `Units = metre` |
| `SourcePosition` | M, C | one row per speaker | `Type = spherical`, `Units = degree, degree, metre` |
| `EmitterPosition` | E, C, I | `[0, 0, 0]` | `Type = cartesian`, `Units = metre` |
| `Data.IR` | M, R, N | receiver 0 = left ear, 1 = right ear | |
| `Data.SamplingRate` | I | output rate | `Units = hertz` |
| `Data.Delay` | I, R | `[0, 0]` | |

pyfar `sofar` 1.3.0 accepts exactly this set (checked 2026-11-14 with `verify`).

## HDF5 container

The file must open in libmysofa (and its Rust port `sofar` 0.4.0), in netCDF-C, and in libhdf5. libmysofa accepts much less of HDF5 than libhdf5 does, so the writer follows the layout netCDF-C produces. `crates/impulcifer-io/tests/fixtures/sofa_netcdf_c_reference.sofa` is such a file (written by netCDF-C through pyfar `sofar`, synthetic data: M = 3, N = 16, sample (m, r, n) = 0.1(m+1) + 0.01r + 0.001n). Read its structure; the public HDF5 file format specification (https://support.hdfgroup.org/documentation/hdf5/latest/_f_m_t3.html) and libmysofa's reader (`src/hdf/*.c`, BSD-3-Clause, https://github.com/hoene/libmysofa) are the references.

Requirements:

- **Groups store their links densely:** a Link Info message with a fractal heap (the link messages are its managed objects) and a version 2 B-tree name index (record type 5). No Link message (type 6) anywhere: libmysofa rejects the whole file when it meets one. The root group is the only group.
- **The global attributes are dense too:** an Attribute Info message with a second fractal heap (the attribute messages are its managed objects) and a version 2 B-tree name index (record type 8). libmysofa refuses a compact Attribute message whose datatype is larger than 64 bytes (`src/hdf/dataobject.c`), and the `Comment` string is longer than that. The root object header holds only Link Info, Group Info and Attribute Info, so it needs no Continuation message. Dataset attributes are short and stay compact.
- **The global heap collection ends below 64 KiB:** libmysofa 1.3.2 keeps the collection reader's file position in a 16-bit variable (`src/hdf/gcol.c`), so a collection placed after the data loses every `DIMENSION_LIST` reference. The writer puts the object headers, both heaps and B-trees and the collection first and the contiguous data after them (`global_heap_ends_below_64_kib`).
- **Only object header messages libmysofa reads:** NIL (0), Dataspace (1), Link Info (2), Datatype (3), Fill Value old/new (4/5), Data Layout (8), Group Info (10), Filter Pipeline (11), Attribute (12), Continuation (16), Attribute Info (21). Use the message versions libmysofa parses (for example Datatype version 1 or 3, Data Layout version 3). Object header version 2 with checksums, no timestamps.
- **netCDF-4 dimensions:** one dataset per dimension named `I`, `C`, `R`, `E`, `N`, `M` with `CLASS = "DIMENSION_SCALE"`, `NAME = "This is a netCDF dimension but not a netCDF variable."` followed by the size as netCDF-C writes it, `_Netcdf4Dimid`, and `REFERENCE_LIST`. Every variable carries `DIMENSION_LIST` (variable-length object references held in a global heap collection), as netCDF-C writes it. The root group carries `_NCProperties` with the value of the reference fixture, so netCDF-C treats the file as netCDF-4.
- **Contiguous data,** no filters, little-endian.
- **Deterministic:** the same input gives the same bytes. Objects are laid out in a fixed order and no timestamps or random values are written.

## Validation

- `cargo test -p impulcifer-io`: files from the writer open in `sofar` 0.4.0 (a dev-dependency) with the right M, N, sample rate, positions and samples; the netCDF-C reference fixture opens too; writing twice gives identical bytes; no Link message; the global heap ends below 64 KiB.
- The Rust `sofar` port opens files that libmysofa refuses (it accepted both problems above), so it is not enough on its own. For a local check against the real library, install `libmysofa-utils` and run `SOFA_SAMPLE_DIR=<dir> cargo test -p impulcifer-io --test sofa -- --ignored write_sample_files_for_external_readers`, then `mysofa2json -c` on each file in `<dir>`.
- `cargo test -p impulcifer-service`: the demo with `sofa` and the split; speakers and samples match `hrir.wav`; the split agreement above.
- CI, `brir-3x.yml`: the demo output is checked with pyfar `sofar` (`read_sofa(verify=True)`) and opened with libmysofa (`mysofa2json -c` from the distribution's `libmysofa-utils`); the integrity script hashes `.sofa` files with `SOURCE_DATE_EPOCH` fixed.
