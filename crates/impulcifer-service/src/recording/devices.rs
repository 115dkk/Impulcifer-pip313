use super::RecordingError;
use impulcifer_types::{
    audio::{AudioBackend, Endpoint},
    ipc::ErrorCode,
};
use serde_json::{Value, json};

fn endpoints(backend: &dyn AudioBackend) -> Result<Vec<Endpoint>, RecordingError> {
    backend
        .enumerate()
        .map_err(|e| RecordingError::device(e.to_string()))
}
fn host_error(api: &str) -> RecordingError {
    RecordingError {
        code: ErrorCode::InvalidRequest,
        message: "Unknown host API.".into(),
        details: json!({"host_api":api}),
        retryable: false,
    }
}
pub fn list_audio_devices(
    backend: &dyn AudioBackend,
    host_api: Option<&str>,
) -> Result<Value, Value> {
    let endpoints = endpoints(backend).map_err(|e| e.envelope())?;
    let mut apis = Vec::new();
    for ep in &endpoints {
        if !apis.contains(&ep.host_api) {
            apis.push(ep.host_api.clone());
        }
    }
    let filter = host_api.filter(|s| !s.is_empty());
    if let Some(api) = filter
        && !apis.iter().any(|a| a == api)
    {
        return Err(host_error(api).envelope());
    }
    let index = |input| {
        endpoints
            .iter()
            .position(|e| {
                if input {
                    e.is_default_input
                } else {
                    e.is_default_output
                }
            })
            .map(|i| i as i64)
            .unwrap_or(-1)
    };
    let devices:Vec<_>=endpoints.iter().enumerate().filter(|(_,e)|filter.is_none_or(|a|a==e.host_api)).map(|(i,e)|json!({"index":i,"name":e.name,"host_api":e.host_api,"max_input_channels":e.max_input_channels,"max_output_channels":e.max_output_channels})).collect();
    Ok(
        json!({"host_apis":apis,"devices":devices,"default_input_index":index(true),"default_output_index":index(false)}),
    )
}
pub fn resolve_devices(
    backend: &dyn AudioBackend,
    input_name: Option<&str>,
    output_name: Option<&str>,
    host_api: Option<&str>,
    playback_channels: u16,
) -> Result<(Endpoint, Endpoint), RecordingError> {
    let eps = endpoints(backend)?;
    let host_api = host_api.filter(|s| !s.is_empty());
    if let Some(api) = host_api
        && !eps
            .iter()
            .any(|e| e.host_api.replace("Windows ", "") == api.replace("Windows ", ""))
    {
        return Err(host_error(api));
    }
    let resolve = |name: Option<&str>, input: bool, min: u16| -> Result<Endpoint, RecordingError> {
        let kind = if input { "input" } else { "output" };
        let channels = |e: &Endpoint| {
            if input {
                e.max_input_channels
            } else {
                e.max_output_channels
            }
        };
        // Defaults are identities, not ambiguous names from another endpoint.
        if name.is_none() {
            let ep = eps
                .iter()
                .find(|e| {
                    if input {
                        e.is_default_input
                    } else {
                        e.is_default_output
                    }
                })
                .ok_or_else(|| {
                    RecordingError::device(
                        "Could not find any device which satisfies minimum channel count.",
                    )
                })?;
            if channels(ep) < min {
                return Err(RecordingError::device(
                    "Could not find any device which satisfies minimum channel count.",
                ));
            }
            return Ok(ep.clone());
        }
        let name = name.unwrap_or_default();
        let api = host_api
            .map(|s| s.replace("Windows ", ""))
            .or_else(|| eps.first().map(|e| e.host_api.replace("Windows ", "")))
            .unwrap_or_default();
        let suffix = eps.iter().find_map(|e| {
            let short = e.host_api.replace("Windows ", "");
            if name.ends_with(&e.host_api) || name.ends_with(&short) {
                Some(e.host_api.as_str())
            } else {
                None
            }
        });
        let query = name.to_lowercase();
        let matches: Vec<_> = eps
            .iter()
            .filter(|e| channels(e) > 0)
            .filter(|e| {
                let selected = suffix.unwrap_or(&api).replace("Windows ", "");
                e.host_api.replace("Windows ", "") == selected
            })
            .filter(|e| {
                let full = format!("{} {}", e.name, e.host_api).to_lowercase();
                query.split_whitespace().all(|word| full.contains(word))
            })
            .collect();
        let exact: Vec<_> = matches
            .iter()
            .copied()
            .filter(|e| {
                e.name.eq_ignore_ascii_case(name)
                    || format!("{} {}", e.name, e.host_api).eq_ignore_ascii_case(name)
                    || format!("{} {}", e.name, e.host_api.replace("Windows ", ""))
                        .eq_ignore_ascii_case(name)
            })
            .collect();
        let ep = if exact.len() == 1 {
            Some(exact[0])
        } else if matches.len() == 1 {
            Some(matches[0])
        } else {
            None
        };
        let ep = ep.ok_or_else(|| {
            RecordingError::device(format!(
                "No device found with name \"{name}\" and host API \"{api}\". "
            ))
        })?;
        if channels(ep) < min {
            let short = ep.host_api.replace("Windows ", "");
            let message = if suffix.is_some() {
                format!(
                    "Found {kind} device \"{} {short}\"\" but minimum number of channels is not satisfied. 1",
                    ep.name
                )
            } else if host_api.is_some() {
                format!(
                    "Found {kind} device \"{} {short}\" but minimum number of channels is not satisfied.",
                    ep.name
                )
            } else {
                "Could not find any device which satisfies minimum channel count.".into()
            };
            return Err(RecordingError::device(message));
        }
        Ok(ep.clone())
    };
    Ok((
        resolve(input_name, true, 1)?,
        resolve(output_name, false, playback_channels)?,
    ))
}
