"""Stage only release sweeps and Harman targets; never modify repository data."""
from pathlib import Path
import shutil

CRATE = Path(__file__).resolve().parent
SOURCE = CRATE.parents[1] / "data"
DESTINATION = CRATE / "python" / "impulcifer" / "data"


def main():
    files = sorted([*SOURCE.glob("sweep*.wav"), *SOURCE.glob("harman*.csv")])
    if not files:
        raise SystemExit(f"No package data found in {SOURCE}")
    DESTINATION.mkdir(parents=True, exist_ok=True)
    for source in files:
        shutil.copyfile(source, DESTINATION / source.name)
        print(f"{source.name}: {source.stat().st_size} bytes")


if __name__ == "__main__":
    main()
