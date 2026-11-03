use super::{
    ExternalError, ExternalFile, ExternalFormat, ExternalMeasurement, Skip,
    java_stream::{self, FieldValue, JavaStream, TopContent, Value, ValueRef},
};

pub(super) fn read(bytes: &[u8]) -> Result<ExternalFile, ExternalError> {
    let stream = java_stream::read(bytes)?;
    validate_header(&stream)?;
    let mut measurements = Vec::new();
    let mut sample_rate = None;
    let mut first_rate = None;
    for reference in stream.contents.iter().filter_map(|content| match content {
        TopContent::Value(Some(reference)) => Some(*reference),
        TopContent::Value(None) | TopContent::BlockData(_) => None,
    }) {
        let Value::Object(object) = stream.value(reference)? else {
            continue;
        };
        if object.class_name != "roomeqwizard.MeasData" {
            continue;
        }
        let rate_i32 = int_field(object.field("sampleRate"), "sampleRate")?;
        let rate =
            u32::try_from(rate_i32).map_err(|_| ExternalError::new("invalid REW sample rate"))?;
        if rate == 0 {
            return Err(ExternalError::new("invalid REW sample rate"));
        }
        first_rate.get_or_insert(rate);
        let name = string_field(&stream, object.field("shortDesc"), "shortDesc")?;
        let impulse = object_reference(object.field("irData"));
        let extracted = impulse
            .and_then(|ir_data| object_at(&stream, ir_data).ok())
            .and_then(|ir_data| object_reference(ir_data.field("ir")))
            .and_then(|sampled| object_at(&stream, sampled).ok())
            .map(|sampled| extract_samples(&stream, sampled, rate))
            .transpose()?;
        let (start_time, samples, skipped) = match extracted {
            Some((start, samples)) if !samples.is_empty() => {
                if let Some(expected) = sample_rate {
                    if expected != rate {
                        return Err(ExternalError::new(
                            "REW impulse responses have different sample rates",
                        ));
                    }
                } else {
                    sample_rate = Some(rate);
                }
                (start, samples, None)
            }
            _ => (0.0, Vec::new(), Some(Skip::NoImpulseResponse)),
        };
        measurements.push(ExternalMeasurement {
            id: (measurements.len() + 1).to_string(),
            name,
            speaker: None,
            position: None,
            start_time,
            samples,
            skipped,
        });
    }
    if measurements.is_empty() {
        return Err(ExternalError::new("not a REW measurement file"));
    }
    Ok(ExternalFile {
        format: ExternalFormat::Rew,
        title: None,
        sample_rate: sample_rate.or(first_rate).unwrap_or(0),
        microphone: None,
        mic_calibration: None,
        speakers: Vec::new(),
        positions: Vec::new(),
        measurements,
    })
}

fn validate_header(stream: &JavaStream) -> Result<(), ExternalError> {
    let Some(TopContent::Value(Some(reference))) = stream.contents.first() else {
        return Err(ExternalError::new("not a REW measurement file"));
    };
    match stream.value(*reference)? {
        Value::String(header) if header.starts_with("REW Measurement Data File") => Ok(()),
        _ => Err(ExternalError::new("not a REW measurement file")),
    }
}

fn extract_samples(
    stream: &JavaStream,
    sampled: &java_stream::ObjectValue,
    rate: u32,
) -> Result<(f64, Vec<f64>), ExternalError> {
    let Some(data_ref) = object_reference(sampled.field("data")) else {
        return Ok((0.0, Vec::new()));
    };
    let Value::Floats(values) = stream.value(data_ref)? else {
        return Err(ExternalError::new("REW impulse data is not a float array"));
    };
    let start = double_field(sampled.field("startTime"), "startTime")?;
    let interval = double_field(sampled.field("T"), "T")?;
    if !start.is_finite() || !interval.is_finite() || interval <= 0.0 {
        return Err(ExternalError::new("invalid REW impulse timing"));
    }
    let derived = 1.0 / interval;
    if (derived - f64::from(rate)).abs() > f64::from(rate) / 1_000_000.0 {
        return Err(ExternalError::new(
            "REW sample interval does not match sample rate",
        ));
    }
    if values.iter().any(|sample| !sample.is_finite()) {
        return Err(ExternalError::new(
            "REW impulse contains a non-finite sample",
        ));
    }
    Ok((start, values.iter().copied().map(f64::from).collect()))
}

fn object_at(
    stream: &JavaStream,
    reference: ValueRef,
) -> Result<&java_stream::ObjectValue, ExternalError> {
    match stream.value(reference)? {
        Value::Object(object) => Ok(object),
        _ => Err(ExternalError::new("REW field is not an object")),
    }
}

fn object_reference(field: Option<&FieldValue>) -> Option<ValueRef> {
    match field {
        Some(FieldValue::Value(reference)) => *reference,
        _ => None,
    }
}

fn string_field(
    stream: &JavaStream,
    field: Option<&FieldValue>,
    name: &str,
) -> Result<String, ExternalError> {
    let reference = object_reference(field)
        .ok_or_else(|| ExternalError::new(format!("REW field {name} is missing")))?;
    match stream.value(reference)? {
        Value::String(value) => Ok(value.clone()),
        _ => Err(ExternalError::new(format!(
            "REW field {name} is not a string"
        ))),
    }
}

fn int_field(field: Option<&FieldValue>, name: &str) -> Result<i32, ExternalError> {
    match field {
        Some(FieldValue::Int(value)) => Ok(*value),
        _ => Err(ExternalError::new(format!(
            "REW field {name} is not an integer"
        ))),
    }
}

fn double_field(field: Option<&FieldValue>, name: &str) -> Result<f64, ExternalError> {
    match field {
        Some(FieldValue::Double(value)) => Ok(*value),
        _ => Err(ExternalError::new(format!(
            "REW field {name} is not a double"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_rew_java_stream_is_rejected() {
        let bytes = [0xac, 0xed, 0, 5, 0x74, 0, 3, b'n', b'o', b't'];
        assert_eq!(
            read(&bytes).unwrap_err().to_string(),
            "not a REW measurement file"
        );
    }
}
