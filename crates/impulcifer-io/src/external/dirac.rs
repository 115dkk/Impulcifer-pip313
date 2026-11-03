use std::collections::BTreeSet;

use super::{
    ExternalError, ExternalFile, ExternalFormat, ExternalMeasurement, ExternalSpeaker,
    MicCalibration, assign::is_subwoofer_name,
};

const SEARCH_LIMIT: usize = 65_536;
const MAX_DEPTH: usize = 64;
const MAX_ELEMENTS: usize = 4096;
const MAX_SAMPLES: usize = 1_048_576;

#[derive(Clone, Copy)]
struct Field<'a> {
    number: u32,
    value: ProtoValue<'a>,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
enum ProtoValue<'a> {
    Varint(u64),
    Fixed64([u8; 8]),
    Bytes(&'a [u8]),
    Fixed32([u8; 4]),
}

pub(super) fn read(bytes: &[u8]) -> Result<ExternalFile, ExternalError> {
    let (title, header_end) = read_header(bytes)?;
    let max = bytes.len().min(SEARCH_LIMIT + 4);
    for p in 4..max.saturating_sub(4) {
        let Some(length) = be_u32_at(bytes, p).and_then(|v| usize::try_from(v).ok()) else {
            continue;
        };
        if length < 16 {
            continue;
        }
        let Some(end) = p.checked_add(4).and_then(|v| v.checked_add(length)) else {
            continue;
        };
        if end > bytes.len() {
            continue;
        }
        // The block follows the QDataStream header. Starting at header_end avoids
        // accepting a length-looking sequence embedded in the title itself.
        if p < header_end {
            continue;
        }
        if let Ok(file) = parse_measurement(&bytes[p + 4..end], title.clone()) {
            return Ok(file);
        }
    }
    Err(ExternalError::new("no measurement block found"))
}

fn read_header(bytes: &[u8]) -> Result<(Option<String>, usize), ExternalError> {
    if bytes.len() < 8 {
        return Err(ExternalError::new("no measurement block found"));
    }
    let _version =
        be_u32_at(bytes, 0).ok_or_else(|| ExternalError::new("truncated Dirac header"))?;
    let length = be_u32_at(bytes, 4).ok_or_else(|| ExternalError::new("truncated Dirac header"))?;
    if length == u32::MAX {
        return Ok((None, 8));
    }
    let n = usize::try_from(length).map_err(|_| ExternalError::new("Dirac title is too long"))?;
    if n % 2 != 0 || n > bytes.len().saturating_sub(8) {
        return Err(ExternalError::new("invalid Dirac title"));
    }
    let raw = &bytes[8..8 + n];
    let units = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair));
    let title: String = std::char::decode_utf16(units)
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    Ok((Some(title), 8 + n))
}

fn parse_measurement(bytes: &[u8], title: Option<String>) -> Result<ExternalFile, ExternalError> {
    let fields = parse_message(bytes, 0)?;
    let sample_rate = required_varint(&fields, 3, "measurement sample rate")?;
    let sample_rate =
        u32::try_from(sample_rate).map_err(|_| ExternalError::new("invalid Dirac sample rate"))?;
    if sample_rate == 0 {
        return Err(ExternalError::new("invalid Dirac sample rate"));
    }
    let matrix = unique_bytes(&fields, 12, "response matrix")?;
    let matrix = parse_matrix(matrix, sample_rate)?;

    let mut speakers = Vec::new();
    for bytes in all_bytes(&fields, 30) {
        let speaker_fields = parse_message(bytes, 1)?;
        let name = String::from_utf8(unique_bytes(&speaker_fields, 1, "speaker name")?.to_vec())
            .map_err(|_| ExternalError::new("invalid UTF-8 speaker name"))?;
        let kind = optional_varint(&speaker_fields, 2).unwrap_or(0);
        speakers.push(ExternalSpeaker {
            index: speakers.len() + 1,
            subwoofer: kind == 4 || is_subwoofer_name(&name),
            name,
        });
    }
    if speakers.len() != matrix.columns {
        return Err(ExternalError::new(
            "Dirac speaker count does not match response matrix columns",
        ));
    }

    let mut positions = BTreeSet::new();
    let mut measurements = Vec::new();
    for (index, element) in matrix.elements.into_iter().enumerate() {
        let Some(samples) = element else { continue };
        let row = index / matrix.columns + 1;
        let column = index % matrix.columns + 1;
        positions.insert(row);
        measurements.push(ExternalMeasurement {
            id: format!("{column}@{row}"),
            name: speakers[column - 1].name.clone(),
            speaker: Some(column),
            position: Some(row),
            start_time: 0.0,
            samples,
            skipped: None,
        });
    }

    let calibration = calibration(&fields)?;
    let microphone = optional_bytes(&fields, 24)
        .map(|bytes| String::from_utf8(bytes.to_vec()))
        .transpose()
        .map_err(|_| ExternalError::new("invalid UTF-8 microphone name"))?;

    Ok(ExternalFile {
        format: ExternalFormat::Dirac,
        title,
        sample_rate,
        microphone,
        mic_calibration: calibration,
        speakers,
        positions: positions.into_iter().collect(),
        measurements,
    })
}

struct Matrix {
    columns: usize,
    elements: Vec<Option<Vec<f64>>>,
}

fn parse_matrix(bytes: &[u8], sample_rate: u32) -> Result<Matrix, ExternalError> {
    let fields = parse_message(bytes, 1)?;
    let matrix_rate = u32::try_from(required_varint(&fields, 1, "matrix sample rate")?)
        .map_err(|_| ExternalError::new("invalid response matrix sample rate"))?;
    if matrix_rate != sample_rate {
        return Err(ExternalError::new("Dirac matrix sample rate mismatch"));
    }
    let columns = usize::try_from(required_varint(&fields, 2, "matrix columns")?)
        .map_err(|_| ExternalError::new("invalid response matrix columns"))?;
    let rows = usize::try_from(required_varint(&fields, 3, "matrix rows")?)
        .map_err(|_| ExternalError::new("invalid response matrix rows"))?;
    let expected = columns
        .checked_mul(rows)
        .filter(|count| *count <= MAX_ELEMENTS)
        .ok_or_else(|| ExternalError::new("Dirac response matrix exceeds element limit"))?;
    let raw_elements = all_bytes(&fields, 4);
    if raw_elements.len() != expected {
        return Err(ExternalError::new(
            "Dirac response matrix element count does not match dimensions",
        ));
    }
    let mut elements = Vec::with_capacity(expected);
    for element in raw_elements {
        elements.push(parse_element(element, sample_rate)?);
    }
    Ok(Matrix { columns, elements })
}

fn parse_element(bytes: &[u8], sample_rate: u32) -> Result<Option<Vec<f64>>, ExternalError> {
    let fields = parse_message(bytes, 2)?;
    if let Some(rate) = optional_varint(&fields, 1)
        && u32::try_from(rate).ok() != Some(sample_rate)
    {
        return Err(ExternalError::new("Dirac element sample rate mismatch"));
    }
    let Some(codes) = optional_bytes(&fields, 6) else {
        return Ok(None);
    };
    if codes.len() % 2 != 0 || codes.len() / 2 > MAX_SAMPLES {
        return Err(ExternalError::new("invalid Dirac response sample count"));
    }
    let scale_bytes = unique_fixed32(&fields, 5, "response scale")?;
    let scale = f32::from_le_bytes(scale_bytes) as f64;
    if !scale.is_finite() {
        return Err(ExternalError::new("non-finite Dirac response scale"));
    }
    let samples = codes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let code = i16::from_be_bytes(*pair);
            let y = (f64::from(code) / 32767.0).clamp(-1.0, 1.0);
            scale * y.signum() * (256f64.powf(y.abs()) - 1.0) / 255.0
        })
        .collect();
    Ok(Some(samples))
}

fn calibration(fields: &[Field<'_>]) -> Result<Option<MicCalibration>, ExternalError> {
    let Some(frequencies) = optional_bytes(fields, 14) else {
        return Ok(None);
    };
    let Some(gains) = optional_bytes(fields, 15) else {
        return Ok(None);
    };
    if frequencies.is_empty() || frequencies.len() != gains.len() || frequencies.len() % 4 != 0 {
        return Ok(None);
    }
    let mut frequency = Vec::new();
    let mut gain_db = Vec::new();
    for (f, g) in frequencies
        .as_chunks::<4>()
        .0
        .iter()
        .zip(gains.as_chunks::<4>().0)
    {
        let f = f32::from_le_bytes(*f) as f64;
        let g = f32::from_le_bytes(*g) as f64;
        if !f.is_finite() || !g.is_finite() {
            return Err(ExternalError::new("non-finite microphone calibration"));
        }
        if f > 0.0 {
            frequency.push(f);
            gain_db.push(g);
        }
    }
    if frequency.is_empty() {
        Ok(None)
    } else {
        Ok(Some(MicCalibration { frequency, gain_db }))
    }
}

fn parse_message(bytes: &[u8], depth: usize) -> Result<Vec<Field<'_>>, ExternalError> {
    if depth > MAX_DEPTH {
        return Err(ExternalError::new("protobuf nesting limit exceeded"));
    }
    let mut pos = 0;
    let mut fields = Vec::new();
    while pos < bytes.len() {
        let key = varint(bytes, &mut pos)?;
        let number = u32::try_from(key >> 3)
            .map_err(|_| ExternalError::new("invalid protobuf field number"))?;
        if number == 0 || number >= (1 << 29) {
            return Err(ExternalError::new("invalid protobuf field number"));
        }
        let value = match key & 7 {
            0 => ProtoValue::Varint(varint(bytes, &mut pos)?),
            1 => ProtoValue::Fixed64(
                take(bytes, &mut pos, 8)?
                    .try_into()
                    .map_err(|_| ExternalError::new("truncated protobuf field"))?,
            ),
            2 => {
                let n = usize::try_from(varint(bytes, &mut pos)?)
                    .map_err(|_| ExternalError::new("protobuf length is too large"))?;
                ProtoValue::Bytes(take(bytes, &mut pos, n)?)
            }
            5 => ProtoValue::Fixed32(
                take(bytes, &mut pos, 4)?
                    .try_into()
                    .map_err(|_| ExternalError::new("truncated protobuf field"))?,
            ),
            _ => return Err(ExternalError::new("unsupported protobuf wire type")),
        };
        fields.push(Field { number, value });
    }
    Ok(fields)
}

fn varint(bytes: &[u8], pos: &mut usize) -> Result<u64, ExternalError> {
    let mut value = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes
            .get(*pos)
            .ok_or_else(|| ExternalError::new("truncated protobuf varint"))?;
        *pos += 1;
        if shift == 63 && byte > 1 {
            return Err(ExternalError::new("protobuf varint exceeds 10 bytes"));
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(ExternalError::new("protobuf varint exceeds 10 bytes"))
}

fn take<'a>(bytes: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], ExternalError> {
    let end = pos
        .checked_add(n)
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| ExternalError::new("truncated protobuf field"))?;
    let result = &bytes[*pos..end];
    *pos = end;
    Ok(result)
}

fn required_varint(fields: &[Field<'_>], number: u32, name: &str) -> Result<u64, ExternalError> {
    optional_varint(fields, number).ok_or_else(|| ExternalError::new(format!("missing {name}")))
}

fn optional_varint(fields: &[Field<'_>], number: u32) -> Option<u64> {
    fields.iter().find_map(|field| match field {
        Field {
            number: n,
            value: ProtoValue::Varint(value),
        } if *n == number => Some(*value),
        _ => None,
    })
}

fn all_bytes<'a>(fields: &[Field<'a>], number: u32) -> Vec<&'a [u8]> {
    fields
        .iter()
        .filter_map(|field| match field {
            Field {
                number: n,
                value: ProtoValue::Bytes(value),
            } if *n == number => Some(*value),
            _ => None,
        })
        .collect()
}

fn optional_bytes<'a>(fields: &[Field<'a>], number: u32) -> Option<&'a [u8]> {
    all_bytes(fields, number).into_iter().next()
}

fn unique_bytes<'a>(
    fields: &[Field<'a>],
    number: u32,
    name: &str,
) -> Result<&'a [u8], ExternalError> {
    let values = all_bytes(fields, number);
    if values.len() != 1 {
        return Err(ExternalError::new(format!("missing or repeated {name}")));
    }
    Ok(values[0])
}

fn unique_fixed32(fields: &[Field<'_>], number: u32, name: &str) -> Result<[u8; 4], ExternalError> {
    let values: Vec<_> = fields
        .iter()
        .filter_map(|field| match field {
            Field {
                number: n,
                value: ProtoValue::Fixed32(value),
            } if *n == number => Some(*value),
            _ => None,
        })
        .collect();
    if values.len() != 1 {
        return Err(ExternalError::new(format!("missing or repeated {name}")));
    }
    Ok(values[0])
}

fn be_u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put_varint(mut value: u64, out: &mut Vec<u8>) {
        while value >= 0x80 {
            out.push((value as u8) | 0x80);
            value >>= 7;
        }
        out.push(value as u8);
    }
    fn v(field: u32, value: u64, out: &mut Vec<u8>) {
        put_varint(u64::from(field) << 3, out);
        put_varint(value, out);
    }
    fn b(field: u32, value: &[u8], out: &mut Vec<u8>) {
        put_varint((u64::from(field) << 3) | 2, out);
        put_varint(value.len() as u64, out);
        out.extend_from_slice(value);
    }
    fn f32_field(field: u32, value: f32, out: &mut Vec<u8>) {
        put_varint((u64::from(field) << 3) | 5, out);
        out.extend_from_slice(&value.to_le_bytes());
    }
    fn speaker(name: &str, kind: u64) -> Vec<u8> {
        let mut x = Vec::new();
        b(1, name.as_bytes(), &mut x);
        v(2, kind, &mut x);
        x
    }
    fn element(codes: Option<&[i16]>) -> Vec<u8> {
        let mut x = Vec::new();
        v(1, 48000, &mut x);
        f32_field(5, 2.0, &mut x);
        if let Some(codes) = codes {
            let raw: Vec<_> = codes.iter().flat_map(|c| c.to_be_bytes()).collect();
            b(6, &raw, &mut x);
        }
        x
    }
    fn project(title: &str, pad: usize) -> Vec<u8> {
        let mut matrix = Vec::new();
        v(1, 48000, &mut matrix);
        v(2, 3, &mut matrix);
        v(3, 9, &mut matrix);
        for row in 0..9 {
            for _ in 0..3 {
                b(
                    4,
                    &element(if row == 0 || row == 2 {
                        Some(&[32767, -32767, 0, 16384, -32768])
                    } else {
                        None
                    }),
                    &mut matrix,
                );
            }
        }
        let mut message = Vec::new();
        v(3, 48000, &mut message);
        b(12, &matrix, &mut message);
        let fs: Vec<_> = [0f32, 10.0, 18000.0]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        let gs: Vec<_> = [1f32, 2.0, 3.0]
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect();
        b(14, &fs, &mut message);
        b(15, &gs, &mut message);
        for (name, kind) in [("Left", 1), ("Sub2", 1), ("Bass", 4)] {
            b(30, &speaker(name, kind), &mut message);
        }
        let mut out = 10u32.to_be_bytes().to_vec();
        let utf: Vec<_> = title.encode_utf16().flat_map(u16::to_be_bytes).collect();
        out.extend_from_slice(&(utf.len() as u32).to_be_bytes());
        out.extend_from_slice(&utf);
        out.extend(std::iter::repeat_n(0u8, pad));
        out.extend_from_slice(&0u32.to_be_bytes());
        out.push(1);
        out.extend_from_slice(&(message.len() as u32).to_be_bytes());
        out.extend_from_slice(&message);
        for i in 0..64 {
            out.push((i * 73 + 19) as u8);
        }
        out
    }

    #[test]
    fn dirac_builder_finds_block_and_decodes_fields() {
        for pad in [0, 7, 101] {
            let file = read(&project("제목 🎧", pad)).unwrap();
            assert_eq!(file.title.as_deref(), Some("제목 🎧"));
            assert_eq!(file.positions, [1, 3]);
            assert_eq!(
                file.measurements
                    .iter()
                    .map(|m| m.id.as_str())
                    .collect::<Vec<_>>(),
                ["1@1", "2@1", "3@1", "1@3", "2@3", "3@3"]
            );
            assert!(!file.speakers[0].subwoofer);
            assert!(file.speakers[1].subwoofer);
            assert!(file.speakers[2].subwoofer);
            let s = &file.measurements[0].samples;
            assert_eq!(s[0], 2.0);
            assert_eq!(s[1], -2.0);
            assert_eq!(s[2], 0.0);
            assert_eq!(s[3], 2.0 * (256f64.powf(16384.0 / 32767.0) - 1.0) / 255.0);
            assert_eq!(s[4], -2.0);
            assert_eq!(
                file.mic_calibration.as_ref().unwrap().frequency,
                [10.0, 18000.0]
            );
        }
    }

    #[test]
    fn file_without_measurement_block_is_an_error() {
        assert_eq!(
            read(&[0; 32]).unwrap_err().to_string(),
            "no measurement block found"
        );
    }

    #[test]
    fn malformed_matrix_dimensions_are_rejected() {
        let mut matrix = Vec::new();
        v(1, 48000, &mut matrix);
        v(2, 2, &mut matrix);
        v(3, 1, &mut matrix);
        b(4, &element(Some(&[0])), &mut matrix);
        let mut msg = Vec::new();
        v(3, 48000, &mut msg);
        b(12, &matrix, &mut msg);
        b(30, &speaker("Left", 1), &mut msg);
        b(30, &speaker("Right", 1), &mut msg);
        assert!(parse_measurement(&msg, None).is_err());
    }

    #[test]
    fn speaker_count_mismatch_is_rejected() {
        let mut matrix = Vec::new();
        v(1, 48000, &mut matrix);
        v(2, 1, &mut matrix);
        v(3, 1, &mut matrix);
        b(4, &element(Some(&[0])), &mut matrix);
        let mut msg = Vec::new();
        v(3, 48000, &mut msg);
        b(12, &matrix, &mut msg);
        assert!(
            parse_measurement(&msg, None)
                .unwrap_err()
                .to_string()
                .contains("speaker count")
        );
    }

    #[test]
    fn fixed64_fields_are_accepted_by_wire_parser() {
        let mut bytes = vec![0x09];
        bytes.extend_from_slice(&[1; 8]);
        let fields = parse_message(&bytes, 0).unwrap();
        assert!(matches!(fields[0].value, ProtoValue::Fixed64(value) if value == [1; 8]));
    }
}
