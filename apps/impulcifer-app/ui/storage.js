// @ts-check
// All-files access on the Android page: the notice above the inputs, the settings row and the chooser's notice.
"use strict";

/* ADR 0020: the app reads measurement folders and writes results by path,
   which needs Android's all-files access. The person turns it on in the
   system screen request_storage_access opens, outside the app, so the state
   is asked again whenever the page becomes visible. Null until the first
   answer. */
/** @type {StorageAccess["state"] | null} */
let storageState = null;

/* Read through a call: the state changes while other code awaits. */
function storageDenied() {
  return storageState === "denied";
}

async function refreshStorageAccess() {
  const response = await callApi(() => api().get_storage_access());
  if (response.ok) storageState = response.data.state;
  renderStorage();
  chooserStorageChanged();
}

async function requestStorageAccess() {
  const response = await callApi(() => api().request_storage_access());
  if (!response.ok) appendLog(errorText(response));
}

/* The notice shows only while access is missing; settings says the state in words. */
function renderStorage() {
  $("storage-notice").hidden = storageState !== "denied";
  const words = storageState === "granted" ? t("storage_state_granted")
    : storageState === "denied" ? t("storage_state_denied")
      : storageState === "not_required" ? t("storage_state_not_required") : "";
  $("sf-storage-state").textContent = words;
  $("sf-storage-dot").dataset.state = storageState || "unknown";
  el("btn-sf-storage-access", HTMLButtonElement).hidden = storageState === "not_required";
}

/* The three buttons that open the system screen, and the check on return. */
function wireStorage() {
  for (const id of ["btn-storage-access", "btn-sf-storage-access", "chooser-storage-access"]) {
    document.getElementById(id)?.addEventListener("click", requestStorageAccess);
  }
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible") void refreshStorageAccess();
  });
}
