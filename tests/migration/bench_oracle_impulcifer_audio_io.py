"""PA05 foreground CABLE-A oracle and synchronous Rust CPU observer.

Core recorder is unmodified. Production observations delegate and are restored.
The opt-in explicit-stream fallback is NOT core.recorder.play_and_record.
The seven-segment file is a 43 s fixture, not the stereo sweep generator.
"""
from __future__ import annotations

import argparse
import ctypes
from contextlib import contextmanager, redirect_stdout
import io
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
OUTPUT = "CABLE-A Input (VB-Audio Cable A)"
INPUT = "CABLE-A Output (VB-Audio Cable A)"
HOST = "Windows WASAPI"
SWEEP = ROOT / "data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav"


def environment():
    import numpy as np
    import psutil
    import scipy
    import scipy.fft._basic_backend as fft_backend
    import sounddevice as sd

    print("executable", sys.executable)
    print("version", sys.version)
    print("GIL", getattr(sys, "_is_gil_enabled", lambda: None)())
    print("platform", platform.platform(), "processor", platform.processor())
    print("cores", psutil.cpu_count(logical=False), psutil.cpu_count())
    print("numpy", np.__version__, "scipy", scipy.__version__, "sounddevice", sd.__version__)
    print("PortAudio", sd.get_portaudio_version())
    print("FFT backend", fft_backend.__file__)
    print("threads", {key: os.environ.get(key) for key in (
        "OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS", "NUMEXPR_NUM_THREADS"
    )})
    apps = []
    for p in psutil.process_iter(["name"]):
        name = p.info["name"] or ""
        if any(s in name.lower() for s in ("discord", "convolver", "realphones", "spotify", "audiodg", "voicemeeter", "foobar", "audacity")):
            apps.append((p.pid, name))
    print("running_audio_apps_read_only", apps)
    np.show_config()
    scipy.show_config()


def row(name, size, values):
    print(f"| {name} | {size} | {statistics.median(values):.6f} | {min(values):.6f} |")


def measure(name, operation, calls=20):
    for _ in range(3):
        for _ in range(calls):
            operation()
    elapsed = []
    for batch in range(5):
        start = time.perf_counter()
        for _ in range(calls):
            operation()
        ms = (time.perf_counter() - start) * 1000
        elapsed.append(ms)
        print(f"raw op={name} batch={batch} calls={calls} wall_ms={ms:.6f}")
    row(name, f"{calls} calls/batch", elapsed)


def pair(sd):
    hosts = sd.query_hostapis()
    devices = sd.query_devices()

    def select(name, kind):
        # Device names here omit the space before '('; require the full identity.
        matches = [d for d in devices if d["name"].replace(" (", "(") == name.replace(" (", "(")
                   and hosts[d["hostapi"]]["name"] == HOST and d[f"max_{kind}_channels"] >= 2]
        if len(matches) != 1:
            raise RuntimeError(f"expected exactly one {HOST} {name}: {matches}")
        return matches[0]

    return select(INPUT, "input"), select(OUTPUT, "output")


def open_close_session(sd, devices, extra):
    # Keep the requested duplex open/close comparator; explicit callbacks are
    # needed only for playback/capture, not for an unstarted audio stream.
    stream = sd.Stream(device=tuple(device["index"] for device in devices),
                       samplerate=48000, channels=2, dtype="float32",
                       extra_settings=extra)
    stream.close()


@contextmanager
def observe(sd, devices):
    """Wrap public stream constructors; timestamp first nonempty capture callback."""
    observed = {"streams": [], "input_callbacks": 0, "output_callbacks": 0, "statuses": [],
                "first": None, "play_start": None, "rec_start": None, "captured_frames": None}
    original = {name: getattr(sd, name) for name in ("InputStream", "OutputStream", "play", "rec")}
    default_device = tuple(sd.default.device)
    default_extra = tuple(sd.default.extra_settings)
    original_thread_hook = threading.excepthook
    thread_errors = []
    streams = []

    def thread_error(args):
        thread_errors.append((args.thread, args.exc_value))
        original_thread_hook(args)

    threading.excepthook = thread_error

    def factory(kind):
        def construct(*args, **kwargs):
            expected = devices[0 if kind == "input" else 1]["index"]
            kwargs["device"] = expected
            callback = kwargs["callback"]

            def observed_callback(data, frames, clock, status):
                now = time.perf_counter()
                observed[f"{kind}_callbacks"] += 1
                if status:
                    observed["statuses"].append((kind, str(status)))
                if kind == "input" and frames > 0 and observed["first"] is None:
                    observed["first"] = now
                    observed["first_frames"] = frames
                return callback(data, frames, clock, status)

            kwargs["callback"] = observed_callback
            stream = original["InputStream" if kind == "input" else "OutputStream"](*args, **kwargs)
            streams.append(stream)
            observed["streams"].append({"kind": kind, "dtype": stream.dtype, "channels": stream.channels,
                                        "samplerate": stream.samplerate, "blocksize": stream.blocksize,
                                        "latency": stream.latency, "device": stream.device})
            real_start = stream.start
            def start():
                if kind == "input":
                    observed["input_start"] = time.perf_counter()
                return real_start()
            stream.start = start
            return stream
        return construct

    def play(*args, **kwargs):
        kwargs["device"] = devices[1]["index"]
        observed["play_start"] = time.perf_counter()
        return original["play"](*args, **kwargs)

    def rec(*args, **kwargs):
        kwargs["device"] = devices[0]["index"]
        observed["rec_start"] = time.perf_counter()
        result = original["rec"](*args, **kwargs)
        observed["captured_frames"] = result.shape[0]
        return result

    sd.InputStream = factory("input")
    sd.OutputStream = factory("output")
    sd.play = play
    sd.rec = rec
    try:
        yield observed
    finally:
        # Include failed starts, which sounddevice never assigns to _last_callback.
        # The production call has returned/joined before normal cleanup here.
        try:
            for stream in streams:
                stream.stop()
                stream.close()
            for thread, _ in thread_errors:
                thread.join()
        finally:
            for name, value in original.items():
                setattr(sd, name, value)
            threading.excepthook = original_thread_hook
            sd.default.device = default_device
            sd.default.extra_settings = default_extra
    if thread_errors:
        raise RuntimeError(f"production recorder worker failed: {observed}") from thread_errors[0][1]


def transport_hash(samples):
    # Hash only the existing float32 transport, outside all timed operations.
    value = 14695981039346656037
    for byte in samples.astype("<f4").tobytes():
        value = ((value ^ byte) * 1099511628211) & ((1 << 64) - 1)
    return f"{value:016x}"


def integrity(reference, captured):
    """Aligned full-band residual and interior zero/repeated-block diagnostics."""
    import numpy as np
    from scipy.signal import correlate, correlation_lags

    results = []
    for ch in range(2):
        x = np.asarray(reference[:, ch], dtype=np.float64)
        y = np.asarray(captured[:, ch], dtype=np.float64)
        corr = correlate(y, x, mode="full", method="fft")
        lag = int(correlation_lags(len(y), len(x))[np.argmax(np.abs(corr))])
        begin_x, begin_y = max(0, -lag), max(0, lag)
        n = min(len(x) - begin_x, len(y) - begin_y)
        a, b = x[begin_x:begin_x+n], y[begin_y:begin_y+n]
        energy = float(np.dot(a, a))
        gain = float(np.dot(a, b) / energy) if energy else 0.0
        residual = b - gain * a
        # Ignore reference silence and endpoints; zeros inside active sweep are suspect.
        active = np.abs(a) > 1e-5
        zero = active & (np.abs(b) < 1e-10)
        runs = np.diff(np.r_[False, zero, False].astype(np.int8))
        lengths = np.flatnonzero(runs == -1) - np.flatnonzero(runs == 1)
        repeated = sum(np.array_equal(b[i:i+128], b[i-128:i]) and np.max(np.abs(b[i:i+128])) > 1e-5
                       for i in range(128, n-127, 128))
        results.append({"channel": ch, "lag_frames": lag, "overlap_frames": n,
                        "truncated_reference_end": len(x)-begin_x-n, "gain": gain,
                        "residual_rms": float(np.sqrt(np.mean(residual**2))),
                        "residual_max": float(np.max(np.abs(residual))),
                        "active_zero_frames": int(zero.sum()),
                        "zero_runs_ge_16": int((lengths >= 16).sum()),
                        "max_zero_run": int(lengths.max()) if len(lengths) else 0,
                        "repeated_128_blocks": int(repeated)})
    return results


@contextmanager
def com_mta():
    """Balance S_OK and S_FALSE on the calling OS thread, outside timing."""
    ole32 = ctypes.windll.ole32
    ole32.CoInitializeEx.argtypes = (ctypes.c_void_p, ctypes.c_uint32)
    ole32.CoInitializeEx.restype = ctypes.c_long
    ole32.CoUninitialize.argtypes = ()
    ole32.CoUninitialize.restype = None
    hr = ole32.CoInitializeEx(None, 0)
    if hr < 0:
        raise OSError(f"CoInitializeEx(MTA) failed HRESULT=0x{hr & 0xffffffff:08x}")
    print(f"caller_COM_MTA HRESULT=0x{hr:08x} thread={threading.get_native_id()}")
    try:
        yield
    finally:
        ole32.CoUninitialize()


def explicit_record(sd, devices, playback, mono_to_stereo):
    """NOT core.recorder.play_and_record; independent explicit callback streams.

    The caller owns COM and constructs, starts and destroys both streams. File
    read, device lookup, allocation and stream destruction are all timed by caller.
    No convenience API and no preopened stream is used.
    """
    import numpy as np
    from core import recorder

    fs, tracks, _ = recorder.read_audio(str(playback), expand=True)
    if mono_to_stereo and tracks.shape[0] == 1:
        tracks = np.broadcast_to(tracks, (2, tracks.shape[1])).copy()
    data = np.ascontiguousarray(tracks.T, dtype="float32")
    devices = pair(sd)
    extra = tuple(recorder.wasapi_extra_settings(d, k, fs, 2)
                  for d, k in zip(devices, ("input", "output")))
    captured = np.empty_like(data)
    positions = [0, 0]
    done_in, done_out = threading.Event(), threading.Event()
    observed = {"streams": [], "statuses": [], "input_callbacks": 0,
                "output_callbacks": 0, "first": None, "first_1024": None}

    def capture(samples, frames, clock, status):
        now = time.perf_counter()
        observed["input_callbacks"] += 1
        if observed["first"] is None:
            observed["first"] = now
            observed["first_frames"] = frames
            observed["first_adc_time"] = clock.inputBufferAdcTime
        if status:
            observed["statuses"].append(("input", str(status)))
        n = min(frames, len(data) - positions[0])
        captured[positions[0]:positions[0]+n] = samples[:n]
        positions[0] += n
        if positions[0] >= 1024 and observed["first_1024"] is None:
            observed["first_1024"] = now
        if positions[0] == len(data):
            raise sd.CallbackStop

    def render(samples, frames, clock, status):
        observed["output_callbacks"] += 1
        if status:
            observed["statuses"].append(("output", str(status)))
        if positions[1] == 0:
            observed["first_dac_time"] = clock.outputBufferDacTime
        n = min(frames, len(data) - positions[1])
        samples[:n] = data[positions[1]:positions[1]+n]
        samples[n:] = 0
        positions[1] += n
        if positions[1] == len(data):
            raise sd.CallbackStop

    streams = []
    try:
        inp = sd.InputStream(device=devices[0]["index"], samplerate=fs, channels=2,
                             dtype="float32", extra_settings=extra[0], callback=capture,
                             finished_callback=done_in.set)
        streams.append(inp)
        observed["input_start"] = time.perf_counter()
        inp.start()
        out = sd.OutputStream(device=devices[1]["index"], samplerate=fs, channels=2,
                              dtype="float32", extra_settings=extra[1], callback=render,
                              finished_callback=done_out.set)
        streams.append(out)
        out.start()
        for kind, stream in zip(("input", "output"), streams):
            observed["streams"].append({"kind": kind, "dtype": stream.dtype,
                                        "channels": stream.channels, "samplerate": stream.samplerate,
                                        "blocksize": stream.blocksize, "latency": stream.latency,
                                        "device": stream.device})
        if not done_out.wait(len(data)/fs + 10) or not done_in.wait(10):
            raise RuntimeError(f"explicit stream callbacks did not complete: {observed}")
    finally:
        for stream in reversed(streams):
            stream.stop()
            stream.close()
    observed["captured_frames"], observed["submitted_frames"] = positions
    return captured, observed


def sweeps(sd, devices, op, directory, explicit=False):
    import numpy as np
    import psutil
    import soundfile as sf
    from core import recorder

    mono, fs = sf.read(SWEEP, dtype="float64", always_2d=True)
    assert mono.shape == (295270, 1) and fs == 48000
    process = psutil.Process()
    for name, segments, repetitions in (("play_record_headphones_sweep", 1, 5),
                                        ("play_record_7_speaker_set", 7, 3),
                                        ("first_sample_latency", 1, 10)):
        if op not in ("all", name, "capture_loop_cpu" if repetitions == 5 else name):
            continue
        frames = len(mono) * segments
        if segments == 1:
            data = np.repeat(mono, 2, axis=1)
            playback = SWEEP
        else:
            data = np.zeros((frames, 2), dtype="float64")
            for segment in range(7):
                data[segment * len(mono):(segment + 1) * len(mono), segment % 2] = mono[:, 0]
            playback = Path(directory) / "seven-segment-stereo.wav"
            sf.write(playback, data, fs, subtype="PCM_32")
        if name == "first_sample_latency":
            frames = 4800
            data = data[:frames]
            playback = Path(directory) / "latency.wav"
            sf.write(playback, data, fs, subtype="PCM_32")
        print(f"workload op={name} frames={frames} channels=2 rate={fs} duration_s={frames/fs:.12f} transport_fnv1a={transport_hash(data)}")
        walls, overheads, cpus, latencies = [], [], [], []
        for index in range(3 + repetitions):
            record = Path(directory) / "capture.wav"
            captured = []
            original_write = recorder.write_wav
            def collect(_path, rate, samples, *args, **kwargs):
                assert rate == fs
                captured.append(samples)
            recorder.write_wav = collect
            try:
                with com_mta():
                    before = process.cpu_times()
                    start = time.perf_counter()
                    if explicit:
                        samples, observed = explicit_record(sd, devices, playback, segments == 1)
                        captured.append(samples.T)
                    else:
                        with observe(sd, devices) as observed, redirect_stdout(io.StringIO()):
                            recorder.play_and_record(play=str(playback), record=str(record),
                                                     input_device=devices[0]["name"], output_device=devices[1]["name"],
                                                     host_api=HOST, channels=2, mono_to_stereo=segments == 1)
                    # Destruction/observer cleanup precedes BOTH wall and CPU end.
                    wall = (time.perf_counter() - start) * 1000
                    after = process.cpu_times()
            finally:
                recorder.write_wav = original_write
            cpu = ((after.user - before.user) + (after.system - before.system)) * 1000
            assert observed["captured_frames"] == frames, observed
            assert len(captured) == 1 and captured[0].shape == (2, frames), observed
            print("integrity", json.dumps(integrity(data, captured[0].T)))
            if observed["first"] is None:
                raise RuntimeError(f"no first capture callback: {observed}")
            latency = (observed["first"] - observed["input_start"]) * 1000
            overhead = wall - frames / fs * 1000
            print(f"raw op={name} run={index} warmup={index < 3} wall_ms={wall:.6f} overhead_ms={overhead:.6f} cpu_ms={cpu:.6f} first_callback_from_input_start_ms={latency:.6f} captured_frames={frames} diagnostics={json.dumps(observed)}")
            if index >= 3:
                walls.append(wall)
                overheads.append(overhead)
                cpus.append(cpu)
                latencies.append(latency)
        if name == "first_sample_latency":
            row(name, "input start to nonempty callback; 10 runs", latencies)
        else:
            row(name, f"{frames} frames; {repetitions} runs", walls)
            row(name + "_overhead", "wall minus playback duration", overheads)
            if segments == 1:
                row("capture_loop_cpu", "process user+kernel; 5 runs", cpus)


def observe_rust(command):
    """Foreground child with exact acknowledged boundaries; wait for exit.

    No polling sampler, detached process, or process left running on return.
    CPU includes the two boundary pipe operations, not loading/building/warmups.
    """
    import psutil

    env = dict(os.environ, IMPULCIFER_PA05_CPU_OBSERVER="1")
    cpu_values = []
    before = None
    process = None
    # Popen is needed for the bidirectional synchronous ACK protocol, not to
    # background the command. The parent only services the child until it exits.
    with subprocess.Popen(command, cwd=ROOT, env=env, stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          text=True, encoding="utf-8", errors="replace", bufsize=1) as child:
        for line in child.stdout:
            print(line, end="", flush=True)
            if line.startswith("PA05_CPU_BEGIN "):
                process = psutil.Process(int(line.split()[1]))
                before = process.cpu_times()
                child.stdin.write("ACK\n")
                child.stdin.flush()
            elif line.startswith("PA05_CPU_END "):
                after = process.cpu_times()
                ms = ((after.user - before.user) + (after.system - before.system)) * 1000
                cpu_values.append(ms)
                print(f"raw op=capture_loop_cpu batch={len(cpu_values)-1} cpu_ms={ms:.6f}", flush=True)
                child.stdin.write("ACK\n")
                child.stdin.flush()
        code = child.wait()
    if code:
        raise SystemExit(code)
    if cpu_values:
        row("capture_loop_cpu", "process user+kernel; 5 runs", cpu_values)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--environment-only", action="store_true")
    parser.add_argument("--explicit-streams", action="store_true",
                        help="authorized fallback; NOT core.recorder.play_and_record")
    parser.add_argument("--op", default="all", choices=["all", "enumerate_devices", "open_close_session",
                        "play_record_headphones_sweep", "play_record_7_speaker_set", "capture_loop_cpu", "first_sample_latency"])
    parser.add_argument("--reproduce-convenience-race", action="store_true")
    parser.add_argument("--observe-rust", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.environment_only:
        environment()
        return
    if sys.platform != "win32":
        raise SystemExit("PA05 requires Windows WASAPI")
    if args.observe_rust:
        observe_rust(args.observe_rust)
        return
    import sounddevice as sd
    from core import recorder

    devices = pair(sd)
    print("selected", devices)
    if args.reproduce_convenience_race:
        import numpy as np
        extra = tuple(recorder.wasapi_extra_settings(d, k, 48000, 2)
                      for d, k in zip(devices, ("input", "output")))
        captured = sd.rec(4800, samplerate=48000, channels=2, device=devices[0]["index"],
                          extra_settings=extra[0], blocking=False)
        recording_stream = sd.get_stream()
        print("before sd.play", recording_stream.active, recording_stream.closed)
        sd.play(np.zeros((4800, 2)), samplerate=48000, device=devices[1]["index"],
                extra_settings=extra[1], blocking=True)
        print("after sd.play", recording_stream.active, recording_stream.closed,
              "capture_shape", captured.shape)
        return
    print("| op | size | python median ms | python min ms |")
    if args.op in ("all", "enumerate_devices"):
        measure("enumerate_devices", lambda: (sd.query_devices(), recorder.get_host_api_names()))
    if args.op in ("all", "open_close_session"):
        extra = tuple(recorder.wasapi_extra_settings(d, k, 48000, 2)
                      for d, k in zip(devices, ("input", "output")))
        print("open policy=2.x shared; no exclusive trial; float32; extras", extra)
        measure("open_close_session", lambda: open_close_session(sd, devices, extra))
    if args.op not in ("enumerate_devices", "open_close_session"):
        with tempfile.TemporaryDirectory(prefix="impulcifer-pa05-") as directory:
            print("implementation", "explicit InputStream/OutputStream; NOT core.recorder.play_and_record"
                  if args.explicit_streams else "core.recorder.play_and_record with caller COM MTA")
            sweeps(sd, devices, args.op, directory, args.explicit_streams)


if __name__ == "__main__":
    # Establish MTA before production imports can initialize a different apartment.
    with com_mta():
        main()
