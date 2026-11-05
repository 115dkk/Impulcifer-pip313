# P37: cancel a recording, and record with an output that has fewer channels than the sweep

Repository (a git worktree): `/tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/wt-macsign`.
Work only inside it. Read its `CLAUDE.md` first (3.x workspace section: `forbid(unsafe_code)`, no new dependencies,
`features.toml`, no subprocesses outside `crates/impulcifer-io/src/ffmpeg.rs`).

Build with the shared target directory so the disk does not fill up:
`export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target` before every cargo command. Another cargo job may hold
the lock for a while; wait for it, do not delete anything in that directory.

A tester on macOS with an 8-channel HDMI receiver reported two problems.

## 1. A recording cannot be cancelled

`crates/impulcifer-service/src/lib.rs` starts recording jobs with `cancellable = false`
(`self.jobs.start(JobKind::Recording, false, ...)`) and bootstrap reports `"recording_cancel": false`, so a wrong start
can only be stopped by quitting the app. The audio session already supports cancellation:
`crates/impulcifer-audio-io/src/session.rs` takes the job's `CancelToken`, and "External cancellation always returns
Cancelled, never a partial Recording".

Change:

- Recording jobs are cancellable (`true`), bootstrap says `"recording_cancel": true`.
- `crates/impulcifer-service/src/recording/run.rs`: when `play_and_record` returns `AudioError::Cancelled` (or the
  token is cancelled when it returns), end the job as cancelled the same way BRIR jobs do (find how
  `JobFailure { cancelled: true, .. }` is produced for BRIR and follow it; `JobContext::check_cancelled` exists).
  Emit one last recorder progress event with `phase: "cancelled"`. Nothing is written: no recording file, no
  `test.wav` sidecar, no append to an existing file (the existing file must stay byte-identical).
- Cancelling before the streams open also ends as cancelled.
- Keep `JOB_NOT_CANCELLABLE` for jobs that still are not cancellable (output recovery); only the recording case
  changes.

Tests (`crates/impulcifer-service/tests/recording.rs`, `tests/ipc.rs`): replace the assertions that a recording refuses
`cancel_job` with: `start_recording` returns `"cancellable": true`; `cancel_job` during the fake backend's playback
returns ok; polling ends with status `cancelled`; the record path does not exist; with `append: true` and an existing
file the file is unchanged; bootstrap's `capabilities.recording_cancel` is `true`. Name the main test
`recording_job_can_be_cancelled_and_writes_nothing`. If a generic registry test in `ipc.rs` uses a non-cancellable
recording job only to exercise `JOB_NOT_CANCELLABLE`, keep that behaviour covered with a job kind that is still not
cancellable.

## 2. An output with fewer channels than the sweep

Today `resolve_devices` (`crates/impulcifer-service/src/recording/devices.rs`) requires the output device to have at
least as many channels as the playback, so a 7.1.4 sweep (12 channels) on an 8-channel output fails at once with
"Could not find any device which satisfies minimum channel count." The maintainer decided: do not block such a
recording; play what the device has, and say clearly what will be missing.

Change:

- `resolve_devices`: the output side no longer rejects a device for having fewer channels than the playback (it
  still needs at least one output channel, and every "device not found" / host API error stays as it is). The input
  side is unchanged. Keep the other error messages byte-identical (a 2.x golden test compares them).
- `run.rs` already truncates the playback rows to `output.max_output_channels` with an English warning. Keep the
  truncation (after the HDMI reordering, so the rows that are dropped are the ones past the device's last channel)
  and replace the plain warning by:
  - a log line `ctx.log(json!({"level":"warning","key":"recording_output_channels_short", "message": <English>,
    "device": <output name>, "available": N, "required": M, "first": N+1, "last": M, "missing": "<list>"}))`
    where `missing` is the speaker slots of the dropped rows joined by ", " (skip `LFE`/`LFE2`), taken from the
    effective track order of the playback (the HDMI order when it was applied, otherwise `sweep_track_order`; for a
    playback file without a known layout use the channel numbers, e.g. `"9, 10, 11, 12"`). English message:
    `"{device} has {available} output channels, so channels {first}–{last} ({missing}) were not played."`
  - the job result gains `"output_channels": {"available": N, "required": M, "missing": [..]}` and keeps the English
    text in `result["warnings"]` as today.
- The recording then proceeds; its checks will report the silent speakers as they do today.

Tests: `short_output_records_the_channels_it_has` (FakeBackend output with 8 channels, generated `7.1.4` sweep of all
its speakers with `channel_order: "standard"`: the job succeeds, the fake output received 8-channel playback, the log
has the key with `missing == "TFL, TFR, TBL, TBR"`, `first == 9`, `last == 12`, the result has `output_channels`;
with `channel_order: "hdmi"` the missing list is the same four). Update any existing test that expected the
minimum-channel error for the output side; keep the input-side and not-found cases.

## 3. Catalog keys

Add to all nine overlay catalogs `crates/impulcifer-service/locales/*.json`, with the English text in every file (a
translator replaces the others later), next to `recording_channel_order_hdmi`:

- `recording_output_channels_short`: `{device} has {available} output channels, so channels {first}–{last} ({missing}) were not played. The recording is missing those speakers.`
- `warning_output_channels_short`: `{device} has {available} output channels, but {layout} needs {required}. Channels {first}–{last} ({missing}) will not play, so the recording will be missing those speakers.`
- `recording_status_cancelled`: `Recording cancelled. Nothing was saved.`
- `button_cancel_recording`: `Cancel recording`

## features.toml

Add `recording.cancel` (tests: the cancel test above and the bootstrap assertion) and `recording.short_output`
(tests: `short_output_records_the_channels_it_has`), `kind = "recording"`, `status = "implemented"`.

## Allowed files

`crates/impulcifer-service/src/lib.rs` (the start flag and the bootstrap capability only),
`crates/impulcifer-service/src/recording/{run,devices}.rs`, `crates/impulcifer-service/tests/{recording,ipc}.rs`,
`crates/impulcifer-service/locales/*.json` (the four keys only), `features.toml`. Do not touch `apps/`, `.github/`,
docs, `Cargo.toml`, `Cargo.lock` or the audio-io crate.

## Verification (foreground; report the tail of each)

```bash
cd /tmp/claude-0/-home-user-Impulcifer-pip313/d119d755-b75f-582a-8401-386d627e2f71/scratchpad/wt-macsign
export CARGO_TARGET_DIR=/home/user/Impulcifer-pip313/target
cargo fmt --all -- --check
cargo clippy -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service --lib recording
cargo test -p impulcifer-service --test recording --test ipc --test recording_layouts --test recording_checks --test ui_catalog
cargo test -p impulcifer-policy
```

Run every command in the foreground and end your turn only after reporting the files changed, the test names and
the command outputs. Do not commit. Do not use an alias or any other trick to get past a policy gate.
