// @ts-check
// The Android page's frame (android.html): the app bar, the view and its action bar, the back button.
"use strict";

/* ------------------------------------------------------------ the frame
   One view at a time under an app bar that names it, with the view's
   primary action pinned above the bottom navigation. The shell's navigation
   handler (app.js wireShell) switches the view; this part follows it: the
   root's data-view (the styles show the view's action button by it), the
   title, aria-current, and each view's own scroll position. */

/** @type {Record<string, () => string>} */
const VIEW_TITLES = {
  processing: () => t("studio_processing_title"),
  recovery: () => t("studio_recovery_title"),
  settings: () => t("sidebar_settings"),
  info: () => t("sidebar_info"),
};

/** Where each view was left, so that coming back finds it there. @type {Record<string, number>} */
const viewScroll = {};

function currentView() {
  return document.documentElement.dataset.view || "processing";
}

/** @param {string} view */
function showView(view) {
  const content = $("content");
  viewScroll[currentView()] = content.scrollTop;
  document.documentElement.dataset.view = view;
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(".nav-item")).forEach((item) => {
    if (item.dataset.view === view) item.setAttribute("aria-current", "page");
    else item.removeAttribute("aria-current");
  });
  content.scrollTop = viewScroll[view] || 0;
  renderAndroidShell();
}

/* Labels that stand above their control carry no colon (the catalogue's
   desktop labels end with one, for a label beside its field). */
const ANDROID_LABELS = ".field-row > label, .field-row > span:first-child, .field-label > :first-child, "
  + ".room-file-picks label > span, .room-file-positions label > span, .layout-force-caption";

/* The title, the labels, and the names of the buttons that show only an icon. */
function renderAndroidShell() {
  $("app-bar-title").textContent = (VIEW_TITLES[currentView()] || VIEW_TITLES.processing)();
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(ANDROID_LABELS)).forEach((node) => {
    if (node.childElementCount === 0) node.textContent = bareLabel(node.textContent || "");
  });
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-i18n-label]")).forEach((node) => {
    node.setAttribute("aria-label", bareLabel(t(node.dataset.i18nLabel || "")).replace(/\s*(\.\.\.|…)$/, ""));
  });
}

/* ------------------------------------------------------------ back
   Android's back button closes the topmost thing: an open tip, the ask
   dialog (as its cancel), the chooser, the update dialog, the job sheet (put
   away, the job goes on), then another view returns to processing. While a
   handler is registered the system's own back (leave the app) does not run,
   so the handler is registered only while there is something to go back
   from. handleBack is global so it can be driven without Tauri.
   While a job runs, back never leaves the app: Android ends the process
   with the activity, and the job with it. The handler stays registered, and
   once the tip, the ask dialog, the chooser and the update dialog are closed,
   back shows the running job and keeps it on screen: a BRIR job in its sheet
   (opened again if it was put away), an output recovery on its view. */

/** @type {{ unregister(): Promise<void> } | null} */
let backListener = null;
/** @type {Promise<void>} */
let backQueue = Promise.resolve();

/* The main channel's job, or the update's download, has not ended. */
function jobRunning() {
  return Boolean(state.lastJob && !isTerminal(state.lastJob.status)) || updateState.jobId !== null;
}

function canGoBack() {
  return jobRunning() || !$("tip-sheet").hidden || !$("ask-modal").hidden || !$("chooser").hidden || !$("update-modal").hidden
    || !$("job-sheet").hidden || currentView() !== "processing";
}

/** @returns {boolean} whether it closed or left anything */
function handleBack() {
  if (!document.getElementById("app-bar")) return false;
  if (!$("tip-sheet").hidden) {
    closeTipSheet();
    return true;
  }
  if (pendingAsk) {
    closeAsk(!pendingAsk.cancel);
    return true;
  }
  if (!$("chooser").hidden) {
    closeChooser(null);
    return true;
  }
  if (!$("update-modal").hidden) {
    hideUpdateModal();
    return true;
  }
  if (jobRunning()) {
    if (state.lastJob?.kind === "output_recovery") {
      if (currentView() !== "recovery") $("nav-recovery").click();
    } else {
      openJobSheet();
    }
    return true;
  }
  if (!$("job-sheet").hidden) {
    putAwayJobSheet();
    return true;
  }
  if (currentView() !== "processing") {
    $("nav-processing").click();
    return true;
  }
  return false;
}

/* Registers or gives back the handler to match canGoBack(). The calls are
   asynchronous, so they run one after another and each reads the page as it
   is when its turn comes. */
function syncBackButton() {
  const app = window.__TAURI__?.app;
  if (!app || !app.onBackButtonPress) return;
  backQueue = backQueue.then(async () => {
    const wanted = canGoBack();
    if (wanted && !backListener) {
      backListener = (await app.onBackButtonPress?.(() => { handleBack(); })) ?? null;
    } else if (!wanted && backListener) {
      const listener = backListener;
      backListener = null;
      await listener.unregister();
    }
  }).catch((error) => appendLog(String(error)));
}

/* ------------------------------------------------------------ wiring */

/* A switch row's head stays where the thumb found it: an open group's head
   sticks to the top while its body scrolls under it, and closing it from
   there would otherwise drop the page back to wherever the head started. The
   capture listener notes the head's place before the shell toggles the
   group; the bubbling one scrolls it back after. */
function keepDisclosureHeads() {
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-disclosure]")).forEach((head) => {
    let top = 0;
    head.addEventListener("click", () => { top = head.getBoundingClientRect().top; }, { capture: true });
    head.addEventListener("click", () => {
      const moved = head.getBoundingClientRect().top - top;
      if (moved) $("content").scrollTop += moved;
    });
  });
}

/* A path filled in by the chooser shows its end (the folder's own name), not
   the volume it starts with. */
function showPathEnds() {
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll("[data-browse]")).forEach((button) => {
    const input = document.getElementById(button.dataset.target || "");
    if (!(input instanceof HTMLInputElement)) return;
    input.addEventListener("input", () => {
      if (document.activeElement !== input) input.scrollLeft = input.scrollWidth;
    });
  });
}

/* Android closes a dialog when the dimmed area around it is tapped, as back
   does. The tap counts only when it starts and ends on the dimmed area itself:
   a drag that began inside the dialog and ended outside closes nothing. */
/** @param {string} id @param {() => void} dismiss */
function dismissOnOutsideTap(id, dismiss) {
  const backdrop = $(id);
  let pressed = false;
  backdrop.addEventListener("pointerdown", (event) => { pressed = event.target === backdrop; });
  backdrop.addEventListener("click", (event) => {
    if (pressed && event.target === backdrop) dismiss();
    pressed = false;
  });
}

/* The navigation (after the shell's own handler), the group heads, the path
   fields, the dialogs' dimmed areas, and the back button following what is
   open. The tip sheet's own scrim closes it (tipsheet.js); the first-run
   language choice stays until a language is chosen. */
function wireAndroidShell() {
  /** @type {NodeListOf<HTMLElement>} */ (document.querySelectorAll(".nav-item")).forEach((item) => {
    item.addEventListener("click", () => showView(item.dataset.view || "processing"));
  });
  keepDisclosureHeads();
  showPathEnds();
  dismissOnOutsideTap("ask-modal", () => { if (pendingAsk) closeAsk(!pendingAsk.cancel); });
  dismissOnOutsideTap("update-modal", hideUpdateModal);
  const observer = new MutationObserver(syncBackButton);
  for (const id of ["tip-sheet", "ask-modal", "chooser", "update-modal", "job-sheet"]) {
    observer.observe($(id), { attributes: true, attributeFilter: ["hidden"] });
  }
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-view"] });
  syncBackButton();
}
