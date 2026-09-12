"""PA07 foreground logger; no process enumeration or termination."""
import json
import os
from pathlib import Path
import subprocess
import sys
from datetime import datetime

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]


def main():
    name, *command = sys.argv[1:]
    log = HERE / f'eighth-{name}.log'
    if log.exists():
        raise FileExistsError(log)
    if not command:
        raise ValueError('a foreground command is required')
    inherited = {k: v for k, v in os.environ.items()
                 if k.startswith('IMPULCIFER_PA05') or k == 'CI'}
    if inherited:
        raise RuntimeError(f'unexpected benchmark environment: {inherited}')
    temporary = HERE / 'eighth-temp'
    temporary.mkdir(exist_ok=True)
    env = dict(os.environ, PYTHONIOENCODING='utf-8', PYTHONDONTWRITEBYTECODE='1',
               TEMP=str(temporary), TMP=str(temporary), TMPDIR=str(temporary))
    with log.open('x', encoding='utf-8') as output:
        output.write(json.dumps(command) + '\n')
        output.write(f'START={datetime.now().astimezone().isoformat()}\n')
        output.flush()
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=output,
                                stderr=subprocess.STDOUT, check=False)
        output.write(f'END={datetime.now().astimezone().isoformat()}\n')
        output.write(f'EXIT_CODE={result.returncode}\n')
    print(f'LOG={log}', flush=True)
    lines = log.read_text(encoding='utf-8', errors='replace').splitlines()
    if len(lines) < 60 or result.returncode:
        print('\n'.join(lines), flush=True)
    else:
        print('\n'.join(line for line in lines if line.startswith((
            '|', 'test result:', 'START=', 'END=', 'EXIT_CODE=', 'error', 'Error',
            'selected', 'implementation', 'Finished', '    Finished', '     Running',
            'buffers', 'rust_unfitted', '{"prefix"'))), flush=True)
    sys.exit(result.returncode)


if __name__ == '__main__':
    main()
