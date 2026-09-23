"""Real-measurement parity: 3.x Rust CLI vs the last 2.x release vs LionLion123/Impulcifer.

Not a CI test (it needs the private measurement set, which must never be
uploaded as a CI artifact). Run it on a scratch machine:

    python tests/migration/realdata_parity.py \
        --data /path/to/drive --work /path/to/work \
        --rust target/release/impulcifer \
        --v2 /path/to/venv-2.14.2/bin/impulcifer \
        --lion-python /path/to/py38/bin/python --lion-repo /path/to/LionLion123/impulcifer \
        --sweep data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav \
        --scenario default --scenario vbass --jobs 4

Cases are every room-recording folder of a measurer combined with every
headphone recording of the same measurer. Each implementation gets its own
directory holding hard links to the same inputs and the same explicit sweep.

With --identity-rust NEXT the run compares two 3.x builds instead: the
--rust binary and NEXT on every case, by SHA-256 of hrir.wav and hesuvi.wav.
--match keeps only the cases whose label contains the given text.

The acceptance rule is the one the 3.x demo parity tests certify
(crates/impulcifer-service/tests/demo_parity.rs, tests/migration/README-service.md
"Numerical contracts"): per named track, identical sample rate and length,
max |a - b| <= 1e-3 * max|ref|, max-abs and RMS ratios within 1e-4, the same
absolute-maximum index, and silent tracks stay silent. The reference is 2.x
for (3.x, 2.x) and (LionLion, 2.x), and LionLion for (3.x, LionLion).
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from concurrent import futures
from pathlib import Path

import numpy as np
import soundfile as sf

HEXADECAGONAL = ['FL-left', 'FL-right', 'FR-left', 'FR-right', 'FC-left', 'FC-right', 'LFE-left',
                 'LFE-right', 'BL-left', 'BL-right', 'BR-left', 'BR-right', 'SL-left', 'SL-right',
                 'SR-left', 'SR-right', 'WL-left', 'WL-right', 'WR-left', 'WR-right', 'TFL-left',
                 'TFL-right', 'TFR-left', 'TFR-right', 'TSL-left', 'TSL-right', 'TSR-left',
                 'TSR-right', 'TBL-left', 'TBL-right', 'TBR-left', 'TBR-right']
HESUVI = ['FL-left', 'FL-right', 'SL-left', 'SL-right', 'BL-left', 'BL-right', 'FC-left', 'FR-right',
          'FR-left', 'SR-right', 'SR-left', 'BR-right', 'BR-left', 'FC-right', 'WL-left', 'WL-right',
          'WR-left', 'WR-right', 'TFL-left', 'TFL-right', 'TFR-left', 'TFR-right', 'TSL-left',
          'TSL-right', 'TSR-left', 'TSR-right', 'TBL-left', 'TBL-right', 'TBR-left', 'TBR-right']
IMPLS = ('rust', 'v2', 'lion')
PAIRS = (('rust', 'v2'), ('lion', 'v2'), ('rust', 'lion'))
HEADPHONE_DIR = re.compile(r'(head|ear)phones impulse', re.IGNORECASE)
ROOM_DIR = re.compile(r'(room|reverb) impulse', re.IGNORECASE)


def discover(data):
    """(case_id, room_dir, headphones_wav) for every room x headphone pair of a measurer."""
    groups = {}
    for wav in sorted(data.rglob('*.wav')):
        rel = wav.relative_to(data)
        parts = rel.parts
        marker = next((i for i, p in enumerate(parts) if HEADPHONE_DIR.fullmatch(p) or ROOM_DIR.fullmatch(p)), None)
        if marker is None or wav.name.endswith('responses.wav'):
            continue
        group = Path(*parts[:marker])
        entry = groups.setdefault(group, {'rooms': set(), 'headphones': []})
        if HEADPHONE_DIR.fullmatch(parts[marker]):
            if wav.name == 'headphones.wav':
                entry['headphones'].append(wav)
        else:
            entry['rooms'].add(wav.parent)
    cases = []
    for group, entry in sorted(groups.items()):
        for room in sorted(entry['rooms']):
            for headphones in sorted(entry['headphones']):
                label = f"{group} | {room.relative_to(data / group)} | {headphones.parent.relative_to(data / group)}"
                case_id = hashlib.sha1(label.encode()).hexdigest()[:10]
                cases.append((case_id, label, room, headphones))
    return cases


def link_inputs(target, room, headphones):
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    for wav in sorted(room.glob('*.wav')):
        if wav.name.endswith('responses.wav') or wav.name == 'headphones.wav':
            continue
        os.link(wav, target / wav.name)
    os.link(headphones, target / 'headphones.wav')


def command(impl, args, directory, scenario):
    base = [f'--dir_path={directory}', f'--test_signal={args.sweep}']
    if impl in ('rust', 'rust_next'):
        cmd = [args.rust if impl == 'rust' else args.identity_rust, *base]
        if scenario == 'vbass':
            cmd += ['--vbass', '--vbass_freq=250', '--vbass_polarity=normal']
        return cmd, None
    if impl == 'v2':
        cmd = [args.v2, *base]
        if scenario == 'vbass':
            cmd += ['--vbass', '--vbass_freq=250', '--vbass_polarity=normal']
        return cmd, None
    cmd = [args.lion_python, 'impulcifer.py', *base]
    if scenario == 'vbass':
        cmd += ['--vbass', '250']
    return cmd, args.lion_repo


def run_one(args, case_id, room, headphones, impl, scenario):
    directory = Path(args.work) / scenario / case_id / impl
    marker = directory / 'run.json'
    if marker.exists():
        return json.loads(marker.read_text())
    link_inputs(directory, room, headphones)
    cmd, cwd = command(impl, args, str(directory), scenario)
    env = dict(os.environ, MPLBACKEND='Agg', RAYON_NUM_THREADS='1', OMP_NUM_THREADS='1',
               OPENBLAS_NUM_THREADS='1', MKL_NUM_THREADS='1', PYTHONIOENCODING='utf-8')
    start = time.perf_counter()
    proc = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, errors="replace", timeout=1800, check=False)
    seconds = time.perf_counter() - start
    (directory / 'stdout.log').write_text(proc.stdout + '\n--- stderr ---\n' + proc.stderr)
    result = {'impl': impl, 'returncode': proc.returncode, 'seconds': seconds}
    marker.write_text(json.dumps(result))
    return result


def tracks(path, order):
    data, fs = sf.read(str(path), dtype='float64', always_2d=True)
    return fs, {order[i]: data[:, i] for i in range(data.shape[1])}


def compare_tracks(test, ref):
    """demo_parity.rs rule; returns (ok, per-track rows)."""
    fs_t, t = test
    fs_r, r = ref
    rows = []
    ok = fs_t == fs_r
    names = [n for n in HEXADECAGONAL if n in t or n in r]
    length = max(len(v) for v in list(t.values()) + list(r.values()))
    for name in names:
        a = t.get(name)
        b = r.get(name)
        if a is None:
            a = np.zeros(len(b))
        if b is None:
            b = np.zeros(len(a))
        row = {'track': name, 'len_test': len(a), 'len_ref': len(b)}
        if len(a) != len(b):
            row.update(ok=False, reason='length')
            ok = False
            rows.append(row)
            continue
        refmax = float(np.max(np.abs(b)))
        tmax = float(np.max(np.abs(a)))
        if refmax == 0.0:
            row.update(ok=tmax == 0.0, silent=True, max_abs_test=tmax)
            ok &= row['ok']
            rows.append(row)
            continue
        error = float(np.max(np.abs(a - b)))
        rms_t = float(np.sqrt(np.mean(a * a)))
        rms_r = float(np.sqrt(np.mean(b * b)))
        peak_t = int(np.argmax(np.abs(a)))
        peak_r = int(np.argmax(np.abs(b)))
        row.update(rel_error=error / refmax, max_ratio=tmax / refmax, rms_ratio=rms_t / rms_r if rms_r else float('inf'),
                   peak_test=peak_t, peak_ref=peak_r)
        row['ok'] = (error <= 1e-3 * refmax and abs(row['max_ratio'] - 1) <= 1e-4
                     and abs(row['rms_ratio'] - 1) <= 1e-4 and peak_t == peak_r)
        ok &= row['ok']
        rows.append(row)
    return ok and bool(rows), rows, length


def compare_case(args, case_id, scenario):
    base = Path(args.work) / scenario / case_id
    status = {impl: json.loads((base / impl / 'run.json').read_text()) for impl in IMPLS}
    out = {'case': case_id, 'scenario': scenario, 'runs': status, 'pairs': {}}
    loaded = {}
    for impl in IMPLS:
        directory = base / impl
        if status[impl]['returncode'] != 0 or not (directory / 'hrir.wav').exists():
            continue
        loaded[impl] = {
            'hrir': tracks(directory / 'hrir.wav', HEXADECAGONAL),
            'hesuvi': tracks(directory / 'hesuvi.wav', HESUVI),
        }
    for test, ref in PAIRS:
        if test not in loaded or ref not in loaded:
            out['pairs'][f'{test}~{ref}'] = {'ok': False, 'reason': 'missing output'}
            continue
        pair = {}
        ok = True
        for product in ('hrir', 'hesuvi'):
            product_ok, rows, length = compare_tracks(loaded[test][product], loaded[ref][product])
            ok &= product_ok
            active = [r for r in rows if not r.get('silent')]
            pair[product] = {
                'ok': product_ok,
                'frames': length,
                'tracks': len(active),
                'worst_rel_error': max((r.get('rel_error', 0.0) for r in active), default=None),
                'worst_max_ratio_dev': max((abs(r.get('max_ratio', 1.0) - 1) for r in active), default=None),
                'worst_rms_ratio_dev': max((abs(r.get('rms_ratio', 1.0) - 1) for r in active), default=None),
                'peak_mismatches': [r['track'] for r in active if r.get('peak_test') != r.get('peak_ref')],
                'failures': [r for r in rows if not r['ok']],
            }
        pair['ok'] = ok
        out['pairs'][f'{test}~{ref}'] = pair
    return out


def identity_case(args, case_id, scenario):
    """Whether two 3.x builds wrote the same bytes (or failed alike) on a case."""
    base = Path(args.work) / scenario / case_id
    runs = {impl: json.loads((base / impl / 'run.json').read_text()) for impl in ('rust', 'rust_next')}
    digests = {
        impl: {name: hashlib.sha256((base / impl / name).read_bytes()).hexdigest()
               for name in ('hrir.wav', 'hesuvi.wav') if (base / impl / name).exists()}
        for impl in runs
    }
    identical = (runs['rust']['returncode'] == runs['rust_next']['returncode']
                 and digests['rust'] == digests['rust_next'])
    return {'case': case_id, 'scenario': scenario, 'runs': runs, 'digests': digests, 'identical': identical}


def run_case(unit):
    """Run the three implementations on one case, compare, then drop the audio.

    Only the logs, run.json and result.json stay on disk; the measurement set
    is large enough that keeping every output fills a scratch disk.
    """
    args, case_id, room, headphones, scenario = unit
    base = Path(args.work) / scenario / case_id
    marker = base / 'result.json'
    if marker.exists():
        return json.loads(marker.read_text())
    impls = ('rust', 'rust_next') if args.identity_rust else IMPLS
    for impl in impls:
        run_one(args, case_id, room, headphones, impl, scenario)
    if args.identity_rust:
        result = identity_case(args, case_id, scenario)
    else:
        result = compare_case(args, case_id, scenario)
    marker.write_text(json.dumps(result, default=float))
    for path in sorted(base.rglob('*'), reverse=True):
        if path.is_file() and path.suffix.lower() in ('.wav', '.png', '.html'):
            path.unlink()
        elif path.is_dir() and path.name == 'plots':
            shutil.rmtree(path, ignore_errors=True)
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--data', required=True)
    parser.add_argument('--work', required=True)
    parser.add_argument('--rust', required=True)
    parser.add_argument('--v2')
    parser.add_argument('--lion-python')
    parser.add_argument('--lion-repo')
    parser.add_argument('--identity-rust')
    parser.add_argument('--match')
    parser.add_argument('--sweep', required=True)
    parser.add_argument('--scenario', action='append', choices=('default', 'vbass'))
    parser.add_argument('--jobs', type=int, default=os.cpu_count())
    parser.add_argument('--limit', type=int)
    parser.add_argument('--report', default='report.json')
    args = parser.parse_args()
    if not args.identity_rust and not (args.v2 and args.lion_python and args.lion_repo):
        parser.error('--v2, --lion-python and --lion-repo are required without --identity-rust')
    args.sweep = str(Path(args.sweep).resolve())
    scenarios = args.scenario or ['default']
    cases = discover(Path(args.data))
    if args.match:
        cases = [case for case in cases if args.match in case[1]]
    if args.limit:
        cases = cases[:args.limit]
    Path(args.work).mkdir(parents=True, exist_ok=True)
    (Path(args.work) / 'cases.json').write_text(json.dumps(
        [{'case': c, 'label': label} for c, label, _, _ in cases], ensure_ascii=False, indent=1))
    units = [(args, c, room, hp, scenario) for scenario in scenarios for c, _label, room, hp in cases]
    print(f'{len(cases)} cases x {len(scenarios)} scenarios x {len(IMPLS)} implementations', flush=True)
    results = []
    with futures.ProcessPoolExecutor(max_workers=args.jobs) as pool:
        for done, result in enumerate(pool.map(run_case, units, chunksize=1), 1):
            results.append(result)
            if done % 10 == 0 or done == len(units):
                print(f'  {done}/{len(units)} cases', flush=True)
    Path(args.report).write_text(json.dumps(results, indent=1, default=float))
    if args.identity_rust:
        for scenario in scenarios:
            subset = [r for r in results if r['scenario'] == scenario]
            same = sum(1 for r in subset if r['identical'])
            print(f'{scenario:8s} identical {same}/{len(subset)}')
        return 0
    for scenario in scenarios:
        subset = [r for r in results if r['scenario'] == scenario]
        for pair in (f'{a}~{b}' for a, b in PAIRS):
            passed = sum(1 for r in subset if r['pairs'][pair]['ok'])
            print(f'{scenario:8s} {pair:11s} {passed}/{len(subset)} within budget')


if __name__ == '__main__':
    sys.exit(main())
