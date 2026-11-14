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
      if (!(await confirmDialog(confirmationText(response)))) {
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
      void noticeDialog(errorText(response));
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
  if (job.kind === "brir") renderAlignment(null);
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
    if (state.jobKind === "brir") renderAlignment(null);
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
      const key = payload.key || "";
      // A recording check is shown from the job result (checkMessage).
      if (key.startsWith("record_check_")) continue;
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
  renderJobState(job);
  if (["succeeded", "failed", "cancelled"].includes(job.status)) {
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
    state.jobId = null;
    return;
  }
  schedulePoll();
}
