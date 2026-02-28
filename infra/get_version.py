"""Read project version from pyproject.toml and print to stdout."""
import os
import sys

# pyproject.toml is always at the project root
script_dir = os.path.dirname(os.path.abspath(__file__))
project_root = os.path.dirname(script_dir)
toml_path = os.path.join(project_root, "pyproject.toml")

try:
    try:
        # Python 3.11+
        import tomllib
        with open(toml_path, "rb") as f:
            config = tomllib.load(f)
    except ImportError:
        # Python < 3.11
        import toml
        with open(toml_path, "r", encoding="utf-8") as f:
            config = toml.load(f)

    print(config["project"]["version"])
except Exception as e:
    print(f"Error reading version: {e}", file=sys.stderr)
    sys.exit(1)
