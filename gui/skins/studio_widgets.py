#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Reusable Studio-skin widgets.

The Studio skin from the Pulse redesign expresses content as a vertical
stack of labelled cards (`.card` in the design tokens). Each card has a
header strip (number pill + title + optional right meta) and a body. Many
options live behind a toggle that, when ON, expands an inline panel of
fields — the design's `.dt` (disclosure toggle) pattern.

These helpers keep the per-tab Studio code declarative — the four Studio
tabs each compose a few cards rather than re-deriving the styling.
"""
from __future__ import annotations

from typing import Callable, Optional, Sequence

import customtkinter as ctk

from gui.theme import COLORS, get_mono_font_family


def make_card(parent: ctk.CTkBaseClass) -> ctk.CTkFrame:
    """Create the Studio card wrapper used as a section container."""
    card = ctk.CTkFrame(
        parent,
        corner_radius=10,
        fg_color=COLORS["bg-2"],
        border_width=1,
        border_color=COLORS["line-soft"],
    )
    card.grid_columnconfigure(0, weight=1)
    return card


def add_card_header(
    card: ctk.CTkFrame,
    *,
    number: str,
    title: str,
    right_meta: Optional[str] = None,
    fonts: dict[str, ctk.CTkFont] | None = None,
) -> ctk.CTkFrame:
    """Render the card's header strip (number pill + title + optional right meta)."""
    header = ctk.CTkFrame(card, corner_radius=0, fg_color=COLORS["bg-3"], height=46)
    header.grid(row=0, column=0, sticky="ew")
    header.grid_columnconfigure(2, weight=1)

    mono = get_mono_font_family()
    pill = ctk.CTkLabel(
        header,
        text=number,
        font=ctk.CTkFont(family=mono, size=11, weight="bold"),
        text_color=COLORS["accent"],
        fg_color=COLORS["accent-soft"],
        corner_radius=3,
        padx=7,
    )
    pill.grid(row=0, column=0, padx=(14, 10), pady=10, sticky="w")

    title_font = (fonts or {}).get("heading") or ctk.CTkFont(size=14, weight="bold")
    title_label = ctk.CTkLabel(header, text=title, font=title_font, anchor="w")
    title_label.grid(row=0, column=1, sticky="w", pady=10)

    if right_meta:
        meta_font = ctk.CTkFont(family=mono, size=11)
        meta_label = ctk.CTkLabel(
            header, text=right_meta, font=meta_font, text_color=COLORS["fg-2"], anchor="e"
        )
        meta_label.grid(row=0, column=2, padx=(10, 14), pady=10, sticky="e")

    return header


def make_card_body(card: ctk.CTkFrame, *, padx: int = 18, pady: int = 14) -> ctk.CTkFrame:
    """Create the card body container — caller fills it with rows/widgets."""
    body = ctk.CTkFrame(card, corner_radius=0, fg_color="transparent")
    body.grid(row=1, column=0, sticky="ew", padx=padx, pady=pady)
    body.grid_columnconfigure(0, weight=1)
    return body


def add_field_row(
    parent: ctk.CTkBaseClass,
    *,
    row: int,
    label: str,
    value_var: ctk.StringVar,
    on_change: Optional[Callable[[], None]] = None,
    change_label: str = "Change",
    fonts: dict[str, ctk.CTkFont] | None = None,
    mono: bool = True,
) -> ctk.CTkFrame:
    """Render a labelled field with an inline ``Change`` link button.

    The design's ``.fld-std`` row layout: 140px label column on the left,
    flexible value column on the right framed in a soft input box, with a
    secondary ``Change`` link in the same row.
    """
    parent.grid_columnconfigure(1, weight=1)

    label_font = (fonts or {}).get("small") or ctk.CTkFont(size=12)
    ctk.CTkLabel(
        parent,
        text=label,
        font=label_font,
        text_color=COLORS["fg-2"],
        anchor="w",
    ).grid(row=row, column=0, sticky="w", padx=(0, 16), pady=6)

    value_frame = ctk.CTkFrame(
        parent,
        corner_radius=4,
        fg_color=COLORS["bg-3"],
        border_width=1,
        border_color=COLORS["line"],
        height=36,
    )
    value_frame.grid(row=row, column=1, sticky="ew", pady=6)
    value_frame.grid_columnconfigure(0, weight=1)
    value_frame.grid_propagate(False)

    val_font = (
        ctk.CTkFont(family=get_mono_font_family(), size=13)
        if mono
        else ((fonts or {}).get("label") or ctk.CTkFont(size=13))
    )
    entry = ctk.CTkEntry(
        value_frame,
        textvariable=value_var,
        font=val_font,
        fg_color=COLORS["bg-3"],
        border_width=0,
        text_color=COLORS["fg-0"],
    )
    entry.grid(row=0, column=0, sticky="ew", padx=(12, 4), pady=4)

    if on_change:
        link = ctk.CTkButton(
            value_frame,
            text=change_label,
            command=on_change,
            width=60,
            height=24,
            corner_radius=3,
            fg_color="transparent",
            hover_color=COLORS["accent-soft"],
            text_color=COLORS["accent"],
            font=ctk.CTkFont(size=11, weight="bold"),
        )
        link.grid(row=0, column=1, padx=(0, 8), pady=4, sticky="e")

    return value_frame


def add_disclosure(
    parent: ctk.CTkBaseClass,
    *,
    row: int,
    label: str,
    desc: str,
    state_var: ctk.BooleanVar,
    fonts: dict[str, ctk.CTkFont] | None = None,
) -> tuple[ctk.CTkFrame, ctk.CTkFrame]:
    """Render the design's disclosure toggle row.

    Returns a ``(outer, body)`` tuple. ``outer`` is the framed disclosure
    box; ``body`` is a CTkFrame inside it that the caller fills with
    fields.

    The head row carries:

    * a ``CTkSwitch`` bound to ``state_var`` (turns the option on/off),
    * the ``label`` and ``desc`` lines describing what the toggle does,
    * a right-aligned caret indicator (``▾`` open / ``▸`` closed) that
      makes the "this row expands" affordance explicit. Without the
      caret the previous build looked like a flat info row whose ``desc``
      text (e.g. ``hp_response.wav 사용``) read as a fixed file pin
      instead of "toggle me to pick a file".

    Clicking anywhere on the head row also flips the toggle, so the
    caret area is hot-zoned the same way the design's ``.dt-head`` is.
    """
    outer = ctk.CTkFrame(
        parent,
        corner_radius=4,
        fg_color=COLORS["bg-1"],
        border_width=1,
        border_color=COLORS["line-soft"],
    )
    outer.grid(row=row, column=0, sticky="ew", pady=4)
    outer.grid_columnconfigure(0, weight=1)

    head = ctk.CTkFrame(outer, fg_color="transparent", corner_radius=0)
    head.grid(row=0, column=0, sticky="ew", padx=12, pady=10)
    head.grid_columnconfigure(1, weight=1)

    body = ctk.CTkFrame(
        outer,
        fg_color="transparent",
        corner_radius=0,
    )
    body.grid_columnconfigure(0, weight=1)

    label_font = ctk.CTkFont(size=13, weight="bold")
    desc_font = ctk.CTkFont(size=11)

    text_col = ctk.CTkFrame(head, fg_color="transparent")
    text_col.grid(row=0, column=1, sticky="w", padx=(10, 0))
    ctk.CTkLabel(text_col, text=label, font=label_font, anchor="w").grid(row=0, column=0, sticky="w")
    ctk.CTkLabel(
        text_col, text=desc, font=desc_font, text_color=COLORS["fg-2"], anchor="w"
    ).grid(row=1, column=0, sticky="w")

    caret = ctk.CTkLabel(
        head,
        text="▸",
        font=ctk.CTkFont(size=12),
        text_color=COLORS["fg-2"],
        width=20,
    )
    caret.grid(row=0, column=2, sticky="e", padx=(8, 0))

    def _toggle() -> None:
        if state_var.get():
            body.grid(row=1, column=0, sticky="ew", padx=24, pady=(0, 12))
            outer.configure(border_color=COLORS["accent-soft"])
            caret.configure(text="▾", text_color=COLORS["accent"])
        else:
            body.grid_remove()
            outer.configure(border_color=COLORS["line-soft"])
            caret.configure(text="▸", text_color=COLORS["fg-2"])

    switch = ctk.CTkSwitch(
        head,
        text="",
        variable=state_var,
        command=_toggle,
        width=40,
        switch_width=30,
        switch_height=18,
    )
    switch.grid(row=0, column=0, sticky="w")

    # Also flip on click of the head row text (improves the affordance —
    # the previous layout looked clickable but only the switch handle
    # actually responded). Bind on the head + text_col + caret + the two
    # text labels so the entire row hot-zone works uniformly.
    def _flip_from_click(_event: object = None) -> None:
        state_var.set(not state_var.get())
        _toggle()

    for widget in (head, text_col, caret, *text_col.winfo_children()):
        widget.bind("<Button-1>", _flip_from_click)

    _toggle()  # Apply initial state

    return outer, body


def add_inline_metric(
    parent: ctk.CTkBaseClass,
    *,
    row: int,
    column: int,
    label: str,
    value_var: ctk.Variable,
    unit: str = "",
) -> ctk.CTkFrame:
    """Render the design's `.nf` numeric pill (label above, value/unit row)."""
    box = ctk.CTkFrame(
        parent,
        corner_radius=4,
        fg_color=COLORS["bg-3"],
        border_width=1,
        border_color=COLORS["line"],
    )
    box.grid(row=row, column=column, sticky="ew", padx=4, pady=4)
    box.grid_columnconfigure(0, weight=1)

    ctk.CTkLabel(
        box,
        text=label,
        font=ctk.CTkFont(size=11),
        text_color=COLORS["fg-2"],
        anchor="w",
    ).grid(row=0, column=0, sticky="w", padx=10, pady=(6, 0))

    val_row = ctk.CTkFrame(box, fg_color="transparent")
    val_row.grid(row=1, column=0, sticky="ew", padx=10, pady=(2, 6))
    val_row.grid_columnconfigure(0, weight=1)

    entry = ctk.CTkEntry(
        val_row,
        textvariable=value_var,
        font=ctk.CTkFont(family=get_mono_font_family(), size=13),
        fg_color=COLORS["bg-3"],
        border_width=0,
        text_color=COLORS["fg-0"],
        width=70,
    )
    entry.grid(row=0, column=0, sticky="ew")

    if unit:
        ctk.CTkLabel(
            val_row,
            text=unit,
            font=ctk.CTkFont(size=11),
            text_color=COLORS["fg-2"],
        ).grid(row=0, column=1, padx=(4, 0))

    return box


def add_inline_dropdown(
    parent: ctk.CTkBaseClass,
    *,
    row: int,
    column: int,
    label: str,
    value_var: ctk.StringVar,
    values: Sequence[str],
    on_change: Optional[Callable[[str], None]] = None,
) -> ctk.CTkFrame:
    """Render an `.nf`-style pill that wraps a CTkOptionMenu instead of an entry.

    Used for finite-option fields (FR combination = average / conservative,
    polarity = auto / normal / invert) so the user picks a valid value
    instead of free-typing one that the backend will silently coerce.
    """
    box = ctk.CTkFrame(
        parent,
        corner_radius=4,
        fg_color=COLORS["bg-3"],
        border_width=1,
        border_color=COLORS["line"],
    )
    box.grid(row=row, column=column, sticky="ew", padx=4, pady=4)
    box.grid_columnconfigure(0, weight=1)

    ctk.CTkLabel(
        box,
        text=label,
        font=ctk.CTkFont(size=11),
        text_color=COLORS["fg-2"],
        anchor="w",
    ).grid(row=0, column=0, sticky="w", padx=10, pady=(6, 0))

    if value_var.get() not in values:
        value_var.set(values[0])

    menu = ctk.CTkOptionMenu(
        box,
        variable=value_var,
        values=list(values),
        command=on_change,
        font=ctk.CTkFont(size=12, weight="bold"),
        height=26,
        corner_radius=3,
        fg_color=COLORS["bg-2"],
        button_color=COLORS["bg-2"],
        button_hover_color=COLORS["accent-soft"],
        text_color=COLORS["fg-0"],
        dropdown_font=ctk.CTkFont(size=12),
    )
    menu.grid(row=1, column=0, sticky="ew", padx=10, pady=(2, 6))
    return box


def make_page_header(
    parent: ctk.CTkBaseClass,
    *,
    title: str,
    subtitle: str,
    fonts: dict[str, ctk.CTkFont] | None = None,
    cta_label: Optional[str] = None,
    cta_command: Optional[Callable[[], None]] = None,
    cta_color: Optional[str] = None,
) -> ctk.CTkFrame:
    """Render the page-level header (title + subtitle + optional right CTA)."""
    header = ctk.CTkFrame(parent, fg_color="transparent")
    header.grid_columnconfigure(0, weight=1)

    text_col = ctk.CTkFrame(header, fg_color="transparent")
    text_col.grid(row=0, column=0, sticky="w")

    title_font = (fonts or {}).get("title") or ctk.CTkFont(size=24, weight="bold")
    ctk.CTkLabel(text_col, text=title, font=title_font, anchor="w"
                 ).grid(row=0, column=0, sticky="w")
    sub_font = (fonts or {}).get("label") or ctk.CTkFont(size=13)
    ctk.CTkLabel(
        text_col, text=subtitle, font=sub_font, text_color=COLORS["fg-2"], anchor="w"
    ).grid(row=1, column=0, sticky="w", pady=(4, 0))

    if cta_label and cta_command:
        cta_fg = cta_color or COLORS["accent"][1]
        cta_hover = COLORS["accent-strong"][1]
        ctk.CTkButton(
            header,
            text=cta_label,
            command=cta_command,
            font=ctk.CTkFont(size=13, weight="bold"),
            fg_color=cta_fg,
            hover_color=cta_hover,
            text_color="#ffffff",
            corner_radius=4,
            height=36,
            width=140,
        ).grid(row=0, column=1, sticky="e", padx=(10, 0))

    return header
