"""Rust-backed Impulcifer API and installed CLI entry point."""
import os
from pathlib import Path
import sys

# Use an owned package-data directory, never cwd or the caller's source tree.
# An explicit user override is retained, including when it is invalid.
os.environ.setdefault("IMPULCIFER_DATA_DIR", str(Path(__file__).resolve().parent / "data"))

from . import impulcifer_native  # noqa: E402

__version__ = impulcifer_native.version()
detect_sweep = impulcifer_native.detect_sweep
generate_sweep_set = impulcifer_native.generate_sweep_set
recover_brir_outputs = impulcifer_native.recover_brir_outputs


def main(**kwargs):
    """Generate BRIR outputs and return the hesuvi.wav path.

    Unknown processing kwargs are ignored. For event callbacks use
    ``impulcifer_native.run(config, progress=callback, log=callback)``.
    """
    return impulcifer_native.run(kwargs)["output_path"]


def cli():
    """Run the shared Rust CLI without replacing Python's output streams."""
    sys.exit(impulcifer_native.cli_main(sys.argv[1:]))
