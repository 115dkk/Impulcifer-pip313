// @ts-check
// Recorder form: devices, sweep source, channel order, layout map and the recording starts.
"use strict";

/* --------------------------------------------------------------- devices */

async function loadDevices(hostApi = "") {
  const response = await api().list_audio_devices(hostApi || null);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  const hostSelect = el("rf-host-api", HTMLSelectElement);
  const previousHost = hostSelect.value;
  hostSelect.replaceChildren(new Option(t("option_device_auto"), ""));
  $("rf-host-api-row").hidden = response.data.host_apis.length <= 1;
  response.data.host_apis.forEach((name) => hostSelect.add(new Option(name, name)));
  if ([...hostSelect.options].some((option) => option.value === previousHost)) {
    hostSelect.value = previousHost;
  }
  const devices = response.data.devices;
  state.audioDevices = devices;
  state.defaultOutputIndex = response.data.default_output_index;
  fillDevices(el("rf-input-device", HTMLSelectElement), devices.filter((item) => item.max_input_channels > 0));
  fillDevices(el("rf-output-device", HTMLSelectElement), devices.filter((item) => item.max_output_channels > 0));
  updateChannelOrder();
}

/** @param {HTMLSelectElement} select @param {AudioDevice[]} devices */
function fillDevices(select, devices) {
  const previous = select.value;
  select.replaceChildren(new Option(t("option_device_default"), ""));
  devices.forEach((device) => select.add(new Option(device.name, device.name)));
  if ([...select.options].some((option) => option.value === previous)) {
    select.value = previous;
  }
}

/* -------------------------------------------------------------- recorder */

function sweepSourceMode() {
  return val("rf-sweep-source") || "default";
}

function gatherSweepPayload() {
  /* null => legacy play-file flow; otherwise the on-the-fly sweep object
     understood by the service (mode/speakers/tracks + fs/duration for
     custom mode). */
  const mode = sweepSourceMode();
  if (mode === "file") return null;
  /** @type {SweepRequest} */
  const sweep = {
    mode,
    speakers: val("rf-sweep-speakers"),
    tracks: val("rf-sweep-layout") || "stereo",
  };
  if (mode === "custom") {
    const defaults = state.sweepDefaults || {};
    sweep.fs = Math.trunc(numOr("rf-sweep-fs", defaults.default_fs || 48000));
    sweep.duration = numOr("rf-sweep-duration", defaults.default_duration || 5.0);
  }
  return sweep;
}

/** @param {SweepRequest | null} sweep */
function sweepDisplayName(sweep) {
  if (!sweep) return val("rf-play");
  const defaults = state.sweepDefaults || {};
  return fmt(t("label_sweep_generated_summary"), {
    speakers: sweep.speakers,
    tracks: sweep.tracks,
    fs: sweep.fs || defaults.default_fs || 48000,
    duration: sweep.duration || defaults.default_duration || 5.0,
  });
}

function updateSweepSourceVisibility() {
  const mode = sweepSourceMode();
  $("rf-sweep-params").hidden = mode === "file";
  $("rf-sweep-custom").hidden = mode !== "custom";
  $("rf-play-row").hidden = mode !== "file";
}

/** @param {string} id */
function immersiveLayout(id) {
  return (state.sweepDefaults?.immersive || []).find((layout) => layout.id === id) || null;
}

/** @param {string[]} layouts */
function populateSweepLayouts(layouts) {
  const select = el("rf-sweep-layout", HTMLSelectElement);
  const previous = select.value;
  select.replaceChildren();
  layouts.forEach((layout) => select.add(new Option(immersiveLayout(layout)?.name || layout, layout)));
  select.value = layouts.includes(previous) ? previous : "stereo";
  updateChannelOrder();
}

/* ------------------------------------------------- HDMI channel order */

/* On a Mac an HDMI or DisplayPort output takes its channels in the
   receiver's slot order (CEA-861: LFE on 3, FC on 4, the side pair on 5-6,
   the back pair on 7-8; ADR 0012), not in the Windows order the sweep tracks
   are written in, so the FC sweep played from the subwoofer. "auto" uses the
   HDMI order when the service reports the chosen output as HDMI. */

/** The output device the recording opens: the chosen one, else the default. */
function selectedOutputDevice() {
  const name = val("rf-output-device");
  return state.audioDevices.find((device) => name
    ? device.name === name && device.max_output_channels > 0
    : device.index === state.defaultOutputIndex) || null;
}

/** @returns {"standard" | "hdmi"} */
function effectiveChannelOrder() {
  const choice = val("rf-channel-order") || "auto";
  if (choice === "standard" || choice === "hdmi") return choice;
  return selectedOutputDevice()?.hdmi ? "hdmi" : "standard";
}

/* The layout the choice reorders: the generated sweep's, or the layout in an
   Impulcifer sweep file name (sweep-seg-FL,FR,...-7.1-...wav). */
function channelOrderLayout() {
  const orders = state.sweepDefaults?.hdmi_orders || {};
  if (sweepSourceMode() !== "file") {
    const layout = val("rf-sweep-layout");
    return orders[layout] ? layout : null;
  }
  const name = val("rf-play").split(/[\\/]/).pop() || "";
  const layout = /^sweep-seg-[^-]+-([^-]+)-/.exec(name)?.[1] || "";
  return orders[layout] ? layout : null;
}

function updateChannelOrder() {
  const shown = state.platform === "darwin" && !!channelOrderLayout();
  $("rf-channel-order-row").hidden = !shown;
  const auto = $("rf-channel-order-auto");
  auto.hidden = !shown || (val("rf-channel-order") || "auto") !== "auto";
  auto.textContent = t(effectiveChannelOrder() === "hdmi"
    ? "hint_channel_order_auto_hdmi" : "hint_channel_order_auto_standard");
  renderLayoutMap();
  updateChannelShortage();
}

/* The chosen layout's device channels in the order the recording plays them:
   an immersive format's official order, otherwise the track order, reordered
   for an HDMI output on a Mac. Empty for mono and stereo. */
function layoutChannels() {
  const id = val("rf-sweep-layout");
  const immersive = immersiveLayout(id);
  const standard = state.sweepDefaults?.track_orders?.[id] || [];
  const hdmi = state.platform === "darwin" ? state.sweepDefaults?.hdmi_orders?.[id] : undefined;
  /** @type {{label: string, slot: string}[]} */
  let channels = immersive ? immersive.channels : standard.map((slot) => ({ label: slot, slot }));
  if (hdmi && effectiveChannelOrder() === "hdmi") {
    channels = hdmi.map((slot) => channels.find((channel) => channel.slot === slot) || { label: slot, slot });
  }
  return channels;
}

/* An output with fewer channels than the layout still records (the maintainer
   chose a warning over a block), but the swept speakers past the device's last
   channel do not play. Null when nothing that is swept is lost. */
function channelShortage() {
  if (sweepSourceMode() === "file") return null;
  const device = selectedOutputDevice();
  const channels = layoutChannels();
  if (!device || channels.length <= device.max_output_channels) return null;
  const swept = new Set(val("rf-sweep-speakers").split(",").map((code) => code.trim().toUpperCase()));
  const missing = channels
    .map((channel, index) => ({ slot: channel.slot, number: index + 1 }))
    .filter((channel) => channel.number > device.max_output_channels
      && !channel.slot.startsWith("LFE") && swept.has(channel.slot));
  if (!missing.length) return null;
  const id = val("rf-sweep-layout");
  return {
    device: device.name, available: device.max_output_channels, required: channels.length,
    layout: immersiveLayout(id)?.name || id,
    first: missing[0].number, last: missing[missing.length - 1].number,
    missing: missing.map((channel) => channel.slot).join(", "),
  };
}

function updateChannelShortage() {
  const shortage = channelShortage();
  const note = $("rf-channel-shortage");
  note.hidden = !shortage;
  note.textContent = shortage ? fmt(t("warning_output_channels_short"), shortage) : "";
  const button = el("btn-start-recording", HTMLButtonElement);
  if (shortage) button.dataset.warn = "true";
  else delete button.dataset.warn;
}

/* An immersive layout (ADR 0007) numbers its device channels in the format's
   official order, and its labels can name other positions than Impulcifer's
   speaker codes (22.2's FL is WL), so the map shows the code each channel
   plays. LFE channels stay silent. On a Mac the 2.x layouts that have an HDMI
   order show the map too, in the order the recording will use. */
function renderLayoutMap() {
  const id = val("rf-sweep-layout");
  const shown = !!immersiveLayout(id) || !!channelShortage()
    || (state.platform === "darwin" && !!state.sweepDefaults?.hdmi_orders?.[id]);
  const channels = shown ? layoutChannels() : [];
  const available = selectedOutputDevice()?.max_output_channels ?? Infinity;
  $("rf-layout-row").hidden = !channels.length;
  if (!channels.length) return;
  $("rf-layout-map").replaceChildren(...channels.map((channel, index) => {
    const lfe = channel.slot.startsWith("LFE");
    const item = document.createElement("li");
    item.className = "layout-ch";
    item.dataset.lfe = String(lfe);
    item.dataset.missing = String(index + 1 > available);
    /** @param {string} className @param {string} text */
    const cell = (className, text) => {
      const span = document.createElement("span");
      span.className = className; span.textContent = text;
      return span;
    };
    const arrow = cell("layout-ch-arrow", "→");
    arrow.setAttribute("aria-hidden", "true");
    item.append(cell("layout-ch-num", String(index + 1)), cell("layout-ch-label", channel.label),
      arrow, cell("layout-ch-slot", lfe ? "—" : channel.slot));
    return item;
  }));
}

/* Choosing a layout means measuring all of it: Speakers takes every speaker the
   layout sweeps, so the recording is named after them (7.1: FL,FR,FC,BL,BR,SL,SR.wav)
   and processing reads each one without the list being typed. */
function useLayoutSpeakers() {
  const speakers = state.sweepDefaults?.layout_speakers?.[val("rf-sweep-layout")];
  if (speakers) el("rf-sweep-speakers", HTMLInputElement).value = speakers.join(",");
  refreshResolvedPath();
  updateChannelOrder();
}

async function refreshResolvedPath() {
  const node = $("rf-resolved-path");
  const recordDir = val("rf-record-dir");
  const sweep = gatherSweepPayload();
  const playPath = val("rf-play");
  if (!recordDir || (!sweep && !playPath) || !window.pywebview) {
    node.textContent = "";
    return;
  }
  const response = await api().resolve_recording_paths(recordDir, playPath, "speakers", sweep, checked("rf-append"));
  if (response.ok) {
    node.textContent = fmt(t("label_record_resolved_path"), { path: response.data.record_path });
  } else {
    node.textContent = "";
  }
}

function updateChannelGuidance() {
  const node = $("rf-channel-guidance");
  el("rf-channels", HTMLInputElement).disabled = !checked("rf-force-channels");
  if (!checked("rf-force-channels")) {
    node.textContent = t("message_using_default_recording");
    return;
  }
  const channels = Math.trunc(numOr("rf-channels", 0));
  if (channels === 14) {
    node.textContent = fmt(t("message_channel_guidance_standard"), {
      channels, speakers: 7, speaker_list: "FL,FR,FC,BL,BR,SL,SR",
    });
  } else if (channels === 22) {
    node.textContent = fmt(t("message_channel_guidance_atmos_704"), {
      channels, speakers: 11, speaker_list: "FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR",
    });
  } else if (channels === 26) {
    node.textContent = fmt(t("message_channel_guidance_atmos_706"), {
      channels, speakers: 13, speaker_list: "FL,FR,FC,BL,BR,SL,SR,TFL,TFR,TBL,TBR,TSL,TSR",
    });
  } else if (channels > 0) {
    node.textContent = fmt(t("message_channel_guidance_custom"), {
      channels, speakers: Math.trunc(channels / 2),
    });
  } else {
    node.textContent = t("message_channel_guidance_invalid");
  }
}

/** @param {"speakers" | "headphones"} mode */
function gatherRecordingPayload(mode) {
  const sweep = gatherSweepPayload();
  /** @type {RecordingRequest} */
  const payload = {
    mode,
    record_dir: val("rf-record-dir"),
    input_device: val("rf-input-device") || null,
    output_device: val("rf-output-device") || null,
    host_api: val("rf-host-api") || null,
    share_mode: selectedShareMode(),
    channel_order: /** @type {ChannelOrder} */ (val("rf-channel-order") || "auto"),
  };
  if (sweep) {
    payload.sweep = sweep;
  } else {
    payload.play_path = val("rf-play");
  }
  // Earlier speaker takes in this folder, for angle and polarity checks and for
  // comparing the speaker and headphone recordings' left/right balance.
  payload.reference_measurements = (state.takeMeasurements[folderKey(payload.record_dir)] || []).map((item) => ({
    speaker: item.speaker, itd_us: item.itd_us, lf_db: item.lf_db ?? null, azimuth: item.azimuth ?? null,
    lf_ild_db: item.lf_ild_db ?? null, direct: item.direct ?? null,
  }));
  if (mode === "speakers") {
    payload.force_channels = checked("rf-force-channels");
    payload.channels = payload.force_channels ? Math.trunc(numOr("rf-channels", 2)) : 2;
    payload.append = checked("rf-append");
    payload.debug_plots = checked("rf-debug-plots");
  }
  return payload;
}

/** @param {RecordingRequest} payload */
async function recordingConfirmationValues(payload) {
  const response = await api().resolve_recording_paths(payload.record_dir, payload.play_path || null, payload.mode, payload.sweep || null, Boolean(payload.append));
  return {
    play_file: sweepDisplayName(payload.sweep || null),
    record_file: response.ok ? response.data.record_path : payload.record_dir,
    input_device: payload.input_device || t("option_device_default"),
    output_device: payload.output_device || t("option_device_default"),
    channels: payload.channels || 2,
    share_mode: sharePreferenceLabel(payload.share_mode || "auto"),
  };
}

async function startSpeakersRecording() {
  const payload = gatherRecordingPayload("speakers");
  const setup = fmt(t("message_recording_setup_info"), await recordingConfirmationValues(payload));
  if (!(await confirmDialog(setup))) return;
  await begin((request) => api().start_recording(request), payload, "recording");
}

async function startHeadphonesRecording() {
  const payload = gatherRecordingPayload("headphones");
  if (!(await confirmDialog(fmt(t("message_record_headphones_confirm"), await recordingConfirmationValues(payload))))) return;
  await begin((request) => api().start_recording(request), payload, "recording");
}

async function generateSweepSet() {
  const dirResponse = await api().select_directory();
  const folder = dirResponse.ok ? dirResponse.data.path : null;
  if (!folder) return;
  const button = el("btn-sweep-set", HTMLButtonElement);
  button.disabled = true;
  try {
    const response = await api().generate_sweep_set(folder);
    if (!response.ok) {
      appendLog(errorText(response));
      void noticeDialog(t("message_sweep_set_error"));
      return;
    }
    if (response.data.play_path) {
      el("rf-play", HTMLInputElement).value = response.data.play_path;
      refreshResolvedPath();
    }
    void noticeDialog(fmt(t("message_sweep_set_complete"), {
      count: response.data.files.length,
      folder,
    }));
  } finally {
    button.disabled = false;
  }
}
