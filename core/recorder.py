# -*- coding: utf-8 -*-

import os
import re
import time
import sounddevice as sd
from core.utils import read_wav, write_wav, read_audio, is_truehd_atmos_object_master
from core.recording_progress import (
    RecorderProgressEvent,
    event_for_elapsed,
    infer_sweep_segments,
)
import numpy as np
from threading import Event, Thread
import argparse


class DeviceNotFoundError(Exception):
    pass


def _emit_progress(progress_callback, event):
    """Call the optional progress callback without letting UI errors affect audio."""
    if progress_callback is None:
        return
    try:
        progress_callback(event)
    except Exception:
        pass


def _monitor_recording_progress(
        progress_callback,
        stop_event,
        start_time,
        duration,
        segments,
        interval):
    """Emit wall-clock playback progress while ``sd.play(blocking=True)`` runs."""
    if progress_callback is None:
        return

    _emit_progress(
        progress_callback,
        event_for_elapsed(
            elapsed=0.0,
            duration=duration,
            segments=segments,
        ),
    )
    while not stop_event.wait(interval):
        elapsed = time.monotonic() - start_time
        _emit_progress(
            progress_callback,
            event_for_elapsed(
                elapsed=elapsed,
                duration=duration,
                segments=segments,
            ),
        )


def print_cli_progress(event):
    """Default CLI progress renderer for recorder sessions."""
    if event.phase == "recording" and event.speaker:
        label = (
            f"Now recording {event.speaker} "
            f"({event.segment_index}/{event.segment_total})"
        )
    elif event.phase == "recording":
        label = "Recording silence / waiting for the next sweep"
    elif event.phase == "loading":
        label = "Loading playback file"
    elif event.phase == "devices":
        label = "Audio devices are ready"
    elif event.phase == "saving":
        label = "Saving recording"
    elif event.phase == "complete":
        label = "Recording complete"
    elif event.phase == "error":
        label = f"Recording error: {event.message}"
    else:
        label = event.message or event.phase

    key = (event.phase, event.speaker, event.segment_index, label)
    if getattr(print_cli_progress, "_last_key", None) == key:
        return
    print_cli_progress._last_key = key
    progress = f"{event.progress * 100:5.1f}%" if event.progress else "  -- "
    print(f"[Recorder] {progress} | {label}", flush=True)


def _debug_recording(debug_plots, message):
    """Print verbose recorder diagnostics only when the debug option is enabled."""
    if debug_plots:
        print(message)


def record_target(file_path, length, fs, channels=2, append=False, debug_plots=False):
    """Records audio and writes it to a file.

    Args:
        file_path: Path to output file
        length: Audio recording length in samples
        fs: Sampling rate
        channels: Number of channels in the recording
        append: Add track(s) to an existing file? Silence will be added to end of each track to make all equal in
                length
        debug_plots: Print detailed recording diagnostics for troubleshooting

    Returns:
        None
    """
    _debug_recording(debug_plots, ">>>>>>>>> Recording Target Debug Info:")
    _debug_recording(debug_plots, f"  File: {file_path}")
    _debug_recording(debug_plots, f"  Length: {length} samples ({length/fs:.2f} seconds)")
    _debug_recording(debug_plots, f"  Sample rate: {fs} Hz")
    _debug_recording(debug_plots, f"  Channels: {channels}")
    _debug_recording(debug_plots, f"  Append mode: {append}")
    
    recording = sd.rec(length, samplerate=fs, channels=channels, blocking=True)
    _debug_recording(debug_plots, f"  Raw recording shape: {recording.shape}")
    
    # Analyze recording content
    if debug_plots:
        print("  Recording content analysis:")
        for ch in range(recording.shape[1] if len(recording.shape) > 1 else 1):
            if len(recording.shape) > 1:
                ch_data = recording[:, ch]
            else:
                ch_data = recording
            max_val = np.max(np.abs(ch_data))
            rms_val = np.sqrt(np.mean(ch_data ** 2))
            state = "ACTIVE" if max_val > 1e-6 else "EMPTY"
            print(f"    Channel {ch}: Max={max_val:.6f}, RMS={rms_val:.6f}, {state}")
    
    # Transpose to have channels as rows (soundfile expects columns, but our system uses rows)
    if len(recording.shape) == 2 and recording.shape[1] == channels:
        recording = np.transpose(recording)
        _debug_recording(debug_plots, f"  After transpose: {recording.shape}")
    elif len(recording.shape) == 1:
        # Mono recording, expand dimensions
        recording = np.expand_dims(recording, axis=0)
        _debug_recording(debug_plots, f"  Mono expanded to: {recording.shape}")
    
    max_gain = 20 * np.log10(np.max(np.abs(recording) + 1e-10))
    _debug_recording(debug_plots, f"  Maximum gain: {max_gain:.2f} dB (headroom: {-1.0*max_gain:.1f} dB)")
    
    if append and os.path.isfile(file_path):
        # Adding to existing file, read the file
        _debug_recording(debug_plots, "  Appending to existing file...")
        _fs, data = read_wav(file_path, expand=True)
        _debug_recording(debug_plots, f"  Existing file shape: {data.shape}")
        
        # Zero pad shorter to the length of the longer
        if recording.shape[1] > data.shape[1]:
            n = recording.shape[1] - data.shape[1]
            data = np.pad(data, [(0, 0), (0, n)])
            _debug_recording(debug_plots, f"  Padded existing data by {n} samples")
        elif data.shape[1] > recording.shape[1]:
            padding = data.shape[1] - recording.shape[1]
            recording = np.pad(recording, [(0, 0), (0, padding)])
            _debug_recording(debug_plots, f"  Padded new recording by {padding} samples")
        
        # Add recording to the end of the existing data
        recording = np.vstack([data, recording])
        _debug_recording(debug_plots, f"  Final appended shape: {recording.shape}")
    
    write_wav(file_path, fs, recording)
    _debug_recording(debug_plots, "  File written successfully")
    _debug_recording(debug_plots, f'>>>>>>>>> Headroom: {-1.0*max_gain:.1f} dB')


def get_host_api_names():
    """Gets names of available host APIs in a list"""
    return [hostapi['name'] for hostapi in sd.query_hostapis()]


def get_device(device_name, kind, host_api=None, min_channels=1):
    """Finds device with name, kind and host API

    Args:
        device_name: Device name
        kind: Device type. "input" or "output"
        host_api: Host API name
        min_channels: Minimum number of channels in the device

    Returns:
        Device, None if no device was found which satisfies the parameters
    """
    if device_name is None:
        raise TypeError('Device name is required and cannot be None')
    if kind is None:
        raise TypeError('Kind is required and cannot be None')
    # Available host APIs
    host_api_names = get_host_api_names()

    for i in range(len(host_api_names)):
        host_api_names[i] = host_api_names[i].replace('Windows ', '')

    if host_api is not None:
        host_api = host_api.replace('Windows ', '')

    # Host API check pattern
    host_api_pattern = f'({"|".join([re.escape(name) for name in host_api_names])})$'

    # Find with the given name
    device = None
    if re.search(host_api_pattern, device_name):
        # Host API in the name, this should return only one device
        device = sd.query_devices(device_name, kind=kind)
        if device[f'max_{kind}_channels'] < min_channels:
            # Channel count not satisfied
            raise DeviceNotFoundError(f'Found {kind} device "{device["name"]} {host_api_names[device["hostapi"]]}"" '
                                      f'but minimum number of channels is not satisfied. 1')
    elif not re.search(host_api_pattern, device_name) and host_api is not None:
        # Host API not specified in the name but host API is given as parameter
        try:
            # This should give one or zero devices
            device = sd.query_devices(f'{device_name} {host_api}', kind=kind)
        except ValueError:
            # Zero devices
            raise DeviceNotFoundError(f'No device found with name "{device_name}" and host API "{host_api}". ')
        if device[f'max_{kind}_channels'] < min_channels:
            # Channel count not satisfied
            raise DeviceNotFoundError(f'Found {kind} device "{device["name"]} {host_api_names[device["hostapi"]]}" '
                                      f'but minimum number of channels is not satisfied.')
    else:
        # Host API not in the name and host API is not given as parameter
        host_api_preference = [x for x in ['DirectSound', 'MME', 'WASAPI'] if x in host_api_names]
        for host_api_name in host_api_preference:
            # Looping in the order of preference
            try:
                device = sd.query_devices(f'{device_name} {host_api_name}', kind=kind)
                if device[f'max_{kind}_channels'] >= min_channels:
                    break
                else:
                    device = None
            except ValueError:
                pass
        if device is None:
            raise DeviceNotFoundError('Could not find any device which satisfies minimum channel count.')

    return device


def get_devices(input_device=None, output_device=None, host_api=None, min_channels=1):
    """Finds input and output devices

    Args:
        input_device: Input device name. System default is used if not given.
        output_device: Output device name. System default is used if not given.
        host_api: Host API name
        min_channels: Minimum number of output channels that the output device needs to support

    Returns:
        - Input device object
        - Output device object
    """
    # Find devices
    devices = sd.query_devices()

    # Select input device
    if input_device is None:
        # Not given, use default
        input_device = devices[sd.default.device[0]]['name']
    input_device = get_device(input_device, 'input', host_api=host_api)

    # Select output device
    if output_device is None:
        # Not given, use default
        output_device = devices[sd.default.device[1]]['name']
    output_device = get_device(output_device, 'output', host_api=host_api, min_channels=min_channels)

    return input_device, output_device


def set_default_devices(input_device, output_device):
    """Sets sounddevice default devices

    Args:
        input_device: Input device object
        output_device: Output device object

    Returns:
        - Input device name and host API as string
        - Output device name and host API as string
    """
    host_api_names = get_host_api_names()
    input_device_str = f'{input_device["name"]} {host_api_names[input_device["hostapi"]]}'
    output_device_str = f'{output_device["name"]} {host_api_names[output_device["hostapi"]]}'
    sd.default.device = (input_device_str, output_device_str)
    return input_device_str, output_device_str


def play_and_record(
        play=None,
        record=None,
        input_device=None,
        output_device=None,
        host_api=None,
        channels=2,
        append=False,
        progress_callback=None,
        progress_interval=0.25,
        debug_plots=False,
        mono_to_stereo=False):
    """Plays one file and records another at the same time.

    Now supports TrueHD/MLP files in addition to WAV.

    Args:
        mono_to_stereo: When ``True`` and the loaded play file is
            1-channel, the mono signal is duplicated onto both output
            channels. **Speaker-side capture must keep this off**:
            for files like ``sweep-seg-FL-mono-…wav`` the mono signal is
            meant to drive output channel 0 alone (i.e. the FL speaker)
            and broadcasting would also fire output channel 1 (FR),
            contaminating the FL impulse response with the FR speaker's
            response. The dedicated headphone-compensation flow opts in
            so a true mono playback can excite both headphone drivers
            simultaneously (yielding a generic L=R EQ — the user is
            warned about this trade-off in the GUI).
    """
    # Create output directory
    out_dir, out_file = os.path.split(os.path.abspath(record))
    os.makedirs(out_dir, exist_ok=True)

    _emit_progress(
        progress_callback,
        RecorderProgressEvent(phase="loading", message=str(play or "")),
    )

    # ``expand=True`` keeps mono inputs (e.g. the bundled
    # ``sweep-6.15s-...wav`` headphone-compensation sweep) on a 2-D
    # ``(channels, samples)`` shape so the channel/duration math below
    # never indexes a missing axis. Dropping ``expand`` previously caused
    # ``data.shape[1]`` to raise ``tuple index out of range`` for any
    # 1-channel file.
    fs, data, channel_info = read_audio(play, expand=True)
    if channel_info:
        print(f"Detected TrueHD/MLP file: {play}")
        print(f"Channel layout ({len(channel_info)} channels): {', '.join(channel_info)}")

        # 채널 수가 많은 경우 경고
        if len(channel_info) > 8:
            print("WARNING: This file contains more than 8 channels.")
            print("Make sure your audio interface supports this many output channels.")
    elif (
        os.path.splitext(play)[1].lower() in {'.mlp', '.thd', '.truehd'}
        and is_truehd_atmos_object_master(play)
    ):
        # Only Atmos-object masters (profile "Dolby TrueHD + Dolby Atmos",
        # e.g. the bundled 11cmaster.mlp / 13cmaster.mlp) are rejected:
        # FFmpeg only decodes the 7.1 bed and silently drops the
        # height/wide objects, so the user ends up with fewer channels
        # than the file name promises. Ordinary "Dolby TrueHD" 5.1/7.1
        # streams decode fully and fall through to normal playback —
        # they just have channel_info=None because CHANNEL_LAYOUT_MAP
        # only maps the custom 11/13-channel surround orders.
        raise ValueError(
            f"TrueHD/MLP file '{os.path.basename(play)}' is a Dolby Atmos "
            f"object master; FFmpeg decodes only its {data.shape[0]}-channel "
            "bed and the height/wide objects cannot be played as discrete "
            "outputs through a normal audio interface. Please use a "
            "multi-channel sweep WAV generated by Impulcifer instead."
        )

    # Opt-in mono→stereo broadcast — only the dedicated headphone path
    # asks for this. For speaker-side capture the mono signal must be
    # left on output channel 0 alone (see the ``mono_to_stereo`` docstring
    # above for the rationale).
    if mono_to_stereo and data.shape[0] == 1:
        data = np.broadcast_to(data, (2, data.shape[1])).copy()

    n_channels = data.shape[0]
    duration = data.shape[1] / fs
    segments = infer_sweep_segments(play, duration)
    print(f"Audio info: {fs}Hz, {n_channels} channels, {data.shape[1]} samples")
    print(f"Duration: {duration:.2f} seconds")

    # Find and set devices as default
    try:
        input_device, output_device = get_devices(
            input_device=input_device,
            output_device=output_device,
            host_api=host_api,
            min_channels=n_channels
        )
    except DeviceNotFoundError as e:
        print(f"Error: {e}")
        if n_channels > 8:
            print(f"This file requires {n_channels} output channels.")
            print("Consider using a professional audio interface with sufficient outputs.")
        raise
    
    input_device_str, output_device_str = set_default_devices(input_device, output_device)

    print(f'Input device:  "{input_device_str}"')
    print(f'Output device: "{output_device_str}" (max {output_device["max_output_channels"]} channels)')
    _emit_progress(
        progress_callback,
        RecorderProgressEvent(
            phase="devices",
            duration=duration,
            speakers=tuple(segment.speaker for segment in segments),
            message=f'{input_device_str} → {output_device_str}',
        ),
    )

    # If recording with TrueHD source, save channel info
    if channel_info and record:
        info_file = os.path.splitext(record)[0] + '_channels.txt'
        with open(info_file, 'w') as f:
            f.write(','.join(channel_info))
        print(f"Channel info saved to: {info_file}")

    # Check if output device supports required channels
    if output_device["max_output_channels"] < n_channels:
        print(f"WARNING: Output device only supports {output_device['max_output_channels']} channels")
        print(f"but file has {n_channels} channels. Audio will be truncated.")
        data = data[:output_device["max_output_channels"], :]

    recorder = Thread(
        target=record_target,
        args=(record, data.shape[1], fs),
        kwargs={'channels': channels, 'append': append, 'debug_plots': debug_plots}
    )
    recorder.start()

    progress_stop_event = Event()
    progress_thread = None
    if progress_callback is not None:
        progress_thread = Thread(
            target=_monitor_recording_progress,
            args=(
                progress_callback,
                progress_stop_event,
                time.monotonic(),
                duration,
                segments,
                max(0.05, progress_interval),
            ),
            daemon=True,
        )
        progress_thread.start()

    try:
        sd.play(np.transpose(data), samplerate=fs, blocking=True)
    except Exception as e:
        print(f"Playback error: {e}")
        progress_stop_event.set()
        _emit_progress(
            progress_callback,
            RecorderProgressEvent(
                phase="error",
                duration=duration,
                progress=0.0,
                message=str(e),
            ),
        )
        raise
    finally:
        progress_stop_event.set()
        if progress_thread is not None:
            progress_thread.join(timeout=1.0)

    _emit_progress(
        progress_callback,
        RecorderProgressEvent(
            phase="saving",
            elapsed=duration,
            duration=duration,
            progress=0.99,
            speakers=tuple(segment.speaker for segment in segments),
        ),
    )
    recorder.join()
    print("Recording completed.")
    _emit_progress(
        progress_callback,
        RecorderProgressEvent(
            phase="complete",
            elapsed=duration,
            duration=duration,
            progress=1.0,
            speakers=tuple(segment.speaker for segment in segments),
        ),
    )


def create_cli():
    """Create command line interface

    Returns:
        Parsed CLI arguments
    """
    arg_parser = argparse.ArgumentParser(
        description='Play and record audio files simultaneously. Supports WAV and TrueHD/MLP formats.'
    )
    arg_parser.add_argument('--play', type=str, required=True, 
                            help='File path to audio file to play. Supports .wav, .mlp, .thd, .truehd formats.')
    arg_parser.add_argument('--record', type=str, required=True,
                            help='File path to write the recording. This must have ".wav" extension and be either'
                                 '"headphones.wav" or any combination of supported speaker names separated by commas '
                                 'eg. FL,FC,FR.wav to be recognized by Impulcifer as a recording file. It\'s '
                                 'convenient to point the file path directly to the recording directory such as '
                                 '"data\\my_hrir\\FL,FR.wav".')
    arg_parser.add_argument('--input_device', type=str, default=argparse.SUPPRESS,
                            help='Name or number of the input device. Use "python -m sounddevice to '
                                 'find out which devices are available. It\'s possible to add host API at the end of '
                                 'the input device name separated by space to specify which host API to use. For '
                                 'example: "Zoom H1n DirectSound".')
    arg_parser.add_argument('--output_device', type=str, default=argparse.SUPPRESS,
                            help='Name or number of the output device. Use "python -m sounddevice to '
                                 'find out which devices are available. It\'s possible to add host API at the end of '
                                 'the output device name separated by space to specify which host API to use. For '
                                 'example: "Zoom H1n WASAPI"')
    arg_parser.add_argument('--host_api', type=str, default=argparse.SUPPRESS,
                            help='Host API name to prefer for input and output devices. Supported options on Windows '
                                 'are: "MME", "DirectSound" and "WASAPI". This is used when input and '
                                 'output devices have not been specified (using system defaults) or if they have no '
                                 'host API specified.')
    arg_parser.add_argument('--channels', type=int, default=2, help='Number of output channels.')
    arg_parser.add_argument('--append', action='store_true',
                            help='Add track(s) to existing file? Silence will be added to the end of all tracks to '
                                 'make the equal in length.')
    arg_parser.add_argument('--debug_plots', action='store_true',
                            help='Print detailed recorder diagnostics for troubleshooting.')
    args = vars(arg_parser.parse_args())
    return args


if __name__ == '__main__':
    cli_args = create_cli()
    cli_args["progress_callback"] = print_cli_progress
    play_and_record(**cli_args)
