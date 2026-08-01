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
  brirDefaults: {},
  language: "en",
  theme: "dark",
  skin: "studio",
  jobId: null,
  jobKind: null,
  lastJob: null,
  nextSeq: 0,
  pollTimer: null,
  resolvedRecordPath: "",
  lastOutputDir: null,
  systemThemeQuery: null,
  stageIndex: -1,
  stageAbort: null,
  modalDismissed: false,
  recPhase: null,
  recDoneSpeakers: new Set(),
};

const DECAY_CHANNELS = ["FL", "FC", "FR", "SL", "SR", "BL", "BR"];

/* BRIR pipeline stages for the Studio activity checklist. The logger
   emits these exact localized strings (optionally with a suffix), so
   prefix-matching against the same locale table works in any language. */
const BRIR_STAGES = [
  "cli_opening_measurements",
  "cli_cropping_responses",
  "cli_running_room_correction",
  "cli_running_headphone_compensation",
  "cli_equalizing",
  "cli_correcting_deviation",
  "cli_adjusting_decay",
  "cli_correcting_balance",
  "cli_normalizing_gain",
  "cli_writing_brirs",
];

const $ = (id) => document.getElementById(id);
const api = () => window.pywebview.api;

/* ------------------------------------------------------------------ i18n */

function t(key) {
  return state.strings[key] || key;
}

/* Strings needed BEFORE bootstrap delivers the locale table. If bootstrap
   itself dies (e.g. a packaging regression on the Python side), t() would
   render raw keys — 2.10.0 shipped exactly that. Language comes from the
   OS/browser locale since the persisted choice is unreachable then. */
const PREBOOT_STRINGS = {
  en: {
    webview_bridge_failed: "Python bridge unavailable.",
    webview_bridge_connecting: "Connecting to Python…",
  },
  ko: {
    webview_bridge_failed: "Python 브리지를 사용할 수 없습니다.",
    webview_bridge_connecting: "Python 브리지에 연결하는 중…",
  },
};

function tPreboot(key) {
  if (state.strings[key]) return state.strings[key];
  const lang = String(navigator.language || "en").toLowerCase().startsWith("ko") ? "ko" : "en";
  return PREBOOT_STRINGS[lang][key] || PREBOOT_STRINGS.en[key] || key;
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
  renderSteps();
  renderJobState(state.lastJob);
  applySkin(state.skin);
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

function applySkin(code) {
  state.skin = code === "stable" ? "stable" : "studio";
  document.documentElement.dataset.skin = state.skin;
  const desc = $("sf-skin-desc");
  if (desc) desc.textContent = t(state.skin === "stable" ? "tooltip_skin_stable" : "tooltip_skin_studio");
  // Re-evaluate the job dialog: switching skins mid-job moves the running
  // display between the inline activity card and the Stable modal.
  renderJobState(state.lastJob);
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

/* Canonical pipeline default shipped by bootstrap(); the literal fallback
   only applies when bootstrap itself failed to load ProcessingConfig. */
function brirDefault(name, fallback) {
  const value = state.brirDefaults[name];
  return typeof value === "number" ? value : fallback;
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
  updateJobModal(job, busy);
}

/* Stable skin shows running jobs in a separate dialog, mirroring the CTk
   RecordingProgressDialog / ProcessingDialog convention. */
function updateJobModal(job, busy) {
  const modal = $("job-modal");
  if (state.skin !== "stable" || !job || state.modalDismissed) {
    modal.hidden = true;
    return;
  }
  $("job-modal-title").textContent = t(
    job.kind === "recording" ? "dialog_recording_title" : "dialog_processing_title",
  );
  const cancel = $("job-modal-cancel");
  cancel.hidden = !busy;
  cancel.disabled = !job.cancellable;
  $("job-modal-close").hidden = busy;
  modal.hidden = false;
}

/* --------------------------------------------------- pipeline checklist */

function resetSteps(visible) {
  state.stageIndex = -1;
  state.stageAbort = null;
  $("brir-steps").hidden = !visible;
  renderSteps();
}

function renderSteps() {
  const list = $("brir-steps");
  list.classList.toggle("aborted", Boolean(state.stageAbort));
  list.replaceChildren();
  BRIR_STAGES.forEach((key, index) => {
    const item = document.createElement("li");
    let className = "";
    let glyphText = "";
    if (index < state.stageIndex) {
      className = "done";
      glyphText = "✓";
    } else if (index === state.stageIndex) {
      if (state.stageAbort === "failed") {
        className = "error";
        glyphText = "✕";
      } else if (state.stageAbort === "cancelled") {
        className = "cancelled";
        glyphText = "–";
      } else {
        className = "current";
        glyphText = "▸";
      }
    }
    item.className = className;
    const glyph = document.createElement("span");
    glyph.className = "step-glyph";
    glyph.textContent = glyphText;
    const label = document.createElement("span");
    label.textContent = t(key);
    item.append(glyph, label);
    list.append(item);
  });
}

function updateSteps(message) {
  if (!message || state.jobKind !== "brir") return;
  for (let index = BRIR_STAGES.length - 1; index >= 0; index -= 1) {
    if (message.startsWith(t(BRIR_STAGES[index]))) {
      if (index >= state.stageIndex) {
        state.stageIndex = index;
        renderSteps();
      }
      return;
    }
  }
}

function completeSteps() {
  state.stageIndex = BRIR_STAGES.length;
  state.stageAbort = null;
  renderSteps();
}

/* Failure/cancel semantics: the stage that was in flight gets ✕ (err) or
   – (warn); finished stages keep their checkmarks; unreached stages stay
   as dimmed circles so it reads "never got there", not "skipped okay". */
function abortSteps(kind) {
  if (state.stageIndex < 0) state.stageIndex = 0;
  if (state.stageIndex >= BRIR_STAGES.length) state.stageIndex = BRIR_STAGES.length - 1;
  state.stageAbort = kind;
  renderSteps();
}

/* ---------------------------------------------------- recorder status
   Port of the CTk RecordingStatusController presentation: speaker chips
   (Studio segment-chip visual), a bold phase status line, and an
   elapsed/duration detail line, driven by RecorderProgressEvent payloads. */

function fmtDuration(seconds) {
  if (!Number.isFinite(seconds) || seconds < 0) return "--:--";
  const total = Math.round(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  const mm = hours ? String(minutes).padStart(2, "0") : String(minutes);
  return `${hours ? `${hours}:` : ""}${mm}:${String(secs).padStart(2, "0")}`;
}

function resetRecorderStatus() {
  state.recPhase = null;
  state.recDoneSpeakers = new Set();
  document.querySelectorAll("[data-rec-chips]").forEach((node) => {
    node.hidden = true;
    node.replaceChildren();
  });
  setRecorderStatus("", "");
}

function setRecorderStatus(statusText, detailText) {
  document.querySelectorAll("[data-rec-status]").forEach((node) => {
    node.hidden = !statusText;
    node.textContent = statusText;
  });
  document.querySelectorAll("[data-rec-detail]").forEach((node) => {
    node.hidden = !detailText;
    node.textContent = detailText || "";
  });
}

function renderRecorderChips(speakers, activeSpeaker) {
  if (!Array.isArray(speakers) || !speakers.length) return;
  if (activeSpeaker) {
    for (const speaker of speakers) {
      if (speaker === activeSpeaker) break;
      state.recDoneSpeakers.add(speaker);
    }
  }
  document.querySelectorAll("[data-rec-chips]").forEach((node) => {
    node.hidden = false;
    node.replaceChildren(...speakers.map((speaker) => {
      const chip = document.createElement("span");
      chip.className = "chip mono"
        + (speaker === activeSpeaker ? " active" : state.recDoneSpeakers.has(speaker) ? " done" : "");
      chip.textContent = speaker;
      return chip;
    }));
  });
}

function finishRecorderChips() {
  document.querySelectorAll("[data-rec-chips] .chip").forEach((chip) => {
    chip.classList.remove("active");
    chip.classList.add("done");
  });
}

function updateRecorderStatus(payload) {
  renderRecorderChips(payload.speakers, payload.phase === "recording" ? payload.speaker : null);
  const phase = payload.phase;
  let status = "";
  let detail = "";
  if (phase === "recording" && payload.speaker) {
    status = fmt(t("recording_status_recording_speaker"), {
      speaker: payload.speaker,
      index: payload.segment_index || 0,
      total: payload.segment_total || 0,
    });
    detail = fmt(t("recording_status_recording"), {
      elapsed: fmtDuration(payload.elapsed),
      duration: fmtDuration(payload.duration),
    });
  } else if (phase === "recording") {
    status = t("recording_status_recording_gap");
    detail = fmt(t("recording_status_recording"), {
      elapsed: fmtDuration(payload.elapsed),
      duration: fmtDuration(payload.duration),
    });
  } else if (phase === "devices") {
    status = t("recording_status_devices_ready");
    detail = payload.message || "";
  } else if (phase === "saving") {
    status = t("recording_status_saving");
  } else if (phase === "complete") {
    status = t("recording_status_complete");
  } else if (phase === "error") {
    status = t("recording_status_error");
    detail = payload.message || "";
  } else {
    status = t("recording_status_preparing");
    detail = payload.message || "";
  }
  setRecorderStatus(status, detail);
  if (phase !== state.recPhase) {
    state.recPhase = phase;
    appendLog(status);
  }
}

function finishRecorderStatus(job) {
  if (job.status === "succeeded" && job.result) {
    finishRecorderChips();
    const file = String(job.result.record_path || "").split(/[\\/]/).pop();
    const summary = job.result.summary;
    const detail = summary
      ? fmt(t("recording_status_summary"), {
          file,
          channels: summary.channels,
          duration: fmtDuration(summary.duration),
          peak_db: Number(summary.peak_db).toFixed(1),
          active: summary.active_channels,
          total: summary.channels,
        })
      : fmt(t("recording_status_summary_unavailable"), { file });
    setRecorderStatus(t("recording_status_complete"), detail);
    appendLog(detail);
  } else if (job.status === "failed") {
    setRecorderStatus(t("recording_status_error"), job.error ? job.error.message : "");
  }
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
    // Stable hides the inline activity cards (jobs run in the modal), so a
    // pre-start validation error would otherwise be invisible there.
    if (state.skin === "stable") window.alert(errorText(response));
    return;
  }
  const job = response.data.job;
  state.jobId = job.job_id;
  state.jobKind = job.kind;
  state.nextSeq = 0;
  state.lastOutputDir = job.kind === "brir" ? val("bf-dir-path") : null;
  state.modalDismissed = false;
  resetSteps(job.kind === "brir");
  resetRecorderStatus();
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

async function cancelActiveJob() {
  if (!state.jobId) return;
  const response = await api().cancel_job(state.jobId);
  if (!response.ok) appendLog(errorText(response));
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
      if (payload.phase) {
        updateRecorderStatus(payload);
      } else if (payload.message) {
        appendLog(payload.message);
        updateSteps(payload.message);
      }
    }
    if (event.type === "log") {
      appendLog(`[${payload.level}] ${payload.message}`);
      updateSteps(payload.message);
    }
    if (event.type === "status") appendLog(`· ${t(`webview_status_${payload.status}`)}`);
  }
  renderJobState(job);
  if (["succeeded", "failed", "cancelled"].includes(job.status)) {
    if (job.status === "succeeded") setProgress(1);
    if (job.error) appendLog(`${job.error.code}: ${job.error.message}`);
    if (job.kind === "brir") {
      if (job.status === "succeeded") {
        completeSteps();
        if (state.lastOutputDir) $("btn-open-output").hidden = false;
      } else {
        abortSteps(job.status);
      }
    }
    if (job.kind === "recording") finishRecorderStatus(job);
    state.jobId = null;
    return;
  }
  schedulePoll();
}

/* ----------------------------------------------------------- auto-update */

/* Port of the CTk flow: background check 2s after startup →
   UpdateDialog (notes + Update Now / Remind / Skip) → UpdateExecutor with
   progress → completion message → optional apply-and-restart (Velopack). */

const updateState = {
  info: null,
  jobId: null,
  nextSeq: 0,
  pollTimer: null,
};

function tOr(key, fallback) {
  return (key && state.strings[key]) || fallback || key || "";
}

function setUpdateProgress(value) {
  $("update-progress").style.width = `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
}

function showUpdateModal(info) {
  updateState.info = info;
  $("update-version-line").textContent = fmt(t("update_version_info"), {
    current: info.current_version,
    latest: info.latest_version,
  });
  $("update-notes").textContent = info.release_notes || t("update_no_notes");
  $("update-progress-row").hidden = true;
  setUpdateProgress(0);
  $("update-status").textContent = "";
  $("update-result").hidden = true;
  $("update-now").hidden = false;
  $("update-now").disabled = false;
  $("update-remind").hidden = false;
  $("update-skip").hidden = false;
  $("update-restart").hidden = true;
  $("update-close").hidden = true;
  $("update-modal").hidden = false;
}

function showUpdateProgressOnly() {
  // Resume path: an update job survived a frontend reload — no check info,
  // just live progress until the terminal state arrives.
  updateState.info = null;
  $("update-version-line").textContent = "";
  $("update-notes").textContent = "";
  $("update-progress-row").hidden = false;
  $("update-result").hidden = true;
  $("update-now").hidden = true;
  $("update-remind").hidden = true;
  $("update-skip").hidden = true;
  $("update-restart").hidden = true;
  $("update-close").hidden = true;
  $("update-modal").hidden = false;
}

function hideUpdateModal() {
  window.clearTimeout(updateState.pollTimer);
  $("update-modal").hidden = true;
}

async function checkForUpdates(manual) {
  const statusLine = $("update-check-status");
  if (manual) {
    statusLine.hidden = false;
    statusLine.textContent = t("update_checking");
  }
  let response;
  try {
    response = await api().check_for_updates();
  } catch (error) {
    response = null;
  }
  if (!response || !response.ok) {
    // Startup checks fail silently, mirroring the CTk background check.
    if (manual) statusLine.textContent = response ? errorText(response) : t("update_error_apply");
    return;
  }
  const data = response.data;
  if (data.update_available) {
    if (manual) statusLine.hidden = true;
    showUpdateModal(data);
  } else if (manual) {
    statusLine.textContent = fmt(t("update_up_to_date"), {
      latest: data.latest_version || data.current_version,
    });
  }
}

async function beginUpdate() {
  const info = updateState.info;
  if (!info) return;
  $("update-now").disabled = true;
  $("update-remind").hidden = true;
  $("update-skip").hidden = true;
  $("update-progress-row").hidden = false;
  $("update-status").textContent = t("update_downloading");
  const response = await api().start_update({
    download_url: info.download_url,
    latest_version: info.latest_version,
  });
  if (!response.ok) {
    finishUpdate(errorText(response), false);
    return;
  }
  updateState.jobId = response.data.job.job_id;
  updateState.nextSeq = 0;
  pollUpdateJob();
}

async function pollUpdateJob() {
  if (!updateState.jobId) return;
  const response = await api().poll_job(updateState.jobId, updateState.nextSeq);
  if (!response.ok) {
    finishUpdate(errorText(response), false);
    return;
  }
  const { job, events, next_seq: nextSeq } = response.data;
  updateState.nextSeq = nextSeq;
  for (const event of events) {
    const payload = event.payload || {};
    if (event.type === "progress") {
      if (typeof payload.progress === "number") setUpdateProgress(payload.progress);
      // The executor sends either an i18n key ("update_downloading") or
      // preformatted text ("Downloading: 42%").
      if (payload.message) $("update-status").textContent = tOr(payload.message, payload.message);
    }
  }
  if (["succeeded", "failed", "cancelled"].includes(job.status)) {
    updateState.jobId = null;
    if (job.status === "succeeded") {
      const result = job.result || {};
      setUpdateProgress(typeof result.progress === "number" ? result.progress : 1);
      $("update-status").textContent = tOr(result.status_key, result.status_default);
      finishUpdate(
        tOr(result.message_key, result.message_default),
        true,
        Boolean(result.requires_restart),
      );
    } else {
      finishUpdate(job.error ? job.error.message : t("update_error_apply"), false);
    }
    return;
  }
  updateState.pollTimer = window.setTimeout(pollUpdateJob, 250);
}

function finishUpdate(message, success, requiresRestart = false) {
  $("update-result").hidden = false;
  $("update-result").textContent = message;
  $("update-now").hidden = true;
  $("update-remind").hidden = true;
  $("update-skip").hidden = true;
  // Velopack stages an apply-and-restart: confirming OK hands over to
  // Update.exe and the window closes. pip/legacy end with a plain Close.
  $("update-restart").hidden = !(success && requiresRestart);
  $("update-close").hidden = success && requiresRestart;
}

async function applyStagedUpdate() {
  $("update-restart").disabled = true;
  const response = await api().apply_pending_update();
  if (!response.ok) {
    $("update-restart").disabled = false;
    finishUpdate(errorText(response), false);
    return;
  }
  $("update-result").hidden = false;
  $("update-result").textContent = t("update_restart_message");
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
    payload.force_channels = checked("rf-force-channels");
    payload.channels = payload.force_channels ? Math.trunc(numOr("rf-channels", 2)) : 2;
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
    args.specific_limit = numOr("bf-specific-limit", brirDefault("specific_limit", 400));
    args.generic_limit = numOr("bf-generic-limit", brirDefault("generic_limit", 300));
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
      args.bass_boost_fc = numOr("bf-bass-fc", brirDefault("bass_boost_fc", 105));
      args.bass_boost_q = numOr("bf-bass-q", brirDefault("bass_boost_q", 0.76));
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

    args.head_ms = numOr("bf-head-ms", brirDefault("head_ms", 1.0));
    args.jamesdsp = checked("bf-jamesdsp");
    args.hangloose = checked("bf-hangloose");
    args.interactive_plots = checked("bf-interactive-plots");
    args.microphone_deviation_correction = checked("bf-mic-deviation");
    args.mic_deviation_strength = numOr("bf-mic-strength", brirDefault("mic_deviation_strength", 0.7));
    args.mic_deviation_debug_plots = checked("bf-mic-debug");
    args.output_truehd_layouts = checked("bf-truehd");
  }
  if (isOpen("dis-vbass")) {
    args.vbass = true;
    args.vbass_freq = Math.max(30, Math.min(500, Math.trunc(numOr("bf-vbass-freq", brirDefault("vbass_freq", 250)))));
    args.vbass_hp = numOr("bf-vbass-hp", brirDefault("vbass_hp", 15.0));
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

async function changeSkin(code) {
  const response = await api().set_skin(code);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  applySkin(response.data.skin);
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
  $("btn-cancel-brir").addEventListener("click", cancelActiveJob);
  $("job-modal-cancel").addEventListener("click", cancelActiveJob);
  $("job-modal-close").addEventListener("click", () => {
    state.modalDismissed = true;
    $("job-modal").hidden = true;
  });
  $("btn-open-output").addEventListener("click", () => {
    if (state.lastOutputDir) api().open_path(state.lastOutputDir);
  });

  $("btn-open-data").addEventListener("click", () => api().open_path());
  $("sf-skin").addEventListener("change", (event) => changeSkin(event.target.value));
  $("sf-theme").addEventListener("change", (event) => changeTheme(event.target.value));
  $("sf-language").addEventListener("change", (event) => changeLanguage(event.target.value));
  $("sf-frontend").addEventListener("change", async (event) => {
    // Persisted for the next launch; the current session keeps running.
    const response = await api().set_frontend(event.target.value);
    if (!response.ok) appendLog(errorText(response));
  });

  $("btn-check-updates").addEventListener("click", () => checkForUpdates(true));
  $("update-now").addEventListener("click", beginUpdate);
  $("update-remind").addEventListener("click", hideUpdateModal);
  $("update-skip").addEventListener("click", hideUpdateModal);
  $("update-close").addEventListener("click", hideUpdateModal);
  $("update-restart").addEventListener("click", applyStagedUpdate);
}

/* ------------------------------------------------------------------ boot */

async function boot() {
  if (state.booted) return;
  state.booted = true;

  let response;
  try {
    response = await api().bootstrap();
  } catch (error) {
    $("runtime-status").textContent = tPreboot("webview_bridge_failed");
    appendLog(String(error));
    return;
  }
  if (!response.ok) {
    $("runtime-status").textContent = tPreboot("webview_bridge_failed");
    appendLog(errorText(response));
    return;
  }
  const data = response.data;
  state.version = data.version;
  state.platform = data.platform;
  state.brirDefaults = data.brir_defaults || {};
  if (data.ui) {
    state.strings = data.ui.strings || {};
    state.language = data.ui.language || "en";
    populateLanguages(data.ui.languages || []);
    $("sf-theme").value = data.ui.theme || "dark";
    applyTheme(data.ui.theme || "dark");
    $("sf-skin").value = data.ui.skin === "stable" ? "stable" : "studio";
    applySkin(data.ui.skin);
    $("sf-frontend").value = data.ui.frontend === "ctk" ? "ctk" : "webview";
  }
  applyStrings();
  const backendLabels = { edgechromium: "WebView2", cocoa: "WKWebView", gtk: "WebKitGTK" };
  const backendLabel = backendLabels[data.webview_backend] || "WebView";
  $("brand-version").textContent = `v${data.version}`;
  $("runtime-status").textContent = `v${data.version} · ${data.platform} · ${backendLabel}`;
  appendLog(t("webview_bridge_connected"));

  const activeJob = data.active_job;
  if (activeJob && activeJob.kind === "update") {
    if (!["succeeded", "failed", "cancelled"].includes(activeJob.status)) {
      updateState.jobId = activeJob.job_id;
      updateState.nextSeq = 0;
      showUpdateProgressOnly();
      pollUpdateJob();
    }
  } else {
    renderJobState(activeJob);
    if (activeJob && !["succeeded", "failed", "cancelled"].includes(activeJob.status)) {
      state.jobId = activeJob.job_id;
      state.jobKind = activeJob.kind;
      state.nextSeq = 0;
      resetSteps(activeJob.kind === "brir");
      resetRecorderStatus();
      schedulePoll(0);
    }
  }

  await Promise.all([loadDevices(), loadSystemInfo()]);
  refreshResolvedPath();

  // First run: ask for the language before anything else (CTk parity).
  if (data.ui && data.ui.first_run) {
    showFirstRunLanguageModal(data.ui.languages || []);
  }

  // Mirror the CTk root.after(2000) startup update check; failures stay
  // silent and never block the UI.
  window.setTimeout(() => checkForUpdates(false), 2000);
}

function showFirstRunLanguageModal(languages) {
  const list = $("language-modal-list");
  list.replaceChildren();
  languages.forEach(({ code, name }) => {
    const button = document.createElement("button");
    button.className = "btn btn-secondary";
    button.type = "button";
    button.textContent = name;
    button.addEventListener("click", async () => {
      // set_language persists the choice and marks language_selected.
      await changeLanguage(code);
      $("sf-language").value = code;
      $("language-modal").hidden = true;
    });
    list.appendChild(button);
  });
  if (list.childElementCount > 0) $("language-modal").hidden = false;
}

buildDecayGrid();
wireEvents();
updateChannelGuidance();

window.addEventListener("pywebviewready", boot);
if (window.pywebview && window.pywebview.api) boot();
