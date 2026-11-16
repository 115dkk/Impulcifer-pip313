// @ts-check
// The Android page's choice lists: a select opens its options as a sheet from the bottom, titled with its label.
"use strict";

/* Android's web view answers a press on a <select> with a dialog of its own:
   the system's grey list, the same in every app and in none of this page's
   colours. On a page with #option-sheet the select stays what the page reads
   and writes (its value, its options, its events) and still shows the chosen
   option; only the list it opens is the page's. The press no longer reaches
   the web view's handler (it opens its dialog from mousedown's default
   action), and the tap opens the options in a sheet. A chosen option sets
   the select and sends the `input` and `change` events a choice in the
   system's list would send; the same option again changes nothing. The
   scrim, Escape and Android's back close the sheet without a choice.
   The listeners sit on the document, so the selects made later (the
   measurement tables) open the sheet as well.

   Focus moves into the sheet and back to the select by script. The browser
   draws the keyboard's ring on such focus unless it has just seen a focus by
   pointer, and the press on a select never focuses it (its default action is
   cancelled). So the sheet notes how it was opened (data-by: "press" or
   "key") and the select it returns to after a press carries
   data-quiet-focus until it loses the focus or a key is pressed on it; the
   styles draw the ring only for the keyboard. */

/** The select whose options the sheet shows. @type {HTMLSelectElement | null} */
let optionSheetSelect = null;

/** The keys that open a select's list. */
const OPTION_SHEET_KEYS = new Set(["Enter", " ", "ArrowDown", "ArrowUp", "F4"]);

/** The select a press or a key landed on. @param {EventTarget | null} target */
function pressedSelect(target) {
  const select = target instanceof Element ? target.closest("select") : null;
  return select && !select.multiple && select.size <= 1 ? select : null;
}

/** What names a select: the words it gives assistive technology, or its label.
 * @param {HTMLSelectElement} select */
function optionSheetLabel(select) {
  const named = select.getAttribute("aria-label");
  if (named) return named;
  const parts = (select.getAttribute("aria-labelledby") || "").split(/\s+/).filter(Boolean)
    .map((id) => bareLabel((document.getElementById(id)?.textContent || "").trim())).filter(Boolean);
  if (parts.length > 0) return parts.join(" ");
  const label = select.labels[0];
  if (!label) return "";
  // A label that wraps its select names it with its first element; the rest is the select itself.
  return (label.contains(select) ? label.firstElementChild?.textContent : label.textContent) || "";
}

function optionCheck() {
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("class", "icon option-check");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "22");
  svg.setAttribute("height", "22");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG_NS, "path");
  path.setAttribute("d", "M20 6 9 17l-5-5");
  svg.append(path);
  return svg;
}

/* The title and one row for each option, as the select has them now. */
function fillOptionSheet() {
  const select = optionSheetSelect;
  if (!select) return;
  const title = bareLabel(optionSheetLabel(select).trim());
  $("option-sheet-title").textContent = title;
  $("option-sheet-title").hidden = !title;
  $("option-sheet-list").replaceChildren(...Array.from(select.options, (option) => {
    const row = document.createElement("button");
    row.type = "button";
    row.className = "option-row";
    row.setAttribute("role", "option");
    row.setAttribute("aria-selected", String(option.index === select.selectedIndex));
    row.disabled = option.disabled;
    const label = document.createElement("span");
    label.className = "option-label";
    label.textContent = option.label;
    row.append(label, optionCheck());
    row.addEventListener("click", () => chooseOption(option));
    return row;
  }));
}

/** @param {HTMLSelectElement} select @param {boolean} [byKey] a key opened it, not a press */
function openOptionSheet(select, byKey = false) {
  if (select.disabled || select.options.length === 0) return;
  optionSheetSelect = select;
  fillOptionSheet();
  $("option-sheet").dataset.by = byKey ? "key" : "press";
  $("option-scrim").hidden = false;
  $("option-sheet").hidden = false;
  // The chosen option in the middle of the list, however long the list is.
  const list = $("option-sheet-list");
  const chosen = list.querySelector('[aria-selected="true"]') || list.querySelector("button:not(:disabled)");
  if (!(chosen instanceof HTMLElement)) return;
  const top = chosen.getBoundingClientRect().top - list.getBoundingClientRect().top + list.scrollTop;
  list.scrollTop = top - (list.clientHeight - chosen.offsetHeight) / 2;
  chosen.focus({ preventScroll: true });
}

function closeOptionSheet() {
  const select = optionSheetSelect;
  if (!select) return;
  optionSheetSelect = null;
  $("option-sheet").hidden = true;
  $("option-scrim").hidden = true;
  $("option-sheet-list").replaceChildren();
  if (!select.isConnected) return;
  if ($("option-sheet").dataset.by === "press") {
    select.dataset.quietFocus = "";
    select.addEventListener("blur", () => { delete select.dataset.quietFocus; }, { once: true });
  }
  select.focus({ preventScroll: true });
}

/* The page may have drawn the select again while the sheet was open (a
   measurement table that an inspection refills); an option that is no longer
   the select's chooses nothing. */
/** @param {HTMLOptionElement} option */
function chooseOption(option) {
  const select = optionSheetSelect;
  closeOptionSheet();
  if (!select || select.disabled || option.disabled || !select.contains(option)) return;
  if (option.index === select.selectedIndex) return;
  select.selectedIndex = option.index;
  select.dispatchEvent(new Event("input", { bubbles: true }));
  select.dispatchEvent(new Event("change", { bubbles: true }));
}

/** The arrow keys walk the rows that can be chosen. @param {KeyboardEvent} event */
function walkOptions(event) {
  const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
  if (!step) return;
  event.preventDefault();
  $("option-sheet").dataset.by = "key";
  const rows = Array.from($("option-sheet-list").querySelectorAll("button:not(:disabled)"));
  const at = rows.findIndex((row) => row === document.activeElement);
  const next = rows[Math.max(0, Math.min(rows.length - 1, at + step))];
  if (next instanceof HTMLElement) next.focus();
}

function wireOptionSheet() {
  // Capture: before anything else can let the press through to the web view's own list.
  document.addEventListener("mousedown", (event) => {
    if (pressedSelect(event.target)) event.preventDefault();
  }, true);
  document.addEventListener("click", (event) => {
    const select = pressedSelect(event.target);
    if (select) openOptionSheet(select);
  });
  document.addEventListener("keydown", (event) => {
    const select = pressedSelect(event.target);
    if (select) delete select.dataset.quietFocus;
    if (select && OPTION_SHEET_KEYS.has(event.key)) {
      event.preventDefault();
      openOptionSheet(select, true);
    } else if (event.key === "Escape" && optionSheetSelect) {
      closeOptionSheet();
    }
  }, true);
  $("option-sheet-list").addEventListener("keydown", walkOptions);
  $("option-scrim").addEventListener("click", closeOptionSheet);
}
