"""Run bounded sequential integrity batches, retaining every result."""
import subprocess
import sys
from seventh_run import HERE, ROOT

period, start, end, op, *executable = sys.argv[1:]
exe = executable[0] if executable else str(ROOT / 'target/release/deps/perf-8808d997687b49b0.exe')
failed = False
for run in range(int(start), int(end) + 1):
    command = [sys.executable, str(HERE / 'seventh_run.py'), f'p{period}-{run}-{op}',
               sys.executable, str(HERE / 'sixth_paired.py'), '--period', period,
               '--run', str(run), '--op', f'play_record_{op}', '--exe',
               exe]
    result = subprocess.run(command, cwd=ROOT, check=False)
    failed |= result.returncode != 0
    print(f'TRIAL_EXIT period={period} run={run} op={op} code={result.returncode}', flush=True)
sys.exit(1 if failed else 0)
