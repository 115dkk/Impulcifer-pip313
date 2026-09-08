"""Run P14 gates in the foreground and retain their complete output.

The optional free-threaded interpreter is supplied explicitly, without changing
an existing interpreter's installed packages. Its venv is disposable.
"""
import argparse
from importlib.metadata import distribution
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile

CRATE = Path(__file__).resolve().parent
ROOT = CRATE.parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--free-threaded")
    args = parser.parse_args()
    env = os.environ.copy()
    env["PYTHONIOENCODING"] = "utf-8"
    env["NO_COLOR"] = "1"
    log_path = CRATE / "verification.log"
    with log_path.open("wb") as log:
        def emit(text):
            payload = text.encode("utf-8")
            log.write(payload)
            log.flush()
            sys.stdout.buffer.write(payload)
            sys.stdout.buffer.flush()

        def run(command):
            emit("\n$ " + subprocess.list2cmdline(command) + "\n")
            result = subprocess.run(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            log.write(result.stdout)
            log.flush()
            sys.stdout.buffer.write(result.stdout)
            sys.stdout.buffer.flush()
            emit(f"exit_code={result.returncode}\n")
            if result.returncode:
                raise SystemExit(result.returncode)

        run(["cargo", "fmt", "-p", "impulcifer-python", "--", "--check"])
        run(["cargo", "clippy", "-p", "impulcifer-python", "--all-targets", "--", "--no-deps", "-D", "warnings"])
        run(["cargo", "clippy", "-p", "impulcifer-python", "--all-targets", "--features", "python", "--", "--no-deps", "-D", "warnings"])
        run(["cargo", "test", "-p", "impulcifer-python"])
        build = ["py", "-3.14", "-m", "maturin", "build", "--release", "--features", "python", "-m", str(CRATE / "Cargo.toml")]
        run(build)
        run(["py", "-3.14", "-m", "pytest", str(CRATE / "tests"), "-q"])
        run(["cargo", "test", "-p", "impulcifer-policy"])
        if args.free_threaded:
            run(build + ["-i", args.free_threaded])
            with tempfile.TemporaryDirectory(prefix="impulcifer-p14-ft-") as directory:
                run([args.free_threaded, "-m", "venv", directory])
                python = str(Path(directory) / ("Scripts/python.exe" if os.name == "nt" else "bin/python"))
                # pytest and its dependencies are pure Python. Copy the known
                # installed distributions into this temporary venv so a second
                # interpreter does not require another network installation.
                site = Path(directory) / "Lib" / "site-packages" if os.name == "nt" else next(Path(directory).glob("lib/python*/site-packages"))
                for name in ["pytest", "pluggy", "iniconfig", "packaging", "pygments", "colorama"]:
                    dist = distribution(name)
                    for relative in dist.files or []:
                        if ".." in relative.parts:
                            continue
                        source = Path(dist.locate_file(relative))
                        if source.is_file():
                            target = site / relative
                            target.parent.mkdir(parents=True, exist_ok=True)
                            shutil.copyfile(source, target)
                    emit(f"Staged pure-Python test dependency {name} {dist.version}\n")
                run([python, "-m", "pytest", str(CRATE / "tests"), "-q"])
        for wheel in sorted((ROOT / "target" / "wheels").glob("impulcifer_py313-*.whl")):
            emit(f"wheel={wheel.name} size_bytes={wheel.stat().st_size}\n")


if __name__ == "__main__":
    main()
