#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Studio Impulcifer tab — card-based BRIR generator.

Mirrors the focused option set from the Pulse redesign mockup:

    Card 01  Input files          (recordings dir + test signal)
    Card 02  Processing options   (room / headphone / custom EQ / plot)
    Card 03  Virtual bass         (crossover / sub HP / polarity)

Users who need the full advanced options (resample, target_level, bass
boost, tilt, channel balance, decay, etc.) should switch back to the
Stable skin. By design the Studio variant is the focused experience.
"""
from __future__ import annotations

import os
import shutil
import threading
from typing import TYPE_CHECKING

import customtkinter as ctk

import impulcifer
from gui.constants import FILETYPES_AUDIO_WITH_PKL, FILETYPES_TEXT, FILETYPES_WAV
from gui.dialogs import ProcessingDialog
from gui.skins.studio_widgets import (
    add_card_header,
    add_disclosure,
    add_field_row,
    add_inline_metric,
    make_card,
    make_card_body,
    make_page_header,
)
from gui.theme import COLORS
from gui.utils import (
    browse_directory,
    browse_file,
    install_smooth_scrolling,
    safe_get_double,
    safe_get_int,
)
from infra.logger import get_logger, set_gui_callbacks

if TYPE_CHECKING:
    from gui.modern_gui import ModernImpulciferGUI


class StudioImpulciferTab:
    """Card-based Impulcifer tab in Studio skin."""

    def __init__(self, app: ModernImpulciferGUI, parent: ctk.CTkBaseClass) -> None:
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.root = app.root
        self.parent = parent

        # Tk variables — Studio's own, independent of Stable's
        self.dir_path_var = ctk.StringVar(value=os.path.join("data", "my_hrir"))
        self.test_signal_var = ctk.StringVar(
            value=os.path.join("data", "sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav")
        )

        self.do_room_correction_var = ctk.BooleanVar(value=False)
        self.room_target_var = ctk.StringVar()
        self.room_mic_calibration_var = ctk.StringVar()
        self.specific_limit_var = ctk.IntVar(value=20000)
        self.generic_limit_var = ctk.IntVar(value=1000)
        self.fr_combination_var = ctk.StringVar(value="average")

        self.do_headphone_compensation_var = ctk.BooleanVar(value=False)
        self.headphone_compensation_file_var = ctk.StringVar()

        self.do_equalization_var = ctk.BooleanVar(value=False)
        self.eq_file_var = ctk.StringVar(value="eq.csv")
        self.eq_left_file_var = ctk.StringVar(value="eq-left.csv")
        self.eq_right_file_var = ctk.StringVar(value="eq-right.csv")

        self.plot_var = ctk.BooleanVar(value=False)

        self.vbass_enable_var = ctk.BooleanVar(value=False)
        self.vbass_freq_var = ctk.IntVar(value=250)
        self.vbass_hp_var = ctk.DoubleVar(value=15.0)
        self.vbass_polarity_var = ctk.StringVar(value="auto")

        self.generate_button: ctk.CTkButton | None = None

        self._build()

    def _build(self) -> None:
        self.parent.grid_columnconfigure(0, weight=1)
        self.parent.grid_rowconfigure(0, weight=1)

        scroll = ctk.CTkScrollableFrame(self.parent, fg_color="transparent")
        scroll.grid(row=0, column=0, sticky="nsew", padx=24, pady=24)
        scroll.grid_columnconfigure(0, weight=1)
        install_smooth_scrolling(scroll)

        page_header = make_page_header(
            scroll,
            title=self.loc.get("studio_processing_title"),
            subtitle=self.loc.get("studio_processing_subtitle"),
            fonts=self.fonts,
            cta_label=self.loc.get("studio_brir_generate"),
            cta_command=self.generate_brir,
        )
        page_header.grid(row=0, column=0, sticky="ew", pady=(0, 18))
        # Snapshot the CTA so we can disable/restore during processing
        for child in page_header.winfo_children():
            if isinstance(child, ctk.CTkButton):
                self.generate_button = child
                break

        self._build_input_card(scroll, row=1)
        self._build_options_card(scroll, row=2)
        self._build_vbass_card(scroll, row=3)

    # ------------------------------------------------------------------
    # Card 01 — Input files
    # ------------------------------------------------------------------
    def _build_input_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="01", title=self.loc.get("studio_card_input_files"), fonts=self.fonts)
        body = make_card_body(card)

        add_field_row(
            body,
            row=0,
            label=self.loc.get("label_your_recordings"),
            value_var=self.dir_path_var,
            on_change=lambda: browse_directory(self.dir_path_var),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        add_field_row(
            body,
            row=1,
            label=self.loc.get("label_test_signal"),
            value_var=self.test_signal_var,
            on_change=lambda: browse_file(self.test_signal_var, "open", FILETYPES_AUDIO_WITH_PKL),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )

    # ------------------------------------------------------------------
    # Card 02 — Processing options (disclosures)
    # ------------------------------------------------------------------
    def _build_options_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="02", title=self.loc.get("studio_card_processing_options"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        # 1. Room correction
        _, rc_body = add_disclosure(
            body,
            row=0,
            label=self.loc.get("section_room_correction"),
            desc=self.loc.get("studio_disclosure_room_correction_desc"),
            state_var=self.do_room_correction_var,
            fonts=self.fonts,
        )
        add_field_row(
            rc_body, row=0,
            label=self.loc.get("label_mic_calibration"),
            value_var=self.room_mic_calibration_var,
            on_change=lambda: browse_file(self.room_mic_calibration_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        add_field_row(
            rc_body, row=1,
            label=self.loc.get("label_target_curve"),
            value_var=self.room_target_var,
            on_change=lambda: browse_file(self.room_target_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        rc_inline = ctk.CTkFrame(rc_body, fg_color="transparent")
        rc_inline.grid(row=2, column=0, columnspan=2, sticky="ew", pady=(6, 0))
        rc_inline.grid_columnconfigure((0, 1, 2), weight=1, uniform="rcm")
        add_inline_metric(rc_inline, row=0, column=0, label="Specific limit", value_var=self.specific_limit_var, unit="Hz")
        add_inline_metric(rc_inline, row=0, column=1, label="Generic limit", value_var=self.generic_limit_var, unit="Hz")
        add_inline_metric(rc_inline, row=0, column=2, label="FR combination", value_var=self.fr_combination_var)

        # 2. Headphone compensation
        _, hp_body = add_disclosure(
            body,
            row=1,
            label=self.loc.get("section_headphone_compensation"),
            desc=self.loc.get("studio_disclosure_headphone_comp_desc"),
            state_var=self.do_headphone_compensation_var,
            fonts=self.fonts,
        )
        add_field_row(
            hp_body, row=0,
            label=self.loc.get("label_headphone_file"),
            value_var=self.headphone_compensation_file_var,
            on_change=lambda: browse_file(self.headphone_compensation_file_var, "open", FILETYPES_WAV),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )

        # 3. Custom EQ
        _, eq_body = add_disclosure(
            body,
            row=2,
            label=self.loc.get("section_equalization"),
            desc=self.loc.get("studio_disclosure_custom_eq_desc"),
            state_var=self.do_equalization_var,
            fonts=self.fonts,
        )
        add_field_row(
            eq_body, row=0,
            label="eq.csv",
            value_var=self.eq_file_var,
            on_change=lambda: browse_file(self.eq_file_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        add_field_row(
            eq_body, row=1,
            label="eq-left.csv",
            value_var=self.eq_left_file_var,
            on_change=lambda: browse_file(self.eq_left_file_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )
        add_field_row(
            eq_body, row=2,
            label="eq-right.csv",
            value_var=self.eq_right_file_var,
            on_change=lambda: browse_file(self.eq_right_file_var, "open", FILETYPES_TEXT),
            change_label=self.loc.get("studio_change_button"),
            fonts=self.fonts,
        )

        # 4. Plot toggle (no disclosure body)
        plot_box = ctk.CTkFrame(
            body,
            corner_radius=4,
            fg_color=COLORS["bg-1"],
            border_width=1,
            border_color=COLORS["line-soft"],
        )
        plot_box.grid(row=3, column=0, sticky="ew", pady=4)
        plot_box.grid_columnconfigure(1, weight=1)
        ctk.CTkSwitch(
            plot_box,
            text="",
            variable=self.plot_var,
            width=40,
            switch_width=30,
            switch_height=18,
        ).grid(row=0, column=0, sticky="w", padx=12, pady=10)
        plot_text = ctk.CTkFrame(plot_box, fg_color="transparent")
        plot_text.grid(row=0, column=1, sticky="w", padx=(10, 0))
        ctk.CTkLabel(
            plot_text,
            text=self.loc.get("checkbox_plot_results"),
            font=ctk.CTkFont(size=12, weight="bold"),
            anchor="w",
        ).grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(
            plot_text,
            text=self.loc.get("studio_toggle_plot_desc"),
            font=ctk.CTkFont(size=10),
            text_color=COLORS["fg-2"],
            anchor="w",
        ).grid(row=1, column=0, sticky="w")

    # ------------------------------------------------------------------
    # Card 03 — Virtual bass
    # ------------------------------------------------------------------
    def _build_vbass_card(self, parent: ctk.CTkBaseClass, *, row: int) -> None:
        card = make_card(parent)
        card.grid(row=row, column=0, sticky="ew", pady=(0, 14))
        add_card_header(card, number="03", title=self.loc.get("studio_card_virtual_bass"), fonts=self.fonts)
        body = make_card_body(card)
        body.grid_columnconfigure(0, weight=1)

        _, vb_body = add_disclosure(
            body,
            row=0,
            label=self.loc.get("vbass_enable"),
            desc=self.loc.get("studio_disclosure_virtual_bass_desc"),
            state_var=self.vbass_enable_var,
            fonts=self.fonts,
        )
        vb_inline = ctk.CTkFrame(vb_body, fg_color="transparent")
        vb_inline.grid(row=0, column=0, sticky="ew")
        vb_inline.grid_columnconfigure((0, 1, 2), weight=1, uniform="vbm")
        add_inline_metric(vb_inline, row=0, column=0, label="Crossover", value_var=self.vbass_freq_var, unit="Hz")
        add_inline_metric(vb_inline, row=0, column=1, label="Sub HP", value_var=self.vbass_hp_var, unit="Hz")
        add_inline_metric(vb_inline, row=0, column=2, label="Polarity", value_var=self.vbass_polarity_var)

    # ------------------------------------------------------------------
    # Generate
    # ------------------------------------------------------------------
    def generate_brir(self) -> None:
        """Run :func:`impulcifer.main` in a worker thread with progress dialog."""
        args = {
            "dir_path": self.dir_path_var.get(),
            "test_signal": self.test_signal_var.get(),
            "plot": self.plot_var.get(),
            "do_room_correction": self.do_room_correction_var.get(),
            "do_headphone_compensation": self.do_headphone_compensation_var.get(),
            "do_equalization": self.do_equalization_var.get(),
        }

        if self.do_room_correction_var.get():
            args["room_target"] = self.room_target_var.get() or None
            args["room_mic_calibration"] = self.room_mic_calibration_var.get() or None
            args["specific_limit"] = safe_get_int(self.specific_limit_var, 20000)
            args["generic_limit"] = safe_get_int(self.generic_limit_var, 1000)
            args["fr_combination_method"] = self.fr_combination_var.get()

        if self.do_headphone_compensation_var.get() and self.headphone_compensation_file_var.get():
            source_file = self.headphone_compensation_file_var.get()
            if not os.path.isabs(source_file):
                source_file = os.path.join(self.dir_path_var.get(), source_file)
            target_file = os.path.join(self.dir_path_var.get(), "headphones.wav")
            if os.path.exists(source_file):
                try:
                    shutil.copy2(source_file, target_file)
                except Exception as e:
                    print(f"Error copying headphone file: {e}")

        if self.vbass_enable_var.get():
            args["vbass"] = True
            args["vbass_freq"] = max(30, min(500, safe_get_int(self.vbass_freq_var, 250)))
            args["vbass_hp"] = safe_get_double(self.vbass_hp_var, 15.0)
            args["vbass_polarity"] = self.vbass_polarity_var.get() or "auto"

        # Disable CTA during processing
        if self.generate_button:
            self.generate_button.configure(state="disabled", text=self.loc.get("button_processing"))

        dialog = ProcessingDialog(self.root, self.loc, fonts=self.fonts)
        logger = get_logger()
        logger.set_localization(self.loc)
        set_gui_callbacks(log_callback=dialog.add_log, progress_callback=dialog.update_progress)

        def _run() -> None:
            try:
                with impulcifer.cancellation_scope(dialog.cancel_event):
                    impulcifer.main(**args)
                dialog.mark_complete(success=True)
            except impulcifer.CancelledError:
                logger.warning("message_processing_cancelled")
                dialog.mark_cancelled()
            except Exception as e:
                logger.error(f"Processing failed: {e}")
                dialog.mark_complete(success=False)
            finally:
                set_gui_callbacks(log_callback=None, progress_callback=None)
                self.root.after(0, self._restore_cta)

        threading.Thread(target=_run, daemon=True).start()

    def _restore_cta(self) -> None:
        if self.generate_button:
            self.generate_button.configure(
                state="normal",
                text=self.loc.get("studio_brir_generate"),
            )
