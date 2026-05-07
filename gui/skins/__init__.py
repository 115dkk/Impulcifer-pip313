"""GUI skin variants — Stable (compact tabview) and Studio (sidebar + cards).

The skin choice is persisted via :class:`i18n.localization.LocalizationManager`
(see ``set_skin`` / ``get_skin``). ``ModernImpulciferGUI`` reads the value at
construction time and dispatches to the matching shell builder. Switching at
runtime tears down the existing widget tree and reuses the same instance,
mirroring the language-change rebuild path already shipped in
``refresh_localized_ui``.
"""
from __future__ import annotations

SKIN_STABLE = "stable"
SKIN_STUDIO = "studio"
SKIN_CHOICES: tuple[str, ...] = (SKIN_STABLE, SKIN_STUDIO)
