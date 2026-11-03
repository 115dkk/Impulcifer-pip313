# Room measurements from REW and Dirac Live files

ADR 0010 decides that a REW measurement file (`.mdat`) or a Dirac Live project
(`.liveproject`) can replace the `room-*.wav` recordings of a measurement folder.
This document is the specification of both readers, of the automatic
speaker/ear assignment and of the map syntax. Everything here was established
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
