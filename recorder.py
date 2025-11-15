# -*- coding: utf-8 -*-

import os
import re
import sounddevice as sd
from utils import read_wav, write_wav, read_audio, is_truehd_file
import numpy as np
from threading import Thread
import argparse


class DeviceNotFoundError(Exception):
    pass


def record_target(file_path, length, fs, channels=2, append=False):
    """Records audio and writes it to a file.

    Args:
        file_path: Path to output file
        length: Audio recording length in samples
        fs: Sampling rate
        channels: Number of channels in the recording
        append: Add track(s) to an existing file? Silence will be added to end of each track to make all equal in
                length

    Returns:
        None
    """
    print(">>>>>>>>> Recording Target Debug Info:")
    print(f"  File: {file_path}")
    print(f"  Length: {length} samples ({length/fs:.2f} seconds)")
    print(f"  Sample rate: {fs} Hz")
    print(f"  Channels: {channels}")
    print(f"  Append mode: {append}")
    
    recording = sd.rec(length, samplerate=fs, channels=channels, blocking=True)
    print(f"  Raw recording shape: {recording.shape}")
    
    # Analyze recording content
    print("  Recording content analysis:")
    for ch in range(recording.shape[1] if len(recording.shape) > 1 else 1):
        if len(recording.shape) > 1:
            ch_data = recording[:, ch]
        else:
            ch_data = recording
        max_val = np.max(np.abs(ch_data))
        rms_val = np.sqrt(np.mean(ch_data ** 2))
        print(f"    Channel {ch}: Max={max_val:.6f}, RMS={rms_val:.6f}, {'ACTIVE' if max_val > 1e-6 else 'EMPTY'}")
    
    # Transpose to have channels as rows (soundfile expects columns, but our system uses rows)
    if recording.shape[1] == channels and len(recording.shape) == 2:
        recording = np.transpose(recording)
        print(f"  After transpose: {recording.shape}")
    elif len(recording.shape) == 1:
        # Mono recording, expand dimensions
        recording = np.expand_dims(recording, axis=0)
        print(f"  Mono expanded to: {recording.shape}")
    
    max_gain = 20 * np.log10(np.max(np.abs(recording) + 1e-10))
    print(f"  Maximum gain: {max_gain:.2f} dB (headroom: {-1.0*max_gain:.1f} dB)")
    
    if append and os.path.isfile(file_path):
        # Adding to existing file, read the file
        print("  Appending to existing file...")
        _fs, data = read_wav(file_path, expand=True)
        print(f"  Existing file shape: {data.shape}")
        
        # Zero pad shorter to the length of the longer
        if recording.shape[1] > data.shape[1]:
            n = recording.shape[1] - data.shape[1]
            data = np.pad(data, [(0, 0), (0, n)])
            print(f"  Padded existing data by {n} samples")
        elif data.shape[1] > recording.shape[1]:
            padding = data.shape[1] - recording.shape[1]
            recording = np.pad(recording, [(0, 0), (0, padding)])
            print(f"  Padded new recording by {padding} samples")
        
        # Add recording to the end of the existing data
        recording = np.vstack([data, recording])
        print(f"  Final appended shape: {recording.shape}")
    
    write_wav(file_path, fs, recording)
    print("  File written successfully")
    print(f'>>>>>>>>> Headroom: {-1.0*max_gain:.1f} dB')


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
        append=False):
    """Plays one file and records another at the same time
    
    Now supports TrueHD/MLP files in addition to WAV
    """
    # Create output directory
    out_dir, out_file = os.path.split(os.path.abspath(record))
    os.makedirs(out_dir, exist_ok=True)

    # Read playback file (now supports TrueHD)
    channel_info = None
    if is_truehd_file(play):
        print(f"Detected TrueHD/MLP file: {play}")
        fs, data, channel_info = read_audio(play)
        if channel_info:
            print(f"Channel layout ({len(channel_info)} channels): {', '.join(channel_info)}")
            
            # 채널 수가 많은 경우 경고
            if len(channel_info) > 8:
                print("WARNING: This file contains more than 8 channels.")
                print("Make sure your audio interface supports this many output channels.")
    else:
        # Original WAV reading
        fs, data = read_wav(play)
    
    n_channels = data.shape[0]
    print(f"Audio info: {fs}Hz, {n_channels} channels, {data.shape[1]} samples")
    print(f"Duration: {data.shape[1] / fs:.2f} seconds")

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
        kwargs={'channels': channels, 'append': append}
    )
    recorder.start()
    
    try:
        sd.play(np.transpose(data), samplerate=fs, blocking=True)
    except Exception as e:
        print(f"Playback error: {e}")
        raise
    
    recorder.join()
    print("Recording completed.")


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
    args = vars(arg_parser.parse_args())
    return args


if __name__ == '__main__':
    play_and_record(**create_cli())
