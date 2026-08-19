"""Tests for CLI/GUI logger translation behavior."""

from __future__ import annotations

from infra.logger import ImpulciferLogger


class DummyLocalization:
    def get(self, key: str, **kwargs: object) -> str:
        return f"{key}:{kwargs['total_steps']}"


def test_logger_translates_cli_keys_without_explicit_localization(monkeypatch) -> None:
    """CLI progress logs should not print raw i18n keys by default."""
    monkeypatch.setattr(
        "i18n.localization.get_localization_manager",
        lambda: DummyLocalization(),
    )

    logger = ImpulciferLogger()

    assert logger._translate("cli_starting_brir_generation", total_steps=7) == (
        "cli_starting_brir_generation:7"
    )


def test_logger_keeps_legacy_two_argument_callback_compatible() -> None:
    class LegacyCallback:
        def __init__(self):
            self.metadata_attempts = 0
            self.received = []

        def __call__(self, *args, **kwargs):
            if kwargs:
                self.metadata_attempts += 1
                raise TypeError("legacy callback only accepts two positional arguments")
            self.received.append(args)

    legacy_callback = LegacyCallback()
    logger = ImpulciferLogger()
    logger.set_gui_callback(legacy_callback)

    logger.info("First message")
    logger.warning("Second message")

    assert legacy_callback.received == [
        ("INFO", "First message"),
        ("WARNING", "Second message"),
    ]
    assert legacy_callback.metadata_attempts == 1
    assert logger._gui_callback_accepts_metadata is False


def test_logger_passes_original_key_and_args_to_metadata_callback() -> None:
    received = []

    def metadata_callback(level, message, *, key, args):
        received.append((level, message, key, args))

    logger = ImpulciferLogger()
    logger.set_gui_callback(metadata_callback)
    logger.info("Untranslated message", speaker="FL")

    assert received == [
        (
            "INFO",
            "Untranslated message",
            "Untranslated message",
            {"speaker": "FL"},
        )
    ]
    assert logger._gui_callback_accepts_metadata is True


def test_progress_and_step_callbacks_preserve_original_key_and_args() -> None:
    received = []

    def progress_callback(progress, message, *, key, args):
        received.append((progress, message, key, args))

    logger = ImpulciferLogger()
    logger.set_total_steps(4)
    logger.set_progress_callback(progress_callback)
    logger.step("Stage message", speaker="FR")

    assert received == [
        (25, "Stage message", "Stage message", {"speaker": "FR"})
    ]
    assert logger._progress_callback_accepts_metadata is True
