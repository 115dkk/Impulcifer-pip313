# PA04 (ASTRA): performance audit of `impulcifer-service` (IPC call and job event overhead) against the 2.x `ImpulciferService`

Follow `E:/Impulcifer/docs/rust/packets/PA-astra-perf-audit.md` exactly (method, allowed files, hard rules, verification, report). This file only names the crate and its operations. Read the crate first: `E:/Impulcifer/crates/impulcifer-service/src/{lib.rs,args.rs,paths.rs,settings.rs}`, `crates/impulcifer-jobs/src/registry.rs` (event journal, `poll`), `crates/impulcifer-service/examples/demo_brir.rs` (the PA03 driver), and the 2.x oracle `E:/Impulcifer/application/impulcifer_service.py` (`ImpulciferService` methods called directly, `_start_job`, `poll_job`, `_emit`).

Run every command in the foreground. Never use background execution. The Python side runs with `py -3.14` and, for the job-event operations (threads), also with the free-threaded venv named in the template. Other workers may be editing `crates/impulcifer-service/src/update/**`, `apps/**`, `crates/impulcifer-audio-io/**`, `crates/impulcifer-sys-win/**`; treat those as read-only. Your optimisations are limited to `crates/impulcifer-service/src/{lib.rs,args.rs,paths.rs,settings.rs,brir/run.rs}` and `crates/impulcifer-jobs/src/**` (semantics unchanged, all existing tests green).

## Operations and sizes

| op | Rust | Python oracle | size |
|---|---|---|---|
| `bootstrap` | `service.call("bootstrap", vec![])` | `ImpulciferService().bootstrap()` | one call, repeated 200 times (median per call) |
| `get_ui_settings` | `call("get_ui_settings")` | `get_ui_settings()` | 200 calls |
| `set_language_round_trip` | `set_language("ko")` then `("en")` | same | 100 pairs (includes the settings file write) |
| `resolve_recording_paths` | `call("resolve_recording_paths", [request])` | `resolve_recording_paths(request)` | 200 calls with the demo directory |
| `start_brir_to_first_event` | `start_brir` on a demo copy, time until the first `poll_job` returns an event | same with `poll_job` | 5 runs (this is job start latency, not the BRIR) |
| `poll_job_drain` | a job that emits 2000 log events; time to drain them with `poll_job(after_seq)` in pages | same (`_emit` 2000 events from a job thread, `poll_job` pages) | 2000 events, 5 runs |
| `job_event_emit` | emitting 10,000 events into the registry from the job thread | `_emit` 10,000 | 5 runs |
| `detect_sweep` | `call("detect_sweep", [demo_dir])` | `detect_sweep(demo_dir)` | 20 calls |
| `catalog_translate` | `Catalog::translate` of `cli_readme_processed` with two args | `loc.get` + `format` | 100,000 calls |

Use a temp copy of the demo inputs for the job operations, delete it at the end. The 2.x service constructs its own `JobRegistry`; call its methods in-process (no pywebview), exactly as the Rust bench calls `ImpulciferService::call` (no Tauri). The Tauri round trip itself has no Python opponent on this machine (WebView2 152 opens no CDP port for the 2.x app); measure it once anyway through the app smoke driver if `tests/app_smoke/driver.js` can time 200 `bootstrap` invokes (`performance.now()`), report it as "Rust app round trip, no opponent", and do not block on it.

Known suspects to check before measuring: `serde_json::Value` cloning of whole event journals in `poll`, settings file read on every `get_ui_settings`, catalogue cloning per call, `Mutex` held across the settings write, `format!` allocations in `translate`, and the demo input discovery walking the directory more than once.

Report to `docs/rust/perf/impulcifer-service.md`. The registry test name is `bench_smoke_impulcifer_service` (in `crates/impulcifer-service/tests/perf_smoke.rs`, next to the PA03 smoke).
