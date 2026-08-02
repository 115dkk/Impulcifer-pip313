"""Unit tests for the ``infra.environment`` probe ladder (audit #138 F041/C3).

Every install-kind decision in the app branches on these probes. The
``infra._build_info`` marker is faked through ``sys.modules``: a
``SimpleNamespace`` stands in for the generated module, and ``None`` forces
``from infra._build_info import BUILD_TYPE`` to raise ``ImportError`` (the
no-marker dev/py-source case). ``updater.environment`` must remain an
identity re-export shim for legacy importers.
"""

import sys
import types

import pytest

from infra import environment


@pytest.fixture
def no_marker(monkeypatch):
    """Simulate a checkout without the build-time infra/_build_info.py marker."""
    monkeypatch.setitem(sys.modules, 'infra._build_info', None)
    monkeypatch.delattr(sys, 'frozen', raising=False)
    sys.modules.pop('__nuitka__', None)
    monkeypatch.delattr(environment, '__compiled__', raising=False)


def _set_marker(monkeypatch, build_type):
    monkeypatch.setitem(
        sys.modules, 'infra._build_info', types.SimpleNamespace(BUILD_TYPE=build_type)
    )


class TestIsStandaloneBuild:
    def test_marker_standalone_wins(self, monkeypatch):
        _set_marker(monkeypatch, 'standalone')
        assert environment.is_standalone_build() is True

    def test_marker_pip_wins_over_frozen(self, monkeypatch):
        """The build marker has precedence over the sys.frozen fallback probe."""
        _set_marker(monkeypatch, 'pip')
        monkeypatch.setattr(sys, 'frozen', True, raising=False)
        assert environment.is_standalone_build() is False

    def test_no_marker_frozen_fallback(self, no_marker, monkeypatch):
        monkeypatch.setattr(sys, 'frozen', True, raising=False)
        assert environment.is_standalone_build() is True

    def test_no_marker_nuitka_module_fallback(self, no_marker, monkeypatch):
        monkeypatch.setitem(sys.modules, '__nuitka__', types.ModuleType('__nuitka__'))
        assert environment.is_standalone_build() is True

    def test_no_marker_compiled_global_fallback(self, no_marker, monkeypatch):
        """The __compiled__ probe (formerly only in infra.resource_helper)."""
        monkeypatch.setattr(environment, '__compiled__', True, raising=False)
        assert environment.is_standalone_build() is True

    def test_no_marker_plain_python(self, no_marker):
        assert environment.is_standalone_build() is False


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
        try:
            import pip  # noqa: F401
        except ImportError:
            pytest.skip('pip module not importable; subprocess fallback not pinned here')
        assert environment.is_pip_environment() is True


class TestVelopackGate:
    def test_not_standalone_is_not_velopack(self, no_marker):
        assert environment.is_velopack_environment() is False

    def test_not_standalone_has_no_update_exe(self, no_marker):
        assert environment.get_velopack_update_exe() is None


class TestGetInstallKind:
    def test_velopack_first(self, monkeypatch):
        monkeypatch.setattr(environment, 'is_velopack_environment', lambda: True)
        monkeypatch.setattr(environment, 'is_pip_environment', lambda: True)
        assert environment.get_install_kind() == 'velopack'

    def test_pip_second(self, monkeypatch):
        monkeypatch.setattr(environment, 'is_velopack_environment', lambda: False)
        monkeypatch.setattr(environment, 'is_pip_environment', lambda: True)
        assert environment.get_install_kind() == 'pip'

    def test_dev_last(self, monkeypatch):
        monkeypatch.setattr(environment, 'is_velopack_environment', lambda: False)
        monkeypatch.setattr(environment, 'is_pip_environment', lambda: False)
        assert environment.get_install_kind() == 'dev'


class TestNormalizedPlatform:
    def test_vocabulary(self):
        assert environment.normalized_platform() in ('windows', 'darwin', 'linux')


class TestUpdaterEnvironmentShim:
    def test_shim_reexports_identity(self):
        """Legacy updater.environment importers keep working via re-export."""
        from updater import environment as shim

        assert shim.is_pip_environment is environment.is_pip_environment
        assert shim.is_velopack_environment is environment.is_velopack_environment
        assert shim.get_velopack_update_exe is environment.get_velopack_update_exe
        assert shim._is_standalone_build is environment.is_standalone_build
