"""First unit tests for the ``updater.environment`` probe ladder (audit #138 F041).

Every install-kind decision in the app branches on these probes, but they had
zero direct tests. The ``infra._build_info`` marker is faked through
``sys.modules``: a ``SimpleNamespace`` stands in for the generated module, and
``None`` forces ``from infra._build_info import BUILD_TYPE`` to raise
``ImportError`` (the no-marker dev/py-source case).
"""

import sys
import types

import pytest

from updater import environment


@pytest.fixture
def no_marker(monkeypatch):
    """Simulate a checkout without the build-time infra/_build_info.py marker."""
    monkeypatch.setitem(sys.modules, 'infra._build_info', None)
    monkeypatch.delattr(sys, 'frozen', raising=False)
    sys.modules.pop('__nuitka__', None)


def _set_marker(monkeypatch, build_type):
    monkeypatch.setitem(
        sys.modules, 'infra._build_info', types.SimpleNamespace(BUILD_TYPE=build_type)
    )


class TestIsStandaloneBuild:
    def test_marker_standalone_wins(self, monkeypatch):
        _set_marker(monkeypatch, 'standalone')
        assert environment._is_standalone_build() is True

    def test_marker_pip_wins_over_frozen(self, monkeypatch):
        """The build marker has precedence over the sys.frozen fallback probe."""
        _set_marker(monkeypatch, 'pip')
        monkeypatch.setattr(sys, 'frozen', True, raising=False)
        assert environment._is_standalone_build() is False

    def test_no_marker_frozen_fallback(self, no_marker, monkeypatch):
        monkeypatch.setattr(sys, 'frozen', True, raising=False)
        assert environment._is_standalone_build() is True

    def test_no_marker_nuitka_module_fallback(self, no_marker, monkeypatch):
        monkeypatch.setitem(sys.modules, '__nuitka__', types.ModuleType('__nuitka__'))
        assert environment._is_standalone_build() is True

    def test_no_marker_plain_python(self, no_marker):
        assert environment._is_standalone_build() is False


class TestIsPipEnvironment:
    def test_marker_pip_wins(self, monkeypatch):
        _set_marker(monkeypatch, 'pip')
        assert environment.is_pip_environment() is True

    def test_marker_standalone_is_not_pip(self, monkeypatch):
        _set_marker(monkeypatch, 'standalone')
        assert environment.is_pip_environment() is False

    def test_no_marker_standalone_probe_blocks_bundled_pip(self, no_marker, monkeypatch):
        """A standalone build's bundled pip module must not count as pip install."""
        monkeypatch.setattr(sys, 'frozen', True, raising=False)
        assert environment.is_pip_environment() is False

    def test_no_marker_falls_back_to_pip_probe(self, no_marker):
        expected = True
        try:
            import pip  # noqa: F401
        except ImportError:
            expected = None  # probe falls through to the subprocess check
        if expected is None:
            pytest.skip('pip module not importable; subprocess fallback not pinned here')
        assert environment.is_pip_environment() is True


class TestVelopackGate:
    def test_not_standalone_is_not_velopack(self, no_marker):
        assert environment.is_velopack_environment() is False

    def test_not_standalone_has_no_update_exe(self, no_marker):
        assert environment.get_velopack_update_exe() is None
