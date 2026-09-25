"""The sdist must build where no wheel exists: it has to carry every file the crates compile in."""
import os
from pathlib import Path
import posixpath
import re
import subprocess
import sys
import tarfile

import pytest

CRATE = Path(__file__).resolve().parents[1]
INCLUDE = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"\s*\)')


@pytest.fixture(scope="module")
def sdist(tmp_path_factory):
    out = tmp_path_factory.mktemp("sdist")
    env = {**os.environ, "MATURIN_NO_MISSING_BUILD_BACKEND_WARNING": "1"}
    subprocess.run(
        [sys.executable, "-m", "maturin", "sdist", "-m", str(CRATE / "Cargo.toml"), "-o", str(out)],
        check=True,
        env=env,
    )
    (archive,) = out.glob("*.tar.gz")
    with tarfile.open(archive) as tar:
        files = {member.name for member in tar.getmembers() if member.isfile()}
        sources = {
            name: tar.extractfile(name).read().decode("utf-8")
            for name in files
            if name.endswith(".rs") and "/src/" in name
        }
    return files, sources


def test_sdist_carries_every_file_the_crates_include(sdist):
    """impulcifer-service reads <workspace>/i18n/locales, outside every crate (sync_data.py stages it)."""
    files, sources = sdist
    included = []
    missing = []
    for path, text in sources.items():
        for target in INCLUDE.findall(text):
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(path), target))
            if "/tests/" in resolved:
                continue  # migration goldens, read only by #[cfg(test)] modules
            (included if resolved in files else missing).append(resolved)
    assert not missing, f"sdist lacks {sorted(set(missing))}; run sync_data.py before building it"
    assert any(path.endswith("/i18n/locales/en.json") for path in included)


def test_sdist_builds_through_the_repairing_backend(sdist):
    files, _ = sdist
    root = next(iter(files)).split("/", 1)[0]
    assert {f"{root}/pyproject.toml", f"{root}/build_backend.py", f"{root}/repair_record.py"} <= files
