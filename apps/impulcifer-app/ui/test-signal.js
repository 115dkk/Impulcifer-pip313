// @ts-check
// BRIR test signal: the source choice, sweep detection and the length check against the recordings.
"use strict";

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

/* A test signal file the recordings were not made with gives responses that
   mean nothing, or a job that stops half way. While the source is a file, the
   service compares its length with the sweeps in the recordings the job
   would read (the folder's speaker recordings and the headphone recording),
   and the note under the field names the ones that do not fit. */
function scheduleTestSignalCheck(delay = 250) {
  window.clearTimeout(state.testSignalTimer);
  const revision = ++state.testSignalRevision;
  const note = $("bf-ts-warning");
  const dir = val("bf-dir-path");
  const file = val("bf-test-signal");
  if ((val("bf-test-signal-source") || "auto") !== "file" || !dir || !file || !window.pywebview) {
    note.hidden = true;
    return;
  }
  state.testSignalTimer = window.setTimeout(async () => {
    const response = await callApi(() => api().detect_sweep(dir, {
      test_signal: file,
      speakers: val("bf-brir-source") === "folder",
      headphone_file: isOpen("dis-headphone") && !mfileMode("headphones") ? val("bf-headphone-file") : null,
    }));
    const report = response.ok ? response.data.test_signal : undefined;
    if (revision !== state.testSignalRevision) return;
    const mismatches = report?.mismatches ?? [];
    note.hidden = mismatches.length === 0;
    if (!mismatches.length) return;
    note.textContent = fmt(t("message_test_signal_mismatch"), {
      signal: (report?.duration_seconds ?? 0).toFixed(2),
      files: mismatches
        .map((m) => fmt(t("message_test_signal_mismatch_item"), { file: m.file, duration: m.duration_seconds.toFixed(2) }))
        .join(", "),
    });
  }, delay);
}

/* Which recordings the job reads with the test signal: all of them; only the folder's
   headphone and room recordings (one recording file made with another program's sweep,
   which that sweep file reads); or none. */
function testSignalScope() {
  if (val("bf-brir-source") !== "recording" || !isOpen("dis-recording-sweep")) return "all";
  const folderHeadphones = isOpen("dis-headphone") && !mfileMode("headphones");
  const folderRoom = isOpen("dis-room") && !mfileMode("room");
  return folderHeadphones || folderRoom ? "folder" : "none";
}

/* The test signal rows show only while the job reads a recording with them; the note
   says which recordings those are when the recording file has its own sweep. */
function updateTestSignalScope() {
  const scope = testSignalScope();
  $("bf-ts-block").hidden = scope === "none";
  $("bf-ts-scope").hidden = scope !== "folder";
}

/* The test signal block of the request; hidden rows send auto, like a group that is off. */
/** @param {ProcessingRequest} args */
function addTestSignalArgs(args) {
  args.test_signal = testSignalScope() === "none" ? "auto" : resolveTestSignalValue();
}

/* The test signal listeners: the source, the inputs the length check reads, and sweep detection. */
function wireTestSignal() {
  el("bf-test-signal-source", HTMLSelectElement).addEventListener("change", () => {
    updateTestSignalVisibility();
    scheduleTestSignalCheck(0);
  });
  for (const id of ["bf-test-signal", "bf-dir-path", "bf-headphone-file"]) {
    el(id, HTMLInputElement).addEventListener("input", () => scheduleTestSignalCheck());
  }
  el("btn-detect-sweep", HTMLButtonElement).addEventListener("click", detectSweep);
  // The headphone and room groups decide whether the test signal still reads anything.
  document.addEventListener("disclosure-toggle", updateTestSignalScope);
}
