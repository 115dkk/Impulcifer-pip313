"""Console-script target for the legacy Impulcifer GUI."""

from __future__ import annotations

from _impulcifer_entrypoint import run_gui_entrypoint


def main_gui() -> None:
    run_gui_entrypoint("gui.legacy_gui")


if __name__ == "__main__":
    main_gui()
