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
        tracks.push(Vec::new());
    }
    reader.seek(SeekFrom::Start(start))?;
    // Dispatch once, then decode bounded frame blocks into contiguous track runs.
    // Validation above is unchanged, including every RIFF/RF64 chunk boundary.
    match (fmt.float, fmt.bits) {
        (true, 32) => decode_blocks::<4>(&mut reader, &mut tracks, frames, |b| {
            f32::from_le_bytes(b.try_into().unwrap()) as f64
        })?,
        (true, 64) => decode_blocks::<8>(&mut reader, &mut tracks, frames, |b| {
            f64::from_le_bytes(b.try_into().unwrap())
        })?,
        (false, 16) => decode_blocks::<2>(&mut reader, &mut tracks, frames, |b| {
            i16::from_le_bytes(b.try_into().unwrap()) as f64 / 32768.0
        })?,
        (false, 24) => decode_blocks::<3>(&mut reader, &mut tracks, frames, |b| {
            ((u32::from_le_bytes([b[0], b[1], b[2], 0]) << 8) as i32 >> 8) as f64 / 8388608.0
        })?,
        (false, 32) => decode_blocks::<4>(&mut reader, &mut tracks, frames, |b| {
            i32::from_le_bytes(b.try_into().unwrap()) as f64 / 2147483648.0
        })?,
        _ => unreachable!(),
    }
    Ok(Wav {
        sample_rate: fmt.rate,
        tracks,
    })
}

fn decode_blocks<const WIDTH: usize>(
    reader: &mut impl Read,
    tracks: &mut [Vec<f64>],
    frames: usize,
    decode: impl Fn(&[u8]) -> f64 + Sync,
) -> Result<(), IoError> {
    match tracks.len() {
        1 => decode_channels::<WIDTH, 1>(reader, tracks, frames, decode),
        2 => decode_channels::<WIDTH, 2>(reader, tracks, frames, decode),
        8 => decode_channels::<WIDTH, 8>(reader, tracks, frames, decode),
        30 => decode_channels::<WIDTH, 30>(reader, tracks, frames, decode),
        32 => decode_channels::<WIDTH, 32>(reader, tracks, frames, decode),
        _ => decode_channels::<WIDTH, 0>(reader, tracks, frames, decode),
    }
}

fn decode_channels<const WIDTH: usize, const CHANNELS: usize>(
    reader: &mut impl Read,
    tracks: &mut [Vec<f64>],
    frames: usize,
    decode: impl Fn(&[u8]) -> f64 + Sync,
) -> Result<(), IoError> {
    // Common mono, stereo, decoded surround and BRIR layouts have a constant
    // stride, eliminating runtime division and enabling loop unrolling.
    let channels = if CHANNELS == 0 {
        tracks.len()
    } else {
        CHANNELS
    };
    let align = channels * WIDTH;
    // Large BRIR layouts amortize four scoped workers; smaller inputs retain
    // bounded streaming reads. Share only encoded bytes, and allocate each
    // disjoint output group on its worker rather than serializing allocation.
    if CHANNELS >= 30 && frames >= 32768 {
        let size = frames
            .checked_mul(align)
            .ok_or_else(|| format_error("data size exceeds address space"))?;
        thread_local! {
            static INPUT_BYTES: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
        }
        let mut bytes = INPUT_BYTES.take();
        bytes.clear();
        bytes
            .try_reserve_exact(size)
            .map_err(|e| format_error(&e.to_string()))?;
        reader.take(size as u64).read_to_end(&mut bytes)?;
        if bytes.len() != size {
            return Err(std::io::Error::from(std::io::ErrorKind::UnexpectedEof).into());
        }
        let samples = bytes.as_chunks::<WIDTH>().0.as_chunks::<CHANNELS>().0;
        std::thread::scope(|scope| -> Result<(), IoError> {
            let mut workers = Vec::new();
            workers
                .try_reserve_exact(4)
                .map_err(|e| format_error(&e.to_string()))?;
            for (group, tracks) in tracks.chunks_mut(CHANNELS.div_ceil(4)).enumerate() {
                let decode = &decode;
                let worker = std::thread::Builder::new().spawn_scoped(
                    scope,
                    move || -> Result<(), IoError> {
                        for track in tracks.iter_mut() {
                            track
                                .try_reserve_exact(frames)
                                .map_err(|e| format_error(&e.to_string()))?;
                        }
                        for tile in samples.chunks(256) {
                            for (offset, track) in tracks.iter_mut().enumerate() {
                                let channel = group * CHANNELS.div_ceil(4) + offset;
                                assert!(channel < CHANNELS);
                                track.extend(tile.iter().map(|frame| decode(&frame[channel])));
                            }
                        }
                        Ok(())
                    },
                );
                workers.push(worker?);
            }
            for worker in workers {
                worker.join().unwrap()?;
            }
            Ok(())
        })?;
        // Reuse capacity only; every call rereads the file. Cap the
        // per-caller cache so an unusually large RF64 read cannot pin its buffer.
        if bytes.capacity() <= 16 * 1024 * 1024 {
            bytes.clear();
            INPUT_BYTES.set(bytes);
        }
        return Ok(());
    }
    for track in tracks.iter_mut() {
        track
            .try_reserve_exact(frames)
            .map_err(|e| format_error(&e.to_string()))?;
    }
    let block_frames = (262144 / align).max(1);
    let mut bytes = vec![0; block_frames * align];
    for start in (0..frames).step_by(block_frames) {
        let count = (frames - start).min(block_frames);
        let block = &mut bytes[..count * align];
        reader.read_exact(block)?;
        // Transpose cache-sized tiles without reducing the file read size.
        // In particular, 32-channel strides must not revisit a 256 KiB block
        // once per channel, displacing the other channels' cache lines.
        for tile in block.chunks(256 * align) {
            if CHANNELS >= 8 {
                let samples = tile.as_chunks::<WIDTH>().0.as_chunks::<CHANNELS>().0;
                for (channel, track) in tracks.iter_mut().enumerate() {
                    assert!(channel < CHANNELS);
                    track.extend(samples.iter().map(|frame| decode(&frame[channel])));
                }
            } else {
                for (channel, track) in tracks.iter_mut().enumerate() {
                    track.extend(
                        tile.chunks_exact(align)
                            .map(|frame| decode(&frame[channel * WIDTH..(channel + 1) * WIDTH])),
                    );
                }
            }
        }
    }
    Ok(())
}

/// libsndfile 1.2.2 normalized/clipped double conversion: scale by 2^31,
/// round ties to even, saturate to i32; PCM16/24 discard low 16/8 bits using
/// an arithmetic shift. This is NOT nearest rounding at the target bit depth.
fn quantize(value: f64) -> i32 {
    // After saturation, adding signed 2^52 rounds to an integral f64 with
    // ties-to-even. Subtraction is exact. Avoid a scalar roundeven libcall
    // per sample on baseline x86 targets without an enabled SSE4.1 target.
    let scaled = (value * 2147483648.0).clamp(i32::MIN as f64, i32::MAX as f64);
    let integral_grid = 4503599627370496.0_f64.copysign(scaled);
    ((scaled + integral_grid) - integral_grid) as i32
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

fn encode_blocks<const WIDTH: usize>(
    out: &mut impl Write,
    tracks: &[Vec<f64>],
    frames: usize,
) -> Result<(), IoError> {
    let align = tracks.len() * WIDTH;
    let block_frames = (65536 / align).max(1);
    let mut bytes = vec![0; block_frames * align];
    for start in (0..frames).step_by(block_frames) {
        let count = (frames - start).min(block_frames);
        let block = &mut bytes[..count * align];
        for (channel, track) in tracks.iter().enumerate() {
            for (&sample, frame) in track[start..start + count]
                .iter()
                .zip(block.chunks_exact_mut(align))
            {
                frame[channel * WIDTH..(channel + 1) * WIDTH]
                    .copy_from_slice(&quantize(sample).to_le_bytes()[4 - WIDTH..]);
            }
        }
        out.write_all(block)?;
    }
    Ok(())
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
    match bit_depth {
        16 => encode_blocks::<2>(&mut out, tracks, frames)?,
        24 => encode_blocks::<3>(&mut out, tracks, frames)?,
        32 => encode_blocks::<4>(&mut out, tracks, frames)?,
        _ => unreachable!(),
    }
    if size & 1 != 0 {
        out.write_all(&[0])?;
    }
    out.flush()?;
    Ok(())
}
