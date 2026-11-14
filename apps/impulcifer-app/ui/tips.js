// @ts-check
// Info tooltips: the bubbles the "i" buttons open.
"use strict";

/* --------------------------------------------------------- info tooltips
   An "i" button after a label holds the description of its control in a
   bubble (role="tooltip"; the control and the button point to it with
   aria-describedby). It opens on mouse hover and on keyboard focus; a click
   or tap pins it open until the next click, a click elsewhere, Escape or
   focus leaving. Escape also keeps it shut while the pointer stays on the
   button. One bubble is open at a time. The bubble is fixed-positioned
   under the button, so a card's overflow cannot clip it, and kept inside the
   window; on scroll or resize it follows the button and closes once the
   button leaves the visible part of the page. */

/** @type {{ button: HTMLButtonElement, bubble: HTMLElement, pinned: boolean } | null} */
let openTip = null;
/** @type {number | undefined} */
let tipCloseTimer;
/** A button whose bubble Escape or a click closed: hover does not reopen it until the pointer leaves. */
/** @type {HTMLButtonElement | null} */
let tipSuppressed = null;

/* Every button sits with its bubble in one `.tip` wrapper, so the bubble is
   found from the markup structure rather than from attribute text. */
/** @param {HTMLButtonElement} button */
function tipBubble(button) {
  const bubble = button.closest(".tip")?.querySelector(".tip-bubble");
  if (!(bubble instanceof HTMLElement)) throw new Error("Missing tooltip bubble");
  return bubble;
}

/* Below the button, starting at its left edge; flipped to end at its right
   edge when that would leave the window, then held 8px inside it. Above the
   button when there is no room below. */
/** @param {HTMLButtonElement} button @param {HTMLElement} bubble */
function placeTip(button, bubble) {
  const margin = 8;
  const gap = 6;
  const anchor = button.getBoundingClientRect();
  const width = bubble.offsetWidth;
  const height = bubble.offsetHeight;
  let left = anchor.left;
  if (left + width > window.innerWidth - margin) left = anchor.right - width;
  left = Math.max(margin, Math.min(left, window.innerWidth - margin - width));
  let top = anchor.bottom + gap;
  if (top + height > window.innerHeight - margin && anchor.top - gap - height >= margin) top = anchor.top - gap - height;
  bubble.style.left = `${Math.round(left)}px`;
  bubble.style.top = `${Math.round(top)}px`;
}

/** @param {HTMLButtonElement} button @param {boolean} pin */
function showTip(button, pin) {
  window.clearTimeout(tipCloseTimer);
  if (openTip && openTip.button !== button) hideTip();
  const bubble = tipBubble(button);
  const pinned = pin || (openTip !== null && openTip.pinned);
  openTip = { button, bubble, pinned };
  placeTip(button, bubble);
  bubble.dataset.open = "true";
  button.classList.add("is-open");
}

function followTip() {
  if (!openTip) return;
  const anchor = openTip.button.getBoundingClientRect();
  const view = $("content").getBoundingClientRect();
  if (anchor.bottom < view.top || anchor.top > view.bottom || anchor.width === 0) hideTip();
  else placeTip(openTip.button, openTip.bubble);
}

function hideTip() {
  window.clearTimeout(tipCloseTimer);
  if (!openTip) return;
  delete openTip.bubble.dataset.open;
  openTip.button.classList.remove("is-open");
  openTip = null;
}

function wireTips() {
  /** @type {NodeListOf<HTMLButtonElement>} */ (document.querySelectorAll("button.info-tip")).forEach((button) => {
    const tip = button.parentElement;
    if (!tip) return;
    // The wrapper holds the bubble too, so moving onto the bubble keeps it open.
    tip.addEventListener("pointerenter", (event) => {
      if (event.pointerType === "mouse" && tipSuppressed !== button) showTip(button, false);
    });
    tip.addEventListener("pointerleave", (event) => {
      if (event.pointerType !== "mouse") return;
      if (tipSuppressed === button) tipSuppressed = null;
      if (openTip?.button === button && !openTip.pinned && !button.matches(":focus-visible")) {
        window.clearTimeout(tipCloseTimer);
        tipCloseTimer = window.setTimeout(hideTip, 150);
      }
    });
    button.addEventListener("focus", () => {
      if (button.matches(":focus-visible")) showTip(button, false);
    });
    // Focus leaving closes it, unless the pointer moved onto the bubble (a click there blurs the button).
    button.addEventListener("blur", () => {
      if (openTip?.button === button && !tip.matches(":hover")) hideTip();
    });
    button.addEventListener("click", () => {
      if (openTip?.button === button && openTip.pinned) {
        hideTip();
        tipSuppressed = button;
      } else {
        tipSuppressed = null;
        showTip(button, true);
      }
    });
  });
  document.addEventListener("keydown", (event) => {
    if (event.key !== "Escape" || !openTip) return;
    tipSuppressed = openTip.button;
    hideTip();
  });
  document.addEventListener("pointerdown", (event) => {
    if (openTip && event.target instanceof Node && !openTip.button.parentElement?.contains(event.target)) hideTip();
  });
  $("content").addEventListener("scroll", followTip, { passive: true });
  window.addEventListener("resize", followTip);
}
