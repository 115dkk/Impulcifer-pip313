// @ts-check
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
  theme: "dark",
  skin: "studio",
  jobId: null,
  jobKind: null,
  lastJob: null,
  lastRecoveryJob: null,
  startPending: false,
  nextSeq: 0,
  pollTimer: undefined,
  resolvedRecordPath: "",
  lastOutputDir: null,
  lastRecoveryOutputDir: null,
  systemThemeQuery: null,
  stageIndex: -1,
  stageAbort: null,
  modalDismissed: false,
  recPhase: null,
  recDoneSpeakers: new Set(),
  shareModes: ["auto"], systemInfo: null,
  recoveryPlan: null, recoveryPlanError: null, recoveryState: "empty",
  recoveryTimer: undefined, recoveryRevision: 0, recoveryCreated: new Set(), recoveryRunRevision: -1,
  recoveryRequestKey: "",
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
  updateChannelGuidance();
  refreshResolvedPath();
  renderSteps();
  renderJobState(state.lastJob);
  renderRecoveryJob(state.lastRecoveryJob);
  applySkin(state.skin);
  populateShareModes();
  renderSystemInfo();
  renderRecoveryInventory();
  if (state.version) $("runtime-status").textContent = `v${state.version} · ${state.platform} · ${t("webview_bridge_connected")}`;
}

/* ----------------------------------------------------------------- theme */

/** @param {string} code */
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
  if (previous !== state.skin) scheduleRecoveryPlan();
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
/** @param {keyof ProcessingRequest} name @param {number} fallback */
function brirDefault(name, fallback) {
  const value = state.brirDefaults[name];
  return typeof value === "number" ? value : fallback;
}

/** @param {string} id */
function isOpen(id) {
  return $(id).classList.contains("open");
}

/** @param {Envelope<unknown>} response */
function errorText(response) {
  if (!response || response.ok) return "Unknown error";
  const shareError = shareModeError(response.error);
  if (shareError) return shareError;
  const detail = response.error.details || {};
  const extra = Object.keys(detail).length ? ` ${JSON.stringify(detail)}` : "";
  return `${response.error.code}: ${response.error.message}${extra}`;
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
  el("btn-cancel-brir", HTMLButtonElement).disabled = !active || !job?.cancellable;
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

/* --------------------------------------------------- pipeline checklist */

/** @param {boolean} visible */
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

/** @param {string | undefined} message @param {string | undefined} key */
function updateSteps(message, key) {
  if (state.jobKind !== "brir") return;
  if (key) {
    // A key identifies the event exactly — never fall through to the fuzzy
    // text match, or non-stage lines could false-match a stage prefix.
    const keyIndex = BRIR_STAGES.indexOf(key);
    if (keyIndex >= 0 && keyIndex >= state.stageIndex) {
      state.stageIndex = keyIndex;
      renderSteps();
    }
    return;
  }
  // Fallback for events without a key (older backends): reverse-match the
  // rendered text against the locale table.
  if (!message) return;
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
/** @param {string} kind */
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

/** @param {number | undefined} seconds */
function fmtDuration(seconds) {
  if (seconds === undefined || !Number.isFinite(seconds) || seconds < 0) return "--:--";
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
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-rec-chips]")).forEach((node) => {
    node.hidden = true;
    node.replaceChildren();
  });
  setRecorderStatus("", "");
}

/** @param {string} statusText @param {string} detailText */
function setRecorderStatus(statusText, detailText) {
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-rec-status]")).forEach((node) => {
    node.hidden = !statusText;
    node.textContent = statusText;
  });
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-rec-detail]")).forEach((node) => {
    node.hidden = !detailText;
    node.textContent = detailText || "";
  });
}

/** @param {string[] | undefined} speakers @param {string | null | undefined} activeSpeaker */
function renderRecorderChips(speakers, activeSpeaker) {
  if (!Array.isArray(speakers) || !speakers.length) return;
  if (activeSpeaker) {
    for (const speaker of speakers) {
      if (speaker === activeSpeaker) break;
      state.recDoneSpeakers.add(speaker);
    }
  }
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-rec-chips]")).forEach((node) => {
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
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-rec-chips] .chip")).forEach((chip) => {
    chip.classList.remove("active");
    chip.classList.add("done");
  });
}

/** @param {ProgressPayload} payload */
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
    state.recPhase = phase || null;
    appendLog(status);
  }
}

/** @param {JobOf<"recording">} job */
function finishRecorderStatus(job) {
  if (job.status === "succeeded" && job.result) {
    finishRecorderChips();
    const file = String(job.result.record_path || "").split(/[\\/]/).pop();
    const summary = job.result.summary;
    let detail = summary
      ? fmt(t("recording_status_summary"), {
          file,
          channels: summary.channels,
          duration: fmtDuration(summary.duration),
          peak_db: Number(summary.peak_db).toFixed(1),
          active: summary.active_channels,
          total: summary.channels,
        })
      : fmt(t("recording_status_summary_unavailable"), { file });
    if (job.result.share) detail += ` · ${shareDescription(job.result.share)}`;
    setRecorderStatus(t("recording_status_complete"), detail);
    appendLog(detail);
  } else if (job.status === "failed") {
    const message = job.error ? shareModeError(job.error) || job.error.message : "";
    setRecorderStatus(t("recording_status_error"), message);
    if (state.skin === "stable" && job.error && shareModeError(job.error)) window.alert(message);
  }
}

/* ------------------------------------------------------ output recovery */

/** @param {string[]} paths @param {string} outputDir */
function recoveryFileNames(paths, outputDir) {
  if (!Array.isArray(paths)) return [];
  const root = String(outputDir || "").replace(/\\/g, "/").replace(/\/$/, "");
  return paths.map((rawPath) => {
    const path = String(rawPath).replace(/\\/g, "/");
    return root && path.startsWith(`${root}/`) ? path.slice(root.length + 1) : path.split("/").pop() || "";
  });
}

/** @param {RecoverySource} kind */
function recoverySourceLabel(kind) {
  return {
    hangloose: "Hangloose",
    hrir: "hrir.wav",
    hesuvi: "hesuvi.wav",
    "hrir+hesuvi": "hrir.wav + hesuvi.wav",
  }[kind] || kind || "—";
}

/** @param {string[] | null} created @param {string[] | null} existing */
function setRecoveryFiles(created, existing) {
  const createdRow = $("recovery-created-row");
  const existingRow = $("recovery-existing-row");
  createdRow.hidden = created === null;
  existingRow.hidden = existing === null;
  if (created !== null) $("recovery-created-files").textContent = created.length
    ? created.join(", ") : t("recovery_none");
  if (existing !== null) $("recovery-existing-files").textContent = existing.length
    ? existing.join(", ") : t("recovery_none");
}

/** @param {JobOf<"output_recovery"> | null} job */
function renderRecoveryJob(job) {
  state.lastRecoveryJob = job;
  const resultCard = /** @type {HTMLElement} */ (document.querySelector(".recovery-result"));
  const meta = $("recovery-status-meta");
  const title = $("recovery-status-title");
  const detail = $("recovery-status-detail");
  const openButton = el("btn-open-recovery-output", HTMLButtonElement);

  if (!job) {
    resultCard.dataset.status = "idle";
    meta.textContent = t("webview_job_idle");
    title.textContent = t("webview_job_idle");
    detail.textContent = t("recovery_idle_detail");
    setRecoveryFiles(null, null);
    openButton.hidden = true;
    return;
  }

  resultCard.dataset.status = job.status;
  meta.textContent = t(`webview_status_${job.status}`);
  title.textContent = t(`webview_status_${job.status}`);
  setRecoveryFiles(null, null);
  openButton.hidden = true;

  if (!["succeeded", "failed", "cancelled"].includes(job.status)) {
    detail.textContent = t("recovery_running_detail");
    return;
  }

  if (job.status === "succeeded" && job.result) {
    const result = job.result;
    const created = recoveryFileNames(result.created_files, result.output_dir);
    const existing = recoveryFileNames(result.existing_files, result.output_dir);
    state.lastRecoveryOutputDir = result.output_dir;
    detail.textContent = fmt(t("recovery_success_summary"), {
      source: recoverySourceLabel(result.source_kind),
      sample_rate: result.sample_rate,
      samples: result.sample_count,
      created: created.length,
      existing: existing.length,
    });
    setRecoveryFiles(created, existing);
    openButton.hidden = false;
    if (state.recoveryRunRevision === state.recoveryRevision) {
      state.recoveryCreated = new Set(result.created_files);
      state.recoveryState = "nothing";
      renderRecoveryInventory();
      updateStartControls(state.lastJob);
    }
    return;
  }

  const error = job.error || { code: "UNKNOWN_ERROR", message: "" };
  state.lastRecoveryOutputDir = null;
  detail.textContent = fmt(t("recovery_failed_summary"), {
    code: error.code,
    message: error.message,
  });
}

/** @returns {RecoveryRequest} */
function recoveryRequest() {
  return { dir_path: val("recovery-dir-path"), include_hangloose: checked("recovery-include-hangloose"),
    remove_silent_channels: checked("recovery-remove-silent-channels") };
}

function scheduleRecoveryPlan() {
  const key = JSON.stringify([state.skin, recoveryRequest()]);
  // input and change may report the same value (including blur on Restore).
  // Do not disable the button in the middle of that pointer click.
  if (key === state.recoveryRequestKey) return;
  state.recoveryRequestKey = key;
  window.clearTimeout(state.recoveryTimer);
  const revision = ++state.recoveryRevision;
  state.recoveryPlanError = null;
  state.recoveryCreated.clear();
  state.recoveryState = "empty";
  if (state.skin !== "studio" || !state.version || !val("recovery-dir-path")) {
    state.recoveryPlan = null;
    renderRecoveryInventory();
    updateStartControls(state.lastJob);
    return;
  }
  state.recoveryState = "planning";
  renderRecoveryInventory();
  updateStartControls(state.lastJob);
  const request = recoveryRequest();
  state.recoveryTimer = window.setTimeout(async () => {
    /** @type {Envelope<RecoveryPlan>} */
    let response;
    try { response = await api().plan_output_recovery(request); }
    catch (error) { response = { ok: false, error: { code: "BRIDGE_ERROR", message: String(error), details: {}, retryable: false } }; }
    if (revision !== state.recoveryRevision || state.skin !== "studio") return;
    if (response.ok) {
      state.recoveryPlan = response.data;
      state.recoveryState = response.data.planned_files.length ? "ready" : "nothing";
    } else {
      state.recoveryPlanError = response.error;
      state.recoveryState = "error";
    }
    renderRecoveryInventory();
    updateStartControls(state.lastJob);
  }, 300);
}

function renderRecoveryInventory() {
  const panel = $("recovery-inventory");
  panel.dataset.state = state.recoveryState;
  panel.setAttribute("aria-busy", String(state.recoveryState === "planning"));
  const plan = state.recoveryPlan;
  // Keep the previous inventory's height during re-planning: a path's blur
  // emits change between pointer-down and pointer-up on the next checkbox.
  const visible = !!plan && ["planning", "ready", "nothing"].includes(state.recoveryState);
  $("recovery-inventory-source").hidden = !visible;
  $("recovery-speakers").hidden = !visible;
  $("recovery-ledger").hidden = !visible;
  const message = $("recovery-inventory-message");
  message.textContent = state.recoveryPlanError
    ? fmt(t("recovery_failed_summary"), { ...state.recoveryPlanError })
    : t(`recovery_inventory_${state.recoveryState}`);
  if (!visible || !plan) return;
  $("recovery-source-badge").textContent = fmt(t("recovery_inventory_source"), { source: recoverySourceLabel(plan.source_kind) });
  $("recovery-source-audio").textContent = fmt(t("recovery_inventory_audio"), {
    rate: plan.sample_rate, duration: Math.round(plan.sample_count / plan.sample_rate * 1000),
  });
  $("recovery-speakers").replaceChildren(...plan.speakers.map(speaker => {
    const chip = document.createElement("span");
    chip.className = "chip mono"; chip.textContent = speaker;
    return chip;
  }));
  const ledger = $("recovery-ledger");
  ledger.replaceChildren();
  const paths = [...new Set([...plan.existing_files, ...plan.planned_files.map(file => file.path)])];
  for (const path of paths) {
    const status = state.recoveryCreated.has(path) ? "created" : plan.existing_files.includes(path) ? "present" : "planned";
    const name = recoveryFileNames([path], plan.output_dir)[0];
    const row = document.createElement("div");
    row.className = "recovery-ledger-row"; row.setAttribute("role", "listitem");
    row.dataset.file = name; row.dataset.status = status;
    const file = document.createElement("span"); file.className = "mono"; file.textContent = name;
    const pill = document.createElement("span"); pill.className = "chip recovery-pill";
    pill.dataset.status = status; pill.textContent = t(`recovery_ledger_${status}`);
    row.append(file, pill); ledger.append(row);
  }
}

/** @param {string} path */
function parentFolder(path) {
  const normalized = path.replace(/\\/g, "/");
  const end = normalized.lastIndexOf("/");
  return end === 0 ? "/" : /^[A-Za-z]:$/.test(normalized.slice(0, end))
    ? normalized.slice(0, end + 1) : normalized.slice(0, end);
}

/** @param {SharePreference} mode */
function sharePreferenceLabel(mode) { return t(`option_share_${mode}`); }
/** @param {ShareMode} mode */
function shareModeLabel(mode) { return t(mode === "exclusive" ? "option_share_exclusive" : "option_share_auto_convert"); }
/** @param {RecordingShare} share */
function shareDescription(share) {
  return fmt(t("recording_share_mode_opened"), { output: shareModeLabel(share.output), input: shareModeLabel(share.input) });
}
/** @returns {SharePreference} */
function selectedShareMode() {
  const value = val("rf-share-mode");
  return value === "exclusive" || value === "shared" ? value : "auto";
}
function populateShareModes() {
  const select = el("rf-share-mode", HTMLSelectElement);
  const previous = selectedShareMode();
  select.replaceChildren(...state.shareModes.map(mode => new Option(sharePreferenceLabel(mode), mode)));
  select.value = state.shareModes.includes(previous) ? previous : "auto";
  select.disabled = state.shareModes.length <= 1;
  $("rf-share-mode-hint").hidden = !select.disabled;
}
/** @param {IpcError} error */
function shareModeError(error) {
  if (error.details.kind === "share_mode_unavailable") return t("error_share_mode_unavailable");
  if (error.details.kind !== "share_mode_refused") return "";
  return fmt(t("error_share_mode_refused"), {
    mode: sharePreferenceLabel(error.details.share_mode === "exclusive" ? "exclusive" : "shared"),
    reason: error.details.reason || error.message,
  });
}

/* ------------------------------------------------------------------ jobs */

/** @template {{confirm_warnings?: boolean}} P @param {(payload: P) => Promise<Envelope<{job: Job}>>} start @param {P} payload @param {JobKind | null} kindHint */
async function begin(start, payload, kindHint = null) {
  if (state.startPending || state.jobId) return;
  state.startPending = true;
  if (kindHint === "output_recovery") state.recoveryRunRevision = state.recoveryRevision;
  updateStartControls(state.lastJob);

  /** @type {Envelope<{job: Job}>} */
  let response;
  try {
    response = await start(payload);
    if (response && !response.ok && response.error.code === "CONFIRMATION_REQUIRED") {
      if (!window.confirm(confirmationText(response))) {
        state.startPending = false;
        updateStartControls(state.lastJob);
        return;
      }
      payload.confirm_warnings = true;
      response = await start(payload);
    }
  } catch (error) {
    response = {
      ok: false,
      error: { code: "BRIDGE_ERROR", message: String(error), details: {}, retryable: false },
    };
  }
  state.startPending = false;
  if (!response) {
    updateStartControls(state.lastJob);
    return;
  }
  if (!response.ok) {
    appendLog(errorText(response));
    if (kindHint === "recording") setRecorderStatus(t("recording_status_error"), errorText(response));
    if (kindHint === "output_recovery") {
      renderRecoveryJob({
        job_id: "", cancellable: false,
        kind: "output_recovery",
        status: "failed",
        result: null,
        error: response.error,
      });
    }
    updateStartControls(state.lastJob);
    // Stable hides the inline activity cards (jobs run in the modal), so a
    // pre-start validation error would otherwise be invisible there.
    if (state.skin === "stable" && kindHint !== "output_recovery") {
      window.alert(errorText(response));
    }
    return;
  }
  const job = response.data.job;
  state.jobId = job.job_id;
  state.jobKind = job.kind;
  state.nextSeq = 0;
  if (job.kind === "brir") state.lastOutputDir = val("bf-dir-path");
  if (job.kind === "output_recovery") {
    state.lastRecoveryOutputDir = val("recovery-dir-path");
  }
  state.modalDismissed = false;
  resetSteps(job.kind === "brir");
  resetRecorderStatus();
  el("btn-open-output", HTMLButtonElement).hidden = true;
  setProgress(0);
  renderJobState(job);
  schedulePoll(0);
}

/** @param {{ok: false, error: IpcError}} response */
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
    if (event.type === "progress") {
      const payload = event.payload;
      if (typeof payload.progress === "number") setProgress(payload.progress);
      if (payload.phase) {
        updateRecorderStatus(payload);
      } else if (payload.message) {
        appendLog(payload.message);
        updateSteps(payload.message, payload.key);
      }
    }
    if (event.type === "log") {
      const payload = event.payload;
      const message = payload.key === "recording_share_mode_opened" && state.strings[payload.key] && payload.share
        ? shareDescription(payload.share) : payload.message;
      appendLog(`[${payload.level}] ${message}`);
      updateSteps(payload.message, payload.key);
    }
    if (event.type === "status") appendLog(`· ${t(`webview_status_${event.payload.status}`)}`);
  }
  renderJobState(job);
  if (["succeeded", "failed", "cancelled"].includes(job.status)) {
    if (job.status === "succeeded") setProgress(1);
    if (job.error) appendLog(shareModeError(job.error) || `${job.error.code}: ${job.error.message}`);
    if (job.kind === "brir") {
      if (job.status === "succeeded") {
        completeSteps();
        if (state.lastOutputDir) el("btn-open-output", HTMLButtonElement).hidden = false;
      } else {
        abortSteps(job.status);
      }
    }
    if (job.kind === "recording") finishRecorderStatus(job);
    if (job.kind === "output_recovery") renderRecoveryJob(job);
    state.jobId = null;
    return;
  }
  schedulePoll();
}

/* ----------------------------------------------------------- auto-update */

/* Port of the CTk flow: background check 2s after startup →
   UpdateDialog (notes + Update Now / Remind / Skip) → UpdateExecutor with
   progress → completion message → optional apply-and-restart (Velopack). */

/** @type {{info: UpdateInfo | null, jobId: string | null, nextSeq: number, pollTimer: number | undefined}} */
const updateState = {
  info: null,
  jobId: null,
  nextSeq: 0,
  pollTimer: undefined,
};

/** @param {string | undefined} key @param {string | undefined} fallback */
function tOr(key, fallback) {
  return (key && state.strings[key]) || fallback || key || "";
}

/** @param {number} value */
function setUpdateProgress(value) {
  $("update-progress").style.width = `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
}

/** @param {UpdateInfo} info */
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
  el("update-now", HTMLButtonElement).hidden = false;
  el("update-now", HTMLButtonElement).disabled = false;
  el("update-remind", HTMLButtonElement).hidden = false;
  el("update-skip", HTMLButtonElement).hidden = false;
  el("update-restart", HTMLButtonElement).hidden = true;
  el("update-close", HTMLButtonElement).hidden = true;
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
  el("update-now", HTMLButtonElement).hidden = true;
  el("update-remind", HTMLButtonElement).hidden = true;
  el("update-skip", HTMLButtonElement).hidden = true;
  el("update-restart", HTMLButtonElement).hidden = true;
  el("update-close", HTMLButtonElement).hidden = true;
  $("update-modal").hidden = false;
}

function hideUpdateModal() {
  window.clearTimeout(updateState.pollTimer);
  $("update-modal").hidden = true;
}

/** @param {boolean} manual */
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
  el("update-now", HTMLButtonElement).disabled = true;
  el("update-remind", HTMLButtonElement).hidden = true;
  el("update-skip", HTMLButtonElement).hidden = true;
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
    if (event.type === "progress") {
      const payload = event.payload;
      if (typeof payload.progress === "number") setUpdateProgress(payload.progress);
      // The executor sends either an i18n key ("update_downloading") or
      // preformatted text ("Downloading: 42%").
      if (payload.message) $("update-status").textContent = tOr(payload.message, payload.message);
    }
  }
  if (["succeeded", "failed", "cancelled"].includes(job.status)) {
    updateState.jobId = null;
    if (job.status === "succeeded" && job.kind === "update" && job.result) {
      const result = job.result;
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

/** @param {string} message @param {boolean} success @param {boolean} requiresRestart */
function finishUpdate(message, success, requiresRestart = false) {
  $("update-result").hidden = false;
  $("update-result").textContent = message;
  el("update-now", HTMLButtonElement).hidden = true;
  el("update-remind", HTMLButtonElement).hidden = true;
  el("update-skip", HTMLButtonElement).hidden = true;
  // Velopack stages an apply-and-restart: confirming OK hands over to
  // Update.exe and the window closes. pip/legacy end with a plain Close.
  el("update-restart", HTMLButtonElement).hidden = !(success && requiresRestart);
  el("update-close", HTMLButtonElement).hidden = success && requiresRestart;
}

async function applyStagedUpdate() {
  el("update-restart", HTMLButtonElement).disabled = true;
  const response = await api().apply_pending_update();
  if (!response.ok) {
    el("update-restart", HTMLButtonElement).disabled = false;
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
  const hostSelect = el("rf-host-api", HTMLSelectElement);
  const previousHost = hostSelect.value;
  hostSelect.replaceChildren(new Option(t("option_device_auto"), ""));
  $("rf-host-api-row").hidden = response.data.host_apis.length <= 1;
  response.data.host_apis.forEach((name) => hostSelect.add(new Option(name, name)));
  if ([...hostSelect.options].some((option) => option.value === previousHost)) {
    hostSelect.value = previousHost;
  }
  const devices = response.data.devices;
  fillDevices(el("rf-input-device", HTMLSelectElement), devices.filter((item) => item.max_input_channels > 0));
  fillDevices(el("rf-output-device", HTMLSelectElement), devices.filter((item) => item.max_output_channels > 0));
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

/** @param {string[]} layouts */
function populateSweepLayouts(layouts) {
  const select = el("rf-sweep-layout", HTMLSelectElement);
  const previous = select.value;
  select.replaceChildren();
  layouts.forEach((layout) => select.add(new Option(layout, layout)));
  select.value = layouts.includes(previous) ? previous : "stereo";
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
  const response = await api().resolve_recording_paths(recordDir, playPath, "speakers", sweep);
  if (response.ok) {
    state.resolvedRecordPath = response.data.record_path;
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
  };
  if (sweep) {
    payload.sweep = sweep;
  } else {
    payload.play_path = val("rf-play");
  }
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
  const response = await api().resolve_recording_paths(payload.record_dir, payload.play_path || null, payload.mode, payload.sweep || null);
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
  if (!window.confirm(setup)) return;
  await begin((request) => api().start_recording(request), payload, "recording");
}

async function startHeadphonesRecording() {
  const payload = gatherRecordingPayload("headphones");
  if (!window.confirm(fmt(t("message_record_headphones_confirm"), await recordingConfirmationValues(payload)))) return;
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
      window.alert(t("message_sweep_set_error"));
      return;
    }
    if (response.data.play_path) {
      el("rf-play", HTMLInputElement).value = response.data.play_path;
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

function resolveTestSignalValue() {
  const source = val("bf-test-signal-source") || "auto";
  if (source === "auto") return "auto";
  if (source === "default") return "default";
  if (source === "manual") {
    const duration = numOr("bf-ts-duration", 6.15);
    const fs = Math.trunc(numOr("bf-ts-fs", 48000));
    return `generate:${duration}s@${fs}`;
  }
  return val("bf-test-signal") || null;
}

function updateTestSignalVisibility() {
  const source = val("bf-test-signal-source") || "auto";
  $("bf-ts-manual").hidden = source !== "manual";
  $("bf-ts-file").hidden = source !== "file";
}

async function detectSweep() {
  const node = $("bf-detect-result");
  const button = el("btn-detect-sweep", HTMLButtonElement);
  button.disabled = true;
  node.hidden = false;
  node.textContent = "…";
  try {
    const response = await api().detect_sweep(val("bf-dir-path"));
    if (!response.ok) {
      node.textContent = errorText(response);
      return;
    }
    const data = response.data;
    if (!data.found) {
      node.textContent = t("message_sweep_detect_failed");
      return;
    }
    const confidence = t(
      data.confidence === "high"
        ? "message_sweep_detect_confidence_high"
        : "message_sweep_detect_confidence_low"
    );
    node.textContent = fmt(t("message_sweep_detected"), {
      fs: data.fs,
      duration: data.duration_seconds.toFixed(2),
      segments: data.n_segments,
      files: data.source_files.join(", "),
      confidence,
    });
    /* Pre-fill the manual fields so the user can switch to manual mode
       and tweak from the detected values. */
    el("bf-ts-duration", HTMLInputElement).value = data.duration_seconds.toFixed(2);
    el("bf-ts-fs", HTMLInputElement).value = String(data.fs);
  } finally {
    button.disabled = false;
  }
}

function gatherBrirPayload() {
  /** @type {ProcessingRequest} */
  const args = {
    dir_path: val("bf-dir-path"),
    test_signal: resolveTestSignalValue(),
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
    /** @type {[string, "eq_file" | "eq_left_file" | "eq_right_file"][]} */
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
      /** @type {Record<string, number>} */
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
    args.remove_silent_channels = checked("bf-remove-silent-channels");
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

/** @param {Language[]} languages */
function populateLanguages(languages) {
  const select = el("sf-language", HTMLSelectElement);
  select.replaceChildren();
  languages.forEach(({ code, name }) => select.add(new Option(name, code)));
  select.value = state.language;
}

/** @param {string} code */
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

/** @param {string} code */
async function changeTheme(code) {
  const response = await api().set_theme(code);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  applyTheme(code);
}

/** @param {string} code */
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
  state.systemInfo = info;
  renderSystemInfo();
}

function renderSystemInfo() {
  const info = state.systemInfo;
  if (!info) return;
  const installKey = info.install_kind === "velopack" ? "info_install_velopack"
    : info.install_kind === "pip" ? "info_install_pip" : "info_install_dev";
  $("info-version-pill").textContent = fmt(t("info_version_rust"), {
    version: info.version, toolchain: info.runtime.toolchain,
    install: info.install_kind === "dev" || info.install_kind === "pip" || info.install_kind === "velopack"
      ? t(installKey) : info.install_kind,
  });
  /** @type {[string, string | number | null | undefined][]} */
  const rows = [
    ["label_runtime", info.runtime.toolchain],
    ["label_shell", info.runtime.shell],
    ["label_web_engine", info.runtime.webview],
    ["label_audio_backend", info.runtime.audio_backend],
    ["label_os", info.os],
    ["label_cpu_cores", info.cpu_count],
    ["label_update_channel", info.update_channel ? t(`option_update_${info.update_channel}`) : null],
    ["label_data_directory", info.paths.data_dir],
    ["label_settings_file", info.paths.settings_path],
  ];
  $("sf-data-path").textContent = info.paths.data_dir;
  $("sf-settings-path").textContent = info.paths.settings_path;
  $("sf-data-row").hidden = !info.paths.data_dir;
  $("sf-settings-row").hidden = !info.paths.settings_path;
  const grid = $("info-system");
  grid.replaceChildren();
  for (const [key, value] of rows) {
    if (value === null || value === undefined || value === "") continue;
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
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(".nav-item")).forEach((item) => {
    item.addEventListener("click", () => {
      /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(".nav-item")).forEach((node) => node.classList.remove("active"));
      /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(".view")).forEach((node) => node.classList.remove("active"));
      item.classList.add("active");
      $(`view-${item.dataset.view}`).classList.add("active");
    });
  });

  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-disclosure]")).forEach((head) => {
    const toggle = () => head.parentElement?.classList.toggle("open");
    head.addEventListener("click", toggle);
    head.querySelector(".switch")?.addEventListener("keydown", (event) => {
      if (event instanceof KeyboardEvent && (event.key === "Enter" || event.key === " ")) {
        event.preventDefault();
        toggle();
      }
    });
  });

  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-browse]")).forEach((button) => {
    button.addEventListener("click", async () => {
      const kind = button.dataset.browse;
      const response = kind === "dir"
        ? await api().select_directory()
        : await api().select_file(kind);
      if (response.ok && response.data.path) {
        const target = el(button.dataset.target || "", HTMLInputElement);
        target.value = response.data.path;
        target.dispatchEvent(new Event("input"));
      }
    });
  });

  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-open-url]")).forEach((button) => {
    button.addEventListener("click", () => api().open_url(button.dataset.openUrl || ""));
  });

  el("btn-refresh-devices", HTMLButtonElement).addEventListener("click", () => loadDevices(val("rf-host-api")));
  el("rf-host-api", HTMLSelectElement).addEventListener("change", () => loadDevices(val("rf-host-api")));

  el("rf-play", HTMLInputElement).addEventListener("input", refreshResolvedPath);
  el("rf-record-dir", HTMLInputElement).addEventListener("input", refreshResolvedPath);
  el("rf-sweep-source", HTMLSelectElement).addEventListener("change", () => {
    updateSweepSourceVisibility();
    refreshResolvedPath();
  });
  el("rf-sweep-speakers", HTMLInputElement).addEventListener("input", refreshResolvedPath);
  el("rf-sweep-layout", HTMLSelectElement).addEventListener("change", refreshResolvedPath);
  el("bf-test-signal-source", HTMLSelectElement).addEventListener("change", updateTestSignalVisibility);
  el("btn-detect-sweep", HTMLButtonElement).addEventListener("click", detectSweep);
  el("rf-force-channels", HTMLInputElement).addEventListener("change", updateChannelGuidance);
  el("rf-channels", HTMLInputElement).addEventListener("input", updateChannelGuidance);

  el("btn-start-recording", HTMLButtonElement).addEventListener("click", startSpeakersRecording);
  el("btn-record-headphones", HTMLButtonElement).addEventListener("click", startHeadphonesRecording);
  el("btn-sweep-set", HTMLButtonElement).addEventListener("click", generateSweepSet);

  el("bf-resample", HTMLInputElement).addEventListener("change", () => {
    el("bf-fs", HTMLSelectElement).disabled = !checked("bf-resample");
  });
  el("bf-balance", HTMLSelectElement).addEventListener("change", () => {
    el("bf-balance-db", HTMLInputElement).disabled = val("bf-balance") !== "number";
  });
  el("bf-decay-per-channel", HTMLInputElement).addEventListener("change", () => {
    const perChannel = checked("bf-decay-per-channel");
    el("bf-decay", HTMLInputElement).disabled = perChannel;
    $("bf-decay-channels").hidden = !perChannel;
  });
  el("bf-mic-deviation", HTMLInputElement).addEventListener("change", () => {
    const enabled = checked("bf-mic-deviation");
    el("bf-mic-strength", HTMLInputElement).disabled = !enabled;
    el("bf-mic-debug", HTMLInputElement).disabled = !enabled;
  });

  el("btn-generate-brir", HTMLButtonElement).addEventListener("click", () =>
    begin((request) => api().start_brir(request), gatherBrirPayload()),
  );
  el("btn-start-recovery", HTMLButtonElement).addEventListener("click", () =>
    begin(
      (request) => api().start_output_recovery(request),
      recoveryRequest(),
      "output_recovery",
    ),
  );
  el("btn-cancel-brir", HTMLButtonElement).addEventListener("click", cancelActiveJob);
  el("job-modal-cancel", HTMLButtonElement).addEventListener("click", cancelActiveJob);
  el("job-modal-close", HTMLButtonElement).addEventListener("click", () => {
    state.modalDismissed = true;
    $("job-modal").hidden = true;
  });
  el("btn-open-output", HTMLButtonElement).addEventListener("click", () => {
    if (state.lastOutputDir) api().open_path(state.lastOutputDir);
  });
  el("btn-open-recovery-output", HTMLButtonElement).addEventListener("click", () => {
    if (state.lastRecoveryOutputDir) api().open_path(state.lastRecoveryOutputDir);
  });

  el("btn-open-data", HTMLButtonElement).addEventListener("click", () => api().open_path(state.systemInfo?.paths.data_dir));
  el("btn-open-settings", HTMLButtonElement).addEventListener("click", () => {
    const path = state.systemInfo?.paths.settings_path;
    if (path) api().open_path(parentFolder(path));
  });
  for (const id of ["recovery-dir-path", "recovery-include-hangloose", "recovery-remove-silent-channels"]) {
    $(id).addEventListener("input", scheduleRecoveryPlan);
    $(id).addEventListener("change", scheduleRecoveryPlan);
  }
  el("sf-skin", HTMLSelectElement).addEventListener("change", () => changeSkin(val("sf-skin")));
  el("sf-theme", HTMLSelectElement).addEventListener("change", () => changeTheme(val("sf-theme")));
  el("sf-language", HTMLSelectElement).addEventListener("change", () => changeLanguage(val("sf-language")));

  el("btn-check-updates", HTMLButtonElement).addEventListener("click", () => checkForUpdates(true));
  el("update-now", HTMLButtonElement).addEventListener("click", beginUpdate);
  el("update-remind", HTMLButtonElement).addEventListener("click", hideUpdateModal);
  el("update-skip", HTMLButtonElement).addEventListener("click", hideUpdateModal);
  el("update-close", HTMLButtonElement).addEventListener("click", hideUpdateModal);
  el("update-restart", HTMLButtonElement).addEventListener("click", applyStagedUpdate);
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
  state.shareModes = data.capabilities.share_modes;
  state.brirDefaults = data.brir_defaults || {};
  state.sweepDefaults = data.sweep || {};
  populateSweepLayouts(
    (data.sweep && data.sweep.layouts) || ["mono", "stereo", "5.1", "7.1", "7.1.4", "7.1.6"]
  );
  updateSweepSourceVisibility();
  updateTestSignalVisibility();
  if (data.ui) {
    state.strings = data.ui.strings || {};
    state.language = data.ui.language || "en";
    populateLanguages(data.ui.languages || []);
    el("sf-theme", HTMLSelectElement).value = data.ui.theme || "dark";
    applyTheme(data.ui.theme || "dark");
    el("sf-skin", HTMLSelectElement).value = data.ui.skin === "stable" ? "stable" : "studio";
    applySkin(data.ui.skin);
  }
  applyStrings();
  /** @type {Record<string, string>} */
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
  scheduleRecoveryPlan();

  // First run: ask for the language before anything else (CTk parity).
  if (data.ui && data.ui.first_run) {
    showFirstRunLanguageModal(data.ui.languages || []);
  }

  // Mirror the CTk root.after(2000) startup update check; failures stay
  // silent and never block the UI.
  window.setTimeout(() => checkForUpdates(false), 2000);
}

/** @param {Language[]} languages */
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
      el("sf-language", HTMLSelectElement).value = code;
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
