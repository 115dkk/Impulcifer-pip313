//! core/sweep_detection.py, including edited-recording envelope fallback.
use super::{
    BrirError,
    discovery::{MeasurementDir, discover, listing, recording_speakers},
};
use impulcifer_io::{IoError, read_wav};
use serde_json::{Value, json};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

// Keep the same RIFF/RF64 validation as impulcifer_io::read_wav, but seek over
// audio instead of decoding it. The IO crate has no public metadata reader.
// Only envelope fallback needs samples (all frames/channels, as in Python).
struct WavHeader {
    frames: u64,
    rate: u32,
    channels: u16,
}
fn header_error(message: &str) -> IoError {
    IoError::Format(message.into())
}
fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes(b[..2].try_into().unwrap())
}
fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes(b[..4].try_into().unwrap())
}
fn u64le(b: &[u8]) -> u64 {
    u64::from_le_bytes(b[..8].try_into().unwrap())
}
fn header_end(start: u64, length: u64, bound: u64) -> Result<u64, IoError> {
    start
        .checked_add(length)
        .filter(|end| *end <= bound)
        .ok_or_else(|| header_error("chunk exceeds declared container or physical file bounds"))
}
fn wav_header(path: &Path) -> Result<WavHeader, IoError> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let physical = reader.get_ref().metadata()?.len();
    let mut header = [0; 12];
    if physical < 12 {
        return Err(header_error("truncated RIFF header"));
    }
    reader.read_exact(&mut header)?;
    let rf64 = &header[..4] == b"RF64";
    if (!rf64 && &header[..4] != b"RIFF") || &header[8..] != b"WAVE" {
        return Err(header_error("not RIFF/RF64 WAVE"));
    }
    let mut bound = if rf64 {
        physical
    } else {
        header_end(8, u32le(&header[4..]) as u64, physical)?
    };
    if bound < 12 || (rf64 && u32le(&header[4..]) != u32::MAX) {
        return Err(header_error("invalid RIFF size"));
    }
    let mut ds64: Option<(u64, u64)> = None;
    let mut table = Vec::<([u8; 4], u64, bool)>::new();
    let mut fmt = None;
    let mut data = None;
    let mut pos = 12;
    while pos < bound {
        header_end(pos, 8, bound)?;
        reader.seek(SeekFrom::Start(pos))?;
        let mut chunk = [0; 8];
        reader.read_exact(&mut chunk)?;
        let id: [u8; 4] = chunk[..4].try_into().unwrap();
        let start = pos + 8;
        let mut size = u64::from(u32le(&chunk[4..]));
        if rf64 && pos == 12 && &id != b"ds64" {
            return Err(header_error("RF64 must start with ds64"));
        }
        if size == u32::MAX as u64 && rf64 {
            size = if &id == b"data" && data.is_none() {
                ds64.ok_or_else(|| header_error("missing ds64"))?.0
            } else {
                let entry = table
                    .iter_mut()
                    .find(|(key, _, used)| *key == id && !*used)
                    .ok_or_else(|| header_error("missing RF64 chunk size"))?;
                entry.2 = true;
                entry.1
            };
        }
        let end = header_end(start, size, bound)?;
        let padded = header_end(end, size & 1, bound)?;
        match &id {
            b"ds64" if rf64 => {
                if ds64.is_some() || size < 28 {
                    return Err(header_error("invalid ds64"));
                }
                let mut b = [0; 28];
                reader.read_exact(&mut b)?;
                bound = header_end(8, u64le(&b), physical)?;
                if padded > bound {
                    return Err(header_error("ds64 exceeds RF64 size"));
                }
                let count = u32le(&b[24..]) as u64;
                if 28 + count * 12 > size {
                    return Err(header_error("truncated ds64 size table"));
                }
                ds64 = Some((u64le(&b[8..]), u64le(&b[16..])));
                for _ in 0..count {
                    let mut entry = [0; 12];
                    reader.read_exact(&mut entry)?;
                    table
                        .try_reserve(1)
                        .map_err(|e| header_error(&e.to_string()))?;
                    table.push((entry[..4].try_into().unwrap(), u64le(&entry[4..]), false));
                }
            }
            b"fmt " => {
                if fmt.is_some() {
                    return Err(header_error("duplicate fmt chunk"));
                }
                if size < 16 {
                    return Err(header_error("short fmt chunk"));
                }
                let mut b = [0; 40];
                reader.read_exact(&mut b[..size.min(40) as usize])?;
                let mut tag = u16le(&b);
                let channels = u16le(&b[2..]);
                let rate = u32le(&b[4..]);
                let align = u16le(&b[12..]);
                let bits = u16le(&b[14..]);
                if tag == 0xfffe {
                    if size < 40 || u16le(&b[16..]) < 22 || u64::from(u16le(&b[16..])) + 18 > size {
                        return Err(header_error("short extensible fmt chunk"));
                    }
                    let valid = u16le(&b[18..]);
                    if valid == 0 || valid > bits {
                        return Err(header_error("invalid valid-bits count"));
                    }
                    if b[26..40] != [0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113] {
                        return Err(header_error("unsupported extensible subformat GUID"));
                    }
                    tag = u16le(&b[24..]);
                    if tag == 3 && valid != bits {
                        return Err(header_error("invalid float valid-bits count"));
                    }
                }
                if !matches!((tag, bits), (1, 16 | 24 | 32) | (3, 32 | 64)) {
                    return Err(header_error("unsupported WAVE encoding"));
                }
                if channels == 0
                    || rate == 0
                    || u32::from(align) != u32::from(channels) * u32::from(bits / 8)
                    || u64::from(u32le(&b[8..])) != u64::from(rate) * u64::from(align)
                {
                    return Err(header_error(
                        "invalid channel count, rate or frame alignment",
                    ));
                }
                fmt = Some((channels, rate, align));
            }
            b"data" => {
                if data.is_some() {
                    return Err(header_error("multiple data chunks are unsupported"));
                }
                data = Some(size);
            }
            _ => {}
        }
        pos = padded;
    }
    let (channels, rate, align) = fmt.ok_or_else(|| header_error("missing fmt chunk"))?;
    let size = data.ok_or_else(|| header_error("missing data chunk"))?;
    if size % u64::from(align) != 0 {
        return Err(header_error("partial audio frame"));
    }
    let frames = size / u64::from(align);
    if rf64 && ds64.is_none_or(|(bytes, count)| bytes != size || (count != 0 && count != frames)) {
        return Err(header_error("inconsistent RF64 data size/sample count"));
    }
    usize::try_from(frames).map_err(|_| header_error("sample count exceeds address space"))?;
    Ok(WavHeader {
        frames,
        rate,
        channels,
    })
}

/// Samples in one unit of the M-grid: every generated sweep is a whole number
/// of these long.
fn grid_unit(fs: u32) -> f64 {
    let p = (fs as f64 / 10.0).log2().ceil();
    let power = 2.0_f64.powf(p);
    2.0 * power.ln() * power
}
pub fn snap_sweep_samples(estimate: f64, fs: u32) -> (usize, usize, f64) {
    let unit = grid_unit(fs);
    let m = (estimate / unit).round_ties_even().max(1.0) as usize;
    (
        m,
        (m as f64 * unit).round_ties_even() as usize,
        (estimate - m as f64 * unit).abs() / unit,
    )
}
#[derive(Clone, Debug)]
pub struct SweepDetection {
    pub fs: u32,
    pub m: usize,
    pub sweep_samples: usize,
    pub n_segments: usize,
    pub speakers: Vec<String>,
    pub confidence: &'static str,
    pub deviation: f64,
    pub source_files: Vec<String>,
}
impl SweepDetection {
    pub fn duration(&self) -> f64 {
        self.sweep_samples as f64 / self.fs as f64
    }
    pub fn is_default(&self) -> bool {
        self.fs == 48000 && self.m == 2
    }
    pub fn generate_spec(&self) -> String {
        format!("generate:{:.2}s@{}", self.duration(), self.fs)
    }
    pub fn payload(&self, sidecar: bool) -> Value {
        json!({"found":true,"sidecar":sidecar,"fs":self.fs,"duration_seconds":(self.duration()*10000.0).round_ties_even()/10000.0,
            "n_segments":self.n_segments,"speakers":self.speakers,"confidence":self.confidence,"is_default":self.is_default(),"generate_spec":self.generate_spec(),"source_files":self.source_files})
    }
}
/// `numpy.percentile(values, 100 * q)` with its default linear interpolation.
fn percentile(values: &[f64], q: f64) -> f64 {
    let mut sorted = values.to_vec();
    let position = q * (sorted.len() - 1) as f64;
    let low = position.floor() as usize;
    let (_, &mut below, above) = sorted.select_nth_unstable_by(low, f64::total_cmp);
    let next = above.iter().copied().fold(f64::INFINITY, f64::min);
    if above.is_empty() {
        below
    } else {
        below + (next - below) * (position - low as f64)
    }
}
fn envelope_estimate(tracks: &[Vec<f64>], fs: u32) -> Option<(f64, usize)> {
    let n = tracks.first()?.len();
    if n == 0 {
        return None;
    }
    let kernel = ((0.05 * fs as f64) as usize).max(1);
    let values: Vec<_> = (0..n)
        .map(|i| tracks.iter().map(|t| t[i].abs()).fold(0.0, f64::max))
        .collect();
    let mut prefix = Vec::with_capacity(n + 1);
    prefix.push(0.0);
    for x in values {
        prefix.push(prefix.last().unwrap() + x);
    }
    let smoothed: Vec<_> = (0..n)
        .map(|i| {
            let start = i.saturating_sub(kernel / 2);
            let end = (i + kernel.div_ceil(2)).min(n);
            (prefix[end] - prefix[start]) / kernel as f64
        })
        .collect();
    let peak = smoothed.iter().copied().fold(0.0, f64::max);
    if peak <= 0.0 {
        return None;
    }
    // 40 dB under the peak, raised to 10 dB over the noise floor (NumPy's
    // linear 10th percentile) for noisy captures, whose floor otherwise reads
    // as one sweep spanning the whole file; never above 20 dB under the peak.
    let floor = percentile(&smoothed, 0.1);
    let threshold = (peak * 0.01).max((floor * 10f64.sqrt()).min(peak * 0.1));
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, sample) in smoothed
        .iter()
        .copied()
        .chain(std::iter::once(0.0))
        .enumerate()
    {
        let active = i < n && sample > threshold;
        if active && start.is_none() {
            start = Some(i);
        }
        if !active && let Some(s) = start.take() {
            if let Some(last) = runs.last_mut()
                && ((s - last.1) as f64) < 0.3 * fs as f64
            {
                last.1 = i;
            } else {
                runs.push((s, i));
            }
        }
    }
    runs.retain(|(s, e)| (e - s) as f64 >= 0.5 * fs as f64);
    // Every sweep of a sequence has the same length, so a region shorter than
    // half the longest is a noise burst whose onset would skew the spacing.
    let longest = runs.iter().map(|(s, e)| e - s).max().unwrap_or(0);
    runs.retain(|(s, e)| 2 * (e - s) >= longest);
    if runs.len() >= 2 {
        let mut intervals: Vec<_> = runs.windows(2).map(|w| (w[1].0 - w[0].0) as f64).collect();
        intervals.sort_by(f64::total_cmp);
        let k = intervals.len();
        Some((
            (intervals[(k - 1) / 2] + intervals[k / 2]) / 2.0 - 2.0 * fs as f64,
            runs.len(),
        ))
    } else {
        runs.first().map(|(s, e)| ((e - s) as f64, 1))
    }
}
/// Largest distance from the M-grid, in grid units, that still counts as a
/// sweep of that grid length (core/sweep_detection.py).
const GRID_TOLERANCE: f64 = 0.15;
/// Sweep length a recording of `count` sweeps holds, were its length exactly
/// the sequence the recorder plays: 2 s of silence around every sweep.
fn length_estimate(header: &WavHeader, count: usize) -> f64 {
    let fs = f64::from(header.rate);
    (header.frames as f64 - 2.0 * fs * (count + 1) as f64) / count as f64
}
/// Grid sweep of one recording as `(fs, m, samples, deviation, segments)`: its
/// length snapped to the grid, or the spacing of its sweeps when the length is
/// off the grid (an edited recording). None for an unreadable file and for one
/// too short to say anything.
fn recording_grid(
    path: &Path,
    count: usize,
    decode: &mut impl FnMut(&Path) -> Result<impulcifer_io::wav::Wav, IoError>,
) -> Option<(u32, usize, usize, f64, usize)> {
    let header = wav_header(path).ok()?;
    debug_assert!(header.channels > 0);
    let fs = header.rate;
    let estimate = length_estimate(&header, count);
    let (mut m, mut n, mut deviation) = if estimate > 0.0 {
        snap_sweep_samples(estimate, fs)
    } else {
        (0, 0, 0.0)
    };
    let mut segments = count;
    if estimate <= 0.0 || deviation > GRID_TOLERANCE {
        let wav = decode(path).ok()?;
        if let Some((estimate, count)) = envelope_estimate(&wav.tracks, fs) {
            (m, n, deviation) = snap_sweep_samples(estimate, fs);
            segments = count;
        } else if estimate <= 0.0 {
            return None;
        }
    }
    Some((fs, m, n, deviation, segments))
}
/// Rate and frame count of a WAV file, read from its header alone.
pub(crate) fn wav_length(path: &Path) -> Option<(u32, usize)> {
    let header = wav_header(path).ok()?;
    Some((header.rate, header.frames as usize))
}
/// Share of the test signal's length by which the spacing of the sweep onsets
/// may differ from it and still fit.
const SPACING_TOLERANCE: f64 = 0.15;
/// Sweep length in samples that a recording named for `speakers` speakers
/// appears to hold, when that is not the test signal (`sweep` samples at
/// `fs`). None when the test signal fits the recording and when the recording
/// says nothing either way.
///
/// The recorder writes exactly the sequence it plays, so the file length
/// gives the sweep length. A length that does not fit is not proof: a
/// recording cut or padded afterwards still shows its sweep in the spacing of
/// the sweep onsets, and that is asked before anything is reported. With one
/// sweep in the file, or silence, there is no spacing, and only a length that
/// lands on a generated sweep counts.
///
/// The spacing is coarse. An onset is where a sweep rises out of the noise,
/// and that moment differs between two speakers or earcups with their bass
/// (the reported headphone recording reads 6.64 s between sweeps of 6.15 s),
/// so the test signal fits a spacing within `SPACING_TOLERANCE` of its
/// length, and a length that lands on the grid is reported before a spacing.
///
/// Sweeps follow each other in time once per two tracks, as ingestion cuts
/// them (`ingest_recording`): a four-track file named FL,FR holds one column.
/// A file at another sampling rate is left to ingestion, which rejects it.
pub(crate) fn mismatched_sweep(path: &Path, speakers: usize, fs: u32, sweep: usize) -> Option<f64> {
    let header = wav_header(path).ok()?;
    let pairs = usize::from(header.channels / 2);
    if pairs == 0 || speakers == 0 {
        return None;
    }
    let columns = ((speakers as f64 / pairs as f64).round_ties_even() as usize).max(1);
    mismatched_sweep_columns(path, &header, columns, fs, sweep)
}

fn mismatched_sweep_columns(
    path: &Path,
    header: &WavHeader,
    columns: usize,
    fs: u32,
    sweep: usize,
) -> Option<f64> {
    if header.rate != fs || columns == 0 {
        return None;
    }
    let tolerance = GRID_TOLERANCE * grid_unit(fs);
    let fits = |estimate: f64| (estimate - sweep as f64).abs() <= tolerance;
    let length = length_estimate(header, columns);
    if fits(length) {
        return None;
    }
    let spacing = read_wav(path)
        .ok()
        .and_then(|wav| envelope_estimate(&wav.tracks, fs))
        .filter(|(_, sweeps)| *sweeps >= 2)
        .map(|(estimate, _)| estimate);
    let loose = tolerance.max(SPACING_TOLERANCE * sweep as f64);
    if spacing.is_some_and(|estimate| (estimate - sweep as f64).abs() <= loose) {
        return None;
    }
    let on_grid = |estimate: f64| {
        let (_, n, deviation) = snap_sweep_samples(estimate.max(0.0), fs);
        (estimate > 0.0 && deviation <= GRID_TOLERANCE).then_some(n as f64)
    };
    on_grid(length).or_else(|| spacing.map(|estimate| on_grid(estimate).unwrap_or(estimate)))
}
/// A recording the test signal does not fit, with the sweep length in
/// seconds it appears to hold.
#[derive(Clone, Debug, PartialEq)]
pub struct SweepMismatch {
    pub file: String,
    pub seconds: f64,
}

/// A configured single recording that was not made with the job's test signal.
pub(crate) fn recording_test_signal_mismatch(
    path: &Path,
    entries: usize,
    fs: u32,
    sweep: usize,
) -> Option<SweepMismatch> {
    let header = wav_header(path).ok()?;
    let samples = mismatched_sweep_columns(path, &header, entries, fs, sweep)?;
    Some(SweepMismatch {
        file: path.file_name()?.to_string_lossy().into_owned(),
        seconds: samples / f64::from(fs),
    })
}

/// Recordings of a measurement folder that were not made with the test signal
/// (`sweep` samples at `fs`): the speaker recordings when `speakers`, the
/// headphone recording the job would read when `headphones`.
pub(crate) fn test_signal_mismatches(
    dir: &MeasurementDir,
    speakers: bool,
    headphones: bool,
    fs: u32,
    sweep: usize,
) -> Vec<SweepMismatch> {
    let recordings = dir
        .recordings
        .iter()
        .filter(|_| speakers)
        .map(|(path, names)| (path, names.len()));
    let headphone = dir.headphones.iter().filter(|_| headphones).map(|p| (p, 2));
    recordings
        .chain(headphone)
        .filter_map(|(path, count)| {
            let samples = mismatched_sweep(path, count, fs, sweep)?;
            Some(SweepMismatch {
                file: path.file_name()?.to_string_lossy().into_owned(),
                seconds: samples / f64::from(fs),
            })
        })
        .collect()
}
/// The `test_signal` part of a `detect_sweep` answer: whether a test signal
/// file fits the recordings a job on `dir` would read. `headphone_file` is the
/// headphone compensation request of that job (empty for the folder's own
/// headphones.wav), or None when the job reads no headphone recording.
/// A file that is not a readable WAV gets no verdict: the job explains it.
pub fn test_signal_check(
    dir: &Path,
    test_signal: &Path,
    speakers: bool,
    headphone_file: Option<&str>,
) -> Result<Value, BrirError> {
    let Some((fs, frames)) = wav_length(test_signal) else {
        return Ok(json!({"readable":false,"mismatches":[]}));
    };
    let config = impulcifer_types::config::ProcessingConfig {
        headphone_compensation_file: headphone_file.filter(|s| !s.is_empty()).map(str::to_owned),
        ..Default::default()
    };
    let dir = discover(dir, &config)?;
    let mismatches: Vec<_> =
        test_signal_mismatches(&dir, speakers, headphone_file.is_some(), fs, frames)
            .into_iter()
            .map(|m| json!({"file":m.file,"duration_seconds":(m.seconds * 100.0).round() / 100.0}))
            .collect();
    Ok(json!({
        "readable":true,
        "fs":fs,
        "duration_seconds":(frames as f64 / f64::from(fs) * 100.0).round() / 100.0,
        "mismatches":mismatches,
    }))
}
pub fn detect_sweep_parameters(dir: &Path) -> Result<Option<SweepDetection>, BrirError> {
    detect_with_reader(dir, read_wav)
}

/// Apply the folder detector's per-file rule to a recording whose speaker list
/// comes from configuration rather than its file name.
pub(crate) fn detect_recording_file(path: &Path, entries: usize) -> Option<SweepDetection> {
    let (fs, m, sweep_samples, deviation, n_segments) =
        recording_grid(path, entries, &mut read_wav)?;
    Some(SweepDetection {
        fs,
        m,
        sweep_samples,
        n_segments,
        speakers: Vec::new(),
        confidence: if deviation <= GRID_TOLERANCE {
            "high"
        } else {
            "low"
        },
        deviation,
        source_files: vec![path.file_name()?.to_string_lossy().into_owned()],
    })
}

fn detect_with_reader(
    dir: &Path,
    mut decode: impl FnMut(&Path) -> Result<impulcifer_io::wav::Wav, IoError>,
) -> Result<Option<SweepDetection>, BrirError> {
    let files: Vec<_> = listing(dir)?
        .into_iter()
        .filter(|n| recording_speakers(n).is_some())
        .collect();
    let mut speakers = Vec::new();
    let mut results = Vec::new();
    for file in &files {
        let names = recording_speakers(file).unwrap();
        let count = names.len();
        speakers.extend(names);
        results.extend(recording_grid(&dir.join(file), count, &mut decode));
    }
    let Some(&(fs, m, n, _, _)) = results.first() else {
        return Ok(None);
    };
    let deviation = results.iter().map(|r| r.3).fold(0.0, f64::max);
    let high = deviation <= 0.15 && results.iter().all(|r| r.0 == fs && r.1 == m);
    Ok(Some(SweepDetection {
        fs,
        m,
        sweep_samples: n,
        n_segments: results.iter().map(|r| r.4).sum(),
        speakers,
        confidence: if high { "high" } else { "low" },
        deviation,
        source_files: files,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(path: &Path, frames: usize, rate: u32) {
        impulcifer_io::write_wav(path, rate, &[vec![0.0; frames]], 16).unwrap();
    }

    #[test]
    fn header_validation_matches_decoder_for_riff_rf64_and_extensible() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("FL.wav");
        for channels in [1, 2, 4] {
            for bits in [16, 24, 32] {
                impulcifer_io::write_wav(&path, 48000, &vec![vec![0.0; 101]; channels], bits)
                    .unwrap();
                let original = std::fs::read(&path).unwrap();
                let mut rf64 = Vec::new();
                rf64.extend_from_slice(b"RF64");
                rf64.extend_from_slice(&u32::MAX.to_le_bytes());
                rf64.extend_from_slice(b"WAVEds64");
                rf64.extend_from_slice(&28_u32.to_le_bytes());
                rf64.extend_from_slice(&(original.len() as u64 + 36 - 8).to_le_bytes());
                let audio_bytes = 101 * channels as u64 * u64::from(bits / 8);
                rf64.extend_from_slice(&audio_bytes.to_le_bytes());
                rf64.extend_from_slice(&101_u64.to_le_bytes());
                rf64.extend_from_slice(&0_u32.to_le_bytes());
                rf64.extend_from_slice(&original[12..]);
                let data = rf64.windows(4).position(|b| b == b"data").unwrap();
                rf64[data + 4..data + 8].copy_from_slice(&u32::MAX.to_le_bytes());
                for bytes in [&original, &rf64] {
                    std::fs::write(&path, bytes).unwrap();
                    let header = wav_header(&path).unwrap();
                    let decoded = read_wav(&path).unwrap();
                    assert_eq!(header.frames, 101);
                    assert_eq!(header.rate, decoded.sample_rate);
                    assert_eq!(header.channels as usize, decoded.tracks.len());
                    // Mutate header fields and truncate payload/trailing chunks.
                    // Both metadata-only and sample readers must reject/accept
                    // the same supported containers, with the same errors.
                    for index in 0..bytes.len().min(140) {
                        let mut modified = bytes.clone();
                        modified[index] ^= 0xff;
                        std::fs::write(&path, &modified).unwrap();
                        assert_eq!(
                            wav_header(&path).err().map(|e| e.to_string()),
                            read_wav(&path).err().map(|e| e.to_string()),
                            "{channels} {bits} {index}"
                        );
                    }
                    for length in [0, 11, 20, bytes.len() - 1] {
                        std::fs::write(&path, &bytes[..length]).unwrap();
                        assert_eq!(
                            wav_header(&path).err().map(|e| e.to_string()),
                            read_wav(&path).err().map(|e| e.to_string())
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn envelope_splits_sweeps_above_a_noise_floor_and_skips_short_bursts() {
        // Two 3 s "sweeps" 5 s apart at 1 kHz: the estimate is 5 s - 2 s.
        let sweeps = |tracks: &mut Vec<Vec<f64>>, starts: [usize; 2]| {
            for start in starts {
                for track in tracks.iter_mut() {
                    track[start..start + 3000].fill(0.5);
                }
            }
        };
        // A floor 28 dB under the sweeps sat above the old peak - 40 dB
        // threshold and read as one region spanning the file.
        let mut noisy: Vec<Vec<f64>> = (0..2)
            .map(|_| {
                (0..12500)
                    .map(|i| if i % 2 == 0 { 0.02 } else { -0.02 })
                    .collect()
            })
            .collect();
        sweeps(&mut noisy, [1000, 6000]);
        assert_eq!(envelope_estimate(&noisy, 1000), Some((3000.0, 2)));
        // A 0.6 s burst in the lead silence used to count as the first onset.
        let mut burst = vec![vec![0.0; 12500]; 2];
        burst[0][0..600].fill(0.8);
        sweeps(&mut burst, [2000, 7000]);
        assert_eq!(envelope_estimate(&burst, 1000), Some((3000.0, 2)));
        assert_eq!(percentile(&[4.0, 1.0, 3.0, 2.0], 0.1), 1.3);
        assert_eq!(percentile(&[7.0], 0.1), 7.0);
    }

    #[test]
    fn mismatched_sweep_names_the_sweep_a_recording_was_made_with() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("headphones.wav");
        let rate = 48000;
        let (_, default, _) = snap_sweep_samples(300000.0, rate);
        let (_, long, _) = snap_sweep_samples(600000.0, rate);
        let recorded = |sweep: usize| 6 * rate as usize + 2 * sweep;
        let stereo = |frames: usize| {
            impulcifer_io::write_wav(&path, rate, &vec![vec![0.0; frames]; 2], 16).unwrap();
        };
        // Two default sweeps, exactly as the recorder writes them.
        stereo(recorded(default));
        assert_eq!(mismatched_sweep(&path, 2, rate, default), None);
        // The reported case: a 13.04 s test signal file. And the other way
        // round, where a shorter signal still cuts two columns out of it.
        assert_eq!(
            mismatched_sweep(&path, 2, rate, 625919),
            Some(default as f64)
        );
        assert_eq!(mismatched_sweep(&path, 2, rate, long), Some(default as f64));
        stereo(recorded(long));
        assert_eq!(mismatched_sweep(&path, 2, rate, default), Some(long as f64));
        // A custom signal the recording was made with fits, on or off the grid.
        stereo(recorded(288000));
        assert_eq!(mismatched_sweep(&path, 2, rate, 288000), None);
        stereo(recorded(240000));
        assert_eq!(mismatched_sweep(&path, 2, rate, 240000), None);
        // Off the grid with nothing to read from the envelope: no verdict.
        assert_eq!(mismatched_sweep(&path, 2, rate, long), None);
        // Another sampling rate and a mono file are left to ingestion.
        stereo(recorded(default));
        assert_eq!(mismatched_sweep(&path, 2, 44100, default), None);
        wav(&path, recorded(default), rate);
        assert_eq!(mismatched_sweep(&path, 2, rate, long), None);
        assert_eq!(
            mismatched_sweep(&temp.path().join("missing.wav"), 2, rate, long),
            None
        );
        // Four tracks named FL,FR hold one sweep per pair of tracks.
        impulcifer_io::write_wav(
            &path,
            rate,
            &vec![vec![0.0; 4 * rate as usize + default]; 4],
            16,
        )
        .unwrap();
        assert_eq!(mismatched_sweep(&path, 2, rate, default), None);
        assert_eq!(mismatched_sweep(&path, 2, rate, long), Some(default as f64));
        // A configured single recording always reads its first two tracks and
        // therefore uses the list's entry count even when the WAV has extras.
        impulcifer_io::write_wav(&path, rate, &vec![vec![0.0; recorded(default)]; 4], 16).unwrap();
        assert_eq!(
            recording_test_signal_mismatch(&path, 2, rate, default),
            None
        );
        assert_eq!(
            recording_test_signal_mismatch(&path, 2, rate, long)
                .unwrap()
                .seconds,
            default as f64 / f64::from(rate)
        );
    }

    #[test]
    fn mismatched_sweep_reads_the_spacing_of_an_edited_recording() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("headphones.wav");
        let rate = 1000;
        let (_, short, _) = snap_sweep_samples(1242.0, rate);
        let (_, long, _) = snap_sweep_samples(2484.0, rate);
        assert_eq!((short, long), (1242, 2484));
        let write = |frames: usize| {
            let mut tracks = vec![vec![0.0; frames]; 2];
            tracks[0][2000..2000 + short].fill(0.5);
            tracks[1][4000 + short..4000 + 2 * short].fill(0.5);
            impulcifer_io::write_wav(&path, rate, &tracks, 16).unwrap();
        };
        // Two short sweeps with the trailing silence cut off: the length fits
        // neither signal, the onsets are still one sweep and 2 s apart.
        write(4 * rate as usize + 2 * short);
        assert_eq!(mismatched_sweep(&path, 2, rate, short), None);
        assert_eq!(mismatched_sweep(&path, 2, rate, long), Some(short as f64));
        // The same sweeps with a second of silence appended instead.
        write(7 * rate as usize + 2 * short);
        assert_eq!(mismatched_sweep(&path, 2, rate, short), None);
        assert_eq!(mismatched_sweep(&path, 2, rate, long), Some(short as f64));
        // Two long sweeps with the tail cut, the second audible 0.3 s late (a
        // speaker with less bass): a spacing 12 % off still fits the long
        // signal, and for the short one it is all there is to report.
        let mut tracks = vec![vec![0.0; 4 * rate as usize + 2 * long]; 2];
        tracks[0][2000..2000 + long].fill(0.5);
        tracks[1][4300 + long..4000 + 2 * long].fill(0.5);
        impulcifer_io::write_wav(&path, rate, &tracks, 16).unwrap();
        assert_eq!(mismatched_sweep(&path, 2, rate, long), None);
        assert_eq!(mismatched_sweep(&path, 2, rate, short), Some(2784.0));
    }

    #[test]
    fn test_signal_mismatches_cover_the_recordings_a_job_reads() {
        let temp = tempfile::tempdir().unwrap();
        let rate = 48000;
        let (_, default, _) = snap_sweep_samples(300000.0, rate);
        let (_, long, _) = snap_sweep_samples(600000.0, rate);
        let stereo = |name: &str, sweeps: usize, sweep: usize| {
            let frames = 2 * rate as usize * (sweeps + 1) + sweeps * sweep;
            impulcifer_io::write_wav(
                &temp.path().join(name),
                rate,
                &vec![vec![0.0; frames]; 2],
                16,
            )
            .unwrap();
        };
        stereo("FL,FR.wav", 2, long);
        stereo("FC.wav", 1, long);
        stereo("headphones.wav", 2, default);
        let config = impulcifer_types::config::ProcessingConfig::default();
        let dir = super::super::discovery::discover(temp.path(), &config).unwrap();
        let files = |speakers, headphones, sweep| -> Vec<String> {
            test_signal_mismatches(&dir, speakers, headphones, rate, sweep)
                .into_iter()
                .map(|m| format!("{} {:.2}", m.file, m.seconds))
                .collect()
        };
        assert_eq!(files(true, true, long), ["headphones.wav 6.15"]);
        assert!(files(true, false, long).is_empty());
        assert_eq!(
            files(true, true, default),
            ["FC.wav 12.30", "FL,FR.wav 12.30"]
        );
        assert!(files(false, true, default).is_empty());
    }

    #[test]
    fn envelope_fallback_and_mixed_rates_keep_detection_contract() {
        let temp = tempfile::tempdir().unwrap();
        let rate = 1000;
        let mut tracks = vec![vec![0.0; 12500]; 2];
        for start in [1000, 6000] {
            tracks[1][start..start + 3000].fill(0.5);
        }
        let path = temp.path().join("FL,FR.wav");
        impulcifer_io::write_wav(&path, rate, &tracks, 16).unwrap();
        let expected = envelope_estimate(&read_wav(&path).unwrap().tracks, rate).unwrap();
        let result = detect_sweep_parameters(temp.path()).unwrap().unwrap();
        let (m, n, deviation) = snap_sweep_samples(expected.0, rate);
        assert_eq!(result.n_segments, 2);
        assert_eq!(result.m, m);
        assert_eq!(result.sweep_samples, n);
        assert_eq!(result.deviation, deviation);
        assert_eq!(expected.0, 3000.0);
        let (_, n, _) = snap_sweep_samples(300000.0, 48000);
        wav(&temp.path().join("FC.wav"), n + 192000, 48000);
        let result = detect_sweep_parameters(temp.path()).unwrap().unwrap();
        assert_eq!(result.confidence, "low");
        assert_eq!(result.n_segments, 3);
        assert_eq!(result.speakers, ["FC", "FL", "FR"]);
    }

    #[test]
    fn header_grid_skips_decoding_and_off_grid_reads_all_frames() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("FL.wav");
        let rate = 48000;
        let (_, samples, _) = snap_sweep_samples(300000.0, rate);
        wav(&path, samples + 4 * rate as usize, rate);
        let result = detect_with_reader(temp.path(), |_| panic!("on-grid decode"))
            .unwrap()
            .unwrap();
        assert_eq!(result.sweep_samples, samples);
        assert_eq!(result.confidence, "high");
        let header = wav_header(&path).unwrap();
        let decoded = read_wav(&path).unwrap();
        assert_eq!(header.frames as usize, decoded.tracks[0].len());
        assert_eq!(header.rate, decoded.sample_rate);
        assert_eq!(usize::from(header.channels), decoded.tracks.len());

        for frames in [96000, 400000] {
            wav(&path, frames, rate);
            let mut calls = 0;
            let result = detect_with_reader(temp.path(), |p| {
                calls += 1;
                let wav = read_wav(p)?;
                assert_eq!(wav.tracks[0].len(), frames);
                Ok(wav)
            })
            .unwrap();
            assert_eq!(calls, 1);
            if frames == 96000 {
                assert!(result.is_none());
            } else {
                assert_eq!(result.unwrap().confidence, "low");
            }
        }
        std::fs::write(path, b"invalid").unwrap();
        assert!(
            detect_with_reader(temp.path(), |_| panic!("invalid header decode"))
                .unwrap()
                .is_none()
        );
    }
}
