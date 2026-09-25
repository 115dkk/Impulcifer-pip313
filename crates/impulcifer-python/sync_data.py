"""Stage release sweeps, Harman targets and UI catalogs; never modify repository data.

The sweeps and targets are wheel package data. The 2.x catalogs in i18n/locales
are compiled into impulcifer-service (include_str! reaches three directories up
to the workspace root), so the sdist must carry them at its root: they are
staged beside pyproject.toml, which the sdist puts at the workspace root.
"""
from pathlib import Path
import shutil

CRATE = Path(__file__).resolve().parent
ROOT = CRATE.parents[1]
STAGES = [
    (ROOT / "data", ["sweep*.wav", "harman*.csv"], CRATE / "python" / "impulcifer" / "data"),
    (ROOT / "i18n" / "locales", ["*.json"], CRATE / "i18n" / "locales"),
]


def main():
    for source, patterns, destination in STAGES:
        files = sorted(path for pattern in patterns for path in source.glob(pattern))
        if not files:
            raise SystemExit(f"No package data found in {source}")
        destination.mkdir(parents=True, exist_ok=True)
        for path in files:
            shutil.copyfile(path, destination / path.name)
            print(f"{path.relative_to(ROOT)}: {path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
