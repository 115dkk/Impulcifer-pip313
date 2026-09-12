"""Run a bounded PA07 trial series serially and wait for every child."""
import argparse
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--period', type=int, choices=(2, 3, 4), required=True)
    parser.add_argument('--first', type=int, required=True)
    parser.add_argument('--last', type=int, required=True)
    parser.add_argument('--exe', type=Path, required=True)
    args = parser.parse_args()
    failures = []
    for run in range(args.first, args.last + 1):
        for suffix, op in (('headphones', 'play_record_headphones_sweep'),
                           ('seven', 'play_record_7_speaker_set')):
            command = [sys.executable, str(HERE / 'eighth_run.py'),
                       f'p{args.period}-{run}-{suffix}', 'py', '-3.14',
                       str(HERE / 'sixth_paired.py'), '--period', str(args.period),
                       '--run', str(run), '--op', op, '--exe', str(args.exe)]
            result = subprocess.run(command, check=False)
            if result.returncode:
                failures.append((run, op, result.returncode))
    print('COMMAND_FAILURES', failures, flush=True)
    raise SystemExit(bool(failures))


if __name__ == '__main__':
    main()
