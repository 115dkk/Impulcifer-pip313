// @ts-check
// The Android page's folder and file chooser: a full-height sheet over list_directory.
"use strict";

/* ADR 0020: the system pickers cannot hand over a folder, or a file as a
   path, so every browse button on the Android page opens this sheet
   (app.js pickPath). It lists one folder at a time, folders first. In folder
   mode ("dir") files are shown dimmed, only to tell the folders apart, and
   "use this folder" chooses the folder on screen; in file mode the files of
   the field's kind are the choices and a tap chooses one. The storage
   volumes are chips when there is more than one. It opens at the folder in
   the field when that exists, otherwise at the first volume. */

/** @type {readonly BrowseKind[]} */
const BROWSE_KINDS = ["dir", "audio", "text", "wav", "measurement"];

/**
 * @typedef {object} ChooserSession
 * @property {BrowseKind} kind
 * @property {(path: string | null) => void} resolve
 * @property {string | null} path the folder on screen (also while it loads or cannot be read)
 * @property {string | null} parent
 * @property {StorageRoot[]} roots
 * @property {DirectoryListing | null} listing the last folder that answered
 * @property {IpcError | null} error
 * @property {boolean} loading
 * @property {boolean} noAccess
 * @property {number} revision
 * @property {HTMLElement | null} focus where focus goes back to
 */
/** @type {ChooserSession | null} */
let chooserSession = null;

/** @param {string} kind @returns {BrowseKind} */
function browseKind(kind) {
  return BROWSE_KINDS.find((known) => known === kind) ?? "dir";
}

/** The folder a field's text points at: itself for a folder, its parent for a file.
 * @param {BrowseKind} kind @param {string} text */
function chooserStart(kind, text) {
  const path = text.trim();
  if (!path) return null;
  if (kind === "dir") return path;
  return /[\\/]/.test(path) ? parentFolder(path) : null;
}

/** @param {string} kind @param {string} start @returns {Promise<string | null>} */
function openChooser(kind, start) {
  closeChooser(null);
  const browse = browseKind(kind);
  const focus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  return new Promise((resolve) => {
    chooserSession = { kind: browse, resolve, path: null, parent: null, roots: [], listing: null, error: null,
      loading: true, noAccess: false, revision: 0, focus };
    const sheet = $("chooser");
    sheet.dataset.mode = browse === "dir" ? "dir" : "file";
    $("chooser-list").replaceChildren();
    sheet.hidden = false;
    renderChooserWords();
    $("chooser-close").focus();
    void browseTo(chooserStart(browse, start), true);
  });
}

/** @param {string | null} path */
function closeChooser(path) {
  const session = chooserSession;
  if (!session) return;
  chooserSession = null;
  $("chooser").hidden = true;
  if (session.focus && session.focus.isConnected) session.focus.focus({ preventScroll: true });
  session.resolve(path);
}

/* Lists a folder (null: the first volume). A start folder that is not there
   falls back to the first volume; one that cannot be read stays on screen
   as such, with "up" and the volumes to leave it. */
/** @param {string | null} path @param {boolean} [start] */
async function browseTo(path, start = false) {
  const session = chooserSession;
  if (!session) return;
  const revision = ++session.revision;
  session.loading = true;
  session.error = null;
  if (path) session.path = path;
  renderChooser();
  if (storageDenied()) {
    session.loading = false;
    session.noAccess = true;
    renderChooser();
    return;
  }
  session.noAccess = false;
  const response = await callApi(() => api().list_directory({ path, kind: session.kind }));
  if (chooserSession !== session || revision !== session.revision) return;
  if (!response.ok) {
    const denied = response.error.details.kind === "access_denied";
    if (start && path && !denied) {
      void browseTo(null);
      return;
    }
    session.loading = false;
    session.noAccess = denied && storageDenied();
    session.error = response.error;
    session.parent = path && !session.roots.some((root) => root.path === path) ? parentFolder(path) : null;
    renderChooser();
    // Opened straight at a folder that cannot be read: ask for the volumes, so their chips offer a way out.
    if (!session.roots.length && !session.noAccess) {
      const first = await callApi(() => api().list_directory({ path: null, kind: session.kind }));
      if (chooserSession === session && revision === session.revision && first.ok && first.data.roots.length) {
        session.roots = first.data.roots;
        renderChooser();
      }
    }
    return;
  }
  const listing = response.data;
  session.loading = false;
  session.listing = listing;
  session.path = listing.path;
  session.parent = listing.parent;
  if (listing.roots.length) session.roots = listing.roots;
  renderChooser();
  renderChooserEntries(listing);
}

/** @param {number | null} size */
function fileSize(size) {
  if (size === null) return "";
  if (size < 1024) return `${size} B`;
  const units = ["KB", "MB", "GB"];
  let value = size / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

/** @param {"folder" | "file" | "go"} name */
function chooserIcon(name) {
  const paths = {
    folder: ["M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"],
    file: ["M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z", "M14 2v4a2 2 0 0 0 2 2h4"],
    go: ["m9 18 6-6-6-6"],
  }[name];
  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("class", `icon entry-icon entry-icon-${name}`);
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "24");
  svg.setAttribute("height", "24");
  svg.setAttribute("aria-hidden", "true");
  for (const d of paths) {
    const path = document.createElementNS(SVG_NS, "path");
    path.setAttribute("d", d);
    svg.append(path);
  }
  return svg;
}

/** @param {DirectoryListing} listing */
function renderChooserEntries(listing) {
  const session = chooserSession;
  if (!session) return;
  const fileMode = session.kind !== "dir";
  $("chooser-list").replaceChildren(...listing.entries.map((entry) => {
    const item = document.createElement("li");
    const name = document.createElement("span");
    name.className = "entry-name";
    name.textContent = entry.name;
    if (!entry.dir && !fileMode) {
      // A file in folder mode only tells the folders apart.
      item.className = "chooser-file";
      const size = document.createElement("span");
      size.className = "entry-meta mono";
      size.textContent = fileSize(entry.size);
      item.append(chooserIcon("file"), name, size);
      return item;
    }
    const button = document.createElement("button");
    button.type = "button";
    button.className = "chooser-entry";
    button.dataset.path = entry.path;
    button.dataset.kind = entry.dir ? "dir" : "file";
    if (entry.dir) {
      button.append(chooserIcon("folder"), name, chooserIcon("go"));
    } else {
      const size = document.createElement("span");
      size.className = "entry-meta mono";
      size.textContent = fileSize(entry.size);
      button.append(chooserIcon("file"), name, size);
    }
    item.append(button);
    return item;
  }));
  $("chooser-body").scrollTop = 0;
}

/* The sheet's state, path, volumes, message and actions. The entries stay
   (dimmed) while the next folder loads, so nothing jumps under the thumb. */
function renderChooser() {
  const session = chooserSession;
  if (!session) return;
  const sheet = $("chooser");
  const listing = session.listing;
  const readable = !session.loading && !session.noAccess && !session.error && listing !== null;
  const state = session.loading ? "loading" : session.noAccess ? "noaccess"
    : session.error ? (session.error.details.kind === "access_denied" ? "unreadable" : "error")
      : listing && listing.entries.length === 0 ? "empty" : "ready";
  sheet.dataset.state = state;
  const fileMode = session.kind !== "dir";

  const pathNode = $("chooser-path");
  pathNode.textContent = session.path || "";
  pathNode.scrollLeft = pathNode.scrollWidth;
  el("chooser-up", HTMLButtonElement).disabled = !session.parent || session.loading;

  const roots = $("chooser-roots");
  roots.hidden = session.roots.length < 2;
  const current = session.path || "";
  const active = session.roots
    .filter((root) => current === root.path || current.startsWith(`${root.path.replace(/\/$/, "")}/`))
    .sort((a, b) => b.path.length - a.path.length)[0];
  roots.replaceChildren(...session.roots.map((root) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "seg-btn";
    button.setAttribute("role", "radio");
    button.setAttribute("aria-checked", String(root === active));
    button.dataset.path = root.path;
    button.textContent = root.label;
    return button;
  }));

  const message = $("chooser-message");
  message.textContent = state === "loading" ? t("chooser_loading")
    : state === "empty" ? t(fileMode ? "chooser_empty_files" : "chooser_empty")
      : state === "unreadable" ? t("chooser_unreadable")
        : state === "error" && session.error ? errorSentence(session.error) : "";
  message.hidden = !message.textContent;
  $("chooser-access").hidden = state !== "noaccess";
  $("chooser-list").hidden = state === "noaccess" || state === "unreadable" || state === "error";
  const truncated = $("chooser-truncated");
  truncated.hidden = !readable || !listing?.truncated;
  truncated.textContent = listing?.truncated ? fmt(t("chooser_truncated"), { count: listing.entries.length }) : "";
  $("chooser-actions").hidden = fileMode;
  el("chooser-select", HTMLButtonElement).disabled = !readable;
}

/* The title follows the mode; everything else is drawn by renderChooser. */
function renderChooserWords() {
  const session = chooserSession;
  if (!session) return;
  $("chooser-title").textContent = t(session.kind === "dir" ? "chooser_title_dir" : "chooser_title_file");
  renderChooser();
}

/* Access came back while the sheet says it is missing: list again. */
function chooserStorageChanged() {
  const session = chooserSession;
  if (session && session.noAccess && !storageDenied()) void browseTo(session.path, true);
}

/* Close, up, the volumes, a folder or file in the list, and "use this folder". */
function wireChooser() {
  el("chooser-close", HTMLButtonElement).addEventListener("click", () => closeChooser(null));
  el("chooser-up", HTMLButtonElement).addEventListener("click", () => {
    if (chooserSession?.parent) void browseTo(chooserSession.parent);
  });
  el("chooser-select", HTMLButtonElement).addEventListener("click", () => {
    if (chooserSession?.listing && chooserSession.path) closeChooser(chooserSession.path);
  });
  $("chooser-roots").addEventListener("click", (event) => {
    const button = event.target instanceof Element ? event.target.closest(".seg-btn") : null;
    if (button instanceof HTMLElement && button.dataset.path) void browseTo(button.dataset.path);
  });
  $("chooser-list").addEventListener("click", (event) => {
    const button = event.target instanceof Element ? event.target.closest(".chooser-entry") : null;
    if (!(button instanceof HTMLElement) || !chooserSession || chooserSession.loading) return;
    const path = button.dataset.path || "";
    if (button.dataset.kind === "dir") void browseTo(path);
    else closeChooser(path);
  });
}
