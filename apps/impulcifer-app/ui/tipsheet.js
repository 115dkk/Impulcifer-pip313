// @ts-check
// The Android page's info buttons: the text opens as a sheet from the bottom, titled with the field's label.
"use strict";

/* On a phone a bubble beside a 20px button covers the field it explains and
   leaves no room to read, so on a page with #tip-sheet an info button opens
   its text (the same .tip-bubble the desktop shows, copied as it is at that
   moment, since some follow the chosen option) in a sheet from the bottom,
   with the field's label as the title. The scrim, the close button, Escape
   and Android's back close it. */

/** @type {HTMLButtonElement | null} */
let tipSheetButton = null;

/** The label an info button explains. @param {HTMLButtonElement} button */
function tipLabel(button) {
  const fieldLabel = button.closest(".field-label")?.firstElementChild;
  if (fieldLabel && !fieldLabel.classList.contains("tip")) return fieldLabel.textContent || "";
  const row = button.closest(".check-row, .file-opt");
  const label = row?.querySelector(":scope > label, .file-name");
  return label?.textContent || t("button_more_info");
}

function fillTipSheet() {
  const button = tipSheetButton;
  if (!button) return;
  $("tip-sheet-title").textContent = bareLabel(tipLabel(button).trim());
  $("tip-sheet-body").replaceChildren(...Array.from(tipBubble(button).childNodes, (node) => node.cloneNode(true)));
}

/** @param {HTMLButtonElement} button */
function openTipSheet(button) {
  tipSheetButton = button;
  fillTipSheet();
  $("tip-scrim").hidden = false;
  $("tip-sheet").hidden = false;
  el("tip-sheet-close", HTMLButtonElement).focus({ preventScroll: true });
}

function closeTipSheet() {
  const button = tipSheetButton;
  if (!button) return;
  tipSheetButton = null;
  $("tip-sheet").hidden = true;
  $("tip-scrim").hidden = true;
  button.focus({ preventScroll: true });
}

function wireTipSheet() {
  /** @type {NodeListOf<HTMLButtonElement>} */ (document.querySelectorAll("button.info-tip")).forEach((button) => {
    button.addEventListener("click", () => openTipSheet(button));
  });
  el("tip-sheet-close", HTMLButtonElement).addEventListener("click", closeTipSheet);
  $("tip-scrim").addEventListener("click", closeTipSheet);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && tipSheetButton) closeTipSheet();
  });
}
