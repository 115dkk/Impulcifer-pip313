/* 3.x wire contract. Keep method names and positional arguments in sync with
 * impulcifer-service and IpcMethod; declarations do not add runtime imports. */
type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };
type Envelope<T> = { ok: true; data: T } | { ok: false; error: IpcError };
interface IpcError { code: string; message: string; details: Record<string, JsonValue>; retryable: boolean }
type SharePreference = "auto" | "exclusive" | "shared";
type ShareMode = "exclusive" | "shared_auto_convert";
interface RecordingShare { requested: SharePreference; output: ShareMode; input: ShareMode }
interface Language { code: string; name: string }
interface UiSettings {
  language: string; theme: string; skin: string; frontend: string;
  first_run: boolean; languages: Language[]; strings: Record<string, string>;
}
interface SweepDefaults { layouts: string[]; default_fs: number; default_duration: number; speaker_names: string[] }
interface Bootstrap {
  version: string; platform: string; install_kind: string; webview_backend: string;
  brir_defaults: ProcessingRequest; sweep: SweepDefaults; ui: UiSettings; active_job: Job | null;
  capabilities: { recording: boolean; brir: boolean; output_recovery: boolean;
    recording_cancel: boolean; brir_cancel: boolean; output_recovery_cancel: boolean; share_modes: SharePreference[] };
}
interface SystemInfo {
  version: string; install_kind: string; os: string; cpu_count: number | null;
  runtime: { language: string; toolchain: string; audio_backend: string; shell?: string; webview?: string };
  paths: { data_dir: string; settings_path: string }; update_channel: "prerelease" | "stable";
}
interface AudioDevice { index: number; name: string; host_api: string; max_input_channels: number; max_output_channels: number }
interface AudioDevices { host_apis: string[]; devices: AudioDevice[]; default_input_index: number; default_output_index: number }
interface SweepRequest { mode: string; speakers: string; tracks: string; fs?: number; duration?: number }
interface RecordingRequest {
  mode: "speakers" | "headphones"; record_dir: string; play_path?: string; sweep?: SweepRequest;
  input_device: string | null; output_device: string | null; host_api: string | null;
  share_mode?: SharePreference | null; force_channels?: boolean; channels?: number;
  append?: boolean; debug_plots?: boolean; confirm_warnings?: boolean;
}
interface RecordingResult {
  record_path: string; summary: { sample_rate: number; channels: number; duration: number; peak_db: number; active_channels: number } | null;
  mode: "speakers" | "headphones"; sweep: string | null; sidecar_path: string | null;
  share: RecordingShare; warnings?: string[];
}
interface ProcessingRequest {
  dir_path?: string | null; test_signal?: string | null; fs?: number | null;
  room_target?: string | null; room_mic_calibration?: string | null; headphone_compensation_file?: string | null;
  eq_file?: string; eq_left_file?: string; eq_right_file?: string;
  plot?: boolean; interactive_plots?: boolean; channel_balance?: string | number | null;
  decay?: number | Record<string, number> | null; target_level?: number | null;
  fr_combination_method?: string; specific_limit?: number; generic_limit?: number;
  bass_boost_gain?: number; bass_boost_fc?: number; bass_boost_q?: number; tilt?: number;
  do_room_correction?: boolean; do_headphone_compensation?: boolean; do_equalization?: boolean;
  remove_silent_channels?: boolean; head_ms?: number; jamesdsp?: boolean; hangloose?: boolean;
  microphone_deviation_correction?: boolean; mic_deviation_strength?: number; mic_deviation_debug_plots?: boolean;
  output_truehd_layouts?: boolean; vbass?: boolean; vbass_freq?: number; vbass_hp?: number; vbass_polarity?: string;
  confirm_warnings?: boolean;
}
interface RecoveryRequest { dir_path: string; include_hangloose?: boolean; remove_silent_channels?: boolean; confirm_warnings?: boolean }
type RecoverySource = "hrir" | "hesuvi" | "hrir+hesuvi" | "hangloose";
interface RecoveryMetadata {
  source_kind: RecoverySource; source_path: string; output_dir: string; sample_rate: number;
  sample_count: number; speakers: string[]; existing_files: string[];
}
interface RecoveryPlan extends RecoveryMetadata {
  planned_files: { path: string; kind: "hrir" | "hesuvi" | "hangloose"; channels: number; speaker: string | null }[];
  hangloose_dir: string | null;
}
interface RecoveryResult extends RecoveryMetadata { created_files: string[] }
interface UpdateInfo {
  update_available: boolean; current_version: string; latest_version: string | null;
  download_url: string | null; release_notes: string | null; release_url: string | null;
}
interface UpdateRequest { download_url: string | null; latest_version: string | null }
interface UpdateResult { progress: number; status_key: string; status_default: string;
  title_key: string; title_default: string; message_key: string; message_default: string; requires_restart: boolean }
interface JobResults { recording: RecordingResult; brir: { output_path: string }; output_recovery: RecoveryResult; update: UpdateResult }
type JobKind = keyof JobResults;
type JobStatus = "running" | "cancel_requested" | "succeeded" | "failed" | "cancelled";
type JobOf<K extends JobKind> = { job_id: string; kind: K; status: JobStatus; cancellable: boolean; result: JobResults[K] | null; error: IpcError | null };
type Job = { [K in JobKind]: JobOf<K> }[JobKind];
interface ProgressPayload {
  progress?: number; message?: string; key?: string; phase?: string; elapsed?: number; duration?: number;
  speakers?: string[]; speaker?: string | null; segment_index?: number | null; segment_total?: number;
  segment_progress?: number | null;
}
type JobEvent = { seq: number; timestamp_ms: number } & (
  { type: "progress"; payload: ProgressPayload } |
  { type: "log"; payload: { level: string; message: string; key?: string; share?: RecordingShare } } |
  { type: "status"; payload: { status: JobStatus } }
);
interface JobPoll { job: Job; events: JobEvent[]; next_seq: number }
type SweepDetection = { found: false; sidecar: boolean } | { found: true; sidecar: boolean; confidence: string;
  fs: number; duration_seconds: number; n_segments: number; source_files: string[];
  speakers: string[]; is_default: boolean; generate_spec: string };
interface IpcApi {
  bootstrap(): Promise<Envelope<Bootstrap>>;
  list_audio_devices(host_api?: string | null): Promise<Envelope<AudioDevices>>;
  start_recording(request: RecordingRequest): Promise<Envelope<{ job: JobOf<"recording"> }>>;
  start_brir(request: ProcessingRequest): Promise<Envelope<{ job: JobOf<"brir"> }>>;
  start_output_recovery(request: RecoveryRequest): Promise<Envelope<{ job: JobOf<"output_recovery"> }>>;
  plan_output_recovery(request: RecoveryRequest): Promise<Envelope<RecoveryPlan>>;
  poll_job(job_id: string, after_seq?: number): Promise<Envelope<JobPoll>>;
  cancel_job(job_id: string): Promise<Envelope<{ job: Job }>>;
  get_ui_settings(): Promise<Envelope<UiSettings>>;
  set_language(code: string): Promise<Envelope<UiSettings>>;
  set_theme(theme: string): Promise<Envelope<{ theme: string }>>;
  set_skin(skin: string): Promise<Envelope<{ skin: string }>>;
  set_frontend(frontend: string): Promise<Envelope<{ frontend: string }>>;
  get_system_info(): Promise<Envelope<SystemInfo>>;
  resolve_recording_paths(record_dir: string, play_path?: string | null, mode?: string, sweep?: SweepRequest | null): Promise<Envelope<{ record_path: string }>>;
  detect_sweep(dir_path: string): Promise<Envelope<SweepDetection>>;
  generate_sweep_set(dir_path: string): Promise<Envelope<{ files: string[]; play_path: string | null }>>;
  open_path(path?: string | null): Promise<Envelope<{ path: string }>>;
  check_for_updates(): Promise<Envelope<UpdateInfo>>;
  start_update(request: UpdateRequest): Promise<Envelope<{ job: JobOf<"update"> }>>;
  apply_pending_update(): Promise<Envelope<{ restarting: boolean }>>;
  select_file(kind?: string | null): Promise<Envelope<{ path: string | null }>>;
  select_directory(): Promise<Envelope<{ path: string | null }>>;
  open_url(name: string): Promise<Envelope<{ url: string }>>;
}
type IpcMethod = keyof IpcApi;
type IpcResponse<M extends IpcMethod> = Awaited<ReturnType<IpcApi[M]>>;
interface AppState {
  booted: boolean; version: string; platform: string; strings: Record<string, string>; brirDefaults: ProcessingRequest;
  sweepDefaults?: Partial<SweepDefaults>; language: string; theme: string; skin: string;
  jobId: string | null; jobKind: JobKind | null; lastJob: Job | null; lastRecoveryJob: JobOf<"output_recovery"> | null;
  startPending: boolean; nextSeq: number; pollTimer: number | undefined; resolvedRecordPath: string;
  lastOutputDir: string | null; lastRecoveryOutputDir: string | null; systemThemeQuery: MediaQueryList | null;
  stageIndex: number; stageAbort: string | null; modalDismissed: boolean; recPhase: string | null;
  recDoneSpeakers: Set<string>; shareModes: SharePreference[]; systemInfo: SystemInfo | null;
  recoveryPlan: RecoveryPlan | null; recoveryPlanError: IpcError | null;
  recoveryState: "empty" | "planning" | "ready" | "nothing" | "error";
  recoveryTimer: number | undefined; recoveryRevision: number; recoveryCreated: Set<string>;
  recoveryRunRevision: number; recoveryRequestKey: string;
}
interface Window {
  pywebview?: { api: IpcApi };
  __TAURI__?: { core: { invoke<T = unknown>(command: string, payload?: Record<string, unknown>, options?: unknown): Promise<T> } };
  __impulciferSmoke?: SmokeObserver;
  __impulciferSmokeParams?: SmokeParams;
}
interface SmokeParams { demo: string; recovery: string; recording: string; hardware: boolean; en: Record<string, string>; labels: Record<string, string> }
type SmokeIpcResponse = { [M in IpcMethod]: { kind: "ipc-response"; method: M; response: IpcResponse<M>; time: number; step: string } }[IpcMethod];
type SmokeEvent = SmokeIpcResponse | { kind: "ipc-request" | "console" | "pageerror" | "resource-error"; time: number; step: string;
  method?: string; args?: unknown; level?: string; text?: string; failed?: boolean; stack?: string; filename?: string; line?: number };
interface SmokeObserver { events: SmokeEvent[]; startedAt: number; currentStep?: string }
interface SmokeStep { step: string; status: string; details?: Record<string, unknown>; error?: string; stack?: string;
  duration_seconds?: number; ui_log?: string; evidence_error?: string }
