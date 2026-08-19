# -*- coding: utf-8 -*-
"""Regression test for the lazy FFmpeg setup path.

Importing ``core.utils`` used to call ``setup_ffmpeg()`` at module load,
which spawns ffmpeg/ffprobe subprocess probes (and possibly an auto-install
attempt) every time *any* downstream module — including ProcessPool workers
that never touch TrueHD/MLP — imports it. This test pins the lazy contract:
import is side-effect-free, the setup is performed only when something
actually needs FFmpeg, and the regular WAV reading path no longer triggers
TrueHD detection.

The lazy FFmpeg state and discovery/install live in ``core.ffmpeg_discovery``;
TrueHD/MLP decode + ``read_audio`` in ``core.audio_truehd``. ``core.utils``
re-exports their public functions, so the spies below are anchored on the
modules that actually own the behavior.
"""

from __future__ import annotations

import importlib
import os
import sys
import tempfile
import unittest
import unittest.mock as mock

import numpy as np
import soundfile as sf

# Modules whose live state / call sites the test reaches into.
_FFMPEG_MODULES = (
    "core.utils",
    "core.ffmpeg_discovery",
    "core.audio_truehd",
)


def _purge_ffmpeg_modules() -> None:
    for module_name in list(sys.modules):
        if any(
            module_name == m or module_name.startswith(m + ".")
            for m in _FFMPEG_MODULES
        ):
            del sys.modules[module_name]


class FfmpegLazySetupTest(unittest.TestCase):
    """Verify that ``setup_ffmpeg`` is only called when actually needed."""

    def setUp(self):
        # Ensure we re-import the FFmpeg modules with the spy in place.
        _purge_ffmpeg_modules()

    def _import_with_spies(self):
        """Import ``core.utils`` / the FFmpeg modules after patching the
        setup_ffmpeg + install_ffmpeg call sites.

        Returns (core.utils module, setup_spy, install_spy). The spies are
        anchored on ``core.ffmpeg_discovery`` because that is where the lazy
        initialisation actually runs; ``core.utils`` only re-exports it.
        """
        import core.utils as fresh
        importlib.reload(fresh)
        import core.ffmpeg_discovery as discovery
        importlib.reload(discovery)
        import core.audio_truehd  # noqa: F401  (ensure decode layer is loaded)

        setup_spy = mock.patch.object(
            discovery, "setup_ffmpeg", wraps=discovery.setup_ffmpeg
        ).start()
        install_spy = mock.patch.object(
            discovery, "install_ffmpeg", return_value=(None, None)
        ).start()
        # Reset the lazy gate so subsequent calls trigger setup_ffmpeg again.
        discovery.FFMPEG_PATH = None
        discovery.FFPROBE_PATH = None
        discovery._FFMPEG_DETECTION_DONE = False
        discovery._FFMPEG_AUTO_INSTALL_ATTEMPTED = False
        discovery._FFMPEG_SETUP_DONE = False
        discovery._FFMPEG_UNAVAILABLE_REASON = None
        return fresh, setup_spy, install_spy

    def tearDown(self):
        mock.patch.stopall()

    def test_import_does_not_call_setup_ffmpeg(self):
        """Plain ``import core.utils`` must not run setup_ffmpeg()."""
        # Patch the underlying primitives BEFORE the import. setup_ffmpeg
        # itself is module-level so we can't easily patch it pre-import,
        # but the side effects (shutil.which, install_ffmpeg) we can.
        with mock.patch("shutil.which") as which_mock, \
             mock.patch("subprocess.run") as run_mock:
            _purge_ffmpeg_modules()
            import core.utils  # noqa: F401
            import core.ffmpeg_discovery as discovery
            # No FFmpeg-related work should happen during import.
            self.assertFalse(which_mock.called,
                             "shutil.which must not be called during core.utils import")
            self.assertFalse(run_mock.called,
                             "subprocess.run must not be called during core.utils import")
            self.assertIsNone(discovery.FFMPEG_PATH,
                              "FFMPEG_PATH must be None until ensure_ffmpeg_available() runs")
            self.assertIsNone(discovery.FFPROBE_PATH,
                              "FFPROBE_PATH must be None until ensure_ffmpeg_available() runs")
            self.assertFalse(discovery._FFMPEG_DETECTION_DONE,
                             "_FFMPEG_DETECTION_DONE must be False until ensure runs")
            self.assertFalse(discovery._FFMPEG_AUTO_INSTALL_ATTEMPTED,
                             "_FFMPEG_AUTO_INSTALL_ATTEMPTED must be False until ensure runs")

    def test_get_ffmpeg_paths_returns_none_when_ensure_fails(self):
        """The accessor should expose failure as ``None``."""
        import core.ffmpeg_discovery as discovery

        with mock.patch.object(
            discovery, "ensure_ffmpeg_available", return_value=False
        ) as ensure_mock:
            self.assertIsNone(discovery.get_ffmpeg_paths(auto_install=True))

        ensure_mock.assert_called_once_with(auto_install=True)

    def test_get_ffmpeg_paths_returns_initialized_paths(self):
        """The accessor should return the live paths after successful setup."""
        import core.ffmpeg_discovery as discovery

        discovery.FFMPEG_PATH = "/tools/ffmpeg"
        discovery.FFPROBE_PATH = "/tools/ffprobe"
        with mock.patch.object(
            discovery, "ensure_ffmpeg_available", return_value=True
        ) as ensure_mock:
            self.assertEqual(
                discovery.get_ffmpeg_paths(auto_install=False),
                ("/tools/ffmpeg", "/tools/ffprobe"),
            )

        ensure_mock.assert_called_once_with(auto_install=False)

    def test_detection_only_path_is_cached(self):
        """Repeated detection-only checks should not repeat setup work."""
        utils, setup_spy, _ = self._import_with_spies()

        utils.ensure_ffmpeg_available(auto_install=False)
        first_calls = setup_spy.call_count

        utils.ensure_ffmpeg_available(auto_install=False)
        utils.ensure_ffmpeg_available(auto_install=False)
        self.assertEqual(setup_spy.call_count, first_calls,
                         "setup_ffmpeg detection must be cached after the first ensure call")

    def test_auto_install_true_can_retry_after_detection_only_failure(self):
        """A detection miss with auto_install=False must not poison install later."""
        utils, setup_spy, install_spy = self._import_with_spies()
        import core.ffmpeg_discovery as discovery

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(discovery, "find_ffmpeg_in_common_paths",
                               return_value=(None, None)):
            self.assertFalse(utils.ensure_ffmpeg_available(auto_install=False))
            self.assertFalse(install_spy.called,
                             "detection-only lookup must not install FFmpeg")

            self.assertFalse(utils.ensure_ffmpeg_available(auto_install=True))
            self.assertGreaterEqual(
                setup_spy.call_count,
                2,
                "auto_install=True must retry setup after detection miss",
            )
            self.assertTrue(install_spy.called,
                            "auto_install=True must still get one install opportunity")

    def test_install_ffmpeg_on_linux_requires_manual_installation(self):
        """Linux installation guidance must not invoke a package manager."""
        import core.ffmpeg_discovery as discovery

        with mock.patch.object(discovery.platform, "system", return_value="Linux"), \
             mock.patch.object(discovery.subprocess, "run") as run_mock:
            self.assertEqual(discovery.install_ffmpeg(), (None, None))

        run_mock.assert_not_called()

    def test_latched_install_failure_prints_reason_on_repeated_ensure(self):
        """Repeated checks must explain a previously latched install failure."""
        utils, _, _ = self._import_with_spies()
        import core.ffmpeg_discovery as discovery

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(
                 discovery,
                 "find_ffmpeg_in_common_paths",
                 return_value=(None, None),
             ), mock.patch("builtins.print") as print_mock:
            self.assertFalse(utils.ensure_ffmpeg_available(auto_install=True))
            reason = discovery.get_ffmpeg_unavailable_reason()

            print_mock.reset_mock()
            self.assertFalse(utils.ensure_ffmpeg_available(auto_install=True))

        print_mock.assert_any_call(reason)

    def test_get_ffmpeg_unavailable_reason_returns_install_failure(self):
        """The unavailable reason accessor must expose the last failure."""
        utils, _, _ = self._import_with_spies()
        import core.ffmpeg_discovery as discovery

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(
                 discovery,
                 "find_ffmpeg_in_common_paths",
                 return_value=(None, None),
             ):
            self.assertFalse(utils.ensure_ffmpeg_available(auto_install=True))

        reason = discovery.get_ffmpeg_unavailable_reason()
        self.assertIsInstance(reason, str)
        self.assertTrue(reason)

    def test_check_ffmpeg_available_does_not_auto_install_by_default(self):
        """check_ffmpeg_available() defaults to auto_install=False."""
        utils, _, install_spy = self._import_with_spies()
        import core.ffmpeg_discovery as discovery

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(discovery, "find_ffmpeg_in_common_paths",
                               return_value=(None, None)):
            utils.check_ffmpeg_available()
            self.assertFalse(install_spy.called,
                             "check_ffmpeg_available() default must not trigger install")

    def test_check_ffmpeg_available_with_auto_install_triggers_install(self):
        """check_ffmpeg_available(auto_install=True) must reach install_ffmpeg()."""
        utils, _, install_spy = self._import_with_spies()
        import core.ffmpeg_discovery as discovery

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(discovery, "find_ffmpeg_in_common_paths",
                               return_value=(None, None)):
            utils.check_ffmpeg_available(auto_install=True)
            self.assertTrue(install_spy.called,
                            "check_ffmpeg_available(auto_install=True) must call install_ffmpeg")

    def test_truehd_helpers_trigger_lazy_setup(self):
        """is_truehd_file/convert/get_info trigger ensure_ffmpeg_available(True)."""
        utils, setup_spy, install_spy = self._import_with_spies()
        import core.ffmpeg_discovery as discovery

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(discovery, "find_ffmpeg_in_common_paths",
                               return_value=(None, None)):
            self.assertFalse(utils.is_truehd_file("/dev/null/nonexistent.mlp"))
            self.assertTrue(setup_spy.called,
                            "is_truehd_file must trigger setup")
            self.assertTrue(install_spy.called,
                            "is_truehd_file must use auto_install=True")

    def test_atmos_object_master_detected_by_profile(self):
        """``is_truehd_atmos_object_master`` keys off the codec profile.

        Regression (Codex PR #98): rejecting *every* TrueHD file without
        a custom 11/13-channel layout map also nuked ordinary 5.1/7.1
        TrueHD. The discriminator must be the ffprobe ``profile``:
        ``Dolby TrueHD + Dolby Atmos`` → object master (reject);
        plain ``Dolby TrueHD`` → ordinary stream (allow).
        """
        import core.audio_truehd as audio_truehd

        with mock.patch.object(
            audio_truehd, "get_truehd_profile",
            return_value="Dolby TrueHD + Dolby Atmos",
        ):
            self.assertTrue(
                audio_truehd.is_truehd_atmos_object_master("11cmaster.mlp")
            )

        with mock.patch.object(
            audio_truehd, "get_truehd_profile", return_value="Dolby TrueHD"
        ):
            self.assertFalse(
                audio_truehd.is_truehd_atmos_object_master("plain-71.thd")
            )

        # No profile (ffprobe failed / not TrueHD) → not an object master.
        with mock.patch.object(
            audio_truehd, "get_truehd_profile", return_value=None
        ):
            self.assertFalse(
                audio_truehd.is_truehd_atmos_object_master("whatever.mlp")
            )

    def test_read_audio_wav_skips_ffmpeg_setup(self):
        """Reading a regular .wav must not trigger FFmpeg setup at all."""
        utils, setup_spy, install_spy = self._import_with_spies()

        with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as tf:
            wav_path = tf.name
        try:
            sf.write(wav_path, np.zeros(1024, dtype=np.float32), 48000)

            fs, data, channel_info = utils.read_audio(wav_path)
            self.assertEqual(fs, 48000)
            self.assertEqual(data.shape[-1], 1024)
            self.assertIsNone(channel_info)
            self.assertFalse(setup_spy.called,
                             "read_audio for .wav must not call setup_ffmpeg")
            self.assertFalse(install_spy.called,
                             "read_audio for .wav must not call install_ffmpeg")
        finally:
            if os.path.exists(wav_path):
                os.remove(wav_path)

if __name__ == "__main__":
    unittest.main()
