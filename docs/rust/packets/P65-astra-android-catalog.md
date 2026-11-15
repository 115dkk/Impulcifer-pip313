# P65: the Android page's words in nine languages (ADR 0020)

Repository: `/data/data/com.termux/files/home/bridge/Impulcifer-pip313`, branch `claude/android-port`. Work only
in `crates/impulcifer-service/locales/*.json` (the nine overlay catalogs). Read `CLAUDE.md` (the bullet "3.x
문자열은 오버레이 카탈로그에 쓴다") and `docs/rust/UI.md` (the section "안드로이드 화면") first.

P63 and P64 added 37 keys for the Android page. Every catalog has all of them, with the same placeholders, but
only `en.json` and `ko.json` carry real text: the Korean is a draft, and the other seven files hold the English
text. This packet writes the words.

## The keys

`nav_recovery`, `action_generate_brir`, `storage_notice_title`, `storage_notice_body`, `button_storage_access`,
`label_storage_access`, `hint_storage_access`, `storage_state_granted`, `storage_state_denied`,
`storage_state_not_required`, `hint_folders_private`, `label_device`, `label_update_channel`,
`update_channel_stable`, `update_channel_prerelease`, `info_install_apk`, `chooser_title_dir`,
`chooser_title_file`, `chooser_roots`, `chooser_up`, `chooser_select`, `chooser_loading`, `chooser_empty`,
`chooser_empty_files`, `chooser_unreadable`, `chooser_truncated` (`{count}`), `job_sheet_hide`, `job_sheet_log`,
`job_sheet_output`, `open_folder_failed` (`{path}`), `update_button_download`, `update_apk_status`,
`update_apk_title`, `update_apk_message`, `update_new_version`, `update_button_skip_version`,
`update_button_later`.

Where each one appears is in P63's report and in `apps/impulcifer-app/ui/android.html` (search the key). Look
before you write: a navigation label sits in a slot about 90 px wide under an icon, a button label on one line of
a 360 px wide phone, `chooser_roots`, `chooser_up` and `job_sheet_hide` are names read by a screen reader, and
`storage_notice_body` is the one paragraph that explains why the app asks for "all files access".

## What to do

1. **English (`en.json`)**: review the 37 texts as a native writer of UI text would and change only what is
   wrong or awkward. They are the source for the other languages, so settle them first. `update_apk_status`,
   `update_apk_title` and `update_apk_message` are also the service's built-in defaults
   (`crates/impulcifer-service/src/update/mod.rs`); if you change their English, say so in the report, do not
   edit the Rust file.
2. **Korean (`ko.json`)**: rewrite the 37 drafts so they read as Korean written by a Korean, in the register of
   the existing catalog (합니다체 for sentences, noun phrases for labels and buttons). No translationese: no
   sentence that maps word for word onto the English, no "~하는 것", no needless "~에 대한", no passive where
   Korean uses the active, no em-dash, no emoji. Use the words Android itself uses in its Korean system screens
   where the text refers to them (the permission is "모든 파일에 대한 접근 허용" in system settings; keep the
   app's own sentence natural and short around it).
3. **The other seven** (`de`, `es`, `fr`, `ja`, `ru`, `zh_CN`, `zh_TW`): translate from the settled English, in
   the register each catalog already uses, with the terms each language's Android uses for storage, permission,
   folder, download and update. Keep product and format names as they are (Impulcifer, BRIR, APK, Android).
   Labels stay short enough for their place.
4. Keep every placeholder exactly (`{count}`, `{path}`), the key set identical in all nine files, the files'
   existing order and formatting (two-space indent, one key per line, UTF-8, final newline), and touch no other
   key.

## Not in this packet

Any file other than the nine catalogs. The 2.x catalogs in `i18n/locales/`. Keys that existed before this
branch. Do not commit, stash, reset or check out anything.

## Verification (run all, in the foreground, and report the output)

From the repository root, on this Android phone (Rust runs through the wrapper, one cargo command at a time):

1. `~/.local/bin/ubuntu-run cargo test -p impulcifer-service --lib settings`
2. `~/.local/bin/ubuntu-run cargo test -p impulcifer-service --test ui_catalog`
3. A script of your own (outside the repository) that loads the nine files and reports: the same 37 keys present
   in each, placeholders identical to `en.json` for every key, no key changed other than the 37 (compare with
   `git show HEAD:crates/impulcifer-service/locales/<file>`), no emoji and no em-dash in the 37 texts, and for
   the seven translated files no text left identical to the English except names that should be (list them).
4. The longest text of each label and button key per language, in characters, so the reviewer can see what may
   not fit: `nav_recovery`, `action_generate_brir`, `button_storage_access`, `chooser_select`,
   `update_button_download`, `update_button_skip_version`, `update_button_later`, `job_sheet_log`,
   `job_sheet_output`.

## Report

The final English and Korean of all 37 keys in a table, what you changed in the English and why, the terms you
chose per language for "all files access", "storage", "folder" and "download", the output of every verification
step, and anything you were unsure of. Finish the report before you end your turn.
