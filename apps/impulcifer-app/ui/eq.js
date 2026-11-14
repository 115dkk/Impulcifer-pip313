// @ts-check
// Custom EQ: the three slots, their inspection and the preview.
"use strict";

/* ------------------------------------------------------------ custom EQ
   Three slots (both ears, left ear, right ear). Each follows the recording
   folder, names a file anywhere (read in place by the service), or is off.
   inspect_eq reports what every slot resolves to, which file feeds which
   ear, and the resulting curves for the preview. */

/** @type {EqSlotName[]} */
const EQ_SLOTS = ["both", "left", "right"];
/** @type {Record<EqSlotName, "eq_file" | "eq_left_file" | "eq_right_file">} */
const EQ_FIELDS = { both: "eq_file", left: "eq_left_file", right: "eq_right_file" };
const SVG_NS = "http://www.w3.org/2000/svg";

/** Request fields: a folder slot is omitted, a chosen file is its path, off is false. */
function eqRequestFields() {
  /** @type {Pick<ProcessingRequest, "eq_file" | "eq_left_file" | "eq_right_file">} */
  const fields = {};
  for (const slot of EQ_SLOTS) {
    const choice = state.eqChoices[slot];
    if (choice.mode === "file") fields[EQ_FIELDS[slot]] = choice.path;
    else if (choice.mode === "off") fields[EQ_FIELDS[slot]] = false;
  }
  return fields;
}

function scheduleEqInspection(delay = 250) {
  window.clearTimeout(state.eqTimer);
  const revision = ++state.eqRevision;
  const dir = val("bf-dir-path");
  if (!dir || !window.pywebview) {
    state.eqInspection = null;
    state.eqError = null;
    renderEq();
    return;
  }
  state.eqTimer = window.setTimeout(async () => {
    const response = await callApi(() => api().inspect_eq({ dir_path: dir, ...eqRequestFields() }));
    if (revision !== state.eqRevision) return;
    state.eqInspection = response.ok ? response.data : null;
    state.eqError = response.ok ? null : response.error;
    renderEq();
  }, delay);
}

/** @param {EqSlotName} slot @param {EqChoice} choice */
function setEqChoice(slot, choice) {
  state.eqChoices[slot] = choice;
  renderEq();
  scheduleEqInspection(0);
}

/** @param {EqSlotName} slot */
async function chooseEqFile(slot) {
  const response = await api().select_file("text");
  if (response.ok && response.data.path) setEqChoice(slot, { mode: "file", path: response.data.path });
}

/** @param {string} text @param {string} [className] */
function eqTag(text, className = "") {
  const tag = document.createElement("span");
  tag.className = `chip eq-tag ${className}`.trim();
  tag.textContent = text;
  return tag;
}

/** @param {number} db */
function fmtDb(db) {
  const rounded = Math.round(db * 10) / 10;
  return `${rounded > 0 ? "+" : rounded < 0 ? "−" : ""}${Math.abs(rounded)}`;
}

/** @param {EqSlotName} slot */
function renderEqSlot(slot) {
  const choice = state.eqChoices[slot];
  const info = state.eqInspection?.slots.find((item) => item.slot === slot) || null;
  const row = document.createElement("div");
  row.className = "eq-slot";
  row.setAttribute("role", "listitem");
  row.dataset.source = choice.mode;

  const label = document.createElement("span");
  label.className = "eq-slot-label";
  label.textContent = t(`eq_slot_${slot}`);

  const body = document.createElement("div");
  body.className = "eq-slot-file";
  const name = document.createElement("span");
  name.className = "eq-file-name mono";
  const folderDefault = info?.folder_default || (slot === "both" ? "eq.csv" : `eq-${slot}.csv`);
  const txtDefault = folderDefault.replace(/\.csv$/, ".txt");
  const tags = document.createElement("span");
  tags.className = "eq-tags";
  /** @type {HTMLElement[]} */
  const details = [];

  if (choice.mode === "off") {
    row.dataset.state = "off";
    name.textContent = t("eq_slot_off_detail");
  } else if (choice.mode === "folder" && !info?.name) {
    row.dataset.state = "missing";
    name.textContent = fmt(t("eq_folder_missing"), { csv: folderDefault, txt: txtDefault });
  } else {
    const fileName = info?.name || (choice.mode === "file" ? choice.path.split(/[\\/]/).pop() || choice.path : "");
    row.dataset.state = info?.error ? "error" : "found";
    name.textContent = fileName;
    if (info?.path) name.title = info.path;
    tags.append(eqTag(t(choice.mode === "file" ? "eq_source_file" : "eq_source_folder")));
    if (info?.format) tags.append(eqTag(t(`eq_format_${info.format}`), `eq-format-${info.format}`));
    if (info?.channels === "split") tags.append(eqTag(t("eq_channels_split")));
    if (info?.eqapo) {
      const [left, right] = info.eqapo.preamp_db;
      const parts = [];
      if (left || right) {
        parts.push(left === right
          ? fmt(t("eq_preamp"), { value: fmtDb(left) })
          : fmt(t("eq_preamp_split"), { left: fmtDb(left), right: fmtDb(right) }));
      }
      if (info.eqapo.bypassed) parts.push(fmt(t("eq_bypassed"), { count: info.eqapo.bypassed }));
      if (parts.length) {
        const line = document.createElement("span");
        line.className = "hint";
        line.textContent = parts.join(" · ");
        details.push(line);
      }
    }
    if (info?.error) {
      const error = document.createElement("span");
      error.className = "eq-error";
      error.textContent = fmt(t("eq_error"), { message: info.error });
      details.push(error);
    }
  }
  if (choice.mode === "folder" && info?.ignored.length && info.name) {
    const ignored = document.createElement("span");
    ignored.className = "hint";
    ignored.textContent = fmt(t("eq_ignored"), { names: info.ignored.join(", "), name: info.name });
    details.push(ignored);
  }
  const head = document.createElement("span");
  head.className = "eq-file-head";
  head.append(name, tags);
  body.append(head, ...details);

  const actions = document.createElement("div");
  actions.className = "eq-slot-actions";
  /** @param {string} key @param {() => void} handler */
  const action = (key, handler) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn-link";
    button.textContent = t(key);
    button.addEventListener("click", handler);
    actions.append(button);
  };
  action("button_eq_choose", () => { chooseEqFile(slot); });
  if (choice.mode !== "folder") action("button_eq_use_folder", () => setEqChoice(slot, { mode: "folder" }));
  if (choice.mode !== "off") action("button_eq_off", () => setEqChoice(slot, { mode: "off" }));

  row.append(label, body, actions);
  return row;
}

/** @param {"left" | "right"} ear */
function renderEqEar(ear) {
  const source = state.eqInspection?.ears[ear] || null;
  const item = document.createElement("span");
  item.className = "eq-ear";
  item.dataset.ear = ear;
  const swatch = document.createElement("i");
  swatch.className = "eq-swatch";
  const label = document.createElement("b");
  label.textContent = t(`eq_slot_${ear}`);
  const text = document.createElement("span");
  if (!source) {
    text.textContent = t("eq_ear_none");
    item.dataset.empty = "true";
  } else {
    const slot = state.eqInspection?.slots.find((info) => info.slot === source.slot);
    const file = slot?.name || "";
    text.textContent = source.channel === "both" ? file : `${file} · ${t(`eq_channel_${source.channel}`)}`;
  }
  item.append(swatch, label, text);
  return item;
}

/** Log-frequency preview of the per-ear curves; one line when both ears match. */
function renderEqPreview() {
  const svg = $("bf-eq-preview");
  const note = $("bf-eq-preview-label");
  svg.replaceChildren();
  const inspection = state.eqInspection;
  const curves = inspection?.curves;
  const series = /** @type {{ear: string, values: number[]}[]} */ ([]);
  if (curves?.left) series.push({ ear: "left", values: curves.left });
  if (curves?.right) {
    if (curves.left && curves.left.every((v, i) => v === curves.right?.[i])) series[0].ear = "both";
    else series.push({ ear: "right", values: curves.right });
  }
  // SVG elements have no `hidden` property; the attribute is what CSS sees.
  svg.toggleAttribute("hidden", !series.length);
  $("bf-eq-ears").hidden = Boolean(inspection?.blocked);
  $("bf-eq-ears").dataset.shared = String(series.length === 1 && series[0].ear === "both");
  note.classList.toggle("eq-error", Boolean(inspection?.blocked));
  if (!inspection) {
    note.textContent = state.eqError ? errorText({ ok: false, error: state.eqError }) : t("eq_dir_needed");
    return;
  }
  if (inspection.blocked) {
    note.textContent = t("eq_preview_blocked");
    return;
  }
  if (!series.length || !curves) {
    note.textContent = t("eq_preview_empty");
    return;
  }
  note.textContent = t("eq_preview_note");
  // Drawn in CSS pixels of the current width so text never scales.
  const width = Math.max(320, Math.round(svg.getBoundingClientRect().width) || 640);
  const [height, left, right, top, bottom] = [180, 40, 12, 10, 22];
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  const fMin = Math.log10(20);
  const fSpan = Math.log10(20000) - fMin;
  /** @param {number} f */
  const x = (f) => left + ((Math.log10(f) - fMin) / fSpan) * (width - left - right);
  const peak = Math.max(...series.flatMap((s) => s.values.map(Math.abs)), 0);
  const step = peak <= 6 ? 3 : peak <= 12 ? 6 : peak <= 24 ? 6 : 12;
  const span = Math.max(6, Math.ceil(peak / step) * step);
  /** @param {number} db */
  const y = (db) => top + ((span - db) / (2 * span)) * (height - top - bottom);
  /** @param {string} tag @param {Record<string, string | number>} attrs @param {string} [text] */
  const add = (tag, attrs, text) => {
    const node = document.createElementNS(SVG_NS, tag);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, String(value));
    if (text !== undefined) node.textContent = text;
    svg.append(node);
    return node;
  };
  for (const f of [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000]) {
    add("line", { class: "eq-grid", x1: x(f), x2: x(f), y1: top, y2: height - bottom });
    if ([20, 100, 1000, 10000, 20000].includes(f)) {
      add("text", { class: "eq-axis", x: x(f), y: height - 6, "text-anchor": f === 20 ? "start" : f === 20000 ? "end" : "middle" },
        f >= 1000 ? `${f / 1000}k` : String(f));
    }
  }
  for (let db = -span; db <= span; db += step) {
    add("line", { class: db === 0 ? "eq-zero" : "eq-grid", x1: left, x2: width - right, y1: y(db), y2: y(db) });
    add("text", { class: "eq-axis", x: left - 6, y: y(db) + 4, "text-anchor": "end" }, db > 0 ? `+${db}` : String(db));
  }
  for (const { ear, values } of series) {
    const points = values.map((v, i) => `${x(curves.frequency[i]).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
    add("polyline", { class: `eq-curve eq-curve-${ear}`, points });
  }
}

function renderEq() {
  $("bf-eq-slots").replaceChildren(...EQ_SLOTS.map(renderEqSlot));
  $("bf-eq-ears").replaceChildren(renderEqEar("left"), renderEqEar("right"));
  renderEqPreview();
}
