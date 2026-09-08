"""Foreground independent WASAPI capture around the SAME Rust playback."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT / 'tests/migration'))
from bench_oracle_impulcifer_audio_io import com_mta, pair  # noqa: E402


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('run', type=int)
    args = parser.parse_args()
    import numpy as np
    import sounddevice as sd
    from analyze_fifth import reference, waveform

    ref, nseg = reference()
    ready = threading.Event()
    blocks, statuses = [], []

    def capture(samples, frames, clock, status):
        blocks.append(samples.copy())
        if status:
            statuses.append(str(status))
        ready.set()

    prefix = HERE / f'fifth-paired-{args.run}'
    env = dict(os.environ, IMPULCIFER_PA05_OP='play_record_7_speaker_set',
               IMPULCIFER_PA05_INTEGRITY='1', IMPULCIFER_PA05_INTEGRITY_RUNS='1',
               IMPULCIFER_PA05_TAIL='1', IMPULCIFER_PA05_CAPTURE_PREFIX=str(prefix),
               IMPULCIFER_PA05_TRACE_PREFIX=str(prefix))
    with com_mta():
        devices = pair(sd)
        with sd.InputStream(device=devices[0]['index'], samplerate=48000, channels=2,
                            dtype='float32', callback=capture, blocksize=480) as stream:
            if not ready.wait(10):
                raise RuntimeError('independent capture did not start')
            print('independent', stream.device, stream.dtype, stream.latency, flush=True)
            # Synchronous child; callback observes it until its full join/drop.
            subprocess.run(['cargo', 'bench', '--manifest-path', str(ROOT/'Cargo.toml'),
                            '-p', 'impulcifer-audio-io', '--bench', 'perf'],
                           env=env, cwd=ROOT, check=True)
    y = np.concatenate(blocks)
    y.astype('<f4').tofile(str(prefix)+'-python.f32')
    rust = np.fromfile(str(prefix)+'-play_record_7_speaker_set-0.f32', dtype='<f4').reshape(-1, 2)
    result = dict(run=args.run, statuses=statuses, python=waveform(ref, y.astype(np.float64), nseg),
                  rust=waveform(ref, rust.astype(np.float64), nseg))
    Path(str(prefix)+'-comparison.json').write_text(json.dumps(result, indent=2), encoding='utf-8')
    for kind in ('python', 'rust'):
        r = result[kind]
        print(kind, 'pass', r['passed'], 'missing', r['missing_frames'], 'repeated', r['repeated_frames'],
              'zeros', r['active_zero_frames'], 'unfitted', r['unfitted_rms'],
              'splices', r['splices'], flush=True)
    print('statuses', statuses)


if __name__ == '__main__':
    with com_mta():
        main()
