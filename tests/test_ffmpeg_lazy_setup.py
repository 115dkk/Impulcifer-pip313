# -*- coding: utf-8 -*-
"""Regression test for the lazy FFmpeg setup path.

Importing ``core.utils`` used to call ``setup_ffmpeg()`` at module load,
which spawns ffmpeg/ffprobe subprocess probes (and possibly an auto-install
attempt) every time *any* downstream module — including ProcessPool workers
that never touch TrueHD/MLP — imports it. This test pins the lazy contract:
import is side-effect-free, the setup is performed only when something
actually needs FFmpeg, and the regular WAV reading path no longer triggers
TrueHD detection.

Issue #87 Phase 5 split the FFmpeg helpers out into ``core.ffmpeg_utils``;
``core.utils`` re-exports them for backward compatibility, but the lazy
module-level state (``FFMPEG_PATH``, ``FFPROBE_PATH``, ``_FFMPEG_SETUP_DONE``)
and the actual call sites (``setup_ffmpeg`` / ``install_ffmpeg``) live in
``core.ffmpeg_utils`` — that is the module the spies must target.
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


class FfmpegLazySetupTest(unittest.TestCase):
    """Verify that ``setup_ffmpeg`` is only called when actually needed."""

    def setUp(self):
        # Ensure we re-import the FFmpeg modules with the spy in place.
        for module_name in list(sys.modules):
            if (
                module_name == "core.utils"
                or module_name.startswith("core.utils.")
                or module_name == "core.ffmpeg_utils"
                or module_name.startswith("core.ffmpeg_utils.")
            ):
                del sys.modules[module_name]

    def _import_with_spies(self):
        """Import ``core.utils`` / ``core.ffmpeg_utils`` after patching the
        setup_ffmpeg + install_ffmpeg call sites.

        Returns a tuple of (core.utils module, setup_spy, install_spy). The
        spies are anchored on ``core.ffmpeg_utils`` because that is where the
        lazy initialisation actually runs; ``core.utils`` is only a re-export.
        """
        import core.utils as fresh
        importlib.reload(fresh)
        import core.ffmpeg_utils as ffmpeg_utils
        importlib.reload(ffmpeg_utils)

        setup_spy = mock.patch.object(
            ffmpeg_utils, "setup_ffmpeg", wraps=ffmpeg_utils.setup_ffmpeg
        ).start()
        install_spy = mock.patch.object(
            ffmpeg_utils, "install_ffmpeg", return_value=(None, None)
        ).start()
        # Reset the lazy gate so subsequent calls trigger setup_ffmpeg again.
        ffmpeg_utils.FFMPEG_PATH = None
        ffmpeg_utils.FFPROBE_PATH = None
        ffmpeg_utils._FFMPEG_SETUP_DONE = False
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
            for module_name in list(sys.modules):
                if (
                    module_name == "core.utils"
                    or module_name.startswith("core.utils.")
                    or module_name == "core.ffmpeg_utils"
                    or module_name.startswith("core.ffmpeg_utils.")
                ):
                    del sys.modules[module_name]
            import core.utils  # noqa: F401
            import core.ffmpeg_utils as ffmpeg_utils
            # No FFmpeg-related work should happen during import.
            self.assertFalse(which_mock.called,
                             "shutil.which must not be called during core.utils import")
            self.assertFalse(run_mock.called,
                             "subprocess.run must not be called during core.utils import")
            self.assertIsNone(ffmpeg_utils.FFMPEG_PATH,
                              "FFMPEG_PATH must be None until ensure_ffmpeg_available() runs")
            self.assertIsNone(ffmpeg_utils.FFPROBE_PATH,
                              "FFPROBE_PATH must be None until ensure_ffmpeg_available() runs")
            self.assertFalse(ffmpeg_utils._FFMPEG_SETUP_DONE,
                             "_FFMPEG_SETUP_DONE must be False until ensure runs")

    def test_ensure_runs_setup_only_once(self):
        """ensure_ffmpeg_available() must call setup_ffmpeg only on first invocation."""
        utils, setup_spy, _ = self._import_with_spies()

        utils.ensure_ffmpeg_available(auto_install=False)
        first_calls = setup_spy.call_count

        utils.ensure_ffmpeg_available(auto_install=False)
        utils.ensure_ffmpeg_available(auto_install=True)
        utils.ensure_ffmpeg_available(auto_install=False)
        self.assertEqual(setup_spy.call_count, first_calls,
                         "setup_ffmpeg must be cached after the first ensure call")

    def test_check_ffmpeg_available_does_not_auto_install_by_default(self):
        """check_ffmpeg_available() defaults to auto_install=False."""
        utils, _, install_spy = self._import_with_spies()
        import core.ffmpeg_utils as ffmpeg_utils

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(ffmpeg_utils, "find_ffmpeg_in_common_paths",
                               return_value=(None, None)):
            utils.check_ffmpeg_available()
            self.assertFalse(install_spy.called,
                             "check_ffmpeg_available() default must not trigger install")

    def test_check_ffmpeg_available_with_auto_install_triggers_install(self):
        """check_ffmpeg_available(auto_install=True) must reach install_ffmpeg()."""
        utils, _, install_spy = self._import_with_spies()
        import core.ffmpeg_utils as ffmpeg_utils

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(ffmpeg_utils, "find_ffmpeg_in_common_paths",
                               return_value=(None, None)):
            utils.check_ffmpeg_available(auto_install=True)
            self.assertTrue(install_spy.called,
                            "check_ffmpeg_available(auto_install=True) must call install_ffmpeg")

    def test_truehd_helpers_trigger_lazy_setup(self):
        """is_truehd_file/convert/get_info trigger ensure_ffmpeg_available(True)."""
        utils, setup_spy, install_spy = self._import_with_spies()
        import core.ffmpeg_utils as ffmpeg_utils

        with mock.patch("shutil.which", return_value=None), \
             mock.patch.object(ffmpeg_utils, "find_ffmpeg_in_common_paths",
                               return_value=(None, None)):
            self.assertFalse(utils.is_truehd_file("/dev/null/nonexistent.mlp"))
            self.assertTrue(setup_spy.called,
                            "is_truehd_file must trigger setup")
            self.assertTrue(install_spy.called,
                            "is_truehd_file must use auto_install=True")

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
