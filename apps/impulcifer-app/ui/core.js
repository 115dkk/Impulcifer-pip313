// @ts-check
// Shared state and constants, DOM and i18n helpers, theme and skin, error sentences, job display.
"use strict";

/* Impulcifer WebView frontend — Pulse Studio shell.
   Talks to the Rust application service through the pywebview-compatible IPC.
   BRIR payload assembly mirrors gui/brir_args.build_brir_args: gated
   option groups are omitted entirely while their disclosure is closed so
   ProcessingConfig defaults stay authoritative. */

/** @type {AppState} */
const state = {
  booted: false,
  version: "",
  platform: "",
  strings: {},
  brirDefaults: {},
  language: "en",
  skin: "studio",
  jobId: null,
  jobKind: null,
  lastJob: null,
  lastRecoveryJob: null,
  startPending: false,
  nextSeq: 0,
  pollTimer: undefined,
  guided: { index: 0, results: {}, pending: null, timer: undefined, remaining: 0 },
  takeMeasurements: {},
  lastOutputDir: null,
  lastRecoveryOutputDir: null,
  systemThemeQuery: null,
  stageIndex: -1,
  stageAbort: null,
  modalDismissed: false,
  recPhase: null,
  recDoneSpeakers: new Set(),
  shareModes: ["auto"], systemInfo: null,
  audioDevices: [], defaultOutputIndex: -1,
  recoveryPlan: null, recoveryPlanError: null, recoveryState: "empty",
  recoveryTimer: undefined, recoveryRevision: 0, recoveryCreated: new Set(), recoveryRunRevision: -1,
  recoveryRequestKey: "",
  eqChoices: { both: { mode: "folder" }, left: { mode: "folder" }, right: { mode: "folder" } },
  eqInspection: null, eqError: null, eqRevision: 0, eqTimer: undefined,
  testSignalRevision: 0, testSignalTimer: undefined,
  mfiles: {
    room: { data: null, error: null, choices: null, input: "", revision: 0, timer: undefined },
    brir: { data: null, error: null, choices: null, input: "", revision: 0, timer: undefined },
    headphones: { data: null, error: null, choices: null, input: "", revision: 0, timer: undefined },
  },
};

/* Full speaker layout — must mirror core/constants.py SPEAKER_NAMES
   (pinned by tests/test_audit_contracts.py; audit #138 F018). */
const DECAY_CHANNELS = ["FL", "FR", "FC", "BL", "BR", "SL", "SR", "WL", "WR",
  "TFL", "TFR", "TSL", "TSR", "TBL", "TBR"];

/* BRIR pipeline stages for the Studio activity checklist. Events carry the
   logger's original stage key (payload.key); the rendered-text prefix match
   remains only as a fallback for key-less events. */
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

/** @param {string} id @returns {HTMLElement} */
function $(id) {
  const node = document.getElementById(id);
  if (!node) throw new Error(`Missing element #${id}`);
  return node;
}
/** @template {HTMLElement} T @param {string} id @param {{new(): T}} type @returns {T} */
function el(id, type) {
  const node = $(id);
  if (!(node instanceof type)) throw new Error(`Unexpected element #${id}`);
  return node;
}
function api() {
  if (!window.pywebview) throw new Error("Service unavailable");
  return window.pywebview.api;
}

/* ------------------------------------------------------------------ i18n */

/** @param {string} key */
function t(key) {
  return state.strings[key] || key;
}

/* Strings needed BEFORE bootstrap delivers the locale table. If bootstrap
   itself dies (e.g. a packaging regression on the Python side), t() would
   render raw keys — 2.10.0 shipped exactly that. Language comes from the
   OS/browser locale since the persisted choice is unreachable then. */
/** @type {Record<string, Record<string, string>>} */
const PREBOOT_STRINGS = {
  en: {
    webview_bridge_failed: "Service unavailable.",
    webview_bridge_connecting: "Connecting to service…",
    webview_bridge_connected: "Service connected.",
  },
  ko: {
    webview_bridge_failed: "서비스를 사용할 수 없습니다.",
    webview_bridge_connecting: "서비스에 연결하는 중…",
    webview_bridge_connected: "서비스에 연결했습니다.",
  },
};

/** @param {string} key */
function tPreboot(key) {
  if (state.strings[key]) return state.strings[key];
  const lang = String(navigator.language || "en").toLowerCase().startsWith("ko") ? "ko" : "en";
  return PREBOOT_STRINGS[lang][key] || PREBOOT_STRINGS.en[key] || key;
}

/** @param {string} text @param {Record<string, unknown>} vars */
function fmt(text, vars) {
  return text.replace(/\{(\w+)\}/g, (match, name) =>
    Object.prototype.hasOwnProperty.call(vars, name) ? String(vars[name]) : match,
  );
}

function applyStrings() {
  document.documentElement.lang = state.language;
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-i18n]")).forEach((node) => {
    node.textContent = t(node.dataset.i18n || "");
  });
  // Every info button has the same name; its description is the bubble it opens.
  document.querySelectorAll(".info-tip").forEach((button) => button.setAttribute("aria-label", t("button_more_info")));
  updateChannelGuidance();
  updateRoomRows();
  updateVbassRows();
  refreshResolvedPath();
  renderGuided();
  renderSteps();
  renderJobState(state.lastJob);
  renderRecoveryJob(state.lastRecoveryJob);
  applySkin(state.skin);
  populateShareModes();
  renderSystemInfo();
  renderRecoveryInventory();
  renderEq();
  for (const purpose of MFILE_PURPOSES) renderMfile(purpose);
  for (const p of ADV_PREFIXES) refreshAdvanced(p);
  if (state.lastJob?.kind === "brir" && state.lastJob.status === "succeeded") renderAlignment(state.lastJob.result?.alignment ?? null);
  if (state.version) $("runtime-status").textContent = `v${state.version} · ${state.platform} · ${t("webview_bridge_connected")}`;
}

/* ----------------------------------------------------------------- theme */

/** @param {string} code */
function applyTheme(code) {
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

/** @param {MediaQueryListEvent} event */
function onSystemThemeChange(event) {
  document.documentElement.dataset.theme = event.matches ? "dark" : "light";
}

/** @param {string} code */
function applySkin(code) {
  const previous = state.skin;
  state.skin = code === "stable" ? "stable" : "studio";
  document.documentElement.dataset.skin = state.skin;
  const desc = $("sf-skin-desc");
  if (desc) desc.textContent = t(state.skin === "stable" ? "tooltip_skin_stable" : "tooltip_skin_studio");
  // Re-evaluate the job dialog: switching skins mid-job moves the running
  // display between the inline activity card and the Stable modal.
  renderJobState(state.lastJob);
  if (previous !== state.skin) {
    scheduleRecoveryPlan();
    syncAdvancedForms(state.skin);
    syncRoomPhase(state.skin);
  }
}

/* ------------------------------------------------------------ primitives */

/** @param {string} id */
function val(id) {
  const node = $(id);
  if (!(node instanceof HTMLInputElement || node instanceof HTMLSelectElement)) throw new Error(`Not a field: ${id}`);
  return node.value.trim();
}

/** @param {string} id */
function checked(id) {
  return el(id, HTMLInputElement).checked;
}

/** @param {string} id */
function numOrNull(id) {
  const raw = val(id);
  if (!raw) return null;
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? parsed : null;
}

/** @param {string} id @param {number} fallback */
function numOr(id, fallback) {
  const parsed = numOrNull(id);
  return parsed === null ? fallback : parsed;
}

/* Canonical pipeline default shipped by bootstrap(); the literal fallback
   only applies when bootstrap itself failed to load ProcessingConfig. */
/** @overload @param {keyof ProcessingRequest} name @param {number} fallback @returns {number} */
/** @overload @param {keyof ProcessingRequest} name @param {string} fallback @returns {string} */
/** @overload @param {keyof ProcessingRequest} name @param {null} fallback @returns {number | null} */
/** @param {keyof ProcessingRequest} name @param {number | string | null} fallback */
function brirDefault(name, fallback) {
  const value = state.brirDefaults[name];
  if (typeof fallback === "string") return typeof value === "string" ? value : fallback;
  return typeof value === "number" ? value : fallback;
}

/** @param {string} id */
function isOpen(id) {
  return $(id).classList.contains("open");
}

/* The service keeps the 2.x error envelope (a code, an English message and
   machine details); people read a sentence in the page language instead.
   A message the service sends with fixed text maps to its catalogue entry
   (ui_catalog.rs checks that the service still sends each text below), a
   missing file names its path, and anything else shows the service's own
   message. The code and the raw details are never shown. */
/** @param {IpcError} error */
function errorSentence(error) {
  const shareError = shareModeError(error);
  if (shareError) return shareError;
  const details = error.details || {};
  const path = String(details.path ?? details.directory ?? "");
  switch (error.message) {
    case "Measurement directory does not exist.":
      return fmt(t("error_measurement_dir_missing"), { path });
    case "No HRIR recordings found in the directory.":
      return t("error_no_hrir_recordings");
    case "Folder does not exist.":
    case "Recovery directory does not exist.":
    case "The selected recovery directory does not exist.":
      return fmt(t("error_folder_missing"), { path });
    case "Custom EQ file does not exist.":
      return fmt(t("error_eq_file_missing"), { path });
    case "Playback file does not exist.":
      return fmt(t("message_play_file_not_exist"), { file: path });
    case "Another job is already running.":
      return t("error_job_busy");
    case "The recording does not hold the listed number of sweeps.":
      if (details.found === 0) return t("error_brir_recording_none");
      return fmt(t("error_brir_recording_count"), {
        found: details.found,
        listed: details.listed,
        starts: Array.isArray(details.starts_seconds)
          ? details.starts_seconds.map((seconds) => Number(seconds).toFixed(2)).join(", ") : "",
      });
    case "The test signal does not match the recording file.":
      return fmt(t("error_brir_recording_test_signal"), { read: details.read, listed: details.listed });
    case "The sweep file is not a logarithmic sweep.":
      return t("error_brir_recording_sweep_file");
    case "The sweep file and the recording have different sampling rates.":
      return fmt(t("error_brir_recording_sweep_rate"), { sweep: details.sweep_fs, recording: details.recording_fs });
    case "The test signal does not match the headphone recording.": {
      /** @param {unknown} value */
      const seconds = (value) => Number(value).toFixed(2);
      return fmt(t("error_headphone_test_signal"), {
        recording: seconds(details.recording_seconds),
        sweep: seconds(details.sweep_seconds),
        needed: seconds(details.needed_seconds),
      });
    }
  }
  if (error.code === "FILE_NOT_FOUND") return `${t("error_file_not_found")}: ${path || error.message}`;
  return error.message;
}

/** @param {Envelope<unknown>} response */
function errorText(response) {
  if (!response || response.ok) return "Unknown error";
  return errorSentence(response.error);
}

/** @param {string} message */
function appendLog(message) {
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-log]")).forEach((log) => {
    const lines = log.textContent ? log.textContent.split("\n") : [];
    lines.push(message);
    log.textContent = lines.slice(-500).join("\n");
    log.scrollTop = log.scrollHeight;
  });
}

/** @param {number} value */
function setProgress(value) {
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-progress]")).forEach((bar) => {
    bar.style.width = `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
  });
}

/** @param {Job | null} job */
function renderJobState(job) {
  state.lastJob = job;
  updateStartControls(job);
  const label = job
    ? `${jobKindLabel(job.kind)} · ${t(`webview_status_${job.status}`)}`
    : t("webview_job_idle");
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-job-state]")).forEach((node) => {
    node.textContent = label;
  });
  if (job?.kind === "output_recovery") renderRecoveryJob(job);
  const active = Boolean(job && !["succeeded", "failed", "cancelled"].includes(job.status));
  updateJobModal(job, active);
}

/** @param {Job | null} job */
function updateStartControls(job) {
  const active = Boolean(job && !["succeeded", "failed", "cancelled"].includes(job.status));
  const busy = state.startPending || active;
  /** @type {NodeListOf<HTMLButtonElement>} */ (document.querySelectorAll("[data-start]")).forEach((button) => {
    button.disabled = busy;
  });
  const recordingProblem = brirRecordingProblem();
  const generate = el("btn-generate-brir", HTMLButtonElement);
  generate.disabled = busy || recordingProblem !== null;
  generate.title = recordingProblem || "";
  el("btn-cancel-brir", HTMLButtonElement).disabled = !active || !job?.cancellable;
  // Studio cancels a recording from the page header; Stable from its job dialog.
  el("btn-cancel-recording", HTMLButtonElement).hidden =
    !(active && state.skin === "studio" && job?.kind === "recording" && job.cancellable);
  el("btn-start-recovery", HTMLButtonElement).disabled = busy || (state.skin === "studio" && state.recoveryState !== "ready");
}

/** @param {JobKind} kind */
function jobKindLabel(kind) {
  const key = {
    update: "update_available_title",
    recording: "sidebar_recorder",
    brir: "sidebar_processing",
    output_recovery: "sidebar_output_recovery",
  }[kind];
  return key ? t(key) : kind;
}

/* Stable skin shows running jobs in a separate dialog, mirroring the CTk
   RecordingProgressDialog / ProcessingDialog convention. */
/** @param {Job | null} job @param {boolean} busy */
function updateJobModal(job, busy) {
  const modal = $("job-modal");
  if (state.skin !== "stable" || !job || state.modalDismissed) {
    modal.hidden = true;
    return;
  }
  const titleKey = job.kind === "recording"
    ? "dialog_recording_title"
    : job.kind === "output_recovery" ? "dialog_recovery_title" : "dialog_processing_title";
  $("job-modal-title").textContent = t(titleKey);
  const cancel = el("job-modal-cancel", HTMLButtonElement);
  cancel.hidden = !busy || !job.cancellable;
  cancel.disabled = !job.cancellable;
  el("job-modal-close", HTMLButtonElement).hidden = busy;
  modal.hidden = false;
}
