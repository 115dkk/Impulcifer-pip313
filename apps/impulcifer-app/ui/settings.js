// @ts-check
// Settings (language, theme, skin) and the info page.
"use strict";

/* -------------------------------------------------------------- settings */

/** @param {Language[]} languages */
function populateLanguages(languages) {
  const select = el("sf-language", HTMLSelectElement);
  select.replaceChildren();
  languages.forEach(({ code, name }) => select.add(new Option(name, code)));
  select.value = state.language;
}

/** @param {string} code */
async function changeLanguage(code) {
  const response = await api().set_language(code);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  state.language = response.data.language;
  state.strings = response.data.strings;
  applyStrings();
}

/** @param {string} code */
async function changeTheme(code) {
  const response = await api().set_theme(code);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  applyTheme(code);
}

/** @param {string} code */
async function changeSkin(code) {
  const response = await api().set_skin(code);
  if (!response.ok) {
    appendLog(errorText(response));
    return;
  }
  applySkin(response.data.skin);
  showSkinDescription();
}

/* The line under the skin choice describes the chosen skin. */
function showSkinDescription() {
  $("sf-skin-desc").textContent = t(state.skin === "stable" ? "tooltip_skin_stable" : "tooltip_skin_studio");
}

/* The saved language list, theme and skin. A page without a skin choice (the
   Android page has one look) keeps Studio's behaviour. */
/** @param {Bootstrap} data */
function bootSettings(data) {
  if (!data.ui) return;
  populateLanguages(data.ui.languages || []);
  el("sf-theme", HTMLSelectElement).value = data.ui.theme || "dark";
  applyTheme(data.ui.theme || "dark");
  if (document.getElementById("sf-skin")) {
    el("sf-skin", HTMLSelectElement).value = data.ui.skin === "stable" ? "stable" : "studio";
    applySkin(data.ui.skin);
    showSkinDescription();
  }
}

/* First run: ask for the language before anything else (CTk parity). */
/** @param {Bootstrap} data */
function readySettings(data) {
  if (data.ui && data.ui.first_run) {
    showFirstRunLanguageModal(data.ui.languages || []);
  }
}

/* ------------------------------------------------------------------ info */

async function loadSystemInfo() {
  const response = await api().get_system_info();
  if (!response.ok) return;
  const info = response.data;
  state.systemInfo = info;
  renderSystemInfo();
}

/* The words for how the app was installed; an unknown kind shows as sent. */
/** @param {SystemInfo} info */
function installLabel(info) {
  switch (info.install_kind) {
    case "velopack": return t("info_install_velopack");
    case "pip": return t("info_install_pip");
    case "dev": return t("info_install_dev");
    case "apk": return t("info_install_apk");
    default: return info.install_kind;
  }
}

/* The rows the system information grid can show. The grid lists the ones it
   shows in its data-rows attribute; without it, the desktop's rows. */
/** @type {Record<string, (info: SystemInfo) => [string, string | number | null | undefined]>} */
const SYSTEM_ROWS = {
  version: (info) => [t("label_version"), info.version],
  install: (info) => [t("label_installation"), installLabel(info)],
  device: (info) => [t("label_device"), info.device],
  runtime: (info) => [t("label_runtime"), info.runtime.toolchain],
  shell: (info) => [t("label_shell"), info.runtime.shell],
  webview: (info) => [t("label_web_engine"), info.runtime.webview],
  os: (info) => [t("label_os"), info.os],
  cpu: (info) => [t("label_cpu_cores"), info.cpu_count],
  data: (info) => [t("label_data_directory"), info.paths.data_dir],
  settings: (info) => [t("label_settings_file"), info.paths.settings_path],
  channel: (info) => [t("label_update_channel"),
    info.update_channel === "prerelease" ? t("update_channel_prerelease") : t("update_channel_stable")],
};
const DESKTOP_SYSTEM_ROWS = "runtime shell webview os cpu data settings";

function renderSystemInfo() {
  const info = state.systemInfo;
  if (!info) return;
  // The line under the name is the desktop page's. The Android page shows the
  // version among its system rows.
  const pill = document.getElementById("info-version-pill");
  if (pill) {
    pill.textContent = fmt(t("info_version_rust"), {
      version: info.version, toolchain: info.runtime.toolchain, install: installLabel(info),
    });
  }
  // The folder rows are the desktop page's too. On Android both are in the
  // app's own storage, where the person can neither look nor change anything.
  if (document.getElementById("sf-data-row")) {
    $("sf-data-path").textContent = info.paths.data_dir;
    $("sf-settings-path").textContent = info.paths.settings_path;
    $("sf-data-row").hidden = !info.paths.data_dir;
    $("sf-settings-row").hidden = !info.paths.settings_path;
  }
  const grid = $("info-system");
  grid.replaceChildren();
  for (const name of (grid.dataset.rows || DESKTOP_SYSTEM_ROWS).split(" ")) {
    const row = SYSTEM_ROWS[name];
    if (!row) continue;
    const [label, value] = row(info);
    if (value === null || value === undefined || value === "") continue;
    const keyNode = document.createElement("span");
    keyNode.className = "kv-key";
    keyNode.textContent = bareLabel(label);
    const valueNode = document.createElement("span");
    valueNode.className = "kv-val mono";
    valueNode.textContent = String(value);
    grid.append(keyNode, valueNode);
  }
}

/* Key/value grids carry their own separator. Some labels come from the 2.x
   catalogue with a trailing colon ("OS:") and the 3.x overlay ones without,
   so the grid strips it rather than showing a mix. */
/** @param {string} text */
function bareLabel(text) {
  return text.replace(/\s*[:：]\s*$/, "");
}

/* ---------------------------------------------------------------- wiring */

/* The skin's description and the system info again, after the language changes. */
function renderSettings() {
  if (document.getElementById("sf-skin")) {
    applySkin(state.skin);
    showSkinDescription();
  }
  renderSystemInfo();
}

/* The data and settings folders, skin, theme and language. The Android page
   has no skin choice and no buttons for the folders (they are in the app's
   private storage, which no file manager opens). */
function wireSettings() {
  document.getElementById("btn-open-data")?.addEventListener("click", () => api().open_path(state.systemInfo?.paths.data_dir));
  document.getElementById("btn-open-settings")?.addEventListener("click", () => {
    const path = state.systemInfo?.paths.settings_path;
    if (path) api().open_path(parentFolder(path));
  });
  document.getElementById("sf-skin")?.addEventListener("change", () => changeSkin(val("sf-skin")));
  el("sf-theme", HTMLSelectElement).addEventListener("change", () => changeTheme(val("sf-theme")));
  el("sf-language", HTMLSelectElement).addEventListener("change", () => changeLanguage(val("sf-language")));
}
