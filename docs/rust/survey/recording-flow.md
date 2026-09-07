# Impulcifer 2.x recording flow (survey for the Rust port, 2026-09-07, tree at e8b22c5)

Design input for the recording packet (P16). Line numbers are exact for the files named.

## 1. `application/impulcifer_service.py`

Constants: `_RECORDING_FIELDS = {mode, play_path, record_dir, input_device, output_device, host_api, channels, force_channels, append, debug_plots, confirm_warnings, sweep}` (L23-36), `_SWEEP_REQUEST_FIELDS = {mode, fs, duration, speakers, tracks}` (L37), `_SWEEP_MODES = ("default", "custom", "file")` (L38). Envelopes: `{"ok": true, "data": ...}` / `{"ok": false, "error": {code, message, details, retryable}}`; `_json_safe` turns dataclasses into dicts and tuples into arrays (so `RecorderProgressEvent` reaches the UI flat and `speakers` as an array).

### `list_audio_devices(host_api=None)` (L308-347)
Queries `sounddevice.query_hostapis()`/`query_devices()`. Unknown `host_api` name → `INVALID_REQUEST "Unknown host API."` with `details.host_api`. Filters devices by raw host API name only (no channel filtering; the frontend does that). Response:
```json
{"host_apis": ["MME", "Windows DirectSound", "Windows WASAPI", "Windows WDM-KS"],
 "devices": [{"index": 0, "name": "Speakers (Realtek)", "host_api": "Windows WASAPI", "max_input_channels": 0, "max_output_channels": 2}],
 "default_input_index": 1, "default_output_index": 3}
```
Any exception (including no PortAudio) → `DEVICE_ERROR`, `retryable: true`.

### `_validate_recording_request(request)` (L979-1082), in order
1. not a dict → `INVALID_REQUEST "Recording request must be an object."`
2. unknown keys → `INVALID_REQUEST "Unknown recording fields."` `details.fields` sorted
3. `mode` not `speakers|headphones` → `INVALID_REQUEST "mode must be speakers or headphones."` (default `speakers`)
4. blank `record_dir` → `INVALID_REQUEST "record_dir is required."`
5. sweep sub-object errors propagate verbatim (`_validate_sweep_request`)
6. no sweep spec and `play_path` blank or not a file → `FILE_NOT_FOUND "Playback file does not exist."` `details.path`
7. `channels` bool/non-int/outside 1..64 → `INVALID_REQUEST "channels must be an integer from 1 to 64."` (default 2)
8. `force_channels` not bool → `INVALID_REQUEST "force_channels must be a boolean."`
9. `confirm_warnings` not bool → `INVALID_REQUEST "confirm_warnings must be a boolean."`
10. headphones: no sweep spec and `inspect_headphones_playback(play_path)` invalid → `INVALID_REQUEST` whose message is the raw i18n key (`error_headphones_play_file_missing|unreadable|too_many_channels`), `details {path, channels}`; mono play file without `confirm_warnings` → `CONFIRMATION_REQUIRED "Mono playback produces generic L=R headphone compensation."` `details {"warning": "headphones_mono"}`; headphones force `channels = 2`, `append = false`, `record_path = <record_dir>/headphones.wav`
11. speakers: `append` not bool → `INVALID_REQUEST "append must be a boolean."`; channels forced to 2 unless `force_channels`; `validate_recording_setup(...).has_mismatch` without `confirm_warnings` → `CONFIRMATION_REQUIRED "Selected recording channels do not match the sweep speaker count."` with `details = {has_mismatch, expected_speakers, expected_channels, selected_channels}` (mismatch = `len(speakers)*2 != channels`, speakers from the file name); `record_path` = `<record_dir>/<speakers>.wav` (generated sweep: `resolve_record_path_for_speakers`; play file: `resolve_record_path`)
12. `debug_plots` not bool → `INVALID_REQUEST "debug_plots must be a boolean."`
Device strings are `_optional_string` (trimmed, blank → None). Internal data: `{mode, play_path|None, sweep_spec|None, record_path, input_device, output_device, host_api, channels, append, debug_plots}`.

### `_validate_sweep_request(sweep, mode)` (L1084-1159)
`None` → no spec (play-file flow). Non-dict → `"sweep must be an object."`; unknown keys → `"Unknown sweep fields."`; `mode` default `default`, else `"sweep.mode must be default, custom or file."`; `file` → no spec. Headphones ignore speakers/tracks and force `("FL","FR")`/`stereo`. `speakers`: default `["FL","FR"]`, a string is split on `,` (blanks dropped), list/tuple str-mapped, else `"sweep.speakers must be a list or comma-separated string."`; `tracks` default `stereo`, non-string → `"sweep.tracks must be a string."`; `custom`: `fs` default 48000 (non-bool int or integral float, else `"sweep.fs must be an integer."`), `duration` default 5.0 (non-bool number, else `"sweep.duration must be a number."`); non-custom modes force `fs = 48000`, `duration = 5.0`. Then `validate_sweep_spec` errors become `INVALID_REQUEST` with the `core/sweep_signal.py` messages.

### `start_recording(request)` (L349-426)
Job kind `recording`, **not cancellable**. Body: build `play_signal = build_sweep_playback(spec)` when a spec exists; `recorder.play_and_record(play, play_signal, record, input_device, output_device, host_api, channels, append, debug_plots, progress_callback=emit progress, mono_to_stereo = mode == headphones and play_signal is None)`; `DeviceNotFoundError` → `DEVICE_ERROR` (`retryable: true`). Afterwards: record file missing → `OUTPUT_MISSING "Recording finished without producing the expected file."` `details.record_path`; sidecar `test.wav` written only when generated sweep and speakers mode and `!spec.is_default_signal()`; summary `analyze_recording(record_path)` best-effort (`None` on failure). Result:
```json
{"mode": "speakers", "record_path": ".../FL,FR.wav",
 "summary": {"sample_rate": 48000, "channels": 2, "duration": 18.30, "peak_db": -6.4, "active_channels": 2} | null,
 "sweep": "sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz (generated)" | null,
 "sidecar_path": ".../test.wav" | null}
```
Events: only `status` (`{"status": "running"}` then terminal) and `progress` (the flat `RecorderProgressEvent`, always all ten keys). No `log` events. Busy → `JOB_BUSY "Another job is already running."` (`retryable: true`); `cancel_job` → `JOB_NOT_CANCELLABLE "recording jobs cannot be cancelled safely."`.

### `resolve_recording_paths(record_dir, play_path=None, mode="speakers", sweep=None)` (L673-701)
Blank dir → `INVALID_REQUEST "record_dir is required."`; headphones → `{"record_path": <dir>/headphones.wav}` before any sweep validation; spec → `<dir>/<speakers>.wav`; else `play_path` required (`INVALID_REQUEST "play_path is required for speaker recordings."`) and `resolve_record_path(dir, play)`. Returns exactly `{"record_path": os.path.join(record_dir, name)}` (not normalised).

`core/recording_naming.py`: `derive_record_filename(play_path)`: empty → `headphones.wav`; basename matching `sweep-seg-(<speaker list>)-` (case-insensitive, tokens longest-first) → `<UPPER,LIST>.wav`; else `<stem>.wav`. `record_filename_for_speakers(speakers)`: strip/uppercase; `ValueError` for empty, unknown (`'"XX" is not a recognised speaker name.'`) or duplicate (`"Speaker names must be unique."`).

### `generate_sweep_set(dir_path)` (L736-750)
Not a dir → `FILE_NOT_FOUND "Sweep set directory does not exist."`; exceptions → `INTERNAL_ERROR`. `core/sweep_set_generator.generate_sweep_set(dir)` with defaults (5.0 s, 48000, 32-bit): four stereo group files for `(FL,FR) (FC) (SL,SR) (BL,BR)` first, then the combined `FL,FR,FC,SL,SR,BL,BR` on `7.1` (8 channels, LFE silent); one shared estimator; names `sweep-seg-<speakers>-<tracks>-<file_name(32)>.wav`. Response `{"files": [...], "play_path": files[0] | null}`.

### `detect_sweep(dir_path)` (L703-734)
Not a dir → `FILE_NOT_FOUND "Measurement directory does not exist."`; `sidecar = isfile(<dir>/test.wav)`; no recordings → `{"found": false, "sidecar": bool}`; else `{"found": true, "sidecar", "fs", "duration_seconds": round(4), "n_segments", "speakers", "confidence": "high"|"low", "is_default", "generate_spec": "generate:%.2fs@%d", "source_files"}`.

### `bootstrap().sweep`
`{"layouts": SWEEP_TRACK_LAYOUTS, "default_fs": 48000, "default_duration": 5.0, "speaker_names": SPEAKER_NAMES}`; `capabilities.recording = true`, `recording_cancel = false`.

## 2. `core/recorder.py`

`HOST_API_PREFERENCE = ('WASAPI', 'DirectSound', 'MME')` (WDM-KS never), `WASAPI_HOST_API = 'Windows WASAPI'`. `wasapi_extra_settings(device, kind, fs, channels)` (L58-98): only for WASAPI devices with fs and channels; `check_input/output_settings(device index, channels, samplerate)`; on failure attach `WasapiSettings(auto_convert=True)` (shared-mode auto-convert only when the native probe fails; some Windows 11 24H2 communications endpoints deliver silence when auto-converted). `get_device(name, kind, host_api=None, min_channels=1)` (L254-312): strips `"Windows "` from API names, three branches (API in the name; explicit `host_api`; preference walk); errors are `DeviceNotFoundError` with the messages in the source. `get_devices` (L315-338): defaults from `sd.default.device`; the input is resolved with `min_channels = 1` (the requested channel count is never checked against the input device), the output with `min_channels = n_channels` of the play signal. `set_default_devices` (L341-366): pins `"<name> <full host API name>"` strings and per-direction extra settings.

`record_target(file_path, length, fs, channels=2, append=False, debug_plots=False)` (L178-246): `sd.rec(length, samplerate=fs, channels=channels, blocking=True)` → `(frames, channels)` float32 → transposed to tracks; headroom printed; `append`: read the existing file, zero-pad the shorter along samples, `vstack` (new tracks become extra rows); `write_wav(file_path, fs, recording)` = PCM_32.

`play_and_record(play=None, record=None, input_device=None, output_device=None, host_api=None, channels=2, append=False, progress_callback=None, progress_interval=0.25, debug_plots=False, mono_to_stereo=False, play_signal=None)` (L369-579), in order: `TypeError` without a source; `makedirs`; emit `loading` (message = play label); generated path uses `play_signal.data` (tracks × samples) and `fs`; file path `read_audio(play, expand=True)` (mono stays 2-D), TrueHD checks (Atmos object master → `ValueError`); `mono_to_stereo` broadcast; `n_channels = data.shape[0]`, `duration = samples / fs`; `segments` from `play_signal.segments` or `infer_sweep_segments(play, duration)`; prints; `get_devices(min_channels=n_channels)` (errors printed, re-raised); `playback_channels = min(n_channels, max_output_channels)`; `set_default_devices(in, out, fs, channels, playback_channels)`; emit `devices` (`duration`, `speakers`, message `"<in> → <out>"`); TrueHD writes `<stem>_channels.txt`; if the output has fewer channels the data is truncated with a warning; **recorder thread started first** (`record_target(record, samples, fs, channels, append, debug_plots)`), then a daemon progress thread (wall clock, `max(0.05, interval)`), then `sd.play(data.T, samplerate=fs, blocking=True)`; on playback error: emit `error` (message) and re-raise; `finally` stop the progress thread; emit `saving` (`progress 0.99`); `recorder.join()`; emit `complete` (`progress 1.0`). Contracts: one sample rate (the play signal's); the recording is exactly the playback's sample count (no trimming; the 2 s lead silence absorbs the start skew); input channels = the caller's `channels`; output PCM_32; fully blocking; no cancellation; recorder-thread failures surface only as `OUTPUT_MISSING`.

`_monitor_recording_progress` (L111-139): emits `event_for_elapsed(0)` first, then every interval. `print_cli_progress` (L142-169): CLI labels `"Now recording {speaker} ({i}/{n})"`, `"Recording silence / waiting for the next sweep"`, `"Loading playback file"`, `"Audio devices are ready"`, `"Saving recording"`, `"Recording complete"`, `"Recording error: {message}"`; dedupes on `(phase, speaker, segment_index, label)`; prints `"[Recorder] {progress:5.1f}% | {label}"`. CLI (`create_cli` L582-622): `--play`, `--record` required; `--input_device/--output_device/--host_api` suppressed defaults; `--channels` int 2; `--append`, `--debug_plots` flags.

## 3. `core/sweep_signal.py`, `core/recording_progress.py`

`SEQUENCE_SILENCE_SECONDS = 2.0`, `SIDECAR_FILENAME = "test.wav"`, `SIDECAR_BIT_DEPTH = 32`. `SweepSpec {fs = 48000, duration = 5.0 (minimum duration), speakers = ("FL","FR"), tracks = "stereo"}`, `is_default_signal()` ignores speakers/tracks. `validate_sweep_spec` messages: `"At least one speaker name is required."`, `'"{name}" is not a recognised speaker name.'`, `"Speaker names must be unique."`, `'Unsupported track configuration "{tracks}". Supported: mono, stereo, 5.1, 7.1, 7.1.4, 7.1.6.'`, `'"stereo" track configuration requires one or two speakers.'`, `'Speaker "{speaker}" is not available in the "{tracks}" layout.'`, `"Sampling rate must be between 8000 and 384000 Hz."`, `"Sweep duration must be between 0.1 and 60 seconds."`. `_quantize_like_bundled_wav`: PCM_32 round trip so the generated sequence equals file playback bit for bit (`impulcifer-io::pcm32_round_trip`). `build_sweep_playback(spec)`: estimator, `sweep_sequence`, quantise, mono forces `("FL",)`, segments `start = 2 + i*(sweep_seconds + 2)`, `end = start + sweep_seconds` (exact), `display_name = 'sweep-seg-<speakers>-<tracks>-<file_name(32)> (generated)'`. `write_sidecar(dir, estimator)` writes the mono test signal as `test.wav` (PCM_32).

`RecorderProgressEvent {phase: loading|devices|recording|saving|complete|error, elapsed = 0.0, duration = 0.0, progress = 0.0, speaker: Option, segment_index: Option, segment_total = 0, segment_progress: Option, speakers = (), message = ""}`. `infer_sweep_segments(play_file, total_duration)`: regex `sweep-seg-(<speakers>)-(<tracks>)-(<duration>)s-` on the basename (tokens longest-first); no match → empty; `sweep_duration <= 0` fallback `max(0, (total - 2*(n+1))/n)`; segment `i`: `start = 2 + i*(d + 2)`, `end = min(total, start + d)`, skipped when `end <= start`. `event_for_elapsed(elapsed, duration, segments)`: `progress = min(0.98, elapsed/duration)`; inside a segment: `phase recording`, speaker, index, total, `segment_progress = clamp((elapsed - start)/max(0.001, end - start), 0, 1)`; otherwise `recording` with `speaker None`, `segment_total = len(segments)`, `speakers` kept.

## 4. `webview_ui/app.js` contract
- `list_audio_devices(hostApi || null)`: reads `host_apis`, `devices[].name`, `max_input_channels`, `max_output_channels`; option values are device **names** (a `""` "Default" option).
- `resolve_recording_paths(recordDir, playPath, "speakers", sweep)`: reads `data.record_path`.
- Sweep payload: `{mode, speakers: "FL,FR" (comma string), tracks}` plus `fs` (trunc) and `duration` in `custom`; `file` mode sends `play_path` instead.
- `start_recording` payload: `{mode, record_dir, input_device|null, output_device|null, host_api|null, sweep | play_path}` plus, for speakers only, `force_channels`, `channels` (2 unless forced), `append`, `debug_plots`. Only `CONFIRMATION_REQUIRED` is special (confirm dialog, then resend with `confirm_warnings: true`); every other error renders `"CODE: message details"`.
- Poll loop at 250 ms; progress payloads with a `phase` key go to the recorder status renderer, which reads `phase, speaker, speakers, segment_index, segment_total, elapsed, duration, message, progress`. Result fields read: `record_path`, `summary.{channels,duration,peak_db,active_channels}`.
- `generate_sweep_set(folder)` reads `play_path` and `files.length`; `detect_sweep(dir)` reads `found, confidence, fs, duration_seconds, n_segments, source_files`.
- Known gaps to keep as they are: `details.play_channels` is never produced (the dialog falls back to the message), and the headphone play-file errors travel as raw i18n keys in `message`.

## 5. i18n
Neither the recorder nor the service uses the catalogue for messages (English literals and error strings); the UI translates `recording_status_*`, device/file/sweep labels, `message_sweep_set_*`, `message_record_headphones_*`, `message_channel_*`, `webview_status_*`, and sweep-detection keys client-side. The three `error_headphones_play_file_*` keys are transported as messages.

## 6. Pinning tests
`tests/test_recorder_devices.py` (host API preference, WASAPI auto-convert only on native-format failure, non-WASAPI untouched, index-based probes), `tests/test_sweep_signal.py` (default playback within one float32 ULP of the bundled WAV and bit-identical to a locally written PCM_32 file, segment timing, height layouts with silent LFE, filenames, `is_default_signal`, mono forces FL, sidecar round trip), `tests/test_recorder_progress.py` (import without sounddevice, segment inference, lifecycle phases loading → devices → recording → saving → complete, no TrueHD probe for WAV, mono handling, `mono_to_stereo` only on the headphones path, Atmos rejection), `tests/test_application_service.py` (argument mapping, JSON-safe events, confirmation flow, forced values, single active job, non-cancellable, path resolution, sweep set delegation, detect_sweep, sidecar rules), `tests/test_sweep_set_generator.py`, `tests/test_sweep_detection.py`.

Cross-cutting: progress is wall-clock, never stream position; the fraction ladder is `<= 0.98` during, `0.99` saving, `1.0` complete; no cancellation anywhere; recorder-thread failures are invisible in 2.x (worth surfacing in 3.x rather than reproducing); channel-mismatch validation is filename-based only and only when `force_channels`.
