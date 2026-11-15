// @ts-check
// Measurements from a file (REW, Dirac Live) and the single BRIR recording file.
"use strict";

/* ------------------------------------------------ measurements from a file
   ADR 0010 and 0011: a REW .mdat or Dirac Live .liveproject can replace the
   folder's room-*.wav files (room), its speaker recordings (brir) or its
   headphones.wav (headphones). inspect_room_measurements reports what the
   file holds and its automatic assignment for the purpose; the screen starts
   from that assignment, the user may change it, and the job receives the
   screen's assignment as <purpose>_measurement_map.
   - room: REW rows are measurements (slot and ear position each); Dirac rows
     are the project's speakers, and the two ear positions apply to all.
   - brir: the same table, but a REW measurement is one ear, every speaker in
     use needs both ears and FL must be there; Dirac measures the two ears at
     two positions.
   - headphones: two selects pick the left earcup at the left ear and the
     right earcup at the right ear among the file's measurements. */

/** @type {Record<MeasurementPurpose, { prefix: string; source: string; hint: string;
 *   field: "room_measurement_file" | "brir_measurement_file" | "headphone_measurement_file";
 *   map: "room_measurement_map" | "brir_measurement_map" | "headphone_measurement_map" }>} */
const MFILE = {
  room: { prefix: "bf-room-file", source: "bf-room-source", hint: "hint_room_file",
    field: "room_measurement_file", map: "room_measurement_map" },
  brir: { prefix: "bf-brir-file", source: "bf-brir-source", hint: "hint_brir_file",
    field: "brir_measurement_file", map: "brir_measurement_map" },
  headphones: { prefix: "bf-hp-file", source: "bf-hp-source", hint: "hint_headphone_file",
    field: "headphone_measurement_file", map: "headphone_measurement_map" },
};
/** @type {MeasurementPurpose[]} */
const MFILE_PURPOSES = ["brir", "room", "headphones"];

/** @param {MeasurementPurpose} purpose */
function mfileMode(purpose) {
  return val(MFILE[purpose].source) === "file";
}

/** Whether the job would read the purpose's file: room and headphones only
 * while their section is on. @param {MeasurementPurpose} purpose */
function mfileActive(purpose) {
  if (!mfileMode(purpose)) return false;
  if (purpose === "room") return isOpen("dis-room");
  if (purpose === "headphones") return isOpen("dis-headphone");
  return true;
}

/** @returns {string[]} */
function mfileSlotCodes() {
  const names = state.sweepDefaults?.speaker_names;
  return names && names.length ? names : DECAY_CHANNELS;
}

/** @param {MeasurementPurpose} purpose */
function scheduleMfileInspection(purpose, delay = 250) {
  const file = state.mfiles[purpose];
  window.clearTimeout(file.timer);
  const revision = ++file.revision;
  const path = val(MFILE[purpose].prefix);
  // Another path drops the previous file's table at once, so nothing can be
  // sent with an assignment that belongs to a different file.
  if (path !== file.input) {
    file.data = null;
    file.error = null;
    file.input = "";
    renderMfile(purpose);
  }
  if (!mfileMode(purpose) || !path || !window.pywebview) {
    file.data = null;
    file.error = null;
    renderMfile(purpose);
    return;
  }
  file.timer = window.setTimeout(async () => {
    const response = await callApi(() => api().inspect_room_measurements({ path, purpose }));
    if (revision !== file.revision) return;
    file.data = response.ok ? response.data : null;
    file.error = response.ok ? null : response.error;
    file.input = path;
    if (response.ok && file.choices?.path !== response.data.path) {
      file.choices = mfileChoicesFrom(purpose, response.data);
    }
    renderMfile(purpose);
  }, delay);
}

/** The screen's starting point: the file's automatic assignment.
 * @param {MeasurementPurpose} purpose @param {RoomFileInspection} inspection @returns {RoomFileChoices} */
function mfileChoicesFrom(purpose, inspection) {
  /** @type {RoomFileChoices["rows"]} */
  const rows = {};
  const dirac = inspection.format === "dirac";
  const first = inspection.positions[0] || 1;
  let left = first;
  // A binaural Dirac measurement has each ear at its own position.
  let right = purpose === "room" ? first : inspection.positions[1] || first;
  let hpLeft = "";
  let hpRight = "";
  for (const entry of inspection.mapping) {
    if (purpose === "headphones") {
      if (entry.side === "left") hpLeft = entry.id;
      else hpRight = entry.id;
      continue;
    }
    const [speaker, position] = entry.id.split("@");
    const key = dirac ? speaker : entry.id;
    const row = rows[key];
    if (!row) rows[key] = { slot: entry.slot, side: entry.side };
    else if (row.slot === entry.slot && row.side !== entry.side) row.side = "both";
    if (dirac) {
      if (entry.side === "left") left = Number(position);
      else right = Number(position);
    }
  }
  if (dirac) for (const row of Object.values(rows)) row.side = "both";
  return { path: inspection.path, rows, left, right, hpLeft, hpRight };
}

/** The assignment the screen describes, the row keys to mark (a slot and ear
 * claimed twice, a BRIR speaker with one ear, an earcup pair using one
 * measurement), the first problem that keeps the job from starting, and the
 * map string for the job.
 * @param {MeasurementPurpose} purpose
 * @returns {{ map: string; flagged: Set<string>; problem: string | null }} */
function mfileAssignment(purpose) {
  const { data: inspection, choices } = state.mfiles[purpose];
  if (!inspection || !choices) return { map: "", flagged: new Set(), problem: null };
  if (purpose === "headphones") {
    const entries = [];
    if (choices.hpLeft) entries.push(`left=${choices.hpLeft}`);
    if (choices.hpRight) entries.push(`right=${choices.hpRight}`);
    const same = Boolean(choices.hpLeft) && choices.hpLeft === choices.hpRight;
    const problem = !choices.hpLeft || !choices.hpRight ? t("headphone_file_incomplete")
      : same ? t("headphone_file_same") : null;
    return { map: entries.join(","), flagged: new Set(same ? ["hp-left", "hp-right"] : []), problem };
  }
  const dirac = inspection.format === "dirac";
  /** @type {string[]} */
  const entries = [];
  /** @type {Map<string, string[]>} */
  const claims = new Map();
  /** @type {Map<string, { sides: Set<string>; keys: string[] }>} */
  const ears = new Map();
  // A Dirac matrix may lack a speaker at a position (not measured there).
  const measured = new Set(inspection.measurements.filter((m) => !m.skipped).map((m) => m.id));
  /** @type {string[]} */
  const unmeasured = [];
  /** @type {string[]} */
  const unmeasuredKeys = [];
  /** @param {string} key @param {string} slot @param {"left" | "right"} side */
  const claim = (key, slot, side) => {
    claims.set(`${slot}.${side}`, [...(claims.get(`${slot}.${side}`) || []), key]);
    const ear = ears.get(slot) || { sides: new Set(), keys: [] };
    ear.sides.add(side);
    ear.keys.push(key);
    ears.set(slot, ear);
  };
  for (const [key, row] of Object.entries(choices.rows)) {
    if (!row.slot) continue;
    if (dirac) {
      const leftId = `${key}@${choices.left}`;
      const rightId = `${key}@${choices.right}`;
      entries.push(purpose === "room" && leftId === rightId ? `${row.slot}=${leftId}`
        : `${row.slot}.left=${leftId},${row.slot}.right=${rightId}`);
      if (!measured.has(leftId) || !measured.has(rightId)) {
        unmeasured.push(row.slot);
        unmeasuredKeys.push(key);
      }
      claim(key, row.slot, "left");
      claim(key, row.slot, "right");
    } else {
      // A BRIR measurement is always one ear.
      const side = purpose === "brir" && row.side === "both" ? "left" : row.side;
      entries.push(side === "both" ? `${row.slot}=${key}` : `${row.slot}.${side}=${key}`);
      if (side !== "right") claim(key, row.slot, "left");
      if (side !== "left") claim(key, row.slot, "right");
    }
  }
  const flagged = new Set([...claims.values()].filter((keys) => keys.length > 1).flat());
  /** @type {string | null} */
  let problem = flagged.size ? t("room_file_duplicate") : entries.length ? null : t("room_file_none_assigned");
  if (!problem && unmeasured.length) {
    problem = fmt(t("room_file_not_measured"), { speakers: unmeasured.join(", ") });
    for (const key of unmeasuredKeys) flagged.add(key);
  }
  if (!problem && purpose === "brir") {
    const incomplete = [...ears].filter(([, ear]) => ear.sides.size < 2);
    if (dirac && inspection.positions.length < 2) problem = t("brir_file_one_position");
    else if (dirac && choices.left === choices.right) problem = t("brir_file_same_position");
    else if (incomplete.length) {
      problem = fmt(t("brir_file_incomplete"), { speakers: incomplete.map(([slot]) => slot).join(", ") });
      for (const [, ear] of incomplete) for (const key of ear.keys) flagged.add(key);
    } else if (!ears.has("FL")) problem = t("brir_file_needs_fl");
  }
  return { map: entries.join(","), flagged, problem };
}

/** Whether the job cannot use the file as the screen stands: no file, one
 * whose inspection has not answered for the path in the field yet, an
 * unreadable one, or an assignment with a problem.
 * @param {MeasurementPurpose} purpose */
function mfileBlocked(purpose) {
  const file = state.mfiles[purpose];
  const path = val(MFILE[purpose].prefix);
  if (!path || file.error) return true;
  if (!file.data || file.input !== path) return true;
  return mfileAssignment(purpose).problem !== null;
}

/** The job fields of a purpose whose source is a file.
 * @param {ProcessingRequest} args @param {MeasurementPurpose} purpose */
function addMfileArgs(args, purpose) {
  const ids = MFILE[purpose];
  const path = val(ids.prefix);
  const file = state.mfiles[purpose];
  const { map } = mfileAssignment(purpose);
  args[ids.field] = path || null;
  args[ids.map] = file.data && file.input === path && map ? map : null;
}

/** @param {string} value @param {string} label */
function mfileOption(value, label) {
  return new Option(label, value);
}

/** @param {MeasurementPurpose} purpose @param {string} key @param {string} labelText @param {boolean} disabled */
function mfileSlotSelect(purpose, key, labelText, disabled) {
  const select = document.createElement("select");
  select.dataset.roomKey = key;
  select.dataset.roomField = "slot";
  select.setAttribute("aria-label", `${labelText}: ${t("room_file_col_speaker")}`);
  select.add(mfileOption("", t("option_room_file_unused")));
  for (const code of mfileSlotCodes()) select.add(mfileOption(code, code));
  // A measured subwoofer is an LFE track of the channel-order files (BRIR, ADR 0016); a room
  // measurement's subwoofer makes or corrects one (ADR 0017). Headphone measurements have none.
  if (purpose === "brir" || purpose === "room") for (const code of ["LFE", "LFE2"]) select.add(mfileOption(code, code));
  select.value = state.mfiles[purpose].choices?.rows[key]?.slot || "";
  select.disabled = disabled;
  return select;
}

/** @param {MeasurementPurpose} purpose @param {RoomFileMeasurement} measurement */
function renderMfileRewRow(purpose, measurement) {
  const row = document.createElement("tr");
  row.dataset.roomKey = measurement.id;
  const name = document.createElement("td");
  const title = document.createElement("span");
  title.className = "room-map-name mono";
  title.textContent = measurement.name;
  name.append(title);
  if (measurement.skipped) {
    row.dataset.state = "skipped";
    const note = document.createElement("span");
    note.className = "room-map-note";
    note.textContent = t("room_file_skipped");
    name.append(note);
  }
  const slot = document.createElement("td");
  slot.append(mfileSlotSelect(purpose, measurement.id, measurement.name, Boolean(measurement.skipped)));
  const ear = document.createElement("td");
  const side = document.createElement("select");
  side.dataset.roomKey = measurement.id;
  side.dataset.roomField = "side";
  side.setAttribute("aria-label", `${measurement.name}: ${t("room_file_col_ear")}`);
  // A room measurement may stand for both ear positions; a BRIR one is one ear.
  if (purpose === "room") side.add(mfileOption("both", t("option_room_file_both")));
  side.add(mfileOption("left", t("option_room_file_left")));
  side.add(mfileOption("right", t("option_room_file_right")));
  const chosen = state.mfiles[purpose].choices?.rows[measurement.id]?.side;
  side.value = purpose === "room" ? chosen || "both" : chosen === "right" ? "right" : "left";
  side.disabled = Boolean(measurement.skipped);
  ear.append(side);
  row.append(name, slot, ear);
  return row;
}

/** @param {MeasurementPurpose} purpose @param {RoomFileSpeaker} speaker */
function renderMfileDiracRow(purpose, speaker) {
  const key = String(speaker.index);
  const row = document.createElement("tr");
  row.dataset.roomKey = key;
  const name = document.createElement("td");
  const title = document.createElement("span");
  title.className = "room-map-name";
  title.textContent = speaker.name;
  name.append(title);
  if (speaker.subwoofer) {
    const note = document.createElement("span");
    note.className = "room-map-note";
    note.textContent = purpose === "brir" ? t("brir_file_subwoofer") : t("room_file_subwoofer");
    name.append(note);
  }
  const slot = document.createElement("td");
  slot.append(mfileSlotSelect(purpose, key, speaker.name, false));
  row.append(name, slot);
  return row;
}

/** @param {MeasurementPurpose} purpose @param {string} labelText @param {"left" | "right"} ear @param {number[]} positions */
function renderMfilePosition(purpose, labelText, ear, positions) {
  const label = document.createElement("label");
  const text = document.createElement("span");
  text.textContent = labelText;
  const select = document.createElement("select");
  select.dataset.roomField = ear;
  for (const position of positions) {
    select.add(mfileOption(String(position), fmt(t("option_room_file_position"), { n: position })));
  }
  select.value = String(state.mfiles[purpose].choices?.[ear] || positions[0] || 1);
  label.append(text, select);
  return label;
}

/** The two earcup selects of a headphone measurement file.
 * @param {RoomFileInspection} inspection */
function renderHeadphonePicks(inspection) {
  const picks = document.createElement("div");
  picks.className = "room-file-picks";
  const choices = state.mfiles.headphones.choices;
  for (const ear of /** @type {const} */ (["left", "right"])) {
    const field = `hp-${ear}`;
    const label = document.createElement("label");
    label.dataset.roomKey = field;
    const text = document.createElement("span");
    text.textContent = t(ear === "left" ? "label_headphone_file_left" : "label_headphone_file_right");
    const select = document.createElement("select");
    select.dataset.roomField = field;
    select.add(mfileOption("", t("option_headphone_file_none")));
    for (const measurement of inspection.measurements) {
      if (measurement.skipped) continue;
      select.add(mfileOption(measurement.id, inspection.format === "dirac"
        ? fmt(t("option_measurement_at_position"), { name: measurement.name, n: measurement.position ?? "" })
        : measurement.name));
    }
    select.value = (ear === "left" ? choices?.hpLeft : choices?.hpRight) || "";
    label.append(text, select);
    picks.append(label);
  }
  return picks;
}

/** @param {string[]} headings */
function mfileTableHead(headings) {
  const head = document.createElement("thead");
  const row = document.createElement("tr");
  for (const heading of headings) {
    const cell = document.createElement("th");
    cell.scope = "col";
    cell.textContent = heading;
    row.append(cell);
  }
  head.append(row);
  return head;
}

/* ADR 0016: preserve playback order and case. X alone may repeat; the first
   invalid entry wins over the missing-FL check, as in the service. Counts
   describe the valid entries (LFE channels are not speakers). */
/** @param {string} text
 * @returns {{ entries: string[]; speakers: number; lfe: number; skipped: number; problem: string | null }} */
function parseRecordingSpeakers(text) {
  const entries = text.trim() ? text.split(",").map((entry) => entry.trim()) : [];
  const slots = new Set(mfileSlotCodes());
  const seen = new Set();
  let speakers = 0, lfe = 0, skipped = 0;
  /** @type {string | null} */
  let problem = entries.length ? null : t("brir_recording_choose");
  for (const entry of entries) {
    if (!entry) { problem = t("brir_recording_error_empty"); break; }
    if (entry === "X") { skipped += 1; continue; }
    if (!slots.has(entry) && entry !== "LFE" && entry !== "LFE2") {
      problem = fmt(t("brir_recording_error_unknown"), { entry });
      break;
    }
    if (seen.has(entry)) {
      problem = fmt(t("brir_recording_error_duplicate"), { entry });
      break;
    }
    seen.add(entry);
    if (entry === "LFE" || entry === "LFE2") lfe += 1;
    else speakers += 1;
  }
  if (!problem && !seen.has("FL")) problem = t("brir_recording_error_needs_fl");
  return { entries, speakers, lfe, skipped, problem };
}

/** The first reason a single-recording job cannot start; inactive sources never block. */
function brirRecordingProblem() {
  if (val("bf-brir-source") !== "recording") return null;
  const text = val("bf-brir-recording-speakers");
  const { problem } = parseRecordingSpeakers(text);
  if (text && problem) return problem;
  if (!val("bf-brir-recording-file") || !text) return t("brir_recording_choose");
  if (val("bf-brir-recording-sweep-source") === "file" && !val("bf-brir-recording-sweep")) {
    return t("brir_recording_choose_sweep");
  }
  return null;
}

/* List errors take priority over missing files. Missing input is a neutral
   prompt, not an error; outside recording mode the line stays empty. */
function updateBrirRecordingStatus() {
  const active = val("bf-brir-source") === "recording";
  const text = val("bf-brir-recording-speakers");
  const parsed = parseRecordingSpeakers(text);
  const status = $("bf-brir-recording-status");
  status.dataset.state = !active ? "empty" : text && parsed.problem ? "error" : "ok";
  status.textContent = !active ? "" : brirRecordingProblem() || fmt(t("brir_recording_status"), parsed);
  updateStartControls(state.lastJob);
}

/* ADR 0016: the BRIR measurements' third source, one recording with a sweep
   per speaker. Its block shows for that source only, and the sweep file row
   only when the recording's sweep is another program's file. */
function updateBrirRecordingRows() {
  $("bf-brir-recording-block").hidden = val("bf-brir-source") !== "recording";
  $("bf-brir-recording-sweep-row").hidden = val("bf-brir-recording-sweep-source") !== "file";
  updateBrirRecordingStatus();
}

/** @param {MeasurementPurpose} purpose */
function renderMfile(purpose) {
  if (purpose === "brir") updateBrirRecordingRows();
  const ids = MFILE[purpose];
  const fileMode = mfileMode(purpose);
  $(`${ids.prefix}-block`).hidden = !fileMode;
  if (purpose === "headphones") $("bf-hp-folder-row").hidden = fileMode;
  const result = $(`${ids.prefix}-result`);
  result.replaceChildren();
  if (!fileMode) return;
  if (!val(ids.prefix)) {
    const line = document.createElement("p");
    line.className = "hint";
    line.textContent = t("room_file_choose");
    result.append(line);
    return;
  }
  const { data: inspection, error } = state.mfiles[purpose];
  if (error) {
    const line = document.createElement("p");
    line.className = "room-file-error";
    line.textContent = error.code === "FILE_NOT_FOUND" ? t("room_file_missing") : t("room_file_error");
    result.append(line);
    if (error.code !== "FILE_NOT_FOUND" && error.message) {
      const details = document.createElement("details");
      details.className = "room-file-details";
      const summary = document.createElement("summary");
      summary.textContent = t("room_file_error_details");
      const message = document.createElement("code");
      message.textContent = error.message;
      details.append(summary, message);
      result.append(details);
    }
    return;
  }
  if (!inspection) return;

  const summary = document.createElement("p");
  summary.className = "room-file-summary hint";
  const usable = inspection.measurements.filter((m) => !m.skipped).length;
  const text = document.createElement("span");
  if (inspection.format === "dirac") {
    summary.append(eqTag("Dirac Live"));
    text.textContent = fmt(t("room_file_summary_dirac"), {
      title: inspection.title || "", speakers: inspection.speakers.length,
      positions: inspection.positions.length, rate: inspection.sample_rate,
    });
  } else {
    summary.append(eqTag("REW"));
    text.textContent = fmt(t("room_file_summary_rew"), { count: usable, rate: inspection.sample_rate });
  }
  summary.append(text);
  result.append(summary);
  if (inspection.microphone) {
    // The project's calibration belongs to its measurement microphone, so
    // only room measurements use it (ADR 0011, point 6).
    const mic = document.createElement("p");
    mic.className = "hint";
    mic.textContent = purpose === "room" && inspection.mic_calibration
      ? fmt(t("room_file_microphone_calibrated"), { name: inspection.microphone })
      : fmt(t("room_file_microphone"), { name: inspection.microphone });
    result.append(mic);
  }

  if (purpose === "headphones") {
    result.append(renderHeadphonePicks(inspection));
  } else {
    const table = document.createElement("table");
    table.className = "room-map";
    const body = document.createElement("tbody");
    if (inspection.format === "dirac") {
      if (purpose === "brir") {
        const note = document.createElement("p");
        note.className = "hint";
        note.textContent = t("hint_brir_file_dirac");
        result.append(note);
      }
      const positions = document.createElement("div");
      positions.className = "room-file-positions";
      positions.append(
        renderMfilePosition(purpose, t("label_room_file_left_position"), "left", inspection.positions),
        renderMfilePosition(purpose, t("label_room_file_right_position"), "right", inspection.positions),
      );
      result.append(positions);
      table.append(mfileTableHead([t("room_file_col_project_speaker"), t("room_file_col_speaker")]));
      body.append(...inspection.speakers.map((speaker) => renderMfileDiracRow(purpose, speaker)));
    } else {
      table.append(mfileTableHead([t("room_file_col_measurement"), t("room_file_col_speaker"), t("room_file_col_ear")]));
      body.append(...inspection.measurements.map((measurement) => renderMfileRewRow(purpose, measurement)));
    }
    table.append(body);
    result.append(table);
  }

  const status = document.createElement("p");
  status.id = `${ids.prefix}-status`;
  result.append(status);
  updateMfileRows(purpose);
}

/** Row states after a change, without rebuilding the table (keeps focus).
 * @param {MeasurementPurpose} purpose */
function updateMfileRows(purpose) {
  const ids = MFILE[purpose];
  const status = document.getElementById(`${ids.prefix}-status`);
  if (!status) return;
  const { flagged, problem } = mfileAssignment(purpose);
  const marked = /** @type {NodeListOf<HTMLElement>} */ ($(`${ids.prefix}-result`).querySelectorAll("tbody tr, .room-file-picks label"));
  for (const row of marked) {
    const key = row.dataset.roomKey || "";
    if (row.dataset.state !== "skipped") row.dataset.state = flagged.has(key) ? "error" : "";
    const side = /** @type {HTMLSelectElement | null} */ (row.querySelector('select[data-room-field="side"]'));
    if (side && row.dataset.state !== "skipped") side.disabled = !state.mfiles[purpose].choices?.rows[key]?.slot;
  }
  status.className = problem ? "room-file-error" : "hint";
  status.textContent = problem || t(ids.hint);
}

/** @param {MeasurementPurpose} purpose @param {Event} event */
function onMfileChange(purpose, event) {
  const target = event.target;
  const choices = state.mfiles[purpose].choices;
  if (!(target instanceof HTMLSelectElement) || !choices) return;
  const field = target.dataset.roomField;
  if (field === "left" || field === "right") {
    choices[field] = Number(target.value);
  } else if (field === "hp-left") {
    choices.hpLeft = target.value;
  } else if (field === "hp-right") {
    choices.hpRight = target.value;
  } else if (field === "slot" || field === "side") {
    const key = target.dataset.roomKey || "";
    const row = choices.rows[key] || (choices.rows[key] = { slot: "", side: purpose === "room" ? "both" : "left" });
    if (field === "slot") row.slot = target.value;
    else row.side = /** @type {"both" | "left" | "right"} */ (target.value);
  }
  updateMfileRows(purpose);
}

/* ------------------------------------------------ wiring and the request */

/* The three blocks again, after the language changes. */
function renderMfiles() {
  for (const purpose of MFILE_PURPOSES) renderMfile(purpose);
}

/* The BRIR source (a measurement file or the single recording) and the
   headphone block while its section is on (a measurement file, or the
   folder's headphone recording when one is named). */
/** @param {ProcessingRequest} args */
function addMeasurementFileArgs(args) {
  if (mfileMode("brir")) addMfileArgs(args, "brir");
  if (val("bf-brir-source") === "recording") {
    args.brir_recording_file = val("bf-brir-recording-file");
    args.brir_recording_speakers = parseRecordingSpeakers(val("bf-brir-recording-speakers")).entries.join(",");
    if (val("bf-brir-recording-sweep-source") === "file") args.brir_recording_sweep = val("bf-brir-recording-sweep");
  }
  if (isOpen("dis-headphone")) {
    const headphoneFile = val("bf-headphone-file");
    if (mfileMode("headphones")) addMfileArgs(args, "headphones");
    else if (headphoneFile) args.headphone_compensation_file = headphoneFile;
  }
}

/* The single recording's rows and status, and the three measurement file blocks. */
function wireMfile() {
  // The source select also renders the file block (below); this one shows the sweep file row.
  el("bf-brir-recording-sweep-source", HTMLSelectElement).addEventListener("change", updateBrirRecordingRows);
  for (const id of ["bf-brir-recording-file", "bf-brir-recording-speakers", "bf-brir-recording-sweep"]) {
    el(id, HTMLInputElement).addEventListener("input", updateBrirRecordingStatus);
  }
  for (const purpose of MFILE_PURPOSES) {
    const ids = MFILE[purpose];
    el(ids.source, HTMLSelectElement).addEventListener("change", () => {
      renderMfile(purpose);
      scheduleMfileInspection(purpose, 0);
      scheduleTestSignalCheck(0);
    });
    el(ids.prefix, HTMLInputElement).addEventListener("input", () => scheduleMfileInspection(purpose));
    $(`${ids.prefix}-result`).addEventListener("change", (event) => onMfileChange(purpose, event));
  }
}
