# P62: the service's Android share (ADR 0020)

Repository: `/data/data/com.termux/files/home/bridge/Impulcifer-pip313`, branch `claude/android-port`. Work only
inside it. Read first: `CLAUDE.md` (the sections "3.x Rust 워크스페이스" and "안드로이드 이식 (3.x)"),
`docs/adr/0020-android-app.md`, `crates/impulcifer-service/src/lib.rs`, `crates/impulcifer-service/src/update/`,
`crates/impulcifer-io/src/ffmpeg.rs`, `apps/impulcifer-app/ui/ipc.d.ts` (the wire contract; the three new methods
and `SystemInfo.device` are already declared there and are fixed) and `apps/impulcifer-app/src/android.rs` (what
the Android shell answers through `HostAdapter`).

The Android app is the same Tauri shell with its own page. The shell part is done (commit `5c7fa66`). This packet
gives the service its Android answers. Nothing here may change what the desktop app or the CLI does.

## Environment (an Android phone, not a PC)

This checkout lives in Termux. Rust runs inside a proot Ubuntu container through a wrapper, from the repository
root:

- `~/.local/bin/ubuntu-run cargo <args>`: host target (aarch64 Linux). The toolchain is the repository's 1.99.0.
- Android target: source the environment file first, in the same shell:
  `~/.local/bin/ubuntu-run bash -c '. /data/data/com.termux/files/usr/tmp/claude-10518/-data-data-com-termux-files-home-bridge/a788c326-b089-5f70-9d5f-6a4d1556e4b4/scratchpad/imp/android-env.sh; cargo <args> --target aarch64-linux-android'`.
  Test executables built for that target run in place, because the machine is an Android device.
- Run every command in the foreground and one cargo command at a time. Another session may be holding the build
  directory; cargo then waits for the lock, which is expected.
- There is no network need: do not add a crate that is not already in `Cargo.lock`, except as allowed below.
- Do not run `cargo test --workspace`, `cargo test -p impulcifer-service` without a filter (its pipeline tests take
  very long on this machine), the Python tests, or the UI gallery.

## Design (fixed)

### 1. Three IPC methods

Wire names `list_directory`, `get_storage_access`, `request_storage_access`. Register each in
`impulcifer_types::ipc::IpcMethod` (enum, `ALL`, wire name mapping), in `CANONICAL_IPC` of
`crates/impulcifer-policy/tests/gates.rs`, and in `features.toml` as `ipc.<name>` with `status = "implemented"`
and a test that exists (the registry gate checks it; follow the `ipc_<name>_shape` naming of the neighbours).
The response shapes are those of `ipc.d.ts` (`DirectoryListing`, `StorageAccess`, `{ opened: boolean }`).

**`list_directory(request?)`** in a new module `crates/impulcifer-service/src/browse.rs`:

- The request is omitted, `null`, or an object. `path` is an optional string (trimmed; empty means none). `kind` is
  optional, one of `dir`, `audio`, `text`, `wav`, `measurement`, default `dir`. Anything else is `INVALID_REQUEST`.
- `roots` in the response is `self.host.storage_roots()` as `{path, label, removable}`.
- Without a path the listing starts at the first root; with no roots, at the home directory (`std::env::home_dir`).
- The path must be absolute and an existing directory. Otherwise answer `FILE_NOT_FOUND` with the message
  `Folder does not exist.` and `details.path` (the page already turns that message into a sentence).
- A directory that cannot be read (permission denied) answers `INVALID_REQUEST`, message
  `The folder cannot be read.`, `details = {"kind":"access_denied","path":<path>}`, `retryable: true`. The page
  uses `details.kind` to offer the storage access screen. Do not add an `ErrorCode`.
- Entries: skip names that start with `.` and names that are not valid UTF-8. `dir` follows symbolic links
  (`std::fs::metadata`); an entry whose metadata cannot be read is skipped. A folder is always listed. A file is
  listed for `dir` (every file), and otherwise only with an extension of its kind, compared without case:
  `audio` = wav, mlp, thd, truehd; `text` = csv, txt; `wav` = wav; `measurement` = mdat, liveproject (the sets of
  `dialog_filters` in `apps/impulcifer-app/src/lib.rs`). `size` is the file length, `null` for a folder.
  `modified_ms` is milliseconds since the Unix epoch, `null` when the platform gives none. `path` is the entry's
  full path.
- Order: folders first, then by lower-cased name, then by name. List at most 2000 entries; `truncated` is true
  when the folder held more.
- `path` in the response is the listed folder without a trailing separator (the root itself stays as it is), in
  the form it was asked for: do not canonicalize (a person reads `/storage/emulated/0/...`, not `/data/media/...`).
- `parent` is `null` when the folder is one of the roots or has no parent; otherwise its parent.

**`get_storage_access()`**: `{"state": "not_required" | "granted" | "denied"}` from `self.host.storage_access()`
(`None`, `Some(true)`, `Some(false)`).

**`request_storage_access()`**: `{"opened": <bool>}` from `self.host.request_storage_access()`; an `Err` becomes
`INTERNAL_ERROR` with that message.

### 2. bootstrap

- `capabilities.recording` and `capabilities.recording_cancel` say whether this build can record. Add
  `fn can_record(&self) -> bool { true }` to `impulcifer_types::audio::AudioBackend` and return `false` from
  `impulcifer_audio_io`'s `NullBackend`. Nothing else about the null backend changes. `share_modes` stays as it is.
- `webview_backend` is `"android"` on Android (today everything that is not Windows or macOS says `"gtk"`).
- The existing test that pins the capabilities object (`crates/impulcifer-service/tests/ipc.rs`) runs with whatever
  backend that test builds; keep it truthful for both a recording backend and the null backend.

### 3. System information

- `os` is the shell's `platform_info()["os"]` when that is a non-empty string, otherwise today's
  `os_description()`. `device` is added to the response only when `platform_info()["device"]` is a non-empty
  string.
- `os_platform_name()` names Android `Android`.
- Without a shell (tests, the CLI in Termux) Android's `os_version()` cannot use procfs: an app process is denied
  `/proc/sys/kernel/osrelease` (checked on Android 17). Read the kernel release with
  `rustix::system::uname()` and report it as `Linux <release>`, so the description is
  `Android Linux <release> <arch>`. Add `rustix` as a dependency of `impulcifer-service` for
  `cfg(target_os = "android")` only, with the version already in `Cargo.lock` and just the features this needs
  (`std`, `system`); the lock file may change only by that. `rustix::system::uname` is a safe function; write no
  `unsafe`.
- The test `os_description_names_the_platform` must pass on both targets without weakening it.

### 4. Updates

- `InstallKind::Apk`, wire name `"apk"`. `install_kind::detect` returns it when the platform is `"android"` and
  the executable's file name starts with `app_process` (an app's process is `/system/bin/app_process64`; the CLI
  started from a shell is its own executable and stays `Dev`).
- `check::download_url` for the platform `"android"` picks the asset whose lower-cased name ends with
  `-<arch>.apk`, where `<arch>` is `std::env::consts::ARCH` (the release asset is
  `Impulcifer-<version>-aarch64.apk`). Keep the public signature; put the architecture behind a function that
  takes it as an argument so the choice is testable on any host. No APK for the architecture means no
  `download_url`, as on the other platforms.
- `UpdateState::run` for `InstallKind::Apk`: the app does not download or install. Check that
  `request.download_url` starts with the release prefix (`options.releases_url` without its trailing
  `latest/download`, which is `https://github.com/115dkk/Impulcifer-pip313/releases/` in production and a local
  address in tests) and ends with `.apk`, then call `host.open_url(url)`. Otherwise fail with
  `The update link is not a release of this app.`. On success the job's result is

  ```json
  {"status_key":"update_apk_status","status_default":"Opening the download...",
   "title_key":"update_apk_title","title_default":"Download opened",
   "message_key":"update_apk_message",
   "message_default":"The new version is downloading in your browser. Open the file when it finishes to install it.",
   "progress":1.0,"requires_restart":false}
  ```

  Nothing is staged, so `apply_pending_update` keeps answering as it does with nothing pending. Do not add the
  three keys to the catalogs; list them in the report (the catalog is another packet).

### 5. FFmpeg discovery (`crates/impulcifer-io/src/ffmpeg.rs`)

On Android only PATH is searched: `common_dirs()` returns nothing there (today it returns the desktop Linux
directories), and the hint of the "not found" error says that an Android app cannot run FFmpeg, so TrueHD and MLP
files must be converted to WAV first. The CLI in Termux keeps finding an FFmpeg on PATH. Automatic installation
does not apply on Android. Keep every other platform's behaviour and text byte for byte.

### 6. Not in this packet

The app crate (`apps/`), the page, the catalogs (`crates/impulcifer-service/locales/`, `i18n/`), documents,
workflows, `ipc.d.ts`, `HostAdapter`'s existing methods and the four new ones (they are fixed), DSP and io code
other than `ffmpeg.rs`. Do not commit; the maintainer's session reviews and commits.

## Allowed files

`crates/impulcifer-types/src/ipc.rs`, `crates/impulcifer-types/src/audio.rs`,
`crates/impulcifer-audio-io/src/null_backend.rs`, `crates/impulcifer-service/src/**` (new `browse.rs` included),
`crates/impulcifer-service/tests/**`, `crates/impulcifer-service/Cargo.toml`, `crates/impulcifer-io/src/ffmpeg.rs`,
`crates/impulcifer-policy/tests/gates.rs` (the `CANONICAL_IPC` list and its length only), `features.toml`,
`Cargo.lock` (the `rustix` edge only), and tests of those crates that pin the things above.

## Tests to write

With a fake `HostAdapter` (roots, access state, recorded `open_url` calls, `platform_info`) and temporary folders:

- `list_directory`: default start (first root; home without roots), every kind's file filter with mixed-case
  extensions, hidden names skipped, folders before files and the name order, `truncated` at the limit, `parent`
  null at a root and at `/`, a missing folder, a relative path, a file given as the folder, an unreadable folder
  (remove the read permission on a temporary folder; skip that case when the process can read it anyway, as root
  can), `size` and `modified_ms`, the response path without a trailing separator, invalid `kind`.
- storage access: the three states and both `request` outcomes.
- bootstrap: `recording` false with the null backend and true with a backend that records; `webview_backend`.
- system information: the shell's `os` and `device` used when given, the service's own description otherwise,
  `device` absent without one.
- install kind: `app_process64` and `app_process32` on `"android"` are `Apk`; a CLI path on `"android"` and
  `app_process64` on `"linux"` are `Dev`.
- `download_url`: Android with the matching, a non-matching and no APK asset; the other platforms unchanged.
- the APK update job: the link is opened once with the exact URL and the result above; a URL outside the release
  prefix, a non-APK asset and an empty URL fail without opening anything.
- FFmpeg: the Android directory list and hint, as pure functions of the platform so they are tested on any host.

## Verification (run all, in the foreground, and report the output)

From the repository root:

1. `~/.local/bin/ubuntu-run cargo fmt --all -- --check`
2. `~/.local/bin/ubuntu-run cargo clippy -p impulcifer-types -p impulcifer-audio-io -p impulcifer-io -p impulcifer-service -p impulcifer-policy --all-targets -- --no-deps -D warnings`
3. The same for Android, with the environment file sourced as shown above:
   `cargo clippy -p impulcifer-types -p impulcifer-audio-io -p impulcifer-io -p impulcifer-service --no-default-features --target aarch64-linux-android --all-targets -- --no-deps -D warnings`
4. `~/.local/bin/ubuntu-run cargo test -p impulcifer-policy`
5. `~/.local/bin/ubuntu-run cargo test -p impulcifer-types` and `~/.local/bin/ubuntu-run cargo test -p impulcifer-io --lib ffmpeg`
6. `~/.local/bin/ubuntu-run cargo test -p impulcifer-service --lib` and
   `~/.local/bin/ubuntu-run cargo test -p impulcifer-service --test ipc`, plus every other test file of the service
   you changed, by name.
7. On the Android target (environment file sourced): `cargo test -p impulcifer-service --no-default-features --target aarch64-linux-android --lib`
   and `cargo test -p impulcifer-service --no-default-features --target aarch64-linux-android --test ipc`.
   These run on the device itself. If a test that you did not touch fails only there, do not change it: report its
   name and output.
8. `~/.local/bin/ubuntu-run cargo check -p impulcifer-app --all-targets` (the desktop shell still builds).

## Report

Changed files with a one-line summary each; the new `features.toml` entries; the catalog keys the service now
names (`update_apk_*` and any other) with their English defaults; the exact output tail of every verification
step; anything in this packet that turned out not to fit the code, and what you did instead. Finish the report
before you end your turn.
