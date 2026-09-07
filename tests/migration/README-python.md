# P14 Python wheel build and verification

The 3.x distribution is named `impulcifer`; the import package is `impulcifer`
and its native extension is `impulcifer.impulcifer_native`. Do not install it over
the 2.x environment when comparing implementations.

## Build from the checkout

Run these commands in the foreground on Windows, adjusting the checkout prefix
when necessary. The ordinary wheel targets CPython 3.9+ via PyO3 0.27 ABI3.

```powershell
py -3.14 -m pip install maturin pytest
py -3.14 E:/Impulcifer/crates/impulcifer-python/sync_data.py
py -3.14 -m maturin build --release --features python -m E:/Impulcifer/crates/impulcifer-python/Cargo.toml
py -3.14 -m pytest E:/Impulcifer/crates/impulcifer-python/tests -q
```

`sync_data.py` copies the repository's five `data/sweep*.wav` files and four
`data/harman*.csv` files into
`E:/Impulcifer/crates/impulcifer-python/python/impulcifer/data/`.
Those staged files are wheel package data. Sync them again after changing their
repository originals. No demo recordings, generated demo outputs, or TrueHD
master assets are bundled by P14. The staging command never writes to the source
data directory. Wheels appear in `E:/Impulcifer/target/wheels/`.

The package sets `IMPULCIFER_DATA_DIR` to its own installed data directory before
importing the extension, unless the user already supplied that variable. The
existing service `default_data_dir()` handles the override; no service path
changes are required. As in the service, an override that does not name an
existing directory permits fallback to other service candidates.

## Python API

```python
import impulcifer
from impulcifer import impulcifer_native as native

path = impulcifer.main(dir_path="measurements", decay={"FL": 0.35})
result = native.run(
    {"dir_path": "measurements", "decay": 0.35},
    progress=lambda event: print(event["progress"], event["key"]),
    log=lambda event: print(event["level"], event["message"]),
)
assert result["output_path"] == path
```

- `native.version()` and `impulcifer.__version__` expose the Rust workspace version.
- `native.run(config, progress=None, log=None)` returns `{"output_path": ...}`.
  Unknown configuration keys are ignored before JSON conversion, including
  arbitrary Python objects. Known values must be JSON-compatible. Numeric decay
  and speaker-to-number decay dictionaries use **seconds**, not CLI milliseconds.
  Known fields use `ProcessingConfig::from_kwargs` and service validation.
- Progress callbacks receive plain dictionaries with `progress`, `message`, `key`.
  Log callbacks receive `level`, `message`, `key`. They run on the calling thread,
  with interpreter attachment acquired per event. No worker holds Python objects.
- Callback exceptions retain their original Python type. The adapter requests
  cancellation and waits detached until terminal status before propagating them.
  Service failures raise `RuntimeError` containing the service code and message.
- `native.detect_sweep(dir)` and `native.generate_sweep_set(dir)` return the
  corresponding service payloads. Generation writes to the requested directory.
- `native.recover_brir_outputs(dir, **options)` uses P17 service validation and
  recovery, with `include_hangloose` and `remove_silent_channels` options.
  Noncancellable recovery is drained before propagating a pending Python signal.
- `native.cli_main(argv)` accepts arguments without the executable name and returns
  an integer exit code. It calls the shared Rust CLI and writes through the
  caller's Python `sys.stdout` and `sys.stderr`, including redirected streams.
- The installed `impulcifer` console entry calls `impulcifer.cli()`, which uses
  `sys.argv[1:]` and raises `SystemExit` with the CLI exit code.

The current 2.x `impulcifer.py::main` actually returns `None`; the P14 API
intentionally returns the output path required by its work assignment.
The wheel preserves the processing kwargs convention, not the complete 2.x
module surface (`create_cli`, GUI entry points, or Python DSP class imports).
Service validation applies its existing choice/decay checks and `vbass_freq`
clamping. String paths are required for configuration path values.
The shared Rust pipeline's existing unsupported-output/plot restrictions remain.

## Installed-wheel tests

The pytest loader installs the newest matching wheel with `pip --no-deps --target`
into a fresh OS temporary directory, inserts that directory before the repository,
and asserts that it imported the installed package and native `.pyd`/`.so`.
It does not use a mocked extension or the 2.x root module. To select a different
wheel directory, set `IMPULCIFER_WHEEL_DIR` before running pytest.

Tests copy `data/demo` to pytest's temporary directory before running DSP and
remove copied `hesuvi.wav`/`hrir.wav` so existing outputs cannot satisfy checks.
The default test compares the first 256 FL-left samples against
`tests/migration/goldens/p11_default.json` with tolerance `1e-3 * max(abs(reference))`. WAV decoding in the test uses
only Python's standard library. Tests redirect settings to temporary HOME and
USERPROFILE directories and never process the original demo directory.
Windows keeps an imported `.pyd` loaded until process exit; temporary-target
cleanup ignores its file-lock error and may leave that file in the OS temp dir.

Interpreter-free conversion validation is independent of PyO3:

```powershell
cargo test --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-python
```

The complete gate runner captures command output in
`E:/Impulcifer/crates/impulcifer-python/verification.log`:

```powershell
py -3.14 E:/Impulcifer/crates/impulcifer-python/verify.py
```

## Free-threaded CPython

PyO3 0.27 ignores ABI3 on a free-threaded interpreter, which needs a separate
version-specific `cp314-cp314t` wheel. See the
[official PyO3 guide](https://pyo3.rs/v0.27.2/free-threading).
The module explicitly declares `gil_used = false`. The same tests include
concurrent independent jobs, callback cancellation, and an assertion that import
did not enable the GIL on a free-threaded interpreter.

Pass the installed free-threaded executable explicitly; `py -3.14t` need not be
registered even when a uv-managed interpreter exists:

```powershell
py -3.14 -m maturin build --release --features python -m E:/Impulcifer/crates/impulcifer-python/Cargo.toml -i C:/path/to/python3.14t.exe
py -3.14 E:/Impulcifer/crates/impulcifer-python/verify.py --free-threaded C:/path/to/python3.14t.exe
```

The optional verification creates a temporary free-threaded venv. It stages the
already installed pure-Python pytest dependencies into that venv without a second
network installation, then installs and tests the actual free-threaded wheel.
It does not replace the 2.x distribution. Building one wheel does not validate
other operating systems or the complete Python 3.9 through 3.13 runtime range.
