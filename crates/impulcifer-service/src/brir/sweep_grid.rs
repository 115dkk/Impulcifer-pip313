//! core/sweep_detection.py, including edited-recording envelope fallback.
use super::{
    BrirError,
    discovery::{listing, recording_speakers},
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

pub fn snap_sweep_samples(estimate: f64, fs: u32) -> (usize, usize, f64) {
    let p = (fs as f64 / 10.0).log2().ceil();
    let power = 2.0_f64.powf(p);
    let unit = 2.0 * power.ln() * power;
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
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (i, sample) in smoothed
        .iter()
        .copied()
        .chain(std::iter::once(0.0))
        .enumerate()
    {
        let active = i < n && sample > peak * 0.01;
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
pub fn detect_sweep_parameters(dir: &Path) -> Result<Option<SweepDetection>, BrirError> {
    detect_with_reader(dir, read_wav)
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
        let path = dir.join(file);
        let Ok(header) = wav_header(&path) else {
            continue;
        };
        debug_assert!(header.channels > 0);
        let fs = header.rate;
        let estimate = (header.frames as f64 - 2.0 * fs as f64 * (count + 1) as f64) / count as f64;
        let (mut m, mut n, mut deviation) = if estimate > 0.0 {
            snap_sweep_samples(estimate, fs)
        } else {
            (0, 0, 0.0)
        };
        let mut segments = count;
        if estimate <= 0.0 || deviation > 0.15 {
            let Ok(wav) = decode(&path) else {
                continue;
            };
            if let Some((estimate, count)) = envelope_estimate(&wav.tracks, fs) {
                (m, n, deviation) = snap_sweep_samples(estimate, fs);
                segments = count;
            } else if estimate <= 0.0 {
                continue;
            }
        }
        results.push((fs, m, n, deviation, segments));
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
