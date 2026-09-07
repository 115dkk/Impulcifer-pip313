//! RIFF/RF64 WAVE I/O with track-major f64 samples.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::IoError;

#[derive(Debug, Clone, PartialEq)]
pub struct Wav {
    pub sample_rate: u32,
    pub tracks: Vec<Vec<f64>>,
}

fn format_error(message: &str) -> IoError {
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
fn end_at(start: u64, length: u64, bound: u64) -> Result<u64, IoError> {
    start
        .checked_add(length)
        .filter(|end| *end <= bound)
        .ok_or_else(|| format_error("chunk exceeds declared container or physical file bounds"))
}

struct Format {
    channels: usize,
    rate: u32,
    bits: u16,
    float: bool,
    align: usize,
}
fn parse_format(b: &[u8], length: u64) -> Result<Format, IoError> {
    if length < 16 {
        return Err(format_error("short fmt chunk"));
    }
    let mut tag = u16le(b);
    let channels = u16le(&b[2..]) as usize;
    let rate = u32le(&b[4..]);
    let align = u16le(&b[12..]) as usize;
    let bits = u16le(&b[14..]);
    if tag == 0xfffe {
        if length < 40 || u16le(&b[16..]) < 22 || u64::from(u16le(&b[16..])) + 18 > length {
            return Err(format_error("short extensible fmt chunk"));
        }
        let valid = u16le(&b[18..]);
        if valid == 0 || valid > bits {
            return Err(format_error("invalid valid-bits count"));
        }
        // Any channel mask is accepted. Valid PCM bits are left aligned in the container.
        if b[26..40] != [0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113] {
            return Err(format_error("unsupported extensible subformat GUID"));
        }
        tag = u16le(&b[24..]);
        if tag == 3 && valid != bits {
            return Err(format_error("invalid float valid-bits count"));
        }
    }
    let float = tag == 3;
    if !matches!((tag, bits), (1, 16 | 24 | 32) | (3, 32 | 64)) {
        return Err(format_error("unsupported WAVE encoding"));
    }
    if channels == 0
        || rate == 0
        || align != channels * usize::from(bits / 8)
        || u64::from(u32le(&b[8..])) != u64::from(rate) * align as u64
    {
        return Err(format_error(
            "invalid channel count, rate or frame alignment",
        ));
    }
    Ok(Format {
        channels,
        rate,
        bits,
        float,
        align,
    })
}

/// Reads PCM by dividing signed container integers by 2^(bits-1), exactly as
/// soundfile's float64 reader. Float32/64 samples are not clipped. All declared
/// chunk bounds (including padding) are checked before allocating sample arrays.
pub fn read_wav(path: &Path) -> Result<Wav, IoError> {
    let mut reader = BufReader::new(File::open(path)?);
    let physical = reader.get_ref().metadata()?.len();
    let mut header = [0; 12];
    if physical < 12 {
        return Err(format_error("truncated RIFF header"));
    }
    reader.read_exact(&mut header)?;
    let rf64 = &header[..4] == b"RF64";
    if (!rf64 && &header[..4] != b"RIFF") || &header[8..] != b"WAVE" {
        return Err(format_error("not RIFF/RF64 WAVE"));
    }
    let mut bound = if rf64 {
        physical
    } else {
        end_at(8, u32le(&header[4..]) as u64, physical)?
    };
    if bound < 12 || (rf64 && u32le(&header[4..]) != u32::MAX) {
        return Err(format_error("invalid RIFF size"));
    }
    let mut ds64: Option<(u64, u64)> = None;
    let mut table = Vec::<([u8; 4], u64, bool)>::new();
    let mut fmt = None;
    let mut data = None;
    let mut pos = 12;
    while pos < bound {
        end_at(pos, 8, bound)?;
        reader.seek(SeekFrom::Start(pos))?;
        let mut chunk = [0; 8];
        reader.read_exact(&mut chunk)?;
        let id: [u8; 4] = chunk[..4].try_into().unwrap();
        let start = pos + 8;
        let mut size = u64::from(u32le(&chunk[4..]));
        if rf64 && pos == 12 && &id != b"ds64" {
            return Err(format_error("RF64 must start with ds64"));
        }
        if size == u32::MAX as u64 && rf64 {
            size = if &id == b"data" && data.is_none() {
                ds64.ok_or_else(|| format_error("missing ds64"))?.0
            } else {
                let entry = table
                    .iter_mut()
                    .find(|(key, _, used)| *key == id && !*used)
                    .ok_or_else(|| format_error("missing RF64 chunk size"))?;
                entry.2 = true;
                entry.1
            };
        }
        let end = end_at(start, size, bound)?;
        let padded = end_at(end, size & 1, bound)?;
        match &id {
            b"ds64" if rf64 => {
                if ds64.is_some() || size < 28 {
                    return Err(format_error("invalid ds64"));
                }
                let mut b = [0; 28];
                reader.read_exact(&mut b)?;
                bound = end_at(8, u64le(&b), physical)?;
                if padded > bound {
                    return Err(format_error("ds64 exceeds RF64 size"));
                }
                let count = u32le(&b[24..]) as u64;
                if 28 + count * 12 > size {
                    return Err(format_error("truncated ds64 size table"));
                }
                ds64 = Some((u64le(&b[8..]), u64le(&b[16..])));
                // Only a validated, physically present table can request allocation.
                for _ in 0..count {
                    let mut entry = [0; 12];
                    reader.read_exact(&mut entry)?;
                    table
                        .try_reserve(1)
                        .map_err(|e| format_error(&e.to_string()))?;
                    table.push((entry[..4].try_into().unwrap(), u64le(&entry[4..]), false));
                }
            }
            b"fmt " => {
                if fmt.is_some() {
                    return Err(format_error("duplicate fmt chunk"));
                }
                let mut b = [0; 40];
                reader.read_exact(&mut b[..size.min(40) as usize])?;
                fmt = Some(parse_format(&b, size)?);
            }
            b"data" => {
                if data.is_some() {
                    return Err(format_error("multiple data chunks are unsupported"));
                }
                data = Some((start, size));
            }
            _ => {}
        }
        pos = padded;
    }
    let fmt = fmt.ok_or_else(|| format_error("missing fmt chunk"))?;
    let (start, size) = data.ok_or_else(|| format_error("missing data chunk"))?;
    if size % fmt.align as u64 != 0 {
        return Err(format_error("partial audio frame"));
    }
    let frames = size / fmt.align as u64;
    if rf64 && ds64.is_none_or(|(bytes, count)| bytes != size || (count != 0 && count != frames)) {
        return Err(format_error("inconsistent RF64 data size/sample count"));
    }
    let frames =
        usize::try_from(frames).map_err(|_| format_error("sample count exceeds address space"))?;
    let mut tracks = Vec::new();
    tracks
        .try_reserve_exact(fmt.channels)
        .map_err(|e| format_error(&e.to_string()))?;
    for _ in 0..fmt.channels {
        let mut track = Vec::new();
        track
            .try_reserve_exact(frames)
            .map_err(|e| format_error(&e.to_string()))?;
        tracks.push(track);
    }
    reader.seek(SeekFrom::Start(start))?;
    let width = usize::from(fmt.bits / 8);
    for _ in 0..frames {
        for track in &mut tracks {
            let mut b = [0; 8];
            reader.read_exact(&mut b[..width])?;
            let sample = match (fmt.float, fmt.bits) {
                (true, 32) => f32::from_le_bytes(b[..4].try_into().unwrap()) as f64,
                (true, 64) => f64::from_le_bytes(b),
                (false, 16) => i16::from_le_bytes(b[..2].try_into().unwrap()) as f64 / 32768.0,
                (false, 24) => ((u32le(&b) << 8) as i32 >> 8) as f64 / 8388608.0,
                (false, 32) => i32::from_le_bytes(b[..4].try_into().unwrap()) as f64 / 2147483648.0,
                _ => unreachable!(),
            };
            track.push(sample);
        }
    }
    Ok(Wav {
        sample_rate: fmt.rate,
        tracks,
    })
}

/// libsndfile 1.2.2 normalized/clipped double conversion: scale by 2^31,
/// round ties to even, saturate to i32; PCM16/24 discard low 16/8 bits using
/// an arithmetic shift. This is NOT nearest rounding at the target bit depth.
fn quantize(value: f64) -> i32 {
    (value * 2147483648.0).round_ties_even() as i32
}

/// Quantize through PCM_32 and read back as f64, like the sweep oracle.
/// Inputs must be finite (the writer returns an error for nonfinite samples).
pub fn pcm32_round_trip(tracks: &[Vec<f64>]) -> Vec<Vec<f64>> {
    assert!(
        tracks.iter().flatten().all(|x| x.is_finite()),
        "PCM samples must be finite"
    );
    tracks
        .iter()
        .map(|track| {
            track
                .iter()
                .map(|&x| quantize(x) as f64 / 2147483648.0)
                .collect()
        })
        .collect()
}

/// Writes integer PCM only. Multichannel files use extensible DIRECTOUT
/// (mask=0): BRIR ear tracks are not physical surround speaker positions.
/// The extensible header, including fact, matches soundfile's WAVEX oracle.
pub fn write_wav(
    path: &Path,
    sample_rate: u32,
    tracks: &[Vec<f64>],
    bit_depth: u16,
) -> Result<(), IoError> {
    if !matches!(bit_depth, 16 | 24 | 32) || sample_rate == 0 || tracks.is_empty() {
        return Err(IoError::InvalidArgument(
            "expected nonzero rate/channels and PCM16/24/32".into(),
        ));
    }
    let frames = tracks[0].len();
    if tracks
        .iter()
        .any(|t| t.len() != frames || t.iter().any(|x| !x.is_finite()))
    {
        return Err(IoError::InvalidArgument(
            "tracks must be rectangular and finite".into(),
        ));
    }
    let channels = u16::try_from(tracks.len()).map_err(|_| format_error("too many channels"))?;
    let align = channels
        .checked_mul(bit_depth / 8)
        .ok_or_else(|| format_error("frame too large"))?;
    let rate = sample_rate
        .checked_mul(align as u32)
        .ok_or_else(|| format_error("byte rate overflow"))?;
    let size = (frames as u64)
        .checked_mul(align as u64)
        .ok_or_else(|| format_error("data size overflow"))?;
    let extensible = channels > 2;
    let overhead = if extensible { 72 } else { 36 };
    let riff_size = size
        .checked_add(overhead + (size & 1))
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| format_error("output exceeds RIFF size; RF64 writing is not supported"))?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all(b"RIFF")?;
    out.write_all(&riff_size.to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&(if extensible { 40u32 } else { 16u32 }).to_le_bytes())?;
    out.write_all(&(if extensible { 0xfffeu16 } else { 1u16 }).to_le_bytes())?;
    out.write_all(&channels.to_le_bytes())?;
    out.write_all(&sample_rate.to_le_bytes())?;
    out.write_all(&rate.to_le_bytes())?;
    out.write_all(&align.to_le_bytes())?;
    out.write_all(&bit_depth.to_le_bytes())?;
    if extensible {
        out.write_all(&22u16.to_le_bytes())?;
        out.write_all(&bit_depth.to_le_bytes())?;
        out.write_all(&0u32.to_le_bytes())?;
        out.write_all(&[1, 0, 0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113])?;
        out.write_all(b"fact")?;
        out.write_all(&4u32.to_le_bytes())?;
        out.write_all(&(frames as u32).to_le_bytes())?;
    }
    out.write_all(b"data")?;
    out.write_all(&(size as u32).to_le_bytes())?;
    for frame in 0..frames {
        for track in tracks {
            let bytes = quantize(track[frame]).to_le_bytes();
            out.write_all(&bytes[4 - usize::from(bit_depth / 8)..])?;
        }
    }
    if size & 1 != 0 {
        out.write_all(&[0])?;
    }
    out.flush()?;
    Ok(())
}
