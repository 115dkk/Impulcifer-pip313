"use strict";

/* Impulcifer WebView frontend — Pulse Studio shell.
   Talks to application/impulcifer_service.py through the pywebview bridge.
   BRIR payload assembly mirrors gui/brir_args.build_brir_args: gated
   option groups are omitted entirely while their disclosure is closed so
   ProcessingConfig defaults stay authoritative. */

const state = {
  booted: false,
  version: "",
  platform: "",
  strings: {},
  language: "en",
  theme: "dark",
  jobId: null,
  jobKind: null,
  lastJob: null,
  nextSeq: 0,
  pollTimer: null,
  resolvedRecordPath: "",
  lastOutputDir: null,
  systemThemeQuery: null,
};

const DECAY_CHANNELS = ["FL", "FC", "FR", "SL", "SR", "BL", "BR"];

const $ = (id) => document.getElementById(id);
const api = () => window.pywebview.api;

/* ------------------------------------------------------------------ i18n */

function t(key) {
  return state.strings[key] || key;
}

function fmt(text, vars) {
  return text.replace(/\{(\w+)\}/g, (match, name) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : match,
  );
}

function applyStrings() {
  document.documentElement.lang = state.language;
  document.querySelectorAll("[data-i18n]").forEach((node) => {
    node.textContent = t(node.dataset.i18n);
  });
  updateChannelGuidance();
  refreshResolvedPath();
  renderJobState(state.lastJob);
}

/* ----------------------------------------------------------------- theme */

function applyTheme(code) {
  state.theme = code;
  if (state.systemThemeQuery) {
    state.systemThemeQuery.removeEventListener("change", onSystemThemeChange);
    state.systemThemeQuery = null;
  }
  let resolved = code;
  if (code === "system") {
    state.systemThemeQuery = window.matchMedia("(prefers-color-scheme: dark)");
    state.systemThemeQuery.addEventListener("change", onSystemThemeChange);
    resolved = state.systemThemeQuery.matches ? "dark" : "light";
  }
  document.documentElement.dataset.theme = resolved === "dark" ? "dark" : "light";
}

function onSystemThemeChange(event) {
  document.documentElement.dataset.theme = event.matches ? "dark" : "light";
}

/* ------------------------------------------------------------ primitives */

function val(id) {
  return $(id).value.trim();
}

function checked(id) {
  return $(id).checked;
}

function numOrNull(id) {
  const raw = val(id);
  if (!raw) return null;
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? parsed : null;
}

function numOr(id, fallback) {
  const parsed = numOrNull(id);
  return parsed === null ? fallback : parsed;
}

function isOpen(id) {
  return $(id).classList.contains("open");
}

function errorText(response) {
  if (!response || response.ok) return "Unknown error";
  const detail = response.error.details || {};
  const extra = Object.keys(detail).length ? ` ${JSON.stringify(detail)}` : "";
  return `${response.error.code}: ${response.error.message}${extra}`;
}

function appendLog(message) {
  document.querySelectorAll("[data-log]").forEach((log) => {
    const lines = log.textContent ? log.textContent.split("\n") : [];
    lines.push(message);
    log.textContent = lines.slice(-500).join("\n");
    log.scrollTop = log.scrollHeight;
  });
}

function setProgress(value) {
  document.querySelectorAll("[data-progress]").forEach((bar) => {
    bar.style.width = `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
  });
}

function renderJobState(job) {
  state.lastJob = job;
  const busy = Boolean(job && !["succeeded", "failed", "cancelled"].includes(job.status));
  document.querySelectorAll("[data-start]").forEach((button) => {
    button.disabled = busy;
  });
  $("btn-cancel-brir").disabled = !busy || !job.cancellable;
  const label = job
    ? `${job.kind} · ${t(`webview_status_${job.status}`)}`
    : t("webview_job_idle");
  document.querySelectorAll("[data-job-state]").forEach((node) => {
    node.textContent = label;
  });
}

/* ------------------------------------------------------------------ jobs */

async function begin(start, payload) {
  let response = await start(payload);
  if (response?.error?.code === "CONFIRMATION_REQUIRED") {
    if (!window.confirm(confirmationText(response))) return;
    payload.confirm_warnings = true;
    response = await start(payload);
  }
  if (!response) return;
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  const job = response.data.job;
  state.jobId = job.job_id;
  state.jobKind = job.kind;
  state.nextSeq = 0;
  state.lastOutputDir = job.kind === "brir" ? val("bf-dir-path") : null;
  $("btn-open-output").hidden = true;
  setProgress(0);
  renderJobState(job);
  schedulePoll(0);
}

function confirmationText(response) {
  const details = response.error.details || {};
  if (details.warning === "headphones_mono") return t("message_headphones_mono_warning");
  if (details.play_channels !== undefined) {
    return fmt(t("message_channel_mismatch_warning"), {
      play_channels: details.play_channels,
      record_channels: details.record_channels ?? details.selected_channels ?? "?",
    });
  }
  return response.error.message;
}

function schedulePoll(delay = 250) {
  window.clearTimeout(state.pollTimer);
  state.pollTimer = window.setTimeout(pollJob, delay);
}

async function pollJob() {
  if (!state.jobId) return;
  const response = await api().poll_job(state.jobId, state.nextSeq);
  if (!response.ok) {
    appendLog(errorText(response));
    renderJobState(null);
    state.jobId = null;
    return;
  }
  const { job, events, next_seq: nextSeq } = response.data;
  state.nextSeq = nextSeq;
  for (const event of events) {
    const payload = event.payload || {};
    if (event.type === "progress") {
      if (typeof payload.progress === "number") setProgress(payload.progress);
      if (payload.message) appendLog(payload.message);
      else if (payload.phase) {
        const speakers = Array.isArray(payload.speakers) ? ` ${payload.speakers.join(",")}` : "";
        appendLog(`${payload.phase}${speakers}`);
      }
    }
    if (event.type === "log") appendLog(`[${payload.level}] ${payload.message}`);
    if (event.type === "status") appendLog(`· ${t(`webview_status_${payload.status}`)}`);
  }
  renderJobState(job);
  if (["succeeded", "failed", "cancelled"].includes(job.status)) {
    if (job.status === "succeeded") setProgress(1);
    if (job.error) appendLog(`${job.error.code}: ${job.error.message}`);
    if (job.status === "succeeded" && job.kind === "brir" && state.lastOutputDir) {
      $("btn-open-output").hidden = false;
    }
    state.jobId = null;
    return;
  }
  schedulePoll();
}

/* --------------------------------------------------------------- devices */

async function loadDevices(hostApi = "") {
  const response = await api().list_audio_devices(hostApi || null);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  const hostSelect = $("rf-host-api");
  const previousHost = hostSelect.value;
  hostSelect.replaceChildren(new Option("Auto", ""));
  response.data.host_apis.forEach((name) => hostSelect.add(new Option(name, name)));
  if ([...hostSelect.options].some((option) => option.value === previousHost)) {
    hostSelect.value = previousHost;
  }
  const devices = response.data.devices;
  fillDevices($("rf-input-device"), devices.filter((item) => item.max_input_channels > 0));
  fillDevices($("rf-output-device"), devices.filter((item) => item.max_output_channels > 0));
}

function fillDevices(select, devices) {
  const previous = select.value;
  select.replaceChildren(new Option("Default", ""));
  devices.forEach((device) => select.add(new Option(device.name, device.name)));
  if ([...select.options].some((option) => option.value === previous)) {
    select.value = previous;
  }
}

/* -------------------------------------------------------------- recorder */

async function refreshResolvedPath() {
  const node = $("rf-resolved-path");
  const recordDir = val("rf-record-dir");
  const playPath = val("rf-play");
  if (!recordDir || !playPath || !window.pywebview) {
    node.textContent = "";
    return;
  }
  const response = await api().resolve_recording_paths(recordDir, playPath, "speakers");
  if (response.ok) {
    state.resolvedRecordPath = response.data.record_path;
    node.textContent = fmt(t("label_record_resolved_path"), { path: response.data.record_path });
  } else {
    node.textContent = "";
  }
}

function updateChannelGuidance() {
  const node = $("rf-channel-guidance");
  $("rf-channels").disabled = !checked("rf-force-channels");
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

function gatherRecordingPayload(mode) {
  const payload = {
    mode,
    play_path: val("rf-play"),
    record_dir: val("rf-record-dir"),
    input_device: val("rf-input-device") || null,
    output_device: val("rf-output-device") || null,
    host_api: val("rf-host-api") || null,
  };
  if (mode === "speakers") {
    payload.channels = checked("rf-force-channels") ? Math.trunc(numOr("rf-channels", 2)) : 2;
    payload.append = checked("rf-append");
    payload.debug_plots = checked("rf-debug-plots");
  }
  return payload;
}

async function startSpeakersRecording() {
  const payload = gatherRecordingPayload("speakers");
  await refreshResolvedPath();
  const setup = fmt(t("message_recording_setup_info"), {
    play_file: payload.play_path,
    record_file: state.resolvedRecordPath || "—",
    input_device: payload.input_device || "Default",
    output_device: payload.output_device || "Default",
    channels: payload.channels,
    host_api: payload.host_api || "Auto",
  });
  if (!window.confirm(setup)) return;
  await begin((request) => api().start_recording(request), payload);
}

async function startHeadphonesRecording() {
  if (!window.confirm(t("message_record_headphones_confirm"))) return;
  await begin((request) => api().start_recording(request), gatherRecordingPayload("headphones"));
}

async function generateSweepSet() {
  const dirResponse = await api().select_directory();
  const folder = dirResponse.ok ? dirResponse.data.path : null;
  if (!folder) return;
  const button = $("btn-sweep-set");
  button.disabled = true;
  try {
    const response = await api().generate_sweep_set(folder);
    if (!response.ok) {
      appendLog(errorText(response));
      window.alert(t("message_sweep_set_error"));
      return;
    }
    if (response.data.play_path) {
      $("rf-play").value = response.data.play_path;
      refreshResolvedPath();
    }
    window.alert(fmt(t("message_sweep_set_complete"), {
      count: response.data.files.length,
      folder,
    }));
  } finally {
    button.disabled = false;
  }
}

/* ------------------------------------------------------------------ brir */

function gatherBrirPayload() {
  const args = {
    dir_path: val("bf-dir-path"),
    test_signal: val("bf-test-signal") || null,
    plot: checked("bf-plot"),
    do_room_correction: isOpen("dis-room"),
    do_headphone_compensation: isOpen("dis-headphone"),
    do_equalization: isOpen("dis-eq"),
  };
  if (isOpen("dis-room")) {
    args.room_target = val("bf-room-target") || null;
    args.room_mic_calibration = val("bf-mic-calibration") || null;
    args.specific_limit = numOr("bf-specific-limit", 400);
    args.generic_limit = numOr("bf-generic-limit", 300);
    args.fr_combination_method = val("bf-fr-combination");
  }
  if (isOpen("dis-headphone")) {
    const headphoneFile = val("bf-headphone-file");
    if (headphoneFile) args.headphone_compensation_file = headphoneFile;
  }
  if (isOpen("dis-eq")) {
    const eqFields = [["bf-eq-file", "eq_file"], ["bf-eq-left", "eq_left_file"], ["bf-eq-right", "eq_right_file"]];
    for (const [id, name] of eqFields) {
      const value = val(id);
      if (value) args[name] = value;
    }
  }
  if (isOpen("dis-advanced")) {
    args.fs = checked("bf-resample") ? Math.trunc(numOr("bf-fs", 48000)) : null;
    args.target_level = numOrNull("bf-target-level");

    const balance = val("bf-balance");
    if (balance === "number") args.channel_balance = Math.trunc(numOr("bf-balance-db", 0));
    else if (balance !== "none") args.channel_balance = balance;

    const bassGain = numOr("bf-bass-gain", 0);
    if (bassGain) {
      args.bass_boost_gain = bassGain;
      args.bass_boost_fc = numOr("bf-bass-fc", 105);
      args.bass_boost_q = numOr("bf-bass-q", 0.76);
    }
    const tilt = numOr("bf-tilt", 0);
    if (tilt) args.tilt = tilt;

    if (checked("bf-decay-per-channel")) {
      const decay = {};
      for (const channel of DECAY_CHANNELS) {
        const value = numOrNull(`bf-decay-${channel}`);
        if (value !== null && value > 0) decay[channel] = value / 1000;
      }
      if (Object.keys(decay).length) args.decay = decay;
    } else {
      const decayMs = numOrNull("bf-decay");
      if (decayMs !== null && decayMs > 0) args.decay = decayMs / 1000;
    }

    args.head_ms = numOr("bf-head-ms", 1.0);
    args.jamesdsp = checked("bf-jamesdsp");
    args.hangloose = checked("bf-hangloose");
    args.interactive_plots = checked("bf-interactive-plots");
    args.microphone_deviation_correction = checked("bf-mic-deviation");
    args.mic_deviation_strength = numOr("bf-mic-strength", 0.7);
    args.mic_deviation_debug_plots = checked("bf-mic-debug");
    args.output_truehd_layouts = checked("bf-truehd");
  }
  if (isOpen("dis-vbass")) {
    args.vbass = true;
    args.vbass_freq = Math.max(30, Math.min(500, Math.trunc(numOr("bf-vbass-freq", 250))));
    args.vbass_hp = numOr("bf-vbass-hp", 15.0);
    args.vbass_polarity = val("bf-vbass-polarity");
  }
  return args;
}

/* -------------------------------------------------------------- settings */

function populateLanguages(languages) {
  const select = $("sf-language");
  select.replaceChildren();
  languages.forEach(({ code, name }) => select.add(new Option(name, code)));
  select.value = state.language;
}

async function changeLanguage(code) {
  const response = await api().set_language(code);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  state.language = response.data.language;
  state.strings = response.data.strings;
  applyStrings();
}

async function changeTheme(code) {
  const response = await api().set_theme(code);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  applyTheme(code);
}

/* ------------------------------------------------------------------ info */

async function loadSystemInfo() {
  const response = await api().get_system_info();
  if (!response.ok) return;
  const info = response.data;
  const gilKey = info.gil_enabled === true
    ? "info_gil_enabled"
    : info.gil_enabled === false ? "info_gil_disabled" : "info_gil_unknown";
  const installKey = { velopack: "info_install_velopack", pip: "info_install_pip" }[info.install_kind]
    || "info_install_dev";
  $("info-version-pill").textContent =
    `VERSION ${info.version} · PYTHON ${info.python_version} · ${t(installKey)}`;
  const rows = [
    ["label_python", info.python_version],
    ["label_os", info.os],
    ["label_cpu_cores", info.cpu_count],
    ["label_gil_status", t(gilKey)],
    ["label_optimal_workers", info.optimal_workers],
  ];
  const grid = $("info-system");
  grid.replaceChildren();
  for (const [key, value] of rows) {
    const keyNode = document.createElement("span");
    keyNode.className = "kv-key";
    keyNode.textContent = t(key);
    const valueNode = document.createElement("span");
    valueNode.className = "kv-val mono";
    valueNode.textContent = String(value);
    grid.append(keyNode, valueNode);
  }
}

/* ---------------------------------------------------------------- wiring */

function buildDecayGrid() {
  const grid = $("bf-decay-grid");
  for (const channel of DECAY_CHANNELS) {
    const label = document.createElement("span");
    label.className = "mini-label mono";
    label.textContent = `${channel}:`;
    const input = document.createElement("input");
    input.type = "number";
    input.min = "0";
    input.id = `bf-decay-${channel}`;
    input.className = "num mono";
    input.style.width = "78px";
    grid.append(label, input);
  }
}

function wireEvents() {
  document.querySelectorAll(".nav-item").forEach((item) => {
    item.addEventListener("click", () => {
      document.querySelectorAll(".nav-item").forEach((node) => node.classList.remove("active"));
      document.querySelectorAll(".view").forEach((node) => node.classList.remove("active"));
      item.classList.add("active");
      $(`view-${item.dataset.view}`).classList.add("active");
    });
  });

  document.querySelectorAll("[data-disclosure]").forEach((head) => {
    const toggle = () => head.parentElement.classList.toggle("open");
    head.addEventListener("click", toggle);
    head.querySelector(".switch").addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        toggle();
      }
    });
  });

  document.querySelectorAll("[data-browse]").forEach((button) => {
    button.addEventListener("click", async () => {
      const kind = button.dataset.browse;
      const response = kind === "dir"
        ? await api().select_directory()
        : await api().select_file(kind);
      if (response.ok && response.data.path) {
        const target = $(button.dataset.target);
        target.value = response.data.path;
        target.dispatchEvent(new Event("input"));
      }
    });
  });

  document.querySelectorAll("[data-open-url]").forEach((button) => {
    button.addEventListener("click", () => api().open_url(button.dataset.openUrl));
  });

  $("btn-refresh-devices").addEventListener("click", () => loadDevices(val("rf-host-api")));
  $("rf-host-api").addEventListener("change", (event) => loadDevices(event.target.value));

  $("rf-play").addEventListener("input", refreshResolvedPath);
  $("rf-record-dir").addEventListener("input", refreshResolvedPath);
  $("rf-force-channels").addEventListener("change", updateChannelGuidance);
  $("rf-channels").addEventListener("input", updateChannelGuidance);

  $("btn-start-recording").addEventListener("click", startSpeakersRecording);
  $("btn-record-headphones").addEventListener("click", startHeadphonesRecording);
  $("btn-sweep-set").addEventListener("click", generateSweepSet);

  $("bf-resample").addEventListener("change", () => {
    $("bf-fs").disabled = !checked("bf-resample");
  });
  $("bf-balance").addEventListener("change", () => {
    $("bf-balance-db").disabled = val("bf-balance") !== "number";
  });
  $("bf-decay-per-channel").addEventListener("change", () => {
    const perChannel = checked("bf-decay-per-channel");
    $("bf-decay").disabled = perChannel;
    $("bf-decay-channels").hidden = !perChannel;
  });
  $("bf-mic-deviation").addEventListener("change", () => {
    const enabled = checked("bf-mic-deviation");
    $("bf-mic-strength").disabled = !enabled;
    $("bf-mic-debug").disabled = !enabled;
  });

  $("btn-generate-brir").addEventListener("click", () =>
    begin((request) => api().start_brir(request), gatherBrirPayload()),
  );
  $("btn-cancel-brir").addEventListener("click", async () => {
    if (!state.jobId) return;
    const response = await api().cancel_job(state.jobId);
    if (!response.ok) appendLog(errorText(response));
  });
  $("btn-open-output").addEventListener("click", () => {
    if (state.lastOutputDir) api().open_path(state.lastOutputDir);
  });

  $("btn-open-data").addEventListener("click", () => api().open_path());
  $("sf-theme").addEventListener("change", (event) => changeTheme(event.target.value));
  $("sf-language").addEventListener("change", (event) => changeLanguage(event.target.value));
}

/* ------------------------------------------------------------------ boot */

async function boot() {
  if (state.booted) return;
  state.booted = true;

  let response;
  try {
    response = await api().bootstrap();
  } catch (error) {
    $("runtime-status").textContent = t("webview_bridge_failed");
    appendLog(String(error));
    return;
  }
  if (!response.ok) {
    $("runtime-status").textContent = t("webview_bridge_failed");
    appendLog(errorText(response));
    return;
  }
  const data = response.data;
  state.version = data.version;
  state.platform = data.platform;
  if (data.ui) {
    state.strings = data.ui.strings || {};
    state.language = data.ui.language || "en";
    populateLanguages(data.ui.languages || []);
    $("sf-theme").value = data.ui.theme || "dark";
    applyTheme(data.ui.theme || "dark");
  }
  applyStrings();
  $("brand-version").textContent = `v${data.version}`;
  $("runtime-status").textContent = `v${data.version} · ${data.platform} · WebView2`;
  appendLog(t("webview_bridge_connected"));

  renderJobState(data.active_job);
  if (data.active_job && !["succeeded", "failed", "cancelled"].includes(data.active_job.status)) {
    state.jobId = data.active_job.job_id;
    state.jobKind = data.active_job.kind;
    state.nextSeq = 0;
    schedulePoll(0);
  }

  await Promise.all([loadDevices(), loadSystemInfo()]);
  refreshResolvedPath();
}

buildDecayGrid();
wireEvents();
updateChannelGuidance();

window.addEventListener("pywebviewready", boot);
if (window.pywebview && window.pywebview.api) boot();
