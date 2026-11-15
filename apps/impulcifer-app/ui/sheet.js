// @ts-check
// The Android page's job sheet: a BRIR job runs in it, it can be put away, and it ends on the outcome.
"use strict";

/* Starting a BRIR job opens the sheet (#job-sheet) with the progress, the
   stage list, the log (folded) and cancel. Put away, the job goes on and the
   bar above the action bar (#job-bar) shows it; a tap brings the sheet back.
   When the job ends the sheet shows the outcome: the output folder, the
   alignment report (filled by advanced.js) or the failure sentence. The bar
   stays until the outcome has been seen. The elements the shared job code
   fills (#brir-steps, #brir-alignment, [data-progress], [data-log],
   #btn-cancel-brir, #btn-open-output) live inside the sheet. */

/** @type {{ job: Job | null, open: boolean, seen: boolean }} */
const jobSheet = { job: null, open: false, seen: true };

/* The main channel's job (renderJobState). A BRIR job it has not seen before
   opens the sheet while it runs; a job that is already over (one found when
   the page loads) only shows in the bar. */
/** @param {Job | null} job */
function showJobInSheet(job) {
  if (job && job.kind === "brir") {
    if (job.job_id !== jobSheet.job?.job_id) {
      jobSheet.seen = false;
      $("brir-open-failed").hidden = true;
      if (!isTerminal(job.status)) openJobSheet();
    }
    jobSheet.job = job;
  }
  if (job && job.kind === "output_recovery" && !isTerminal(job.status)) $("recovery-open-failed").hidden = true;
  renderJobSheet();
}

function openJobSheet() {
  const opening = !jobSheet.open;
  jobSheet.open = true;
  renderJobSheet();
  if (opening && jobSheet.job) $("job-sheet-close").focus({ preventScroll: true });
}

function putAwayJobSheet() {
  jobSheet.open = false;
  if (jobSheet.job && isTerminal(jobSheet.job.status)) jobSheet.seen = true;
  renderJobSheet();
}

function renderJobSheet() {
  const job = jobSheet.job;
  const active = Boolean(job && !isTerminal(job.status));
  const sheet = $("job-sheet");
  sheet.hidden = !jobSheet.open || !job;
  sheet.dataset.status = job ? job.status : "idle";
  const status = job ? t(`webview_status_${job.status}`) : "";
  $("job-sheet-status").textContent = status;
  el("btn-cancel-brir", HTMLButtonElement).hidden = !active;
  el("job-sheet-done", HTMLButtonElement).hidden = active;

  const error = $("job-sheet-error");
  error.hidden = job?.status !== "failed";
  error.textContent = job?.status === "failed"
    ? errorSentence(job.error || { code: "UNKNOWN_ERROR", message: t("webview_status_failed"), details: {}, retryable: false })
    : "";
  const output = job?.status === "succeeded" ? state.lastOutputDir : null;
  $("job-sheet-output-row").hidden = !output;
  const path = $("job-sheet-output");
  path.textContent = output || "";
  path.scrollLeft = path.scrollWidth;

  // The bar: the stage in flight while the job runs, the outcome until it has been seen.
  const bar = $("job-bar");
  bar.hidden = !job || jobSheet.open || (!active && jobSheet.seen);
  bar.dataset.status = job ? job.status : "idle";
  const stage = active && state.stageIndex >= 0 && state.stageIndex < BRIR_STAGES.length
    ? t(BRIR_STAGES[state.stageIndex]) : "";
  $("job-bar-title").textContent = stage || t("studio_processing_title");
  $("job-bar-status").textContent = status;
  bar.setAttribute("aria-label", `${t("studio_processing_title")} · ${status}`);
}

/* The sheet's chevron and close button put it away; the bar brings it back. */
function wireJobSheet() {
  el("job-sheet-close", HTMLButtonElement).addEventListener("click", putAwayJobSheet);
  el("job-sheet-done", HTMLButtonElement).addEventListener("click", putAwayJobSheet);
  el("job-bar", HTMLButtonElement).addEventListener("click", openJobSheet);
}

/* The status words again, after the language changes. */
function renderJobSheetWords() {
  renderJobSheet();
}
