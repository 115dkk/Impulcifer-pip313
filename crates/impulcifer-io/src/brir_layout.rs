//! Versioned compact BRIR channel maps, core/brir_layout.py:46-100.
use crate::IoError;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

fn invalid(message: &str) -> IoError {
    IoError::Format(message.into())
}

pub fn append_track_names(path: &Path, names: &[&str]) -> Result<(), IoError> {
    // Explicit field order matches Python's insertion-ordered JSON object.
    let names = serde_json::to_string(names).map_err(|e| invalid(&e.to_string()))?;
    let payload = format!("{{\"version\":1,\"tracks\":{names}}}").into_bytes();
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let mut header = [0; 12];
    file.read_exact(&mut header)?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
        return Err(invalid("Compact BRIR metadata requires a RIFF WAV file."));
    }
    let end = file.seek(SeekFrom::End(0))?;
    let size = u32::try_from(end + payload.len() as u64 + (payload.len() % 2) as u64)
        .map_err(|_| invalid("Compact BRIR exceeds the RIFF WAV size limit."))?;
    file.write_all(b"ICHL")?;
    file.write_all(&(payload.len() as u32).to_le_bytes())?;
    file.write_all(&payload)?;
    if payload.len() % 2 != 0 {
        file.write_all(&[0])?;
    }
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&size.to_le_bytes())?;
    Ok(())
}

pub fn read_track_names(
    path: &Path,
    canonical_order: &[&str],
    channel_count: usize,
) -> Result<Option<Vec<String>>, IoError> {
    let mut file = File::open(path)?;
    let physical = file.metadata()?.len();
    if physical < 12 {
        return Ok(None);
    }
    let mut header = [0; 12];
    file.read_exact(&mut header)?;
    if &header[..4] != b"RIFF" || &header[8..] != b"WAVE" {
        return Ok(None);
    }
    let end = (u64::from(u32::from_le_bytes(header[4..8].try_into().unwrap())) + 8).min(physical);
    let mut found = None;
    while file.stream_position()? + 8 <= end {
        let mut chunk = [0; 8];
        file.read_exact(&mut chunk)?;
        let size = u32::from_le_bytes(chunk[4..].try_into().unwrap()) as u64;
        let next = file.stream_position()? + size + size % 2;
        if next > end {
            return Err(invalid("Truncated WAV chunk."));
        }
        if &chunk[..4] == b"ICHL" {
            if found.is_some() || size > 4096 {
                return Err(invalid("Duplicate or oversized BRIR channel mapping."));
            }
            let mut bytes = vec![0; size as usize];
            file.read_exact(&mut bytes)?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|_| invalid("Invalid BRIR channel mapping JSON."))?;
            if value["version"].as_u64() != Some(1) {
                return Err(invalid("Unsupported BRIR channel mapping version."));
            }
            let names = value["tracks"]
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .map(|v| v.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                })
                .ok_or_else(|| invalid("BRIR channel mapping does not match the WAV layout."))?;
            let mut seen = std::collections::HashSet::new();
            if names.len() != channel_count
                || names
                    .iter()
                    .any(|n| !canonical_order.contains(&n.as_str()) || !seen.insert(n))
            {
                return Err(invalid(
                    "BRIR channel mapping does not match the WAV layout.",
                ));
            }
            found = Some(names);
        }
        file.seek(SeekFrom::Start(next))?;
    }
    Ok(found)
}
