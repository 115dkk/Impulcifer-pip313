"""Foreground PA05b command logger; no process enumeration or termination."""
import json
import os
from pathlib import Path
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]


def main():
    name, *command = sys.argv[1:]
    log = HERE / f'seventh-{name}.log'
    if log.exists():
        raise FileExistsError(log)
    env = dict(os.environ, PYTHONIOENCODING='utf-8', PYTHONDONTWRITEBYTECODE='1')
    with log.open('w', encoding='utf-8') as output:
        output.write(json.dumps(command) + '\n')
        output.flush()
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=output,
                                stderr=subprocess.STDOUT, check=False)
        output.write(f'\nEXIT_CODE={result.returncode}\n')
    print(log.read_text(encoding='utf-8'), flush=True)
    sys.exit(result.returncode)


if __name__ == '__main__':
    main()
