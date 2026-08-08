# -*- coding: utf-8 -*-
"""Plotting mixin for :class:`core.hrir.HRIR`.

Provides matplotlib output (``plot``, ``plot_result``,
``plot_interaural_impulse_overlay``) and Bokeh layouts
(``generate_*_bokeh_layout``, ``generate_result_bokeh_figure``). The mixin
pattern keeps the original ``HRIR.plot()`` API usable while keeping
visualization code separate from ``core/hrir.py``.
"""

import os

import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
from PIL import Image

from bokeh.plotting import figure
from bokeh.models import HoverTool, ColumnDataSource, Range1d
from bokeh.palettes import Category10
from bokeh.layouts import gridplot

from core.utils import ADAPTIVE_PALETTE
from core.plotting.analysis import (
    band_interaural_level_difference,
    energy_decay_curve_db,
    band_interaural_phase_difference,
    interaural_cross_correlation,
    octave_bands,
)


class HRIRPlotter:
    """Mixin providing matplotlib + Bokeh visualization for ``HRIR``.

    Methods access ``self.fs`` and ``self.irs`` defined on the host class, so
    behavior is identical to the previous in-class implementation.
    """

    def plot(
        self,
        dir_path=None,
        plot_recording=True,
        plot_spectrogram=True,
        plot_ir=True,
        plot_fr=True,
        plot_decay=True,
        plot_waterfall=True,
        close_plots=True,
    ):
        """Plots all impulse responses with 2-pass axis synchronization.

        Pass 1: Generate each figure to collect axis limits.
        Pass 2: Regenerate each figure with synchronized limits and save.

        PNG로 저장만 하는 경로(``dir_path`` 지정 + ``close_plots=True``)는
        워커 프로세스에서 렌더링한다. matplotlib figure(3D waterfall,
        pcolormesh)는 소형 객체 수십만 개로 힙을 조각내 ``plt.close()`` +
        ``gc.collect()`` 후에도 수백 MB의 RSS가 프로세스 종료까지
        잔류하는데(BRIR 생성 후에도 점유 유지), 워커 프로세스는 종료 시
        메모리를 OS에 반환하므로 잔류가 없다.
        ``tests/test_plot_memory.py``가 이 계약을 고정한다.

        figure 객체 자체가 필요한 경로(``close_plots=False``, 예:
        room_correction)는 기존처럼 호출 프로세스 안에서 렌더링한다.
        """
        if dir_path is not None and close_plots:
            try:
                return self._plot_with_workers(
                    dir_path,
                    plot_recording=plot_recording,
                    plot_spectrogram=plot_spectrogram,
                    plot_ir=plot_ir,
                    plot_fr=plot_fr,
                    plot_decay=plot_decay,
                    plot_waterfall=plot_waterfall,
                )
            except Exception as e:
                # 워커 프로세스를 띄울 수 없는 환경(스폰 제한 등)에서는
                # 메모리 이점을 포기하고 기존 인프로세스 경로로 폴백한다.
                print(f"Worker-process plotting failed ({e!r}), "
                      "falling back to in-process rendering.")
        return self._plot_in_process(
            dir_path=dir_path,
            plot_recording=plot_recording,
            plot_spectrogram=plot_spectrogram,
            plot_ir=plot_ir,
            plot_fr=plot_fr,
            plot_decay=plot_decay,
            plot_waterfall=plot_waterfall,
            close_plots=close_plots,
        )

    # ImpulseResponse.plot()이 만드는 6개 축의 생성 순서와 동일한 인덱스.
    _PLOT_AXIS_FLAGS = (
        "plot_recording",
        "plot_ir",
        "plot_decay",
        "plot_spectrogram",
        "plot_fr",
        "plot_waterfall",
    )

    def _plot_with_workers(self, dir_path, max_workers=None, **plot_flags):
        """워커 프로세스에서 2-pass 렌더링을 수행하고 PNG를 저장한다.

        figure들은 서로 독립이라 병렬 렌더링이 가능하다(4코어에서 직렬
        워커 대비 ~2.5배). matplotlib은 스레드-불안전하므로 free-threaded
        런타임에서도 항상 프로세스 풀을 사용한다. 워커 수는 워커당 일시
        점유(수백 MB~1GB)를 감안해 가용 메모리에 맞춰 산정한다
        (:func:`core.parallel_utils.plot_worker_count`) — 대용량 메모리
        머신은 CPU/작업 수까지 병렬화하고, 저용량 머신은 자동으로 줄인다.
        """
        import multiprocessing
        from concurrent.futures import ProcessPoolExecutor

        from core.parallel_utils import get_available_memory_bytes, plot_worker_count
        from core.parallel_workers import render_hrir_figure_worker

        os.makedirs(dir_path, exist_ok=True)

        tasks = [
            (speaker, side, ir.data, ir.recording)
            for speaker, pair in self.irs.items()
            for side, ir in pair.items()
        ]
        if not tasks:
            return {}

        if max_workers is None:
            max_workers = plot_worker_count(
                len(tasks), os.cpu_count() or 1, get_available_memory_bytes()
            )

        ctx = multiprocessing.get_context("spawn")
        with ProcessPoolExecutor(max_workers=max_workers, mp_context=ctx) as pool:
            # --- Pass 1: 축 한계 수집 ---
            futures = [
                pool.submit(
                    render_hrir_figure_worker,
                    (speaker, side, data, recording, self.fs,
                     plot_flags, None, None, None),
                )
                for speaker, side, data, recording in tasks
            ]
            enabled = [
                idx
                for idx, flag_name in enumerate(self._PLOT_AXIS_FLAGS)
                if plot_flags[flag_name]
            ]
            limits = {
                idx: {'x_mins': [], 'x_maxs': [], 'y_mins': [], 'y_maxs': []}
                for idx in enabled
            }
            for future in futures:
                _speaker, _side, axis_limits = future.result()
                for idx in enabled:
                    if axis_limits[idx] is None:
                        continue
                    x_min, x_max, y_min, y_max = axis_limits[idx]
                    limits[idx]['x_mins'].append(x_min)
                    limits[idx]['x_maxs'].append(x_max)
                    limits[idx]['y_mins'].append(y_min)
                    limits[idx]['y_maxs'].append(y_max)

            sync_limits = {}
            for idx, lim in limits.items():
                if lim['x_mins']:
                    sync_limits[idx] = {
                        'xlim': [float(np.min(lim['x_mins'])), float(np.max(lim['x_maxs']))],
                        'ylim': [float(np.min(lim['y_mins'])), float(np.max(lim['y_maxs']))],
                    }

            # --- Pass 2: 동기화된 한계로 렌더링 + 저장 ---
            futures = [
                pool.submit(
                    render_hrir_figure_worker,
                    (speaker, side, data, recording, self.fs,
                     plot_flags, sync_limits,
                     os.path.join(dir_path, f"{speaker}-{side}.png"),
                     f"{speaker}-{side}"),
                )
                for speaker, side, data, recording in tasks
            ]
            for future in futures:
                future.result()

        return {}

    def _plot_in_process(
        self,
        dir_path=None,
        plot_recording=True,
        plot_spectrogram=True,
        plot_ir=True,
        plot_fr=True,
        plot_decay=True,
        plot_waterfall=True,
        close_plots=True,
    ):
        """호출 프로세스 안에서 렌더링하는 기존 2-pass 구현.

        figure 객체를 반환해야 하는 호출자(``close_plots=False``)와 워커
        스폰이 불가능한 환경의 폴백 경로.
        """
        import gc

        plot_flags = [
            plot_recording,
            plot_ir,
            plot_decay,
            plot_spectrogram,
            plot_fr,
            plot_waterfall,
        ]

        # --- Pass 1: Collect axis limits ---
        limits = {}
        for idx in range(6):
            if plot_flags[idx]:
                limits[idx] = {
                    'x_mins': [], 'x_maxs': [],
                    'y_mins': [], 'y_maxs': [],
                }

        for speaker, pair in self.irs.items():
            for side, ir in pair.items():
                fig = ir.plot(
                    plot_recording=plot_recording,
                    plot_spectrogram=plot_spectrogram,
                    plot_ir=plot_ir,
                    plot_fr=plot_fr,
                    plot_decay=plot_decay,
                    plot_waterfall=plot_waterfall,
                )
                axes_list = fig.get_axes()
                for idx in limits:
                    if idx < len(axes_list):
                        ax = axes_list[idx]
                        xlim = ax.get_xlim()
                        ylim = ax.get_ylim()
                        limits[idx]['x_mins'].append(xlim[0])
                        limits[idx]['x_maxs'].append(xlim[1])
                        limits[idx]['y_mins'].append(ylim[0])
                        limits[idx]['y_maxs'].append(ylim[1])
                plt.close(fig)

        sync_limits = {}
        for idx, lim in limits.items():
            if lim['x_mins']:
                sync_limits[idx] = {
                    'xlim': [float(np.min(lim['x_mins'])), float(np.max(lim['x_maxs']))],
                    'ylim': [float(np.min(lim['y_mins'])), float(np.max(lim['y_maxs']))],
                }

        gc.collect()

        # --- Pass 2: Render with synchronized limits ---
        figs = dict()
        if dir_path is not None:
            os.makedirs(dir_path, exist_ok=True)

        for speaker, pair in self.irs.items():
            for side, ir in pair.items():
                fig = ir.plot(
                    plot_recording=plot_recording,
                    plot_spectrogram=plot_spectrogram,
                    plot_ir=plot_ir,
                    plot_fr=plot_fr,
                    plot_decay=plot_decay,
                    plot_waterfall=plot_waterfall,
                )
                fig.suptitle(f"{speaker}-{side}")

                axes_list = fig.get_axes()
                for idx, sl in sync_limits.items():
                    if idx < len(axes_list):
                        axes_list[idx].set_xlim(sl['xlim'])
                        axes_list[idx].set_ylim(sl['ylim'])

                if dir_path is not None:
                    file_path = os.path.join(dir_path, f"{speaker}-{side}.png")
                    fig.savefig(file_path, bbox_inches="tight")
                    im = Image.open(file_path)
                    im = im.convert("P", palette=ADAPTIVE_PALETTE, colors=60)
                    im.save(file_path, optimize=True)
                    del im

                if close_plots:
                    plt.close(fig)
                else:
                    if speaker not in figs:
                        figs[speaker] = dict()
                    figs[speaker][side] = fig

        gc.collect()
        return figs

    def plot_result(self, dir_path):
        """Plot left and right side results with all impulse responses stacked."""
        # Local import to avoid circular dependency between core.hrir and core.plotting
        from core.impulse_response import ImpulseResponse

        stacks = [[], []]
        for speaker, pair in self.irs.items():
            for i, ir in enumerate(pair.values()):
                stacks[i].append(ir.data)
        left = ImpulseResponse(np.sum(np.vstack(stacks[0]), axis=0), self.fs)
        left_fr = left.frequency_response()
        left_fr.smoothen(
            window_size=1 / 3,
            treble_window_size=1 / 5,
            treble_f_lower=20000,
            treble_f_upper=23999,
        )
        right = ImpulseResponse(np.sum(np.vstack(stacks[1]), axis=0), self.fs)
        right_fr = right.frequency_response()
        right_fr.smoothen(
            window_size=1 / 3,
            treble_window_size=1 / 5,
            treble_f_lower=20000,
            treble_f_upper=23999,
        )

        fig, ax = plt.subplots()
        fig.set_size_inches(12, 9)
        left.plot_fr(
            fig=fig,
            ax=ax,
            fr=left_fr,
            plot_raw=True,
            raw_color="#7db4db",
            plot_smoothed=False,
        )
        right.plot_fr(
            fig=fig,
            ax=ax,
            fr=right_fr,
            plot_raw=True,
            raw_color="#dd8081",
            plot_smoothed=False,
        )
        left.plot_fr(
            fig=fig,
            ax=ax,
            fr=left_fr,
            plot_smoothed=True,
            smoothed_color="#1f77b4",
            plot_raw=False,
        )
        right.plot_fr(
            fig=fig,
            ax=ax,
            fr=right_fr,
            plot_smoothed=True,
            smoothed_color="#d62728",
            plot_raw=False,
        )
        ax.plot(
            left_fr.frequency, left_fr.smoothed - right_fr.smoothed, color="#680fb9"
        )
        ax.legend(
            ["Left raw", "Right raw", "Left smoothed", "Right smoothed", "Difference"]
        )

        file_path = os.path.join(dir_path, "results.png")

        os.makedirs(dir_path, exist_ok=True)

        fig.savefig(file_path, bbox_inches="tight")
        plt.close(fig)
        im = Image.open(file_path)
        im = im.convert("P", palette=ADAPTIVE_PALETTE, colors=60)
        im.save(file_path, optimize=True)

    def plot_interaural_impulse_overlay(self, dir_path, time_range_ms=(-5, 30)):
        """Plots interaural impulse response overlay for each speaker."""
        os.makedirs(dir_path, exist_ok=True)
        sns.set_theme(style="whitegrid")

        for speaker, pair in self.irs.items():
            fig, ax = plt.subplots(figsize=(12, 7))

            ir_left = pair.get("left")
            ir_right = pair.get("right")

            if not ir_left or not ir_right:
                plt.close(fig)
                continue

            peak_idx_left = ir_left.peak_index() if ir_left else None
            peak_idx_right = ir_right.peak_index() if ir_right else None

            if peak_idx_left is None or peak_idx_right is None:
                plt.close(fig)
                continue

            max_val = 0

            for side, ir_obj in [("left", ir_left), ("right", ir_right)]:
                if not ir_obj:
                    continue

                peak_idx = ir_obj.peak_index()
                if peak_idx is None:
                    continue

                start_sample = peak_idx + int(time_range_ms[0] * self.fs / 1000)
                end_sample = peak_idx + int(time_range_ms[1] * self.fs / 1000)

                start_sample = max(0, start_sample)
                end_sample = min(len(ir_obj.data), end_sample)

                if start_sample >= end_sample:
                    continue

                segment = ir_obj.data[start_sample:end_sample]
                time_axis = np.linspace(
                    time_range_ms[0]
                    + (
                        start_sample
                        - (peak_idx + int(time_range_ms[0] * self.fs / 1000))
                    )
                    * 1000
                    / self.fs,
                    time_range_ms[0]
                    + (
                        end_sample
                        - (peak_idx + int(time_range_ms[0] * self.fs / 1000))
                        - 1
                    )
                    * 1000
                    / self.fs,
                    num=len(segment),
                )

                sns.lineplot(x=time_axis, y=segment, label=f"{side.capitalize()} Ear")
                max_val = max(max_val, np.max(np.abs(segment)))

            ax.set_title(f"{speaker} - Interaural Impulse Response Overlay")
            ax.set_xlabel("Time relative to peak (ms)")
            ax.set_ylabel("Amplitude")
            if max_val > 0:
                ax.set_ylim(-max_val * 1.1, max_val * 1.1)
            ax.legend()
            ax.grid(True)

            plot_file_path = os.path.join(dir_path, f"{speaker}_interaural_overlay.png")
            try:
                os.makedirs(dir_path, exist_ok=True)

                fig.savefig(plot_file_path, bbox_inches="tight")
                im = Image.open(plot_file_path)
                im = im.convert(
                    "P", palette=ADAPTIVE_PALETTE, colors=128
                )
                im.save(plot_file_path, optimize=True)
            except Exception as e:
                print(f"Error saving/optimizing image {plot_file_path}: {e}")
            finally:
                plt.close(fig)

    def generate_interaural_impulse_overlay_bokeh_layout(self, time_range_ms=(-5, 30)):
        """Generates Bokeh layout for interaural impulse response overlay for each speaker."""
        plots = []
        num_speakers = len(self.irs.items())
        colors = Category10[max(3, min(10, num_speakers * 2))]
        color_idx = 0

        for speaker, pair in self.irs.items():
            ir_left = pair.get("left")
            ir_right = pair.get("right")

            if not ir_left or not ir_right:
                continue

            peak_idx_left = ir_left.peak_index() if ir_left else None
            peak_idx_right = ir_right.peak_index() if ir_right else None

            if peak_idx_left is None or peak_idx_right is None:
                continue

            align_peak_idx = min(peak_idx_left, peak_idx_right)
            time_vector_ms_left = (
                (np.arange(len(ir_left.data)) - align_peak_idx) / self.fs * 1000
            )
            time_vector_ms_right = (
                (np.arange(len(ir_right.data)) - align_peak_idx) / self.fs * 1000
            )

            source_left = ColumnDataSource(
                data=dict(time=time_vector_ms_left, amplitude=ir_left.data.squeeze())
            )
            source_right = ColumnDataSource(
                data=dict(time=time_vector_ms_right, amplitude=ir_right.data.squeeze())
            )

            p = figure(
                title=f"Interaural Impulse Response - {speaker}",
                x_axis_label="Time (ms relative to peak)",
                y_axis_label="Amplitude",
                tools="pan,wheel_zoom,box_zoom,reset,save,hover",
                active_drag="pan",
                active_scroll="wheel_zoom",
                height=200,
                sizing_mode="scale_both",
            )

            line_left = p.line(
                "time",
                "amplitude",
                source=source_left,
                legend_label="Left Ear",
                line_width=2,
                color=colors[color_idx % len(colors)],
            )
            color_idx += 1
            line_right = p.line(
                "time",
                "amplitude",
                source=source_right,
                legend_label="Right Ear",
                line_width=2,
                color=colors[color_idx % len(colors)],
                line_dash="dashed",
            )
            color_idx += 1

            p.x_range = Range1d(time_range_ms[0], time_range_ms[1])
            hover = p.select(dict(type=HoverTool))
            hover.tooltips = [
                ("Channel", "$name"),
                ("Time", "$x{0.00} ms"),
                ("Amplitude", "$y{0.0000}"),
            ]
            line_left.name = "Left Ear"
            line_right.name = "Right Ear"
            hover.renderers = [line_left, line_right]
            p.legend.location = "top_right"
            p.legend.click_policy = "hide"
            plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode="scale_both")
            return grid
        else:
            return None

    def generate_ild_bokeh_layout(self, freq_bands=None):
        """Generates Bokeh layout for Interaural Level Difference (ILD)."""
        plots = []
        if freq_bands is None:
            freq_bands = octave_bands(self.fs)

        unique_freq_bands_str = [f"{int(fb[0])}-{int(fb[1])}Hz" for fb in freq_bands]
        num_unique_speakers = len(self.irs.keys())
        palette_size = max(
            3, min(10, num_unique_speakers if num_unique_speakers > 0 else 3)
        )
        colors = Category10[palette_size]

        for i, (speaker, pair) in enumerate(self.irs.items()):
            ir_left = pair.get("left")
            ir_right = pair.get("right")
            if not ir_left or not ir_right:
                continue

            data_l_sq = ir_left.data.squeeze()
            data_r_sq = ir_right.data.squeeze()
            if data_l_sq.ndim > 1 or data_r_sq.ndim > 1:
                continue

            ild_values = band_interaural_level_difference(
                data_l_sq, data_r_sq, self.fs, freq_bands
            )

            if not ild_values or all(np.isnan(v) for v in ild_values):
                continue
            valid_indices = [k for k, v in enumerate(ild_values) if not np.isnan(v)]
            if not valid_indices:
                continue

            plot_bands = [unique_freq_bands_str[k] for k in valid_indices]
            plot_ilds = [ild_values[k] for k in valid_indices]
            source = ColumnDataSource(
                data=dict(
                    bands=plot_bands,
                    ilds=plot_ilds,
                    color=[colors[i % palette_size]] * len(plot_bands),
                )
            )

            p = figure(
                x_range=plot_bands,
                title=f"ILD - {speaker}",
                toolbar_location=None,
                tools="hover,save,pan,wheel_zoom,box_zoom,reset",
                height=175,
                sizing_mode="scale_both",
                x_axis_label="Frequency Band",
                y_axis_label="ILD (dB, Left/Right)",
            )
            p.vbar(
                x="bands",
                top="ilds",
                width=0.9,
                source=source,
                legend_label=speaker,
                line_color="color",
            )

            hover = p.select(dict(type=HoverTool))
            hover.tooltips = [("Band", "@bands"), ("ILD", "@ilds{0.0} dB")]
            p.xgrid.grid_line_color = None
            p.legend.orientation = "horizontal"
            p.legend.location = "top_center"
            p.legend.click_policy = "hide"
            plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode="scale_both")
            return grid
        else:
            return None

    def generate_ipd_bokeh_layout(self, freq_bands=None):
        """Generates Bokeh layout for Interaural Phase Difference (IPD).

        IPD는 대역 크로스 스펙트럼 합의 위상각이므로 항상 주각(principal
        angle) [-180°, 180°]로 보고된다. 이전의 ``unwrap_phase`` 인자는
        제거되었다 — ``False``가 반환하던 주각 차(합산 스펙트럼 위상의 raw
        차, 최대 ±360°)는 크로스 스펙트럼 방식에서는 정의되지 않고, 플롯
        y축도 ±180°로 고정되어 있어 범위 밖 값은 표시되지 않았다.
        """
        plots = []
        if freq_bands is None:
            freq_bands = octave_bands(self.fs)

        unique_freq_bands_str = [f"{int(fb[0])}-{int(fb[1])}Hz" for fb in freq_bands]
        num_unique_speakers = len(self.irs.keys())
        palette_size = max(
            3, min(10, num_unique_speakers if num_unique_speakers > 0 else 3)
        )
        colors = Category10[palette_size]

        for i, (speaker, pair) in enumerate(self.irs.items()):
            ir_left = pair.get("left")
            ir_right = pair.get("right")
            if not ir_left or not ir_right:
                continue

            data_l_sq = ir_left.data.squeeze()
            data_r_sq = ir_right.data.squeeze()
            if data_l_sq.ndim > 1 or data_r_sq.ndim > 1:
                continue

            ipd_values = band_interaural_phase_difference(
                data_l_sq, data_r_sq, self.fs, freq_bands
            )

            if not ipd_values or all(np.isnan(v) for v in ipd_values):
                continue
            valid_indices = [k for k, v in enumerate(ipd_values) if not np.isnan(v)]
            if not valid_indices:
                continue

            plot_bands = [unique_freq_bands_str[k] for k in valid_indices]
            plot_ipds = [ipd_values[k] for k in valid_indices]
            source = ColumnDataSource(
                data=dict(
                    bands=plot_bands,
                    ipds=plot_ipds,
                    color=[colors[i % palette_size]] * len(plot_bands),
                )
            )

            p = figure(
                x_range=plot_bands,
                title=f"IPD - {speaker}",
                toolbar_location=None,
                tools="hover,save,pan,wheel_zoom,box_zoom,reset",
                height=175,
                sizing_mode="scale_both",
                x_axis_label="Frequency Band",
                y_axis_label="IPD (Degrees, Left - Right)",
            )
            p.vbar(
                x="bands",
                top="ipds",
                width=0.9,
                source=source,
                legend_label=speaker,
                line_color="color",
            )

            hover = p.select(dict(type=HoverTool))
            hover.tooltips = [("Band", "@bands"), ("IPD", "@ipds{0.0} deg")]
            p.xgrid.grid_line_color = None
            p.y_range = Range1d(-180, 180)
            p.yaxis.ticker = [-180, -135, -90, -45, 0, 45, 90, 135, 180]
            p.legend.orientation = "horizontal"
            p.legend.location = "top_center"
            p.legend.click_policy = "hide"
            plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode="scale_both")
            return grid
        else:
            return None

    def generate_iacc_bokeh_layout(self, max_delay_ms=1):
        """Generates Bokeh layout for Interaural Cross-Correlation (IACC)."""
        plots = []
        num_unique_speakers = len(self.irs.keys())
        palette_size = max(
            3, min(10, num_unique_speakers if num_unique_speakers > 0 else 3)
        )
        colors = Category10[palette_size]

        for i, (speaker, pair) in enumerate(self.irs.items()):
            ir_left = pair.get("left")
            ir_right = pair.get("right")
            if not ir_left or not ir_right:
                continue

            data_l_sq = ir_left.data.squeeze()
            data_r_sq = ir_right.data.squeeze()
            if (
                data_l_sq.ndim > 1
                or data_r_sq.ndim > 1
                or not len(data_l_sq)
                or not len(data_r_sq)
            ):
                continue

            lags_ms, iacf, max_iacc_val, tau_iacc_ms_val = (
                interaural_cross_correlation(
                    data_l_sq, data_r_sq, self.fs, max_delay_ms=max_delay_ms
                )
            )

            if not len(iacf) or np.isnan(max_iacc_val):
                continue

            source = ColumnDataSource(
                data=dict(lags_ms=lags_ms, correlation=iacf)
            )

            p = figure(
                title=f"IACC - {speaker}",
                tools="hover,save,pan,wheel_zoom,box_zoom,reset",
                height=175,
                sizing_mode="scale_both",
                x_axis_label="Interaural Delay (ms)",
                y_axis_label="Cross-Correlation Coefficient",
            )
            p.line(
                "lags_ms",
                "correlation",
                source=source,
                line_width=2,
                color=colors[i % palette_size],
                legend_label=f"Max: {max_iacc_val:.2f} at {tau_iacc_ms_val:.2f}ms",
            )
            hover = p.select(dict(type=HoverTool))
            hover.tooltips = [
                ("Delay", "@lags_ms{0.00} ms"),
                ("Correlation", "@correlation{0.00}"),
            ]
            p.x_range = Range1d(-max_delay_ms * 1.1, max_delay_ms * 1.1)
            p.legend.location = "top_right"
            p.legend.click_policy = "hide"
            plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode="scale_both")
            return grid
        else:
            return None

    def generate_etc_bokeh_layout(self, time_range_ms=(0, 200), y_range_db=(-80, 0)):
        """Generates Bokeh layout for the Schroeder Energy Decay Curve (EDC).

        역사적 이유로 메서드/파일 이름은 ``etc``지만, 계산되는 값은 순간
        에너지(ETC)가 아니라 Schroeder 역적분 감쇠 곡선(EDC)이므로 플롯
        라벨은 EDC로 표기한다.
        """
        plots = []
        num_speakers = len(self.irs.items())
        palette_size = max(3, min(10, num_speakers * 2 if num_speakers > 0 else 3))
        colors = Category10[palette_size]
        color_idx = 0

        for speaker, pair in self.irs.items():
            p = figure(
                title=f"Energy Decay Curve (EDC) - {speaker}",
                tools="hover,save,pan,wheel_zoom,box_zoom,reset",
                height=200,
                sizing_mode="scale_both",
                x_axis_label="Time (ms)",
                y_axis_label="Decay (dB re total energy)",
            )
            has_data_for_speaker = False
            current_plot_lines = []

            for side, ir_obj in pair.items():
                if not ir_obj or len(ir_obj.data) == 0:
                    continue

                data_sq = ir_obj.data.squeeze()
                if data_sq.ndim > 1:
                    continue

                etc_db_vals = energy_decay_curve_db(data_sq, floor_db=y_range_db[0])

                time_axis = np.arange(len(etc_db_vals)) * 1000 / self.fs

                source = ColumnDataSource(data=dict(time=time_axis, etc=etc_db_vals))
                line = p.line(
                    "time",
                    "etc",
                    source=source,
                    legend_label=f"{side.capitalize()} Ear",
                    line_width=2,
                    color=colors[color_idx % palette_size],
                )
                line.name = f"{side.capitalize()} Ear"
                current_plot_lines.append(line)
                color_idx += 1
                has_data_for_speaker = True

            if has_data_for_speaker:
                p.x_range = Range1d(time_range_ms[0], time_range_ms[1])
                p.y_range = Range1d(y_range_db[0], y_range_db[1])
                hover = p.select(dict(type=HoverTool))
                hover.tooltips = [
                    ("Channel", "$name"),
                    ("Time", "$x{0.00} ms"),
                    ("Energy", "$y{0.00} dB"),
                ]
                hover.renderers = current_plot_lines
                p.legend.location = "top_right"
                p.legend.click_policy = "hide"
                plots.append(p)

        if plots:
            grid = gridplot(plots, ncols=min(2, len(plots)), sizing_mode="scale_both")
            return grid
        else:
            return None

    def generate_result_bokeh_figure(self):
        """Generates Bokeh figure for stacked left and right side results."""
        # Local import to avoid circular dependency between core.hrir and core.plotting
        from core.impulse_response import ImpulseResponse

        if not self.irs:
            return None

        stacks = [[], []]
        for speaker, pair in self.irs.items():
            if pair.get("left") and hasattr(pair["left"], "data"):
                stacks[0].append(pair["left"].data)
            if pair.get("right") and hasattr(pair["right"], "data"):
                stacks[1].append(pair["right"].data)

        if not stacks[0] or not stacks[1]:
            return None

        summed_left_data = (
            np.sum(np.vstack(stacks[0]), axis=0) if stacks[0] else np.array([0.0])
        )
        summed_right_data = (
            np.sum(np.vstack(stacks[1]), axis=0) if stacks[1] else np.array([0.0])
        )

        if len(summed_left_data) <= 1 or len(summed_right_data) <= 1:
            return None

        left_ir = ImpulseResponse(summed_left_data, self.fs)
        left_fr = left_ir.frequency_response()
        left_fr.smoothen(
            window_size=1 / 3,
            treble_window_size=1 / 5,
            treble_f_lower=20000,
            treble_f_upper=max(20001, int(self.fs / 2 - 1)),
        )

        right_ir = ImpulseResponse(summed_right_data, self.fs)
        right_fr = right_ir.frequency_response()
        right_fr.smoothen(
            window_size=1 / 3,
            treble_window_size=1 / 5,
            treble_f_lower=20000,
            treble_f_upper=max(20001, int(self.fs / 2 - 1)),
        )

        p = figure(
            title="Overall Smoothed Frequency Response",
            x_axis_label="Frequency (Hz)",
            y_axis_label="Amplitude (dB)",
            x_axis_type="log",
            tools="pan,wheel_zoom,box_zoom,reset,save,hover",
            active_drag="pan",
            active_scroll="wheel_zoom",
            height=300,
            sizing_mode="scale_both",
        )

        source_left_raw = ColumnDataSource(
            data=dict(freq=left_fr.frequency, raw=left_fr.raw)
        )
        source_left_smooth = ColumnDataSource(
            data=dict(freq=left_fr.frequency, smooth=left_fr.smoothed)
        )
        source_right_raw = ColumnDataSource(
            data=dict(freq=right_fr.frequency, raw=right_fr.raw)
        )
        source_right_smooth = ColumnDataSource(
            data=dict(freq=right_fr.frequency, smooth=right_fr.smoothed)
        )

        diff_smooth = left_fr.smoothed - right_fr.smoothed
        source_diff = ColumnDataSource(
            data=dict(freq=left_fr.frequency, diff=diff_smooth)
        )

        p.line(
            "freq",
            "raw",
            source=source_left_raw,
            line_width=1,
            color=Category10[3][0],
            alpha=0.5,
            legend_label="Left Raw",
            muted_alpha=0.1,
        )
        p.line(
            "freq",
            "raw",
            source=source_right_raw,
            line_width=1,
            color=Category10[3][1],
            alpha=0.5,
            legend_label="Right Raw",
            muted_alpha=0.1,
        )

        l_smooth = p.line(
            "freq",
            "smooth",
            source=source_left_smooth,
            line_width=2,
            color=Category10[3][0],
            legend_label="Left Smoothed",
        )
        r_smooth = p.line(
            "freq",
            "smooth",
            source=source_right_smooth,
            line_width=2,
            color=Category10[3][1],
            legend_label="Right Smoothed",
        )
        d_smooth = p.line(
            "freq",
            "diff",
            source=source_diff,
            line_width=2,
            color=Category10[3][2],
            legend_label="Difference (L-R)",
            line_dash="dashed",
        )

        p.x_range = Range1d(20, 20000)
        hover = p.select(dict(type=HoverTool))
        hover.tooltips = [
            ("Legend", "$name"),
            ("Frequency", "$x{0.0} Hz"),
            ("Amplitude", "$y{0.00} dB"),
        ]
        hover.renderers = [l_smooth, r_smooth, d_smooth]
        p.legend.location = "top_right"
        p.legend.click_policy = "mute"

        return p
