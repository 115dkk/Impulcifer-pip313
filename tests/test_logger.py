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
