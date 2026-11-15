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
}

/* ------------------------------------------------------------------ info */

async function loadSystemInfo() {
  const response = await api().get_system_info();
  if (!response.ok) return;
  const info = response.data;
  state.systemInfo = info;
  renderSystemInfo();
}

function renderSystemInfo() {
  const info = state.systemInfo;
  if (!info) return;
  const installKey = info.install_kind === "velopack" ? "info_install_velopack"
    : info.install_kind === "pip" ? "info_install_pip" : "info_install_dev";
  $("info-version-pill").textContent = fmt(t("info_version_rust"), {
    version: info.version, toolchain: info.runtime.toolchain,
    install: info.install_kind === "dev" || info.install_kind === "pip" || info.install_kind === "velopack"
      ? t(installKey) : info.install_kind,
  });
  /** @type {[string, string | number | null | undefined][]} */
  const rows = [
    ["label_runtime", info.runtime.toolchain],
    ["label_shell", info.runtime.shell],
    ["label_web_engine", info.runtime.webview],
    ["label_os", info.os],
    ["label_cpu_cores", info.cpu_count],
    ["label_data_directory", info.paths.data_dir],
    ["label_settings_file", info.paths.settings_path],
  ];
  $("sf-data-path").textContent = info.paths.data_dir;
  $("sf-settings-path").textContent = info.paths.settings_path;
  $("sf-data-row").hidden = !info.paths.data_dir;
  $("sf-settings-row").hidden = !info.paths.settings_path;
  const grid = $("info-system");
  grid.replaceChildren();
  for (const [key, value] of rows) {
    if (value === null || value === undefined || value === "") continue;
    const keyNode = document.createElement("span");
    keyNode.className = "kv-key";
    keyNode.textContent = bareLabel(t(key));
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
  applySkin(state.skin);
  renderSystemInfo();
}

/* The data and settings folders, skin, theme and language. */
function wireSettings() {
  el("btn-open-data", HTMLButtonElement).addEventListener("click", () => api().open_path(state.systemInfo?.paths.data_dir));
  el("btn-open-settings", HTMLButtonElement).addEventListener("click", () => {
    const path = state.systemInfo?.paths.settings_path;
    if (path) api().open_path(parentFolder(path));
  });
  el("sf-skin", HTMLSelectElement).addEventListener("change", () => changeSkin(val("sf-skin")));
  el("sf-theme", HTMLSelectElement).addEventListener("change", () => changeTheme(val("sf-theme")));
  el("sf-language", HTMLSelectElement).addEventListener("change", () => changeLanguage(val("sf-language")));
}
