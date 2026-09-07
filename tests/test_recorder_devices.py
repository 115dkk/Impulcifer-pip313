"""Tk-free contract tests for Windows host API selection in ``core.recorder``.

The recorder resolves devices through ``sounddevice`` name queries. These tests
inject a fake backend that mimics sounddevice's word-based device matching so
the host API preference order and the per-direction WASAPI settings can be
checked without audio hardware.
"""

from types import SimpleNamespace

import pytest

from core import recorder


class _FakeWasapiSettings:
    def __init__(self, exclusive=False, auto_convert=False, explicit_sample_format=False):
        self.exclusive = exclusive
        self.auto_convert = auto_convert
        self.explicit_sample_format = explicit_sample_format

    def __eq__(self, other):
        return isinstance(other, _FakeWasapiSettings) and (
            self.exclusive,
            self.auto_convert,
            self.explicit_sample_format,
        ) == (other.exclusive, other.auto_convert, other.explicit_sample_format)

    def __repr__(self):
        return f"WasapiSettings(exclusive={self.exclusive}, auto_convert={self.auto_convert})"


class _FakeSounddevice:
    """Enough of the sounddevice module for ``core.recorder`` device resolution."""

    WasapiSettings = _FakeWasapiSettings

    def __init__(self, host_apis, devices, failing_checks=()):
        self._host_apis = list(host_apis)
        self._devices = [dict(device, index=index) for index, device in enumerate(devices)]
        self._failing_checks = set(failing_checks)
        self.checked = []
        self.default = SimpleNamespace(device=(0, 0), extra_settings=(None, None))

    def query_hostapis(self):
        return [{"name": name} for name in self._host_apis]

    def query_devices(self, device=None, kind=None):
        if device is None:
            return list(self._devices)
        query = device.lower()
        candidates = []
        for entry in self._devices:
            if kind and entry[f"max_{kind}_channels"] <= 0:
                continue
            full_name = f"{entry['name']} {self._host_apis[entry['hostapi']]}".lower()
            if all(word in full_name for word in query.split()):
                candidates.append((full_name == query, entry))
        exact = [entry for is_exact, entry in candidates if is_exact]
        if len(exact) == 1:
            return exact[0]
        if len(candidates) == 1:
            return candidates[0][1]
        raise ValueError(f"No unique device matching {device!r}")

    def _check(self, kind, device, channels, samplerate):
        self.checked.append((kind, device, channels, samplerate))
        if (kind, device) in self._failing_checks:
            raise ValueError("Invalid sample rate [PaErrorCode -9997]")

    def check_input_settings(self, device=None, channels=None, samplerate=None, **_kwargs):
        self._check("input", device, channels, samplerate)

    def check_output_settings(self, device=None, channels=None, samplerate=None, **_kwargs):
        self._check("output", device, channels, samplerate)


WINDOWS_HOST_APIS = ["MME", "Windows DirectSound", "Windows WASAPI", "Windows WDM-KS"]


def _windows_devices(wasapi_output_channels=8):
    return [
        {"name": "Speakers", "hostapi": 0, "max_input_channels": 0, "max_output_channels": 2},
        {"name": "Speakers", "hostapi": 1, "max_input_channels": 0, "max_output_channels": 8},
        {"name": "Speakers", "hostapi": 2, "max_input_channels": 0, "max_output_channels": wasapi_output_channels},
        {"name": "Speakers", "hostapi": 3, "max_input_channels": 0, "max_output_channels": 8},
        {"name": "Mic", "hostapi": 0, "max_input_channels": 2, "max_output_channels": 0},
        {"name": "Mic", "hostapi": 1, "max_input_channels": 2, "max_output_channels": 0},
        {"name": "Mic", "hostapi": 2, "max_input_channels": 2, "max_output_channels": 0},
    ]


@pytest.fixture
def fake_backend(monkeypatch):
    def install(devices=None, failing_checks=(), host_apis=WINDOWS_HOST_APIS):
        backend = _FakeSounddevice(host_apis, devices or _windows_devices(), failing_checks)
        monkeypatch.setattr(recorder, "_sd", backend)
        return backend

    return install


def test_host_api_preference_starts_with_wasapi():
    assert recorder.HOST_API_PREFERENCE[0] == "WASAPI"
    assert set(recorder.HOST_API_PREFERENCE) == {"WASAPI", "DirectSound", "MME"}


def test_get_device_prefers_wasapi_when_host_api_is_not_pinned(fake_backend):
    fake_backend()

    output = recorder.get_device("Speakers", "output")
    microphone = recorder.get_device("Mic", "input")

    assert output["hostapi"] == 2
    assert microphone["hostapi"] == 2


def test_get_device_falls_back_when_wasapi_device_lacks_channels(fake_backend):
    fake_backend(devices=_windows_devices(wasapi_output_channels=2))

    output = recorder.get_device("Speakers", "output", min_channels=8)

    assert output["hostapi"] == 1  # DirectSound, the next preference


def test_explicit_host_api_still_wins(fake_backend):
    fake_backend()

    output = recorder.get_device("Speakers", "output", host_api="MME")

    assert output["hostapi"] == 0


def test_set_default_devices_attaches_auto_convert_only_where_native_format_fails(fake_backend):
    backend = fake_backend(failing_checks={("output", 2)})
    microphone = recorder.get_device("Mic", "input")
    speakers = recorder.get_device("Speakers", "output")

    input_str, output_str = recorder.set_default_devices(microphone, speakers, 44100, 2, 8)

    assert (input_str, output_str) == ("Mic Windows WASAPI", "Speakers Windows WASAPI")
    assert backend.default.device == (input_str, output_str)
    assert backend.default.extra_settings == (None, _FakeWasapiSettings(auto_convert=True))
    assert ("input", 6, 2, 44100) in backend.checked
    assert ("output", 2, 8, 44100) in backend.checked


def test_set_default_devices_leaves_non_wasapi_devices_untouched(fake_backend):
    backend = fake_backend(failing_checks={("output", 1), ("input", 5)})
    microphone = recorder.get_device("Mic", "input", host_api="DirectSound")
    speakers = recorder.get_device("Speakers", "output", host_api="DirectSound")

    recorder.set_default_devices(microphone, speakers, 48000, 2, 8)

    assert backend.default.extra_settings == (None, None)
    assert backend.checked == []


def test_set_default_devices_without_format_skips_checks(fake_backend):
    backend = fake_backend(failing_checks={("output", 2), ("input", 6)})
    microphone = recorder.get_device("Mic", "input")
    speakers = recorder.get_device("Speakers", "output")

    recorder.set_default_devices(microphone, speakers)

    assert backend.default.extra_settings == (None, None)
    assert backend.checked == []
