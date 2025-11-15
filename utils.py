# -*- coding: utf-8 -*-

import os
import subprocess
import tempfile
import json
import numpy as np
import soundfile as sf
from scipy.fft import rfft
from scipy import signal
from PIL import Image
import matplotlib.ticker as ticker
import matplotlib.pyplot as plt
import platform
import shutil
from pathlib import Path

plt.rcParams['axes.unicode_minus'] = False

# FFmpeg 최소 요구 버전 (major.minor 형태)
MIN_FFMPEG_VERSION = (4, 0)

def get_ffmpeg_version(ffmpeg_path):
    """FFmpeg 버전을 확인합니다."""
    try:
        result = subprocess.run([ffmpeg_path, '-version'], 
                              capture_output=True, text=True, timeout=10)
        if result.returncode == 0:
            # 첫 번째 줄에서 버전 정보 추출
            first_line = result.stdout.split('\n')[0]
            # 'ffmpeg version X.Y.Z' 형태에서 버전 추출
            if 'version' in first_line:
                version_part = first_line.split('version')[1].strip().split()[0]
                
                # Git 빌드 버전 처리 (N-xxxxx-gxxxxxx 형태)
                if version_part.startswith('N-'):
                    # Git 빌드의 경우 빌드 번호를 확인하여 대략적인 버전 추정
                    try:
                        build_num = int(version_part.split('-')[1])
                        # 대략적인 매핑: N-55702는 2013년경 버전 (1.x 대)
                        if build_num < 60000:  # 대략 2014년 이전
                            return (1, 0)  # 구버전으로 분류
                        elif build_num < 80000:  # 대략 2016년 이전
                            return (3, 0)
                        elif build_num < 100000:  # 대략 2019년 이전
                            return (4, 0)
                        else:  # 최신 빌드
                            return (6, 0)
                    except (ValueError, IndexError):
                        return (1, 0)  # 파싱 실패시 구버전으로 간주
                
                # 숫자로 시작하는 일반 버전 추출
                version_nums = []
                for part in version_part.split('.'):
                    try:
                        # 숫자가 아닌 문자가 나오면 중단
                        clean_part = ''
                        for char in part:
                            if char.isdigit():
                                clean_part += char
                            else:
                                break
                        if clean_part:
                            version_nums.append(int(clean_part))
                        else:
                            break
                    except ValueError:
                        break
                
                if len(version_nums) >= 2:
                    return tuple(version_nums[:2])
        return None
    except (subprocess.TimeoutExpired, FileNotFoundError, Exception):
        return None

def find_ffmpeg_in_common_paths():
    """일반적인 경로에서 FFmpeg를 찾습니다."""
    system = platform.system().lower()
    
    common_paths = []
    
    if system == 'windows':
        common_paths = [
            r'C:\ProgramData\chocolatey\bin\ffmpeg.exe',
            r'C:\Program Files\ffmpeg\bin\ffmpeg.exe',
            r'C:\Program Files (x86)\ffmpeg\bin\ffmpeg.exe',
            r'C:\ffmpeg\bin\ffmpeg.exe',
            r'C:\tools\ffmpeg\bin\ffmpeg.exe',
            Path.home() / 'AppData' / 'Local' / 'Microsoft' / 'WinGet' / 'Packages' / 'Gyan.FFmpeg_*' / 'ffmpeg-*' / 'bin' / 'ffmpeg.exe'
        ]
    elif system == 'darwin':  # macOS
        common_paths = [
            '/usr/local/bin/ffmpeg',
            '/opt/homebrew/bin/ffmpeg',
            '/usr/bin/ffmpeg',
            Path.home() / '.local' / 'bin' / 'ffmpeg'
        ]
    else:  # Linux
        common_paths = [
            '/usr/bin/ffmpeg',
            '/usr/local/bin/ffmpeg',
            '/snap/bin/ffmpeg',
            '/opt/ffmpeg/bin/ffmpeg',
            Path.home() / '.local' / 'bin' / 'ffmpeg'
        ]
    
    for path in common_paths:
        if isinstance(path, Path):
            # WinGet 패턴 처리
            if '*' in str(path):
                parent = path.parent.parent
                if parent.exists():
                    for subdir in parent.glob('*'):
                        ffmpeg_dirs = list(subdir.glob('ffmpeg-*'))
                        for ffmpeg_dir in ffmpeg_dirs:
                            ffmpeg_path = ffmpeg_dir / 'bin' / 'ffmpeg.exe'
                            if ffmpeg_path.exists():
                                version = get_ffmpeg_version(str(ffmpeg_path))
                                if version and version >= MIN_FFMPEG_VERSION:
                                    return str(ffmpeg_path), str(ffmpeg_path).replace('ffmpeg.exe', 'ffprobe.exe')
            else:
                path = str(path)
        
        if os.path.isfile(path):
            version = get_ffmpeg_version(path)
            if version and version >= MIN_FFMPEG_VERSION:
                probe_path = path.replace('ffmpeg', 'ffprobe')
                if system == 'windows' and not probe_path.endswith('.exe'):
                    probe_path += '.exe'
                return path, probe_path
    
    return None, None

def install_ffmpeg():
    """운영체제에 맞는 방법으로 FFmpeg를 설치합니다."""
    system = platform.system().lower()
    
    print("FFmpeg가 감지되지 않았거나 버전이 너무 오래되었습니다. 자동 설치를 시도합니다...")
    
    try:
        if system == 'windows':
            # Windows: Chocolatey 또는 winget 사용
            
            # 먼저 chocolatey 시도
            try:
                subprocess.run(['choco', '--version'], capture_output=True, check=True, timeout=10)
                print("Chocolatey를 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['choco', 'install', 'ffmpeg', '-y'], 
                                      capture_output=True, text=True, timeout=300)
                if result.returncode == 0:
                    return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass
            
            # winget 시도
            try:
                subprocess.run(['winget', '--version'], capture_output=True, check=True, timeout=10)
                print("WinGet을 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['winget', 'install', 'Gyan.FFmpeg'], 
                                      capture_output=True, text=True, timeout=300)
                if result.returncode == 0:
                    return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass
                
        elif system == 'darwin':  # macOS
            # Homebrew 사용
            try:
                subprocess.run(['brew', '--version'], capture_output=True, check=True, timeout=10)
                print("Homebrew를 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['brew', 'install', 'ffmpeg'], 
                                      capture_output=True, text=True, timeout=600)
                if result.returncode == 0:
                    return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass
                
        else:  # Linux
            # apt (Ubuntu/Debian) 시도
            try:
                subprocess.run(['apt', '--version'], capture_output=True, check=True, timeout=10)
                print("APT를 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['sudo', 'apt', 'update'], 
                                      capture_output=True, text=True, timeout=120)
                if result.returncode == 0:
                    result = subprocess.run(['sudo', 'apt', 'install', '-y', 'ffmpeg'], 
                                          capture_output=True, text=True, timeout=300)
                    if result.returncode == 0:
                        return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass
            
            # yum (CentOS/RHEL) 시도
            try:
                subprocess.run(['yum', '--version'], capture_output=True, check=True, timeout=10)
                print("YUM을 사용하여 FFmpeg를 설치합니다...")
                result = subprocess.run(['sudo', 'yum', 'install', '-y', 'ffmpeg'], 
                                      capture_output=True, text=True, timeout=300)
                if result.returncode == 0:
                    return find_ffmpeg_in_common_paths()
            except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
                pass
        
        print("자동 설치에 실패했습니다. 수동으로 FFmpeg를 설치해주세요.")
        return None, None
        
    except Exception as e:
        print(f"FFmpeg 설치 중 오류 발생: {e}")
        return None, None

def setup_ffmpeg():
    """FFmpeg를 설정하고 경로를 반환합니다."""
    # 1. 시스템 PATH에서 ffmpeg 확인
    ffmpeg_path = shutil.which('ffmpeg')
    ffprobe_path = shutil.which('ffprobe')
    
    if ffmpeg_path and ffprobe_path:
        version = get_ffmpeg_version(ffmpeg_path)
        if version and version >= MIN_FFMPEG_VERSION:
            print(f"시스템 PATH에서 FFmpeg {version[0]}.{version[1]} 감지됨")
            return ffmpeg_path, ffprobe_path
        else:
            print(f"시스템 PATH의 FFmpeg 버전이 너무 오래됨: {version}")
    
    # 2. 일반적인 경로에서 검색
    ffmpeg_path, ffprobe_path = find_ffmpeg_in_common_paths()
    if ffmpeg_path and ffprobe_path:
        version = get_ffmpeg_version(ffmpeg_path)
        print(f"로컬 경로에서 FFmpeg {version[0]}.{version[1]} 감지됨: {ffmpeg_path}")
        return ffmpeg_path, ffprobe_path
    
    # 3. 자동 설치 시도
    ffmpeg_path, ffprobe_path = install_ffmpeg()
    if ffmpeg_path and ffprobe_path:
        version = get_ffmpeg_version(ffmpeg_path)
        print(f"FFmpeg {version[0]}.{version[1]} 설치 완료: {ffmpeg_path}")
        return ffmpeg_path, ffprobe_path
    
    # 4. 모든 시도 실패
    print("FFmpeg를 찾거나 설치할 수 없습니다. TrueHD/MLP 지원이 비활성화됩니다.")
    return None, None

# FFmpeg 경로 자동 설정
FFMPEG_PATH, FFPROBE_PATH = setup_ffmpeg()

def is_truehd_file(file_path):
    """Check if file is TrueHD/MLP format"""
    # FFmpeg가 사용 불가능하면 False 반환
    if not check_ffmpeg_available():
        return False
    
    try:
        result = subprocess.run(
            [FFPROBE_PATH, '-v', 'error', '-select_streams', 'a:0', 
             '-show_entries', 'stream=codec_name', '-of', 'default=noprint_wrappers=1:nokey=1', 
             file_path],
            capture_output=True, text=True, timeout=10
        )
        if result.returncode != 0:
            return False
        
        codec = result.stdout.strip().lower()
        return codec in ['truehd', 'mlp']
    except Exception:
        return False

def convert_truehd_to_wav(truehd_path, output_path=None):
    """Convert TrueHD/MLP file to WAV format"""
    if not check_ffmpeg_available():
        raise RuntimeError("FFmpeg is not available for TrueHD conversion")
    
    if output_path is None:
        fd, output_path = tempfile.mkstemp(suffix='.wav')
        os.close(fd)
    
    # Get channel layout info first
    channel_info = get_truehd_channel_info(truehd_path)
    
    # Convert to WAV with proper channel mapping
    cmd = [
        FFMPEG_PATH, '-i', truehd_path,
        '-acodec', 'pcm_f32le',  # 32-bit float PCM
        '-ar', '48000',  # Sample rate
        output_path, '-y'
    ]
    
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    if result.returncode != 0:
        raise RuntimeError(f"FFmpeg conversion failed: {result.stderr}")
    
    return output_path, channel_info

def get_truehd_channel_info(file_path):
    """Get channel layout information from TrueHD file"""
    if not check_ffmpeg_available():
        return None
    
    try:
        result = subprocess.run(
            [FFPROBE_PATH, '-v', 'error', '-select_streams', 'a:0',
             '-show_entries', 'stream=channel_layout,channels',
             '-of', 'json', file_path],
            capture_output=True, text=True, timeout=10
        )
        
        if result.returncode != 0:
            return None
            
        info = json.loads(result.stdout)
        stream = info['streams'][0]
        
        channels = stream.get('channels', 0)
        stream.get('channel_layout', '')
        
        # Map channel layouts to speaker names
        from constants import CHANNEL_LAYOUT_MAP
        
        if channels in CHANNEL_LAYOUT_MAP:
            return CHANNEL_LAYOUT_MAP[channels]
        else:
            # Unknown layout, return None
            return None
    except Exception:
        return None

def read_audio(file_path, expand=False):
    """Read audio file (WAV or TrueHD/MLP)
    
    Returns:
        - Sample rate
        - Audio data (channels x samples)
        - Channel info (for TrueHD) or None
    """
    if is_truehd_file(file_path):
        # Convert TrueHD to temporary WAV
        temp_wav, channel_info = convert_truehd_to_wav(file_path)
        try:
            data, fs = sf.read(temp_wav)
            if len(data.shape) > 1:
                # Soundfile has tracks on columns, we want them on rows
                data = np.transpose(data)
            elif expand:
                data = np.expand_dims(data, axis=0)
            
            return fs, data, channel_info
        finally:
            # Clean up temp file
            if os.path.exists(temp_wav):
                os.remove(temp_wav)
    else:
        # Original WAV reading logic
        data, fs = sf.read(file_path)
        if len(data.shape) > 1:
            # Soundfile has tracks on columns, we want them on rows
            data = np.transpose(data)
        elif expand:
            data = np.expand_dims(data, axis=0)
        return fs, data, None

def check_ffmpeg_available():
    """Check if FFmpeg is available"""
    global FFMPEG_PATH, FFPROBE_PATH
    
    # FFmpeg 경로가 None이면 사용 불가
    if FFMPEG_PATH is None or FFPROBE_PATH is None:
        return False
    
    # 실제 파일 존재 및 실행 가능 여부 확인
    try:
        result = subprocess.run([FFMPEG_PATH, '-version'], 
                              capture_output=True, text=True, timeout=10)
        return result.returncode == 0
    except Exception:
        return False

def get_supported_audio_formats():
    """Get list of supported audio formats"""
    formats = {
        'wav': 'WAV Audio',
        'mlp': 'TrueHD/MLP Audio',
        'thd': 'TrueHD Audio',
        'truehd': 'Dolby TrueHD'
    }
    
    if not check_ffmpeg_available():
        # If FFmpeg not available, only support WAV
        return {'wav': 'WAV Audio'}
    
    return formats


def to_db(x):
    """Convert amplitude to dB

    Args:
        x: Amplitude value

    Returns:
        Value in dB
    """
    return 20 * np.log10(np.abs(x) + 1e-10)


def db_to_gain(x):
    """Convert dB to amplitude gain

    Args:
        x: Value in dB

    Returns:
        Amplitude gain
    """
    return 10 ** (x / 20)


def convolve(x, y):
    """Convolve two signals

    Args:
        x: First signal
        y: Second signal

    Returns:
        Convolved signal
    """
    return signal.convolve(x, y, mode='full')


def dB_unweight(x):
    """Remove dB weighting from a signal

    Args:
        x: Signal with dB weighting

    Returns:
        Signal without dB weighting
    """
    return 10 ** (x / 20)


def read_wav(file_path, expand=False):
    """Reads WAV file (backward compatibility wrapper)

    Args:
        file_path: Path to WAV file as string
        expand: Expand dimensions of a single track recording to produce 2-D array?

    Returns:
        - sampling frequency as integer
        - wav data as numpy array with one row per track, samples in range -1..1
    """
    fs, data, _ = read_audio(file_path, expand=expand)
    return fs, data


def write_wav(file_path, fs, data, bit_depth=32):
    """Writes WAV file."""
    # Ensure the directory exists before saving
    os.makedirs(os.path.dirname(file_path), exist_ok=True)
    
    if bit_depth == 16:
        subtype = "PCM_16"
    elif bit_depth == 24:
        subtype = "PCM_24"
    elif bit_depth == 32:
        subtype = "PCM_32"
    else:
        raise ValueError('Invalid bit depth. Accepted values are 16, 24 and 32.')
    if len(data.shape) > 1 and data.shape[1] > data.shape[0]:
        # We have tracks on rows, soundfile want's them on columns
        data = np.transpose(data)
    sf.write(file_path, data, samplerate=fs, subtype=subtype)


def magnitude_response(x, fs):
    """Calculates frequency magnitude response

    Args:
        x: Input signal
        fs: Sampling rate

    Returns:
        - Frequency values as numpy array
        - Frequency magnitudes as numpy array
    """
    # Use rfft for real-valued signals (more efficient than fft)
    X = rfft(x)
    # Magnitude in dB
    X_mag = 20 * np.log10(np.abs(X) + 1e-10)
    # Frequencies (positive only)
    f = np.arange(len(X)) * fs / (2 * len(X))
    return f, X_mag


def sync_axes(axes, sync_x=True, sync_y=True):
    """Synchronizes X and Y limits for axes

    Args:
        axes: List Axis objects
        sync_x: Flag depicting whether to sync X-axis
        sync_y: Flag depicting whether to sync Y-axis

    Returns:

    """
    x_min = []
    x_max = []
    y_min = []
    y_max = []
    for ax in axes:
        x_min.append(ax.get_xlim()[0])
        x_max.append(ax.get_xlim()[1])
        y_min.append(ax.get_ylim()[0])
        y_max.append(ax.get_ylim()[1])
    xlim = [np.min(x_min), np.max(x_max)]
    ylim = [np.min(y_min), np.max(y_max)]
    for ax in axes:
        if sync_x:
            ax.set_xlim(xlim)
        if sync_y:
            ax.set_ylim(ylim)


def get_ylim(x, padding=0.1):
    lower = np.min(x)
    upper = np.max(x)
    diff = upper - lower
    lower -= padding * diff
    upper += padding * diff
    return lower, upper


def versus_distance(angle=30, distance=3, breadth=0.148, ear='primary', sound_field='reverberant', sound_velocity=343):
    """Calculates speaker-ear distance delta, dealy delta and SPL delta

    Speaker-ear distance delta is the difference between distance from speaker to middle of the head and distance from
    speaker to ear.

    Dealy delta is the time it takes for sound to travel speaker-ear distance delta.

    SPL delta is the sound pressure level change in dB for a distance delta.

    Sound pressure attenuates by 3 dB for each distance doubling in reverberant room
    (http://citeseerx.ist.psu.edu/viewdoc/download?doi=10.1.1.10.1442&rep=rep1&type=pdf).

    Sound pressure attenuates by 6 dB for each distance doubling in free field and does not attenuate in diffuse field.

    Args:
        angle: Angle between center and the speaker in degrees
        distance: Distance from speaker to the middle of the head in meters
        breadth: Head breadth in meters
        ear: Which ear? "primary" for same side ear as the speaker or "secondary" for the opposite side
        sound_field: Sound field determines the attenuation over distance. 3 dB for "reverberant", 6 dB for "free"
                     and 0 dB for "diffuse"
        sound_velocity: The speed of sound in meters per second

    Returns:
        - Distance delta in meters
        - Delay delta in seconds
        - SPL delta in dB
    """
    if ear == 'primary':
        aa = (90 - angle) / 180 * np.pi
    elif ear == 'secondary':
        aa = (90 + angle) / 180 * np.pi
    else:
        raise ValueError('Ear must be "primary" or "secondary".')
    b = np.sqrt(distance ** 2 + (breadth / 2) ** 2 - 2 * distance * (breadth / 2) * np.cos(aa))
    d = b - distance
    delay = d / sound_velocity
    spl = np.log(b / distance) / np.log(2)
    if sound_field == 'reverberant':
        spl *= -3
    elif sound_field == 'free':
        spl *= -6
    elif sound_field == 'diffuse':
        spl *= -0
    else:
        raise ValueError('Sound field must be "reverberant", "free" or "diffuse".')
    return d, delay, spl


def optimize_png_size(file_path, n_colors=60):
    """Optimizes PNG file size in place.

    Args:
        file_path: Path to image
        n_colors: Number of colors in the PNG image

    Returns:
        None
    """
    im = Image.open(file_path)
    im = im.convert('P', palette=Image.ADAPTIVE, colors=n_colors)
    im.save(file_path, optimize=True)


def save_fig_as_png(file_path, fig, n_colors=60):
    """Saves figure and optimizes file size."""
    # Ensure the directory exists before saving
    os.makedirs(os.path.dirname(file_path), exist_ok=True)
    
    fig.savefig(file_path, bbox_inches='tight')
    optimize_png_size(file_path, n_colors=n_colors)


def config_fr_axis(ax):
    """Configures given axis instance for frequency response plots."""
    ax.set_xlabel('Frequency (Hz)')
    ax.semilogx()
    ax.set_xlim([20, 20e3])
    ax.set_ylabel('Amplitude (dB)')
    ax.grid(True, which='major')
    ax.grid(True, which='minor')
    ax.xaxis.set_major_formatter(ticker.StrMethodFormatter('{x:.0f}'))


def running_mean(x, N):
    cumsum = np.cumsum(np.insert(x, 0, 0))
    return (cumsum[N:] - cumsum[:-N]) / float(N)
