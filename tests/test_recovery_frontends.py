"""Contract tests for the native and WebView output-recovery adapters."""

from __future__ import annotations

from pathlib import Path

from core.brir_recovery import BrirRecoveryResult
from gui.recovery_actions import RecoveryActionsMixin, _relative_names


class _Var:
    def __init__(self, value):
        self.value = value

    def get(self):
        return self.value

    def set(self, value) -> None:
        self.value = value


class _Widget:
    def __init__(self) -> None:
        self.options: dict[str, object] = {}
        self.visible = True

    def configure(self, **kwargs) -> None:
        self.options.update(kwargs)

    def grid(self) -> None:
        self.visible = True

    def grid_remove(self) -> None:
        self.visible = False


class _Root:
    @staticmethod
    def after(_delay, callback) -> None:
        callback()


class _Loc:
    @staticmethod
    def get(key: str, **kwargs) -> str:
        return key.format(**kwargs)


class _Harness(RecoveryActionsMixin):
    def __init__(self, directory: Path) -> None:
        self.app = object()
        self.root = _Root()
        self.loc = _Loc()
        self.dir_path_var = _Var(str(directory))
        self.include_hangloose_var = _Var(True)
        self.restore_button = _Widget()
        self.status_label = _Widget()
        self.summary_label = _Widget()
        self.files_label = _Widget()
        self.open_button = _Widget()
        self._init_recovery_actions()


def test_ctk_recovery_calls_core_and_blocks_duplicate_submit(monkeypatch, tmp_path: Path) -> None:
    import gui.recovery_actions as actions

    calls: list[tuple[str, bool]] = []

    def fake_recovery(directory, *, include_hangloose=False):
        calls.append((directory, include_hangloose))
        return BrirRecoveryResult(
            source_kind="hesuvi",
            source_path=str(tmp_path / "hesuvi.wav"),
            output_dir=str(tmp_path),
            sample_rate=48_000,
            sample_count=512,
            speakers=("FL", "FR"),
            created_files=(str(tmp_path / "hrir.wav"),),
            existing_files=(str(tmp_path / "hesuvi.wav"),),
        )

    pending = []

    class DeferredThread:
        def __init__(self, *, target, daemon):
            assert daemon is True
            pending.append(target)

        def start(self) -> None:
            return None

    monkeypatch.setattr(actions, "recover_brir_outputs", fake_recovery)
    monkeypatch.setattr(actions.threading, "Thread", DeferredThread)

    harness = _Harness(tmp_path)
    harness.start_recovery()
    harness.start_recovery()

    assert len(pending) == 1
    assert harness.restore_button.options["state"] == "disabled"
    pending[0]()
    assert calls == [(str(tmp_path), True)]
    assert harness.restore_button.options["state"] == "normal"
    assert harness.status_label.options["text"] == "webview_status_succeeded"
    assert "hrir.wav" in harness.files_label.options["text"]
    assert harness.open_button.visible is True


def test_recovery_relative_names_keep_hangloose_subdirectory(tmp_path: Path) -> None:
    output = tmp_path / "output"
    paths = (
        str(output / "hrir.wav"),
        str(output / "Hangloose" / "FL.wav"),
    )

    assert _relative_names(paths, str(output)) == "hrir.wav, Hangloose/FL.wav"


def test_frontend_recovery_boundary_and_navigation_are_pinned() -> None:
    root = Path(__file__).resolve().parents[1]
    native = (root / "gui" / "recovery_actions.py").read_text(encoding="utf-8")
    web = (root / "webview_ui" / "app.js").read_text(encoding="utf-8")
    html = (root / "webview_ui" / "index.html").read_text(encoding="utf-8")
    modern = (root / "gui" / "modern_gui.py").read_text(encoding="utf-8")
    studio = (root / "gui" / "skins" / "studio_shell.py").read_text(encoding="utf-8")

    assert "from application" not in native
    assert "recover_brir_outputs(" in native
    assert "api().start_output_recovery(request)" in web
    assert "state.startPending || state.jobId" in web
    assert 'data-view="recovery"' in html
    assert 'id="view-recovery"' in html
    assert "'recovery': 'tab_output_recovery'" in modern
    assert '("recovery", "↺", "sidebar_output_recovery")' in studio
