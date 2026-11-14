// @ts-check
// Auto-update: the check, the release notes, the download job and the restart.
"use strict";

/* ----------------------------------------------------------- auto-update */

/* Port of the CTk flow: background check 2s after startup →
   UpdateDialog (notes + Update Now / Remind / Skip) → UpdateExecutor with
   progress → completion message → optional apply-and-restart (Velopack). */

/** @type {{info: UpdateInfo | null, jobId: string | null, nextSeq: number, pollTimer: number | undefined}} */
const updateState = {
  info: null,
  jobId: null,
  nextSeq: 0,
  pollTimer: undefined,
};

/** @param {string | undefined} key @param {string | undefined} fallback */
function tOr(key, fallback) {
  return (key && state.strings[key]) || fallback || key || "";
}

/** @param {number} value */
function setUpdateProgress(value) {
  $("update-progress").style.width = `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
}

/* Release notes are the version's CHANGELOG.md section (release-3x.yml), so
   they arrive as Markdown. Render the subset CHANGELOG uses (headings,
   bullets, **bold**, `code`, [text](url), fenced blocks) as DOM nodes; the
   text never goes through innerHTML, and links keep only their text. */
/** @param {HTMLElement} target @param {string} text */
function appendInlineMarkdown(target, text) {
  const pattern = /`([^`]+)`|\*\*(.+?)\*\*|\[([^\]]+)\]\([^)\s]+\)/g;
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    const at = match.index ?? 0;
    if (at > last) target.append(text.slice(last, at));
    if (match[1] !== undefined) {
      const code = document.createElement("code");
      code.textContent = match[1];
      target.append(code);
    } else if (match[2] !== undefined) {
      const strong = document.createElement("strong");
      appendInlineMarkdown(strong, match[2]);
      target.append(strong);
    } else {
      appendInlineMarkdown(target, match[3]);
    }
    last = at + match[0].length;
  }
  if (last < text.length) target.append(text.slice(last));
}

/** @param {HTMLElement} target @param {string} markdown */
function renderReleaseNotes(target, markdown) {
  target.replaceChildren();
  /** @type {HTMLUListElement | null} */
  let list = null;
  /** @type {HTMLElement | null} */
  let fence = null;
  for (const raw of markdown.replace(/\r\n?/g, "\n").split("\n")) {
    if (/^\s*```/.test(raw)) {
      if (fence) {
        fence = null;
      } else {
        fence = document.createElement("pre");
        target.append(fence);
        list = null;
      }
      continue;
    }
    if (fence) {
      fence.textContent += (fence.textContent ? "\n" : "") + raw;
      continue;
    }
    const line = raw.trimEnd();
    if (!line.trim() || /^\s*(-{3,}|\*{3,})$/.test(line)) {
      list = null;
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    const bullet = /^(\s*)[-*]\s+(.*)$/.exec(line);
    if (heading) {
      const node = document.createElement("p");
      node.className = `notes-heading notes-h${heading[1].length}`;
      appendInlineMarkdown(node, heading[2]);
      target.append(node);
      list = null;
    } else if (bullet) {
      if (!list) {
        list = document.createElement("ul");
        target.append(list);
      }
      const item = document.createElement("li");
      if (bullet[1].length >= 2) item.className = "nested";
      appendInlineMarkdown(item, bullet[2]);
      list.append(item);
    } else if (list && /^\s/.test(line) && list.lastElementChild instanceof HTMLElement) {
      const item = list.lastElementChild;
      item.append(" ");
      appendInlineMarkdown(item, line.trim());
    } else {
      const node = document.createElement("p");
      appendInlineMarkdown(node, line.trim());
      target.append(node);
      list = null;
    }
  }
}

/** @param {UpdateInfo} info */
function showUpdateModal(info) {
  updateState.info = info;
  $("update-version-line").textContent = fmt(t("update_version_info"), {
    current: info.current_version,
    latest: info.latest_version,
  });
  renderReleaseNotes($("update-notes"), info.release_notes || t("update_no_notes"));
  $("update-progress-row").hidden = true;
  setUpdateProgress(0);
  $("update-status").textContent = "";
  $("update-result").hidden = true;
  el("update-now", HTMLButtonElement).hidden = false;
  el("update-now", HTMLButtonElement).disabled = false;
  el("update-remind", HTMLButtonElement).hidden = false;
  el("update-skip", HTMLButtonElement).hidden = false;
  el("update-restart", HTMLButtonElement).hidden = true;
  el("update-close", HTMLButtonElement).hidden = true;
  $("update-modal").hidden = false;
}

function showUpdateProgressOnly() {
  // Resume path: an update job survived a frontend reload — no check info,
  // just live progress until the terminal state arrives.
  updateState.info = null;
  $("update-version-line").textContent = "";
  $("update-notes").textContent = "";
  $("update-progress-row").hidden = false;
  $("update-result").hidden = true;
  el("update-now", HTMLButtonElement).hidden = true;
  el("update-remind", HTMLButtonElement).hidden = true;
  el("update-skip", HTMLButtonElement).hidden = true;
  el("update-restart", HTMLButtonElement).hidden = true;
  el("update-close", HTMLButtonElement).hidden = true;
  $("update-modal").hidden = false;
}

function hideUpdateModal() {
  window.clearTimeout(updateState.pollTimer);
  $("update-modal").hidden = true;
}

/** @param {boolean} manual */
async function checkForUpdates(manual) {
  const statusLine = $("update-check-status");
  if (manual) {
    statusLine.hidden = false;
    statusLine.textContent = t("update_checking");
  }
  let response;
  try {
    response = await api().check_for_updates();
  } catch (error) {
    response = null;
  }
  if (!response || !response.ok) {
    // Startup checks fail silently, mirroring the CTk background check.
    if (manual) statusLine.textContent = response ? errorText(response) : t("update_error_apply");
    return;
  }
  const data = response.data;
  if (data.update_available) {
    if (manual) statusLine.hidden = true;
    showUpdateModal(data);
  } else if (manual) {
    statusLine.textContent = fmt(t("update_up_to_date"), {
      latest: data.latest_version || data.current_version,
    });
  }
}

async function beginUpdate() {
  const info = updateState.info;
  if (!info) return;
  el("update-now", HTMLButtonElement).disabled = true;
  el("update-remind", HTMLButtonElement).hidden = true;
  el("update-skip", HTMLButtonElement).hidden = true;
  $("update-progress-row").hidden = false;
  $("update-status").textContent = t("update_downloading");
  const response = await api().start_update({
    download_url: info.download_url,
    latest_version: info.latest_version,
  });
  if (!response.ok) {
    finishUpdate(errorText(response), false);
    return;
  }
  updateState.jobId = response.data.job.job_id;
  updateState.nextSeq = 0;
  pollUpdateJob();
}

async function pollUpdateJob() {
  if (!updateState.jobId) return;
  const response = await api().poll_job(updateState.jobId, updateState.nextSeq);
  if (!response.ok) {
    finishUpdate(errorText(response), false);
    return;
  }
  const { job, events, next_seq: nextSeq } = response.data;
  updateState.nextSeq = nextSeq;
  for (const event of events) {
    if (event.type === "progress") {
      const payload = event.payload;
      if (typeof payload.progress === "number") setUpdateProgress(payload.progress);
      // The executor sends either an i18n key ("update_downloading") or
      // preformatted text ("Downloading: 42%").
      if (payload.message) $("update-status").textContent = tOr(payload.message, payload.message);
    }
  }
  if (["succeeded", "failed", "cancelled"].includes(job.status)) {
    updateState.jobId = null;
    if (job.status === "succeeded" && job.kind === "update" && job.result) {
      const result = job.result;
      setUpdateProgress(typeof result.progress === "number" ? result.progress : 1);
      $("update-status").textContent = tOr(result.status_key, result.status_default);
      finishUpdate(
        tOr(result.message_key, result.message_default),
        true,
        Boolean(result.requires_restart),
      );
    } else {
      finishUpdate(job.error ? errorSentence(job.error) : t("update_error_apply"), false);
    }
    return;
  }
  updateState.pollTimer = window.setTimeout(pollUpdateJob, 250);
}

/** @param {string} message @param {boolean} success @param {boolean} requiresRestart */
function finishUpdate(message, success, requiresRestart = false) {
  $("update-result").hidden = false;
  $("update-result").textContent = message;
  el("update-now", HTMLButtonElement).hidden = true;
  el("update-remind", HTMLButtonElement).hidden = true;
  el("update-skip", HTMLButtonElement).hidden = true;
  // Velopack stages an apply-and-restart: confirming OK hands over to
  // Update.exe and the window closes. pip/legacy end with a plain Close.
  el("update-restart", HTMLButtonElement).hidden = !(success && requiresRestart);
  el("update-close", HTMLButtonElement).hidden = success && requiresRestart;
}

async function applyStagedUpdate() {
  el("update-restart", HTMLButtonElement).disabled = true;
  const response = await api().apply_pending_update();
  if (!response.ok) {
    el("update-restart", HTMLButtonElement).disabled = false;
    finishUpdate(errorText(response), false);
    return;
  }
  $("update-result").hidden = false;
  $("update-result").textContent = t("update_restart_message");
}
