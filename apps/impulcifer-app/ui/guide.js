// @ts-check
// Head-turn guided measurement: steps, diagram, countdown and the session's measurements.
"use strict";

/* ------------------------------------------------------ head-turn steps */

/* One or two speakers stay put and the listener turns between recordings.
   Plans come from bootstrap (sweep.guided). A step plays its speakers on the
   stereo outputs and sends the angles it expects, so the service can tell
   from the ears' time difference whether the turn was right. Angles are ITU:
   degrees, positive to the listener's left; yaw is the turn, positive left. */
const GUIDED_COUNTDOWN = 8;
const GUIDED_TARGET_ORDER = ["stereo", "5.1", "7.1", "9.1"];
/* 9.1 adds the wides; most rooms are measured for 7.1, so it stays the default. */
const GUIDED_DEFAULT_TARGET = "7.1";

function guidedSetup() {
  const setup = val("gd-setup");
  return (setup && state.sweepDefaults?.guided?.[setup]) || null;
}

/** @returns {GuidedTake[]} */
function guidedTakes() {
  return guidedSetup()?.plans[val("gd-target")] || [];
}

/* A step's outcome belongs to the folder it was recorded into. */
/** @param {number} index */
function guidedKey(index) {
  return `${folderKey(val("rf-record-dir"))}|${val("gd-setup")}|${val("gd-target")}|${index}`;
}

function populateGuidedTargets() {
  stopGuidedCountdown();
  const select = el("gd-target", HTMLSelectElement);
  const previous = select.value;
  const plans = Object.keys(guidedSetup()?.plans || {});
  const targets = [...GUIDED_TARGET_ORDER.filter((id) => plans.includes(id)),
    ...plans.filter((id) => !GUIDED_TARGET_ORDER.includes(id))];
  select.replaceChildren(...targets.map((id) => new Option(id, id)));
  const fallback = targets.includes(GUIDED_DEFAULT_TARGET) ? GUIDED_DEFAULT_TARGET : targets[targets.length - 1] || "";
  select.value = targets.includes(previous) ? previous : fallback;
  state.guided.index = 0;
  renderGuided();
}

/** @param {number} azimuth */
function wrapAzimuth(azimuth) {
  const wrapped = ((((azimuth + 180) % 360) + 360) % 360) - 180;
  return wrapped === -180 ? 180 : wrapped;
}

/** @param {number} azimuth */
function guidedDirection(azimuth) {
  const angle = Math.round(Math.abs(azimuth));
  if (angle === 0) return t("guided_dir_ahead");
  if (angle === 180) return t("guided_dir_behind");
  return fmt(t(azimuth > 0 ? "guided_dir_left" : "guided_dir_right"), { angle });
}

/** @param {string} setup @param {GuidedTake} take */
function guidedTurnText(setup, take) {
  const yaw = Math.round(take.yaw);
  if (setup === "stereo" && yaw !== 0
      && take.sources.some((source) => source.output === 0 && Math.round(source.azimuth) === 0)) {
    return t("guided_face_left_speaker");
  }
  if (yaw === 0) return t(setup === "mono" ? "guided_face_speaker" : "guided_face_between");
  if (Math.abs(yaw) === 180) return t("guided_turn_around");
  return fmt(t(yaw > 0 ? "guided_turn_left" : "guided_turn_right"), { degrees: Math.abs(yaw) });
}

/* A speaker at 90° is aimed at with the ear; say which ear. */
/** @param {string} setup @param {GuidedTake} take */
function guidedEarText(setup, take) {
  const source = take.sources.find((item) => item.output === 0 && Math.round(Math.abs(item.azimuth)) === 90);
  if (!source) return "";
  const ear = source.azimuth > 0 ? "left" : "right";
  return t(setup === "mono" ? `guided_ear_speaker_${ear}` : `guided_ear_left_speaker_${ear}`);
}

/** @param {string} tag @param {Record<string, string | number>} attrs */
function svgNode(tag, attrs) {
  const node = document.createElementNS(SVG_NS, tag);
  Object.entries(attrs).forEach(([name, value]) => node.setAttribute(name, String(value)));
  return node;
}

/* Top view with the head facing up: ITU azimuth to screen coordinates. */
/** @param {number} azimuth @param {number} radius */
function polar(azimuth, radius) {
  const angle = (azimuth * Math.PI) / 180;
  return [-Math.sin(angle) * radius, -Math.cos(angle) * radius];
}

/** @param {GuidedSetupInfo} info @param {GuidedTake} take */
function drawGuidedDiagram(info, take) {
  const svg = $("gd-diagram");
  svg.setAttribute("aria-label", t("guided_diagram_label"));
  const defs = svgNode("defs", {});
  const marker = svgNode("marker", { id: "gd-arrow", viewBox: "0 0 10 10", refX: 7, refY: 5,
    markerWidth: 7, markerHeight: 7, orient: "auto-start-reverse" });
  marker.append(svgNode("path", { class: "gd-arrow-head", d: "M0 0L10 5L0 10z" }));
  defs.append(marker);
  const parts = [defs, svgNode("circle", { class: "gd-ring", r: 96 }),
    svgNode("line", { class: "gd-front", x1: 0, y1: -38, x2: 0, y2: -88 })];
  // The direction faced at the start, and the turn from it to now.
  const yaw = Math.round(take.yaw);
  if (yaw !== 0) {
    const [sx, sy] = polar(-yaw, 88);
    parts.push(svgNode("line", { class: "gd-start", x1: 0, y1: 0, x2: sx, y2: sy }));
    const [ax, ay] = polar(-yaw, 54);
    const [bx, by] = polar(0, 54);
    parts.push(svgNode("path", { class: "gd-turn-arc", "marker-end": "url(#gd-arrow)",
      d: `M${ax.toFixed(1)} ${ay.toFixed(1)}A54 54 0 0 ${yaw > 0 ? 0 : 1} ${bx.toFixed(1)} ${by.toFixed(1)}` }));
  }
  parts.push(svgNode("ellipse", { class: "gd-head", cx: -22, cy: 0, rx: 5, ry: 9 }),
    svgNode("ellipse", { class: "gd-head", cx: 22, cy: 0, rx: 5, ry: 9 }),
    svgNode("path", { class: "gd-head", d: "M-7 -23L0 -34L7 -23z" }),
    svgNode("ellipse", { class: "gd-head", cx: 0, cy: 0, rx: 21, ry: 25 }));
  [["L", -38], ["R", 38]].forEach(([text, x]) => {
    const label = svgNode("text", { class: "gd-ear-label", x, y: 4 });
    label.textContent = String(text);
    parts.push(label);
  });
  info.outputs.forEach((room, output) => {
    const azimuth = wrapAzimuth(room - take.yaw);
    const source = take.sources.find((item) => item.output === output);
    const [x, y] = polar(azimuth, 96);
    const box = svgNode("g", { class: "gd-spk", "data-on": String(Boolean(source)),
      transform: `translate(${x.toFixed(1)} ${y.toFixed(1)}) rotate(${(-azimuth).toFixed(1)})` });
    box.append(svgNode("rect", { x: -12, y: -9, width: 24, height: 18, rx: 3 }),
      svgNode("circle", { cx: 0, cy: 2, r: 4.5 }));
    parts.push(box);
    if (source) {
      const [lx, ly] = polar(azimuth, 120);
      const label = svgNode("text", { class: "gd-spk-label", x: lx.toFixed(1), y: (ly + 4).toFixed(1) });
      label.textContent = source.speaker;
      parts.push(label);
    }
  });
  svg.replaceChildren(...parts);
}

/** @param {RecordingCheck} check */
function checkMessage(check) {
  /** @type {Record<string, unknown>} */
  const values = { speaker: check.speaker };
  Object.entries(check.values || {}).forEach(([name, value]) => {
    values[name] = typeof value === "string" ? value
      : name.endsWith("_db") ? Number(value).toFixed(1) : Math.round(Number(value));
  });
  return fmt(t(check.key), values);
}

/** @param {string} dir */
function folderKey(dir) {
  return dir.replace(/[\\/]+$/, "");
}

/* A recording's file relative to the folder it was requested in: `n\FL,FR.wav`
   when the service put it in a subfolder because the folder already held an
   earlier recording, otherwise the file name. */
/** @param {string} path @param {string} dir */
function relativeRecordPath(path, dir) {
  const file = String(path || "");
  const root = folderKey(String(dir || "")).replace(/\\/g, "/");
  return root && file.replace(/\\/g, "/").startsWith(`${root}/`)
    ? file.slice(root.length + 1)
    : file.split(/[\\/]/).pop() || "";
}

/* Keep each speaker's latest measurement per requested recording folder for
   this session (the takes may sit in a subfolder of it); a retake replaces the
   speaker's earlier entry. */
/** @param {RecordingResult} result */
function rememberMeasurements(result) {
  if (result.mode !== "speakers" || !result.measurements?.length) return;
  const dir = folderKey(result.record_dir || String(result.record_path || "").replace(/[\\/][^\\/]*$/, ""));
  const fresh = new Set(result.measurements.map((item) => item.speaker));
  state.takeMeasurements[dir] = [
    ...(state.takeMeasurements[dir] || []).filter((item) => !fresh.has(item.speaker)),
    ...result.measurements.map((item) => ({ ...item, lf_ild_db: item.lf_ild_db ?? null, direct: item.direct ?? null })),
  ];
}

function renderGuided() {
  const info = guidedSetup();
  $("gd-body").hidden = !info;
  if (!info) return;
  const takes = guidedTakes();
  const index = Math.max(0, Math.min(state.guided.index, takes.length - 1));
  state.guided.index = index;
  const results = state.guided.results;
  $("gd-steps").replaceChildren(...takes.map((take, i) => {
    const item = document.createElement("li");
    const button = document.createElement("button");
    const status = results[guidedKey(i)]?.status || "todo";
    button.type = "button";
    button.className = "gd-step mono";
    button.dataset.status = status;
    button.title = t(`guided_status_${status}`);
    if (i === index) button.setAttribute("aria-current", "step");
    button.textContent = `${i + 1} ${take.speakers.join(",")}`;
    button.addEventListener("click", () => showGuidedStep(i));
    item.append(button);
    return item;
  }));
  const take = takes[index];
  if (!take) return;
  const setup = val("gd-setup");
  $("gd-step-label").textContent = fmt(t("label_guided_step"), { index: index + 1, total: takes.length });
  $("gd-turn").textContent = guidedTurnText(setup, take);
  const ear = guidedEarText(setup, take);
  $("gd-ear").textContent = ear;
  $("gd-ear").hidden = !ear;
  $("gd-plays").textContent = take.sources
    .map((source) => `${source.speaker} (${guidedDirection(source.azimuth)})`).join(", ");
  void showGuidedFile(take, index);
  drawGuidedDiagram(info, take);
  const result = results[guidedKey(index)];
  /** @param {string} severity @param {string} text */
  const line = (severity, text) => {
    const item = document.createElement("li");
    item.className = "gd-check";
    item.dataset.severity = severity;
    item.textContent = text;
    return item;
  };
  const lines = !result ? []
    : result.checks.length ? result.checks.map((check) => line(check.severity, checkMessage(check)))
    : [line("ok", t("guided_checks_ok"))];
  if (takes.every((_, i) => ["ok", "warning"].includes(results[guidedKey(i)]?.status || ""))) {
    lines.push(line("done", t("guided_all_done")));
  }
  $("gd-checks").replaceChildren(...lines);
  el("btn-gd-prev", HTMLButtonElement).disabled = index === 0;
  el("btn-gd-next", HTMLButtonElement).disabled = index >= takes.length - 1;
}

/* The step's file, then where the service will put it: a subfolder (n, nn…)
   once the folder already holds an earlier recording of this session's names. */
/** @param {GuidedTake} take @param {number} index */
async function showGuidedFile(take, index) {
  const node = $("gd-file");
  const speakers = take.speakers.join(",");
  node.textContent = fmt(t("label_record_resolved_path"), { path: `${speakers}.wav` });
  const recordDir = val("rf-record-dir");
  if (!recordDir || !window.pywebview) return;
  const key = guidedKey(index);
  const response = await api().resolve_recording_paths(recordDir, null, "speakers",
    { mode: "default", speakers, tracks: take.tracks });
  if (!response.ok || key !== guidedKey(state.guided.index)) return;
  node.textContent = fmt(t("label_record_resolved_path"), {
    path: relativeRecordPath(response.data.record_path, recordDir),
  });
}

/** @param {number} index */
function showGuidedStep(index) {
  stopGuidedCountdown();
  state.guided.index = index;
  renderGuided();
}

/* The sweep starts after a countdown: the listener needs time to turn,
   and after a large turn the screen is behind them. */
function recordGuidedStep() {
  const take = guidedTakes()[state.guided.index];
  if (!take || state.guided.timer !== undefined) return;
  const seconds = Number(el("guided-card", HTMLElement).dataset.countdown ?? GUIDED_COUNTDOWN);
  state.guided.remaining = Number.isFinite(seconds) ? seconds : GUIDED_COUNTDOWN;
  tickGuidedCountdown(take);
}

/** @param {GuidedTake} take */
function tickGuidedCountdown(take) {
  if (state.guided.remaining <= 0) {
    stopGuidedCountdown();
    startGuidedRecording(take);
    return;
  }
  const node = $("gd-countdown");
  node.hidden = false;
  node.textContent = fmt(t("guided_countdown"), { seconds: state.guided.remaining });
  $("btn-gd-record").hidden = true;
  $("btn-gd-cancel").hidden = false;
  state.guided.remaining -= 1;
  state.guided.timer = window.setTimeout(() => tickGuidedCountdown(take), 1000);
}

function stopGuidedCountdown() {
  window.clearTimeout(state.guided.timer);
  state.guided.timer = undefined;
  $("gd-countdown").hidden = true;
  $("btn-gd-record").hidden = false;
  $("btn-gd-cancel").hidden = true;
}

/** @param {GuidedTake} take */
async function startGuidedRecording(take) {
  const payload = gatherRecordingPayload("speakers");
  const custom = sweepSourceMode() === "custom" ? gatherSweepPayload() : null;
  /** @type {SweepRequest} */
  const sweep = { mode: custom ? "custom" : "default", speakers: take.speakers.join(","), tracks: take.tracks };
  if (custom) Object.assign(sweep, { fs: custom.fs, duration: custom.duration });
  delete payload.play_path;
  Object.assign(payload, { sweep, force_channels: false, channels: 2, append: false,
    expected_azimuths: Object.fromEntries(take.sources.map((source) => [source.speaker, source.azimuth])) });
  // Another recording started during the countdown: this step did not start,
  // and that recording's result must not be taken for it.
  if (state.startPending || state.jobId) return;
  state.guided.pending = guidedKey(state.guided.index);
  await begin((request) => api().start_recording(request), payload, "recording");
  if (!state.jobId) state.guided.pending = null;
}

/* A clean step moves on to the next step not yet recorded; a warned one stays
   so its warnings can be read and the step recorded again. */
/** @param {JobOf<"recording">} job */
function finishGuidedStep(job) {
  const key = state.guided.pending;
  state.guided.pending = null;
  if (!key) return;
  if (job.status === "succeeded" && job.result) {
    const checks = job.result.checks || [];
    const status = checks.some((check) => check.severity === "error") ? "error" : checks.length ? "warning" : "ok";
    state.guided.results[key] = { status, checks };
    if (status === "ok" && key === guidedKey(state.guided.index)) {
      const next = guidedTakes().findIndex((_, i) => i > state.guided.index && !state.guided.results[guidedKey(i)]);
      if (next >= 0) state.guided.index = next;
    }
  }
  renderGuided();
}
