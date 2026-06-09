# -*- coding: utf-8 -*-
"""Matplotlib / PNG plotting helpers.

Split out of ``core/utils.py`` (audit #115 finding 8). ``core.utils``
re-exports these names.
"""

import os

import numpy as np
import matplotlib.ticker as ticker
from PIL import Image

try:
    ADAPTIVE_PALETTE = Image.Palette.ADAPTIVE
except AttributeError:
    ADAPTIVE_PALETTE = getattr(Image, 'ADAPTIVE')


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
    im = im.convert('P', palette=ADAPTIVE_PALETTE, colors=n_colors)
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
