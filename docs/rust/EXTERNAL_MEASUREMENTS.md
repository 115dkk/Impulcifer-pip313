# Measurements from REW and Dirac Live files

ADR 0010 decides that a REW measurement file (`.mdat`) or a Dirac Live project
(`.liveproject`) can replace the `room-*.wav` recordings of a measurement folder.
ADR 0011 extends that to the binaural speaker recordings (the BRIR measurement)
and to `headphones.wav`. This document is the specification of both readers, of
the automatic speaker/ear assignment for each purpose and of the map syntax. Everything here was established
from public encodings and from files: the Java Object Serialization
Specification (chapter 6, stream protocol), the protobuf wire format, Qt's
`QDataStream` format, files that REW 5.40 beta 135 wrote through its local API,
and the maintainer's own Dirac Live projects. No REW or Dirac program code was
read, decompiled or hooked. Keep it that way: do not consult Dirac or REW
binaries, decompilations or hooking material when changing these readers.

The readers live in `crates/impulcifer-io/src/external/`. They parse structure
only and never execute anything from the file.

## Shared result

```rust
pub enum ExternalFormat { Rew, Dirac }

pub struct ExternalFile {
    pub format: ExternalFormat,
    pub title: Option<String>,          // Dirac project name; None for REW
    pub sample_rate: u32,               // every measurement shares it
    pub microphone: Option<String>,     // Dirac recording device name
    pub mic_calibration: Option<MicCalibration>,
    pub speakers: Vec<ExternalSpeaker>, // Dirac columns; empty for REW
    pub positions: Vec<usize>,          // Dirac measured positions, 1-based; empty for REW
    pub measurements: Vec<ExternalMeasurement>,
}
pub struct ExternalSpeaker { pub index: usize /* 1-based */, pub name: String, pub subwoofer: bool }
pub struct MicCalibration { pub frequency: Vec<f64>, pub gain_db: Vec<f64> }
pub struct ExternalMeasurement {
    pub id: String,              // REW "1".."N"; Dirac "<speaker>@<position>", both 1-based
    pub name: String,            // REW measurement title; Dirac speaker name
    pub speaker: Option<usize>,  // Dirac speaker index (1-based); None for REW
    pub position: Option<usize>, // Dirac position (1-based); None for REW
    pub start_time: f64,         // seconds of sample 0 (REW startTime; 0.0 for Dirac)
    pub samples: Vec<f64>,       // the impulse response, empty when skipped
    pub skipped: Option<Skip>,   // Some(NoImpulseResponse) for a REW measurement without one
}
```

`read_external(path) -> Result<ExternalFile, ExternalError>` detects the format
from the content, not the extension: the Java stream magic `AC ED 00 05` is REW,
anything else is tried as a Dirac project. Errors carry a short English message
(`not a REW measurement file`, `no measurement block found`, …) and never panic.

## Limits for untrusted input

- A file larger than 1 GiB is refused before reading.
- An array or string length is checked against the bytes that remain before any
  allocation; an element count times the element size must not exceed them.
- Nesting (objects inside objects, protobuf messages inside messages) stops at
  depth 64; the Java handle table stops at 4 000 000 entries; a Dirac response
  matrix may have at most 4096 elements and 1 048 576 samples per element.
- Any violation is an `ExternalError`, never a panic or an abort.

## REW `.mdat`

### Stream

The file is a Java object serialization stream (protocol version 2, the
default since Java 1.2). Implement the grammar of the specification's section
6.4 with these constants:

| Name | Value | | Name | Value |
| --- | --- | --- | --- | --- |
| `STREAM_MAGIC` | `0xACED` | | `TC_BLOCKDATA` | `0x77` |
| `STREAM_VERSION` | `5` | | `TC_ENDBLOCKDATA` | `0x78` |
| `TC_NULL` | `0x70` | | `TC_RESET` | `0x79` |
| `TC_REFERENCE` | `0x71` | | `TC_BLOCKDATALONG` | `0x7A` |
| `TC_CLASSDESC` | `0x72` | | `TC_EXCEPTION` | `0x7B` |
| `TC_OBJECT` | `0x73` | | `TC_LONGSTRING` | `0x7C` |
| `TC_STRING` | `0x74` | | `TC_PROXYCLASSDESC` | `0x7D` |
| `TC_ARRAY` | `0x75` | | `TC_ENUM` | `0x7E` |
| `TC_CLASS` | `0x76` | | `baseWireHandle` | `0x7E0000` |

Class description flags: `SC_WRITE_METHOD 0x01`, `SC_SERIALIZABLE 0x02`,
`SC_EXTERNALIZABLE 0x04`, `SC_BLOCK_DATA 0x08`, `SC_ENUM 0x10`.

- Integers and floats are big-endian. Strings are a `u16` (or `u64` for
  `TC_LONGSTRING`) byte length followed by Java's modified UTF-8: NUL is
  `C0 80`, and a character outside the BMP is a surrogate pair written as two
  three-byte sequences (`ED A0..AF xx`, `ED B0..BF xx`). Decode a valid pair to
  the one character it stands for and any malformed sequence to U+FFFD. The
  fixture's second measurement is named `FL 오른쪽 🎧` and must decode exactly.
- Handles are assigned in the order the specification gives (class
  descriptions, objects before their class data, strings, arrays, enums,
  classes); `TC_REFERENCE` reads a `u32` handle. `TC_RESET` clears the table.
- An object's class data runs from the top superclass down. For a serializable
  class the field values come first in the order of the class description; with
  `SC_WRITE_METHOD` an object annotation follows (contents up to
  `TC_ENDBLOCKDATA`). An externalizable class with `SC_BLOCK_DATA` has only the
  annotation; without it the stream cannot be parsed and is an error.
  `TC_EXCEPTION` and a `TC_PROXYCLASSDESC` object are errors.
- Primitive field type codes are `B C D F I J S Z`; `L` and `[` fields are read
  as contents. Primitive arrays are kept as typed vectors (`[F` as `Vec<f32>`,
  and so on); arrays of objects as values.

The reader returns the top-level contents in order: values (objects, strings,
arrays, enums, null) and block data.

### Measurements

REW 5.40 writes: the string `REW Measurement Data File V2`, block data, the
string `Notes:`, block data whose `i32` is the number of measurements, then one
`javax.swing.ImageIcon` and one notes string per measurement, then per
measurement block data and an object of class `roomeqwizard.MeasData`, and a
final block data. Rely only on this: the first content is a string starting
with `REW Measurement Data File`, and every top-level object of class
`roomeqwizard.MeasData`, in order, is one measurement. Its id is its 1-based
position among them.

A field is looked up by name across the object's class hierarchy. From each
`MeasData`:

| Value | Path | Type |
| --- | --- | --- |
| name | `shortDesc` | string |
| sample rate | `sampleRate` | int |
| samples | `irData` → `ir` → `data` | `[F` |
| start time | `irData` → `ir` → `startTime` | double |
| sample interval | `irData` → `ir` → `T` | double |

`irData` is a `roomeqwizard.IRData` and `ir` a `roomeqwizard.SampledData`.
When `irData`, `ir` or `data` is null or `data` is empty, the measurement is
kept with `skipped = NoImpulseResponse` (REW holds frequency-response-only
measurements, for example imported text files). Otherwise `T` must be positive
and `1 / T` within one part in a million of `sampleRate`, and every sample must
be finite. All measurements that are not skipped must share one sample rate;
the file's `sample_rate` is that rate (or the first measurement's when all are
skipped).

The stored samples are the effective impulse response: REW rewrites them when
the user inverts a measurement or shifts its time zero (checked with REW's
Invert and Offset t=0 commands), and `startTime` is the time of sample 0. Do not
apply `inverted`, `invertImpulse`, `cumulativeStartTimeOffset`, SPL offsets or
calibration flags.

## Dirac Live `.liveproject`

### Container

The file starts with a Qt `QDataStream` header (big-endian): a `u32` format
version (10 in every observed file) followed by `QString` values. A `QString`
is a `u32` byte length, `0xFFFFFFFF` for null, then UTF-16BE code units. The
first `QString` is the project title; decode it, and ignore the other header
values.

The measurement block is a `u32` big-endian byte length followed by a protobuf
message. Its offset varies with the header strings, so find it: for each offset
`p` from 4 up to 65 536 (or the end of the file), read `n` = `u32` BE at `p`;
the block is found at the first `p` where `16 <= n <= len - p - 4` and the `n`
bytes after it form a well-formed message (below) with a well-formed response
matrix in field 12. Nothing after the block is read: in every observed file the
next block's bytes have an entropy of 8.0 bits per byte (compressed or
encrypted) and the project's measurements do not depend on it.

Protobuf wire format: a key is a varint, `field = key >> 3` (1 to 2^29 − 1) and
`wire type = key & 7`; types 0 (varint, at most 10 bytes), 1 (8 bytes, little-
endian), 2 (varint length, then bytes) and 5 (4 bytes, little-endian) are
accepted, types 3 and 4 (groups) make the message malformed. A message is
well-formed when its keys and values exactly fill its bytes. Repeated fields
appear as repeated keys.

### Fields of the measurement message

| Field | Wire | Meaning |
| --- | --- | --- |
| 3 | varint | sample rate in Hz |
| 12 | bytes | response matrix (below) |
| 14 | bytes | microphone calibration frequencies, packed `f32` little-endian, Hz |
| 15 | bytes | microphone calibration gains, packed `f32` little-endian, dB |
| 24 | bytes | recording device name, UTF-8 (the microphone, e.g. `마이크 (Umik-1  Gain: 18dB)`) |
| 30 | bytes, repeated | one speaker per column, in column order |

Speaker message: field 1 is the name (UTF-8, e.g. `Left`, `LeftSurround`,
`Subwoofer`), field 2 a varint kind (1 for every full-range speaker and 4 for
every subwoofer in the observed files), field 5 the varint high cutoff in Hz
(20 000 full range, 200 subwoofer). A speaker is a subwoofer when its kind is 4
or its name is a subwoofer name (see below). The number of speakers must equal
the matrix's columns.

The calibration is used when fields 14 and 15 have the same, non-zero number of
values; points whose frequency is not positive are dropped (the observed files
start at 0 Hz).

### Response matrix (field 12)

| Field | Wire | Meaning |
| --- | --- | --- |
| 1 | varint | sample rate, equal to the measurement's field 3 |
| 2 | varint | columns: speakers |
| 3 | varint | rows: measurement positions (9 in every observed file) |
| 4 | bytes, repeated | elements, `columns × rows` of them |

Elements are in row-major order: element `row * columns + column` is the
response of speaker `column` at position `row` (subwoofer columns are the only
low-passed ones, which fixes the order). An element is a message with field 1
(varint sample rate), field 5 (`f32` little-endian scale) and field 6 (bytes:
`i16` big-endian codes, 32 769 of them in the observed files). An element
without field 6 is a position that was not measured; a position is listed in
`positions` when any of its elements has data. Decode a code `c` as

```text
y = clamp(c / 32767, -1, 1)
x = scale * sign(y) * (256^|y| - 1) / 255
```

μ-law with μ = 255 (ADR 0010, point 3). Measurement ids are
`"<column + 1>@<row + 1>"`, one per element with data, ordered by row then
column; `name` is the speaker's name, `start_time` is 0.

## Automatic assignment

A room measurement feeds one speaker slot and one ear position (left or right)
or both. The automatic assignment is computed in `impulcifer-io` and is what
the service uses when `room_measurement_map` is empty or `auto`.

### Speaker names

Normalise a label by lowercasing it and removing spaces, `_`, `-` and `.`.
It names a slot when it equals one of the 40 slot codes or one of these aliases:

| Slot | Aliases |
| --- | --- |
| FL | `left`, `l`, `frontleft`, `leftfront` |
| FR | `right`, `r`, `frontright`, `rightfront` |
| FC | `center`, `centre`, `c`, `frontcenter`, `frontcentre`, `centerfront` |
| SL | `leftsurround`, `surroundleft`, `leftside`, `sideleft`, `leftsidesurround`, `ls`, `lss` |
| SR | `rightsurround`, `surroundright`, `rightside`, `sideright`, `rightsidesurround`, `rs`, `rss` |
| BL | `leftback`, `backleft`, `leftrear`, `rearleft`, `leftbacksurround`, `leftrearsurround`, `surroundbackleft`, `lb`, `lrs`, `lsb`, `lbs` |
| BR | `rightback`, `backright`, `rightrear`, `rearright`, `rightbacksurround`, `rightrearsurround`, `surroundbackright`, `rb`, `rrs`, `rsb`, `rbs` |
| WL | `leftwide`, `wideleft`, `frontwideleft`, `lw`, `flw` |
| WR | `rightwide`, `wideright`, `frontwideright`, `rw`, `frw` |
| HFL | `leftfrontheight`, `frontleftheight`, `frontheightleft`, `heightfrontleft`, `leftheight`, `lfh`, `flh`, `fhl` |
| HFR | `rightfrontheight`, `frontrightheight`, `frontheightright`, `heightfrontright`, `rightheight`, `rfh`, `frh`, `fhr` |
| HFC | `centerfrontheight`, `frontcenterheight`, `centerheight`, `centreheight`, `cfh`, `fch`, `fhc` |
| HBL | `leftbackheight`, `leftrearheight`, `backheightleft`, `rearheightleft`, `lbh`, `lrh`, `blh` |
| HBR | `rightbackheight`, `rightrearheight`, `backheightright`, `rearheightright`, `rbh`, `rrh`, `brh` |
| TFL | `topfrontleft`, `lefttopfront`, `leftfronttop`, `lft`, `ltf` |
| TFR | `topfrontright`, `righttopfront`, `rightfronttop`, `rft`, `rtf` |
| TSL | `topmiddleleft`, `lefttopmiddle`, `leftmiddletop`, `topsideleft`, `lmt`, `ltm`, `tml` |
| TSR | `topmiddleright`, `righttopmiddle`, `rightmiddletop`, `topsideright`, `rmt`, `rtm`, `tmr` |
| TBL | `toprearleft`, `topbackleft`, `lefttoprear`, `leftbacktop`, `leftreartop`, `lbt`, `ltr`, `ltb` |
| TBR | `toprearright`, `topbackright`, `righttoprear`, `rightbacktop`, `rightreartop`, `rbt`, `rtr`, `rtb` |
| TC | `top`, `topcenter`, `topcentre`, `centertop`, `voiceofgod`, `vog` |

A subwoofer name is `sub`, `subwoofer` or `lfe`, optionally followed by digits
(`sub1`, `subwoofer2`, `lfe1`). A subwoofer never gets a slot.

### Dirac projects

Each speaker gets the slot its whole name names (`Left` → FL,
`LeftSurround` → SL); a subwoofer or an unknown name gets none, and a slot
already taken by an earlier column is not assigned again. Both ear positions of
a slotted speaker use position 1 when it was measured, otherwise the lowest
measured position.

### REW files

Split the title into tokens at whitespace and at `_ - , . ; : / ( ) [ ]`, then
lowercase them. An ear word is `left`, `right`, `l`, `r`, `le`, `re`, or one of
`왼쪽 왼 좌 좌측` (left) and `오른쪽 오른 우 우측` (right). The words `left`,
`right`, `l` and `r` can name a speaker (FL, FR) or an ear; the Korean words
only an ear.

1. A token that names a slot and is not one of `left right l r` is the speaker
   (the first such token).
2. If there is one, the ear is the first ear word, or none.
3. If there is none, the first of `left right l r` is the speaker (FL or FR)
   and the ear is the next ear word after it, or none.
4. A title with a subwoofer token, or with no speaker, is not assigned.

A measurement without an ear feeds both ear positions. Measurements are taken
in file order and a side that is already assigned is not assigned again, so a
measurement for both ears fills only the free sides. Skipped measurements are
never assigned. Examples: `FL left` → FL left; `FL 오른쪽 🎧` → FL right;
`L P1` → FL both; `R ART` → FR both; `C 왼쪽 귀` → FC left; `LS R` → SL right;
`secs aver` → not assigned.

## Map syntax

`room_measurement_map` is empty, `auto`, or a comma-separated list of entries
`SLOT=ID` (both ear positions) or `SLOT.left=ID` / `SLOT.right=ID`. `SLOT` is a
slot code (any case, normalised to upper case), `ID` a measurement id of the
file. Whitespace around entries and around `=` is ignored. It is an error when
an id is unknown or skipped, a slot code is unknown, or two entries assign the
same slot and side. An explicit map may assign any measurement, a subwoofer's
included. `format_map(entries)` writes the canonical form: entries in slot order
(the 40-slot order), `left` before `right`, and `SLOT=ID` when both sides use
the same id.

## Purposes

The same file can serve three purposes, each with its own map:

| Purpose | Config fields | Replaces | Entries |
| --- | --- | --- | --- |
| `room` | `room_measurement_file`, `room_measurement_map` | `room-*.wav` | `SLOT=ID`, `SLOT.left=ID`, `SLOT.right=ID` |
| `brir` | `brir_measurement_file`, `brir_measurement_map` | the speaker recordings (`FL,FR.wav`, …) | `SLOT.left=ID`, `SLOT.right=ID` |
| `headphones` | `headphone_measurement_file`, `headphone_measurement_map` | `headphones.wav` | `left=ID`, `right=ID` |

Everything above this section is the `room` purpose, and it does not change.
In code (`impulcifer_io::external::assign`):

```rust
pub enum Purpose { Room, Brir, Headphones }      // parse("room" | "brir" | "headphones"), as_str()
pub fn auto_assign_for(file: &ExternalFile, purpose: Purpose) -> Vec<Assignment>;
pub fn parse_map_for(file: &ExternalFile, map: &str, purpose: Purpose) -> Result<Vec<Assignment>, ExternalError>;
pub fn format_map_for(assignments: &[Assignment], purpose: Purpose) -> String;
pub fn check_assignments(assignments: &[Assignment], purpose: Purpose) -> Result<(), ExternalError>;
```

`auto_assign`, `parse_map` and `format_map` stay as the `room` versions. A
headphone assignment is expressed with the slots of `headphones.wav`: the left
ear is `Assignment { slot: "FL", side: Left, .. }`, the right ear
`Assignment { slot: "FR", side: Right, .. }`.

### Ear positions in a Dirac project

A Dirac project has one microphone input, so a binaural measurement puts the two
ears at two positions. For `brir` and `headphones` the left ear uses the lowest
measured position and the right ear the next one; with a single measured
position both ears use it (which `check_assignments` then rejects for `brir`,
because one measurement cannot serve two ears). The `room` purpose keeps its
rule (position 1, else the lowest, for both ears).

### Automatic assignment for `brir`

- REW: the title rules of "REW files" above, but only titles with an ear give an
  assignment (a measurement without an ear is not assigned). The first
  measurement for a slot and side wins.
- Dirac: every speaker that gets a slot under "Dirac projects" above and has a
  non-skipped measurement at the left-ear position gets `SLOT.left` from it,
  and likewise `SLOT.right` from the right-ear position.
- The result may be incomplete (a slot with one ear). It is returned as is so
  that a screen can show it; `check_assignments` reports it.

### Automatic assignment for `headphones`

- REW: a measurement's ear is the first token (title split as above) that is
  an ear word, else the first token whose `slot_for_label` is FL (left) or FR
  (right). Measurements are taken in file order and the first one for each ear
  wins. When no usable measurement has an ear and there are exactly two usable
  measurements, the first is left and the second right.
- Dirac: the first speaker whose slot is FL at the left-ear position, and the
  first speaker whose slot is FR at the right-ear position, each when that
  measurement exists and is not skipped.

### Map syntax for `brir` and `headphones`

The common rules of "Map syntax" apply (unknown or skipped id, unknown slot, a
slot and side given twice). In addition:

- `brir` accepts only `SLOT.left=ID` and `SLOT.right=ID`; a bare `SLOT=ID` is
  an error (`a BRIR map entry names one ear: use SLOT.left=ID or SLOT.right=ID`).
- `headphones` accepts only `left=ID` and `right=ID` (any case, whitespace
  ignored), each at most once.
- `format_map_for(_, Brir)` writes `SLOT.left=ID,SLOT.right=ID` in slot order
  and never merges two sides into `SLOT=ID`. `format_map_for(_, Headphones)`
  writes `left=ID,right=ID` (whichever exist, left first).

### Checks

`check_assignments` is what a job requires before it uses the assignment; the
inspection does not call it.

- `room`: nothing (the room loader keeps its own "nothing assigned" error).
- `brir`: at least one assignment (`no BRIR measurements assigned`); every slot
  has both sides (`speaker slot FL has no right ear measurement`); no id appears
  twice (`measurement 3 is assigned more than once`); FL is present
  (`the BRIR measurements must include FL, the reference for alignment`).
- `headphones`: both ears present (`headphone measurement needs a left and a
  right ear; the right ear is missing`); the two ids differ (`measurement 3 is
  assigned more than once`).

### Building the BRIR responses

For `brir` the service places all assigned responses on one time axis before it
builds the `Hrir`, so that each speaker keeps the time difference between its
two ears:

1. `t0` is the smallest `start_time` among the assigned measurements (REW;
   Dirac is always 0).
2. Each response gets `round((start_time - t0) * sample_rate)` leading zeros.
   A difference above 10 seconds is an error (`measurements are more than 10 s
   apart in time`).
3. All responses are cut to the shortest padded length. If that length is not
   larger than the largest padding, the responses do not overlap and it is an
   error (`measurements do not overlap in time`).
4. The `Hrir` holds the slots in slot order at the file's rate and is resampled
   to the job's rate when they differ.

The `room` purpose does not do this (unchanged since ADR 0010); `headphones`
cuts both responses to the shorter length and does nothing else.

After building the BRIR responses, at the job's rate, the service compares the
two ears of each slot. The arrival of an ear is
`ImpulseResponse::peak_index(0, None, 0.12589)` (the measure `crop_heads` uses);
the time difference is `(right - left) / fs` in milliseconds.

- Always: info `cli_brir_measurement_file_itd` with `{"list": "FL +0.26, FR -0.25"}`
  (slot order, two decimals, sign always shown).
- Each slot whose difference exceeds 1.0 ms in magnitude: warning
  `cli_brir_measurement_file_itd_large` with `{"speaker": "FL", "ms": "+9.38"}`.
- When at least two slots off the median plane (slot azimuth not 0 and not 180)
  are present and every one of them differs by at most one sample: warning
  `cli_brir_measurement_file_itd_zero` with `{}`.

Nothing is corrected automatically (ADR 0011, point 5).

## Service

- Config fields `room_measurement_file` (path) and `room_measurement_map`
  (string) are 3.x extension fields; the CLI options are
  `--room_measurement_file PATH` and `--room_measurement_map MAP`.
- With room correction on and `room_measurement_file` set, the service reads
  the file and builds the room responses from the assignment instead of the
  folder's `room-*.wav` files (which it then does not open, logging that it
  ignored them). Each response is an `ImpulseResponse` with the decoded samples
  and no recording. When the file's sample rate differs from the job's, the
  responses are resampled to the job's. Imported responses form no recording
  pairs for virtual room tuning (ADR 0010, point 7).
- Microphone calibration: `room_mic_calibration`, then the folder's
  `room-mic-calibration.csv`/`.txt`, then the Dirac project's calibration.
- `inspect_room_measurements({path, map?})` is read-only. It answers
  `FILE_NOT_FOUND` for a missing file and `INVALID_REQUEST` for an unreadable
  file or an invalid map, otherwise:

```json
{
  "format": "dirac",
  "path": "/abs/path/1.liveproject",
  "title": "1",
  "sample_rate": 48000,
  "microphone": "마이크 (Umik-1  Gain: 18dB)",
  "mic_calibration": true,
  "positions": [1, 2, 3, 4, 5, 6, 7, 8, 9],
  "speakers": [{"index": 1, "name": "Left", "subwoofer": false, "slot": "FL"}],
  "measurements": [{"id": "1@1", "name": "Left", "speaker": 1, "position": 1,
                    "samples": 32769, "skipped": null}],
  "mapping": [{"slot": "FL", "side": "left", "id": "1@1"}],
  "map": "FL=1@1,FR=2@1",
  "auto": true
}
```

  For REW, `title` and `microphone` are null, `mic_calibration` is false,
  `positions` and `speakers` are empty, `speaker` and `position` are null and a
  skipped measurement has `"skipped": "no_impulse_response"` and `samples` 0.
  `slot` in `speakers` is the automatic slot. `mapping` and `map` are the
  effective assignment (from `map` when given, else automatic) in canonical
  order; `auto` says which.

### BRIR and headphone measurement files (ADR 0011)

- Config fields `brir_measurement_file`, `brir_measurement_map`,
  `headphone_measurement_file` and `headphone_measurement_map` are 3.x
  extension fields; the CLI options carry the same names
  (`--brir_measurement_file PATH`, `--brir_measurement_map MAP`,
  `--headphone_measurement_file PATH`, `--headphone_measurement_map MAP`).
  Paths are read in place, like `room_measurement_file`.
- With `brir_measurement_file` set, the job builds the HRIR from the file
  (see "Building the BRIR responses") wherever it would open the folder's
  speaker recordings, including the early opening for virtual bass `auto`. The
  folder's speaker recordings are not opened; when there are any, info
  `cli_brir_measurement_file_replaces_folder` with `{"count": n}`. After
  loading, info `cli_brir_measurement_file_loaded` with `{"file": <file name>,
  "format": "REW" | "Dirac Live", "count": <slots>}`, then the time-difference
  lines above. No microphone calibration is applied (ADR 0011, point 6).
- With `brir_measurement_file` set and no speaker recording in the folder, the
  estimator falls back to the bundled default sweep without the
  `cli_warning_sweep_detect_fallback` warning (nothing could be detected).
- With headphone compensation on and `headphone_measurement_file` set, the job
  builds the headphone `Hrir` (`FL` left and `FR` right only) from the file
  instead of `headphones.wav`, skips the folder's headphone discovery messages,
  and continues as before (`headphone-responses.wav`, compensation, plot). When
  the folder has a headphone recording, info
  `cli_headphone_measurement_file_replaces_folder` with `{"file": <its name>}`.
  After loading, info `cli_headphone_measurement_file_loaded` with
  `{"file": <file name>, "format": "REW" | "Dirac Live", "left": <measurement
  name>, "right": <measurement name>}`.
- The map of each purpose is resolved with `parse_map_for` and then
  `check_assignments`; a missing file is `FILE_NOT_FOUND`, everything else
  (unreadable file, invalid map, failed check) is a job error with the
  English message of the reader or the check.
- `inspect_room_measurements({path, map?, purpose?})`: `purpose` is `room`
  (default when missing or null), `brir` or `headphones`; anything else is
  `INVALID_REQUEST` (`purpose must be room, brir or headphones.`). The response
  gains `"purpose"` (the effective one), and `mapping`, `map` and `auto` follow
  that purpose (`parse_map_for`, `format_map_for`). `speakers[].slot` stays the
  automatic slot of the speaker name, the same for every purpose. The
  inspection never calls `check_assignments`; an incomplete assignment is
  returned for the screen to show.
