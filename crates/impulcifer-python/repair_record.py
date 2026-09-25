"""Rewrite each built wheel's RECORD as CSV and check it against the wheel.

maturin (1.15.0 and earlier) writes RECORD rows as ``path,sha256=...,size``
without CSV quoting. The bundled ``sweep-seg-FL,FR-stereo-...wav`` has a comma
in its name, so a CSV reader splits its row in the wrong place and PyPI finds
the wheel's contents disagreeing with RECORD. The hash and size never contain a
comma, so an unparseable row is split from the right and written back with the
csv module. Every wheel is then read back as CSV and compared with the archive.

Usage: repair_record.py [wheel or directory ...] (default: target/wheels).
"""
import base64
import csv
import hashlib
import io
import os
from pathlib import Path
import sys
import zipfile

CRATE = Path(__file__).resolve().parent
WHEELS = CRATE.parents[1] / "target" / "wheels"


def _record_name(archive):
    names = [info.filename for info in archive.infolist()]
    records = [name for name in names if name.count("/") == 1 and name.endswith(".dist-info/RECORD")]
    if len(records) != 1:
        raise ValueError(f"expected one .dist-info/RECORD, found {records}")
    return records[0]


def _split(line):
    row = next(csv.reader([line]))
    return row if len(row) == 3 else line.rsplit(",", 2)


def repair(wheel):
    """Rewrite RECORD as CSV in place; return False if it already was."""
    wheel = Path(wheel)
    with zipfile.ZipFile(wheel) as archive:
        record = _record_name(archive)
        text = archive.read(record).decode("utf-8")
        buffer = io.StringIO()
        csv.writer(buffer, lineterminator="\n").writerows(_split(line) for line in text.splitlines() if line)
        fixed = buffer.getvalue()
        if fixed == text:
            return False
        partial = wheel.with_name(wheel.name + ".partial")
        with zipfile.ZipFile(partial, "w") as output:
            for info in archive.infolist():
                copy = zipfile.ZipInfo(info.filename, info.date_time)
                copy.compress_type = info.compress_type
                copy.create_system = info.create_system
                copy.external_attr = info.external_attr
                data = fixed.encode("utf-8") if info.filename == record else archive.read(info)
                output.writestr(copy, data)
    os.replace(partial, wheel)
    return True


def verify(wheel):
    """Return every disagreement between RECORD, read as CSV, and the archive."""
    problems = []
    with zipfile.ZipFile(wheel) as archive:
        record = _record_name(archive)
        files = [info.filename for info in archive.infolist() if not info.is_dir()]
        problems += [f"duplicate archive entry {name}" for name in sorted(set(files)) if files.count(name) > 1]
        rows = {}
        for row in csv.reader(io.StringIO(archive.read(record).decode("utf-8"))):
            if len(row) != 3:
                problems.append(f"RECORD row with {len(row)} fields: {row}")
            elif row[0] in rows:
                problems.append(f"duplicate RECORD row {row[0]}")
            else:
                rows[row[0]] = row[1:]
        for name in files:
            if name not in rows:
                problems.append(f"{name} is missing from RECORD")
            elif name != record:
                data = archive.read(name)
                algorithm, _, expected = rows[name][0].partition("=")
                actual = base64.urlsafe_b64encode(hashlib.new(algorithm, data).digest()).rstrip(b"=").decode("ascii")
                if actual != expected or rows[name][1] != str(len(data)):
                    problems.append(f"{name} does not match its RECORD hash or size")
        problems += [f"RECORD lists {name}, which is not in the wheel" for name in rows if name not in files]
    return problems


def main(arguments):
    wheels = []
    for argument in arguments or [WHEELS]:
        path = Path(argument)
        wheels += sorted(path.glob("*.whl")) if path.is_dir() else [path]
    if not wheels:
        raise SystemExit(f"No wheels found in {arguments or WHEELS}")
    failed = False
    for wheel in wheels:
        status = "rewritten" if repair(wheel) else "already CSV"
        problems = verify(wheel)
        print(f"{wheel.name}: RECORD {status}, {'FAILED' if problems else 'matches the wheel'}")
        for problem in problems:
            print(f"  {problem}")
        failed = failed or bool(problems)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
