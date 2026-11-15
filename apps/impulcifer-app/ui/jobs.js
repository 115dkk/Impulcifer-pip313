// @ts-check
// Share modes, recording log fields, the screen's own dialogs, and starting, polling and cancelling jobs.
"use strict";

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

/* Recording log lines the screen words itself, with the fields their text needs. */
/** @type {Record<string, string[]>} */
const LOG_FIELDS = {
  recording_session_folder: ["folder"],
  recording_channel_order_hdmi: ["channels"],
  recording_channel_order_standard: ["channels"],
  recording_output_channels_short: ["device", "available", "first", "last", "missing"],
};

/* --------------------------------------------------------------- dialogs */

/* WKWebView (macOS) answers window.confirm with false and drops window.alert
   and window.prompt unless the host implements them, and wry does not, so on
   macOS every confirmation was refused before it was shown. The screen asks
   with its own dialog on every platform instead. */
/** @type {{resolve: (ok: boolean) => void, cancel: boolean, focus: Element | null} | null} */
let pendingAsk = null;

/** @param {string} message @param {boolean} cancel @returns {Promise<boolean>} */
function askDialog(message, cancel) {
  if (pendingAsk) closeAsk(false);
  $("ask-modal-message").textContent = message;
  $("ask-modal-cancel").hidden = !cancel;
  $("ask-modal").hidden = false;
  const focus = document.activeElement;
  el("ask-modal-ok", HTMLButtonElement).focus();
  return new Promise((resolve) => { pendingAsk = { resolve, cancel, focus }; });
}

/** @param {boolean} ok */
function closeAsk(ok) {
  const ask = pendingAsk;
  if (!ask) return;
  pendingAsk = null;
  $("ask-modal").hidden = true;
  if (ask.focus instanceof HTMLElement && ask.focus.isConnected) ask.focus.focus();
  ask.resolve(ok);
}

/** @param {string} message */
const confirmDialog = (message) => askDialog(message, true);
/** @param {string} message */
const noticeDialog = (message) => askDialog(message, false).then(() => undefined);

/** @param {KeyboardEvent} event */
function onAskKey(event) {
  if (!pendingAsk) return;
  if (event.key === "Escape") {
    event.preventDefault();
    closeAsk(!pendingAsk.cancel);
  } else if (event.key === "Tab") {
    /* Keep focus on the dialog's buttons while it is open. */
    const buttons = [el("ask-modal-cancel", HTMLButtonElement), el("ask-modal-ok", HTMLButtonElement)]
      .filter((button) => !button.hidden);
    const index = buttons.indexOf(/** @type {HTMLButtonElement} */ (document.activeElement));
    const next = buttons[(index + (event.shiftKey ? -1 : 1) + buttons.length) % buttons.length];
    event.preventDefault();
    next.focus();
  }
}

/* The dialog's buttons, and its keys on the whole page. */
function wireDialogs() {
  el("ask-modal-ok", HTMLButtonElement).addEventListener("click", () => closeAsk(true));
  el("ask-modal-cancel", HTMLButtonElement).addEventListener("click", () => closeAsk(false));
  document.addEventListener("keydown", onAskKey);
}

/* ------------------------------------------------------------------ jobs */

/* Every job (a recording, a BRIR, an output recovery, an update) runs the
   same way: the service starts it, the screen polls it in sequence until its
   status is final, and each poll hands over the events since the last one.
   A slot holds the job one channel follows: `state` is the main channel
   (recording, BRIR and recovery, one at a time) and `updateState` the update
   dialog's. A follower is what one channel does with what a poll returns. */

/**
 * @typedef {{jobId: string | null, nextSeq: number, pollTimer: number | undefined}} JobSlot
 * @typedef {object} JobFollower
 * @property {(event: JobEvent) => void} [event] each event, in order
 * @property {(job: Job) => void} [update] the job after each poll's events
 * @property {(job: Job) => void} finish the job once its status is final
 * @property {(response: {ok: false, error: IpcError}) => void} lost a poll failed; the job is no longer followed
 */

/** @param {JobSlot} slot @param {Job} job @param {JobFollower} follower */
function followJob(slot, job, follower) {
  window.clearTimeout(slot.pollTimer);
  slot.jobId = job.job_id;
  slot.nextSeq = 0;
  slot.pollTimer = window.setTimeout(() => pollSlot(slot, follower), 0);
}

/** @param {JobSlot} slot */
function stopFollowing(slot) {
  window.clearTimeout(slot.pollTimer);
  slot.jobId = null;
}

/** @param {JobSlot} slot @param {JobFollower} follower */
async function pollSlot(slot, follower) {
  const jobId = slot.jobId;
  if (!jobId) return;
  const response = await callApi(() => api().poll_job(jobId, slot.nextSeq));
  // Stopped, or another job took the slot, while the poll was out.
  if (slot.jobId !== jobId) return;
  if (!response.ok) {
    slot.jobId = null;
    follower.lost(response);
    return;
  }
  const { job, events, next_seq: nextSeq } = response.data;
  slot.nextSeq = nextSeq;
  for (const event of events) follower.event?.(event);
  follower.update?.(job);
  if (isTerminal(job.status)) {
    slot.jobId = null;
    follower.finish(job);
    return;
  }
  slot.pollTimer = window.setTimeout(() => pollSlot(slot, follower), 250);
}

/* Starts a job, asking first when the service wants a confirmation (a mono
   headphone recording, too few output channels). Null when the person said no. */
/** @template {{confirm_warnings?: boolean}} P @param {(payload: P) => Promise<Envelope<{job: Job}>>} start @param {P} payload @returns {Promise<Envelope<{job: Job}> | null>} */
async function requestJob(start, payload) {
  let response = await callApi(() => start(payload));
  if (!response.ok && response.error.code === "CONFIRMATION_REQUIRED") {
    if (!(await confirmDialog(confirmationText(response)))) return null;
    payload.confirm_warnings = true;
    response = await callApi(() => start(payload));
  }
  return response;
}

/** @template {{confirm_warnings?: boolean}} P @param {(payload: P) => Promise<Envelope<{job: Job}>>} start @param {P} payload @param {JobKind | null} kindHint */
async function begin(start, payload, kindHint = null) {
  if (state.startPending || state.jobId) return;
  state.startPending = true;
  if (kindHint === "output_recovery") state.recoveryRunRevision = state.recoveryRevision;
  updateStartControls(state.lastJob);

  const response = await requestJob(start, payload);
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
    // Stable hides the inline activity cards (jobs run in the modal), and the
    // Android page shows a job only in its sheet, which opens once a job runs,
    // so a pre-start validation error would otherwise be invisible there.
    if ((state.skin === "stable" || document.getElementById("job-sheet")) && kindHint !== "output_recovery") {
      void noticeDialog(errorText(response));
    }
    return;
  }
  const job = response.data.job;
  if (job.kind === "brir") state.lastOutputDir = val("bf-dir-path");
  if (job.kind === "output_recovery") {
    state.lastRecoveryOutputDir = val("recovery-dir-path");
  }
  state.modalDismissed = false;
  el("btn-open-output", HTMLButtonElement).hidden = true;
  setProgress(0);
  followMainJob(job);
  renderJobState(job);
}

/* The main channel's displays start over for a job it begins following: one
   it started, or one still running when the page loaded. */
/** @param {Job} job */
function followMainJob(job) {
  state.jobKind = job.kind;
  resetSteps(job.kind === "brir");
  if (job.kind === "brir") renderAlignment(null);
  resetRecorderStatus();
  followJob(state, job, MAIN_JOB);
}

/** @param {{ok: false, error: IpcError}} response */
function confirmationText(response) {
  const details = response.error.details || {};
  if (details.warning === "headphones_mono") return t("message_headphones_mono_warning");
  // The service checks the output's channels before it starts, for files and the CLI too.
  if (details.warning === "output_channels_short") {
    const layout = String(details.layout || "");
    return fmt(t("warning_output_channels_short"), { ...details, layout: immersiveLayout(layout)?.name || layout });
  }
  if (details.play_channels !== undefined) {
    return fmt(t("message_channel_mismatch_warning"), {
      play_channels: details.play_channels,
      record_channels: details.record_channels ?? details.selected_channels ?? "?",
    });
  }
  return response.error.message;
}

async function cancelActiveJob() {
  const jobId = state.jobId;
  if (!jobId) return;
  const response = await callApi(() => api().cancel_job(jobId));
  if (!response.ok) appendLog(errorText(response));
}

/** @param {JobEvent} event */
function showJobEvent(event) {
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
    const key = payload.key || "";
    // A recording check is shown from the job result (checkMessage).
    if (key.startsWith("record_check_")) return;
    const fields = LOG_FIELDS[key];
    const values = /** @type {Record<string, unknown>} */ (payload);
    const message = key === "recording_share_mode_opened" && state.strings[key] && payload.share
      ? shareDescription(payload.share)
      : fields && state.strings[key] && fields.every((name) => values[name] !== undefined)
        ? fmt(t(key), Object.fromEntries(fields.map((name) => [name, values[name]]))) : payload.message;
    appendLog(`[${payload.level}] ${message}`);
    updateSteps(payload.message, payload.key);
  }
  if (event.type === "status") appendLog(`· ${t(`webview_status_${event.payload.status}`)}`);
}

/** @param {Job} job */
function finishMainJob(job) {
  if (job.status === "succeeded") setProgress(1);
  if (job.error) appendLog(errorSentence(job.error));
  if (job.kind === "brir") {
    if (job.status === "succeeded") {
      completeSteps();
      renderAlignment(job.result?.alignment ?? null);
      if (state.lastOutputDir) el("btn-open-output", HTMLButtonElement).hidden = false;
    } else {
      renderAlignment(null);
      abortSteps(job.status);
    }
  }
  if (job.kind === "recording") finishRecorderStatus(job);
  if (job.kind === "output_recovery") renderRecoveryJob(job);
}

/** @type {JobFollower} */
const MAIN_JOB = {
  event: showJobEvent,
  update: renderJobState,
  finish: finishMainJob,
  lost(response) {
    appendLog(errorText(response));
    if (state.jobKind === "brir") renderAlignment(null);
    renderJobState(null);
  },
};

/* The stage checklist, the job state and a finished BRIR's alignment report again, after the language changes. */
function renderJobs() {
  renderSteps();
  renderJobState(state.lastJob);
  if (state.lastJob?.kind === "brir" && state.lastJob.status === "succeeded") renderAlignment(state.lastJob.result?.alignment ?? null);
}

/* Generate (unless the single recording or a measurement file cannot be used
   yet), cancel, and opening the output folder. */
function wireJobs() {
  el("btn-generate-brir", HTMLButtonElement).addEventListener("click", () => {
    if (brirRecordingProblem()) {
      updateBrirRecordingStatus();
      $("bf-brir-recording-block").scrollIntoView({ block: "center" });
      return;
    }
    const blocked = MFILE_PURPOSES.find((purpose) => mfileActive(purpose) && mfileBlocked(purpose));
    if (blocked) {
      renderMfile(blocked);
      $(`${MFILE[blocked].prefix}-block`).scrollIntoView({ block: "center" });
      return;
    }
    begin((request) => api().start_brir(request), gatherBrirPayload());
  });
  el("btn-cancel-brir", HTMLButtonElement).addEventListener("click", cancelActiveJob);
  el("btn-open-output", HTMLButtonElement).addEventListener("click", () => {
    if (state.lastOutputDir) void openFolder(state.lastOutputDir, "brir-open-failed");
  });
}

/* Stable's job dialog: cancel, and close once the job has ended. */
function wireJobModal() {
  el("job-modal-cancel", HTMLButtonElement).addEventListener("click", cancelActiveJob);
  el("job-modal-close", HTMLButtonElement).addEventListener("click", () => {
    state.modalDismissed = true;
    $("job-modal").hidden = true;
  });
}
