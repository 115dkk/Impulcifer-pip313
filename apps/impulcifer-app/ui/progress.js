// @ts-check
// What jobs show while they run: the BRIR stage checklist, recorder status, output recovery.
"use strict";

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
    const file = relativeRecordPath(job.result.record_path, job.result.record_dir || "");
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
    (job.result.checks || []).forEach((check) => appendLog(checkMessage(check)));
    rememberMeasurements(job.result);
    // The next recording may now land elsewhere (headphones_n.wav, a subfolder).
    refreshResolvedPath();
  } else if (job.status === "cancelled") {
    setRecorderStatus(t("recording_status_cancelled"), "");
  } else if (job.status === "failed") {
    const message = job.error ? errorSentence(job.error) : "";
    setRecorderStatus(t("recording_status_error"), message);
    if (state.skin === "stable" && job.error && shareModeError(job.error)) void noticeDialog(message);
  }
  finishGuidedStep(job);
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

/* A channel-order file (ADR 0007) is named after its format, so the source
   is its file name. */
/** @param {RecoverySource} kind @param {string} sourcePath */
function recoverySourceLabel(kind, sourcePath) {
  if (kind === "layout") return sourcePath.replace(/\\/g, "/").split("/").pop() || kind;
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

  if (!isTerminal(job.status)) {
    detail.textContent = t("recovery_running_detail");
    return;
  }

  if (job.status === "succeeded" && job.result) {
    const result = job.result;
    const created = recoveryFileNames(result.created_files, result.output_dir);
    const existing = recoveryFileNames(result.existing_files, result.output_dir);
    state.lastRecoveryOutputDir = result.output_dir;
    detail.textContent = fmt(t("recovery_success_summary"), {
      source: recoverySourceLabel(result.source_kind, result.source_path),
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

  const error = job.error || { code: "UNKNOWN_ERROR", message: "", details: {}, retryable: false };
  state.lastRecoveryOutputDir = null;
  detail.textContent = fmt(t("recovery_failed_summary"), { message: errorSentence(error) });
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
    const response = await callApi(() => api().plan_output_recovery(request));
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
    ? fmt(t("recovery_failed_summary"), { message: errorSentence(state.recoveryPlanError) })
    : t(`recovery_inventory_${state.recoveryState}`);
  if (!visible || !plan) return;
  $("recovery-source-badge").textContent = fmt(t("recovery_inventory_source"), { source: recoverySourceLabel(plan.source_kind, plan.source_path) });
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

/* The recovery job and inventory again, after the language changes. */
function renderRecovery() {
  renderRecoveryJob(state.lastRecoveryJob);
  renderRecoveryInventory();
}

/* Start, open the output, and the three inputs that plan again. */
function wireRecovery() {
  el("btn-start-recovery", HTMLButtonElement).addEventListener("click", () =>
    begin(
      (request) => api().start_output_recovery(request),
      recoveryRequest(),
      "output_recovery",
    ),
  );
  el("btn-open-recovery-output", HTMLButtonElement).addEventListener("click", () => {
    if (state.lastRecoveryOutputDir) api().open_path(state.lastRecoveryOutputDir);
  });
  for (const id of ["recovery-dir-path", "recovery-include-hangloose", "recovery-remove-silent-channels"]) {
    $(id).addEventListener("input", scheduleRecoveryPlan);
    $(id).addEventListener("change", scheduleRecoveryPlan);
  }
}
