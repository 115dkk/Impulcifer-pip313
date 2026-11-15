# P64: the Android page in the gallery and the catalog check (ADR 0020)

Repository: `/data/data/com.termux/files/home/bridge/Impulcifer-pip313`, branch `claude/android-port`. This
follows P63 (commit `88b8396`): the Android page exists and its checks ran only in a throwaway harness outside
the repository. This packet puts them where CI runs them. Read `docs/rust/UI.md` ("검증과 갤러리") and
`apps/impulcifer-app/tests/ui_gallery.py` first; the environment is the one P63 describes.

## What to do

1. **Gallery scenarios for `android.html`** in `apps/impulcifer-app/tests/ui_gallery.py`, in their own browser
   context (412×915, `device_scale_factor=2`, touch enabled), with the mock extended for the Android bootstrap
   (`platform: "android"`, `install_kind: "apk"`, `capabilities.recording: false`, `webview_backend: "android"`),
   `list_directory`, `get_storage_access`, `request_storage_access`, a system information answer with `device`,
   and the APK update result. The desktop scenarios, their mock answers and their count of PNGs stay as they are.
   - PNGs (keep it near forty; every one must show something a reviewer would look at): the processing view
     closed and with every group open, output recovery ready, settings, information, each in both themes and in
     English and Korean; and once each in dark, English and Korean: the storage notice, the chooser in folder
     mode, in file mode and without access, the job sheet running, succeeded and failed, the bar of a job put
     away, a tip sheet, the APK update dialog, the ask dialog, the first-run language choice. Name them
     `android-<what>-<language>-<theme>.png`. Update `EXPECTED_SHOTS` and the message it prints.
   - Behaviour checks (a new `android_checks(browser)` called from `render_gallery`, failing with a sentence
     that names what was wrong):
     - **same request**: for the nineteen states of P63, `gatherBrirPayload()` on `index.html` (Studio) and on
       `android.html` are equal, and every element id Studio's request building reads exists on `android.html`;
     - storage: the notice for `denied`, gone after `visibilitychange` once the mock answers `granted`;
     - chooser: lists, enters, goes up, stops at a root, chooses a folder into the field that opened it and fires
       `input`, chooses a file in file mode, shows its empty, truncated, unreadable and no-access states;
     - back: the order of P63, the handler registered only while there is something to go back from, and the
       rule for a running job (back never leaves; the sheet comes back; after the job ends the normal order);
     - a BRIR job from start to outcome in the sheet, a failed job's sentence, a failed "open folder" showing the
       path; output recovery through its five states and a run;
     - all nine languages: no raw catalog key anywhere on the page, in the sheets or in the dialogs;
     - at 360 px wide: no horizontal page scroll, and no visible button, input, select, switch or navigation
       item smaller than 48 px in either direction;
     - no page, console or resource error in any of it.
2. **The catalog check covers the Android page.** `crates/impulcifer-service/tests/ui_catalog.rs` reads the keys a
   page uses from `index.html` only. Make it read `android.html` as well (its `data-i18n` and
   `data-i18n-label`-style attributes, and whatever the Android-only scripts pass to `t()`), so a key the Android
   page uses must resolve in every language, like the desktop's. Also require that every `ui/*.js` file is
   loaded by at least one of the two pages, instead of by `index.html`; then take the five Android-only scripts
   out of `index.html` again if nothing else needs them there (the desktop should not load what it never runs).
   If removing them changes what the desktop renders or breaks a check, leave them and say why.
3. **No emoji from the app's own strings on the Android page.** The update dialog reuses three 2.x keys whose
   text starts with an emoji (`update_available_message`, `update_button_skip`, `update_button_remind`). Add
   `update_new_version`, `update_button_skip_version` and `update_button_later` to the nine overlays (English
   text and a Korean draft, as in P63; the catalog packet rewrites them) and make the Android dialog use them;
   the desktop dialog keeps its keys and its text. Release notes are content and stay as they are. Check the
   rest of the Android page's strings for emoji the same way and list what you find.
4. **`docs/rust/UI.md`**: a section on the Android page (what it is, which scripts and parts it shares with the
   desktop and how a part is switched on by its markup, the names other code relies on from P63's "Page marker
   and test hooks", the chooser and storage access, the job sheet, back), and the gallery section's counts and
   list brought up to date. Write it in the document's own Korean register (합니다체, plain sentences, no
   translationese, no em-dash, no emoji).

## Allowed files

`apps/impulcifer-app/tests/ui_gallery.py`, `crates/impulcifer-service/tests/ui_catalog.rs`,
`apps/impulcifer-app/ui/android.html`, `apps/impulcifer-app/ui/update.js` and `index.html` only as far as items
2 and 3 need, `crates/impulcifer-service/locales/*.json` (the three new keys), `docs/rust/UI.md`, and a test that
pins the script list if it must follow. Do not commit, stash, reset or check out anything.

## Verification (run all, in the foreground, and report the output)

1. Type check and the three contract tests (the Node commands of P63).
2. `~/.local/bin/ubuntu-run /opt/pw-venv/bin/python apps/impulcifer-app/tests/ui_gallery.py --out <scratch>`:
   every check passes and it prints the new total. Report how long the Android part adds.
3. Desktop unchanged: the element snapshot comparison of P63 (all 224 desktop states, HEAD `88b8396` against
   your tree) is equal again.
4. Break it on purpose, one at a time, and show that the gallery fails with a sentence that names the cause,
   then restore: remove one request field's element from `android.html`; make `handleBack()` leave while a job
   runs; shrink one button below 48 px; use a key that is in no catalog.
5. `~/.local/bin/ubuntu-run cargo test -p impulcifer-service --test ui_catalog`,
   `~/.local/bin/ubuntu-run cargo test -p impulcifer-service --lib settings`,
   `~/.local/bin/ubuntu-run cargo test -p impulcifer-policy`,
   `~/.local/bin/ubuntu-run cargo test -p impulcifer-app --test packaging`, one at a time.

## Report

Changed files with a one-line summary each; the list of Android PNGs and checks with the new total; the output
of every verification step, including the four deliberate breaks and their failure sentences; the new keys;
emoji found elsewhere; anything decided against, with the reason. Finish the report before you end your turn.
