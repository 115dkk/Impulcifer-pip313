use super::{
    RecordingError,
    channel_order::{ChannelOrder, hdmi_outputs, playback_layout, to_hdmi_order},
    request::ValidatedRecording,
};
use impulcifer_types::{
    audio::{AudioBackend, Endpoint},
    constants::{hdmi_track_order, sweep_track_order},
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
    let hdmi = hdmi_outputs();
    let devices:Vec<_>=endpoints.iter().enumerate().filter(|(_,e)|filter.is_none_or(|a|a==e.host_api)).map(|(i,e)|json!({"index":i,"name":e.name,"host_api":e.host_api,"max_input_channels":e.max_input_channels,"max_output_channels":e.max_output_channels,"hdmi":hdmi.contains(&e.name)})).collect();
    Ok(
        json!({"host_apis":apis,"devices":devices,"default_input_index":index(true),"default_output_index":index(false)}),
    )
}
pub fn resolve_devices(
    backend: &dyn AudioBackend,
    input_name: Option<&str>,
    output_name: Option<&str>,
    host_api: Option<&str>,
    _playback_channels: u16,
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
                "No device found with name \"{name}\" and host API \"{api}\"."
            ))
        })?;
        if channels(ep) < min {
            let short = ep.host_api.replace("Windows ", "");
            let message = if suffix.is_some() || host_api.is_some() {
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
        resolve(output_name, false, 1)?,
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputShortage {
    pub available: u16,
    pub required: u16,
    pub first: usize,
    pub last: usize,
    pub missing: Vec<String>,
}

impl OutputShortage {
    pub fn missing_text(&self) -> String {
        self.missing.join(", ")
    }

    pub fn message(&self, device: &str) -> String {
        format!(
            "{device} has {} output channels, so channels {}–{} ({}) were not played.",
            self.available,
            self.first,
            self.last,
            self.missing_text()
        )
    }
}

pub struct OutputPlan {
    pub channel_order: Option<ChannelOrder>,
    pub slots: Option<Vec<String>>,
    pub layout: String,
    pub shortage: Option<OutputShortage>,
}

pub struct OutputWarning {
    pub device: String,
    pub layout: String,
    pub shortage: OutputShortage,
}

/// Best-effort preflight used by the confirmation flow. Device and playback
/// errors remain the recording job's responsibility.
pub fn output_warning(
    backend: &dyn AudioBackend,
    request: &ValidatedRecording,
) -> Option<OutputWarning> {
    let mut tracks = if let Some(spec) = request.sweep_spec.as_ref() {
        super::sweep::build_sweep_playback(spec).ok()?.tracks
    } else {
        let mut wav =
            impulcifer_io::read_wav(std::path::Path::new(request.play_path.as_deref()?)).ok()?;
        if request.mode == super::request::Mode::Headphones && wav.tracks.len() == 1 {
            wav.tracks.push(wav.tracks[0].clone());
        }
        wav.tracks
    };
    let channels = u16::try_from(tracks.len()).ok()?;
    let (_, output) = resolve_devices(
        backend,
        request.input_device.as_deref(),
        request.output_device.as_deref(),
        request.host_api.as_deref(),
        channels,
    )
    .ok()?;
    let plan = output_plan(request, &output, &mut tracks);
    Some(OutputWarning {
        device: output.name,
        layout: plan.layout,
        shortage: plan.shortage?,
    })
}

/// Applies the effective device channel order and reports only signal-carrying
/// rows which the selected output cannot play.
pub fn output_plan(
    request: &ValidatedRecording,
    output: &Endpoint,
    tracks: &mut Vec<Vec<f64>>,
) -> OutputPlan {
    let layout = playback_layout(
        request.sweep_spec.as_ref(),
        request.play_path.as_deref(),
        tracks.len(),
    );
    let channel_order = layout.as_ref().map(|layout| {
        let order = match request.channel_order {
            ChannelOrder::Hdmi => ChannelOrder::Hdmi,
            ChannelOrder::Standard => ChannelOrder::Standard,
            ChannelOrder::Auto => {
                if hdmi_outputs().contains(&output.name) {
                    ChannelOrder::Hdmi
                } else {
                    ChannelOrder::Standard
                }
            }
        };
        if order == ChannelOrder::Hdmi {
            *tracks = to_hdmi_order(std::mem::take(tracks), layout);
        }
        order
    });
    let slots = layout.as_ref().and_then(|layout| {
        let order = if channel_order == Some(ChannelOrder::Hdmi) {
            hdmi_track_order(layout)
        } else {
            sweep_track_order(layout)
        }?;
        Some(order.into_iter().map(str::to_owned).collect::<Vec<_>>())
    });
    let available = usize::from(output.max_output_channels);
    let active: Vec<_> = tracks
        .iter()
        .enumerate()
        .skip(available)
        .filter(|(_, row)| row.iter().any(|sample| *sample != 0.0))
        .map(|(index, _)| index)
        .collect();
    let shortage = if let (Some(first), Some(last)) = (active.first(), active.last()) {
        let missing = active
            .iter()
            .filter_map(|index| {
                let name = slots
                    .as_ref()
                    .and_then(|slots| slots.get(*index))
                    .cloned()
                    .unwrap_or_else(|| (index + 1).to_string());
                (!matches!(name.as_str(), "LFE" | "LFE2")).then_some(name)
            })
            .collect();
        Some(OutputShortage {
            available: output.max_output_channels,
            required: u16::try_from(tracks.len()).unwrap_or(u16::MAX),
            first: first + 1,
            last: last + 1,
            missing,
        })
    } else {
        None
    };
    OutputPlan {
        channel_order,
        slots,
        layout: layout.unwrap_or_default(),
        shortage,
    }
}
