"""PA05 sixth-run independent capture; synchronous prebuilt child, then analysis."""
import argparse
import csv
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
sys.path.insert(0, str(ROOT / 'tests/migration'))
from bench_oracle_impulcifer_audio_io import com_mta, pair  # noqa: E402
from analyze_fifth import fnv, reference, trace_files, waveform  # noqa: E402


def regions(w):
    result = []
    for s in w['splices']:
        kind = 'missing' if s['missing_frames'] else 'repeated'
        result.append(dict(kind=kind, start=min(s['source_before'], s['source_after']),
                           end=max(s['source_before'], s['source_after'])))
    clean = [v for v in w['windows'] if v['rms'] < 3e-6]
    for z in w['zero_runs']:
        center = (z['start'] + z['end']) / 2
        if not clean:
            raise ValueError('zero region has no independently clean alignment window')
        near = min(clean, key=lambda v: abs(center - (v['start'] + v['end']) / 2 - v['lag']))
        result.append(dict(kind='zero', start=z['start'] - near['lag'], end=z['end'] - near['lag']))
    return sorted(result, key=lambda r: (r['kind'], r['start']))


def verify_trace(prefix, ref):
    renders = trace_files(prefix.name, 'render')
    captures = trace_files(prefix.name, 'capture')
    assert len(renders) == len(captures) == 1
    writes = [r for r in renders[0][2] if r['event'] == 'write']
    packets = [r for r in captures[0][2] if r['event'] == 'packet']
    source = ref.astype('<f4').tobytes()
    cursor = 0
    for w in writes:
        d = dict(kv.split('=', 1) for kv in w['detail'].split())
        start, end = int(w['frame_start']), int(w['frame_end'])
        assert start == cursor and end-start == int(w['frames']) == int(d['written'])
        assert int(d['cursor_before']) == start and int(d['cursor_after']) == end
        assert int(d['requested']) == end-start and int(d['bytes']) == (end-start)*8
        assert int(d['after_us']) >= int(d['before_us'])
        assert d['payload_fnv1a'] == fnv(source[start*8:end*8])
        assert d['outcome'] == 'release_ok_not_hardware_consumption'
        cursor = end
    assert cursor == len(ref)
    period = int(prefix.name.split('-')[1][1:])
    period_rows = [r for r in renders[0][2] if r['event'] == 'shared_period']
    assert period_rows and all(f'requested_periods={period} ' in r['detail'] for r in period_rows)
    deliveries = [r for r in captures[0][2] if r['event'] == 'delivery']
    assert int(deliveries[0]['delivery_start']) == 0
    assert all(a['delivery_end'] == b['delivery_start'] for a, b in zip(deliveries, deliveries[1:]))
    return dict(verified_writes=len(writes), submitted_frames=cursor,
                delivered_frames=int(deliveries[-1]['delivery_end']),
                render_trace=str(renders[0][1]), capture_trace=str(captures[0][1]),
                render_buffer_frames=sorted({int(w['buffer_size']) for w in writes}),
                capture_buffer_frames=sorted({int(r['buffer_size']) for r in captures[0][2]
                                             if r['event'] == 'negotiated_buffer'}),
                periods=[r['detail'] for r in renders[0][2] if r['event'] == 'shared_period'],
                packet_sizes=sorted({int(p['frames']) for p in packets}),
                silent_packets=sum(p['silent'] == 'true' for p in packets),
                post_initial_discontinuities=sum(p['discontinuity'] == 'true' and
                                                p['initial_discontinuity'] != 'true' for p in packets),
                index_gaps=[int(b['buffer_index'])-int(a['buffer_index'])-int(a['frames'])
                            for a, b in zip(packets, packets[1:])
                            if int(b['buffer_index']) != int(a['buffer_index'])+int(a['frames'])],
                underruns=sum(int(w['padding']) == 0 for w in writes[1:]))


def analysis_path(prefix):
    dest = Path(str(prefix)+'-analysis.json')
    revision = 2
    while dest.exists():
        dest = Path(str(prefix)+f'-analysis-v{revision}.json')
        revision += 1
    return dest


def analyze(prefix, observer):
    ref, nseg = reference()
    py = np.fromfile(str(prefix)+'-python.f32', dtype='<f4').reshape(-1, 2)
    rust = np.fromfile(str(prefix)+'-play_record_7_speaker_set-0.f32', dtype='<f4').reshape(-1, 2)
    try:
        pw, rw = waveform(ref, py.astype(np.float64), nseg), waveform(ref, rust.astype(np.float64), nseg)
    except (ValueError, IndexError) as error:
        from bench_oracle_impulcifer_audio_io import integrity
        from scipy.signal import correlate
        # Find an unchanged early anchor only to compare the two recordings.
        # This is not source alignment and cannot classify source loss positions.
        anchor_start, anchor_end = 10000, 14096
        anchor = rust[anchor_start:anchor_end, 0].astype(np.float64)
        corr = correlate(py[:200000, 0].astype(np.float64), anchor, mode='valid', method='fft')
        offset = int(np.argmax(corr)) - anchor_start
        a, b = max(0, offset), max(0, -offset)
        common = min(len(py)-a, len(rust)-b)
        p, r = py[a:a+common], rust[b:b+common]
        result = dict(prefix=str(prefix), observer=observer, trace=verify_trace(prefix, ref),
                      conclusive=False, accepted=False, internal_frames=None, external_frames=None,
                      analysis_error=str(error), common_frames=common,
                      python_minus_rust_offset=offset,
                      pair_bit_exact=np.array_equal(p.view('u4'), r.view('u4')),
                      pair_residual_rms=np.sqrt(np.mean((p.astype(np.float64)-r)**2, axis=0)).tolist(),
                      rust_global_metrics=integrity(ref, rust), python_global_metrics=integrity(ref, py))
        dest = analysis_path(prefix)
        dest.write_text(json.dumps(result, indent=2), encoding='utf-8')
        print(json.dumps({k: result[k] for k in ('prefix', 'analysis_error', 'conclusive',
                         'internal_frames', 'external_frames', 'pair_bit_exact', 'common_frames')}), flush=True)
        return
    offsets = [p['lag']-r['lag'] for p, r in zip(pw['windows'], rw['windows'])
               if p['rms'] < 3e-6 and r['rms'] < 3e-6]
    offset = max(set(offsets), key=offsets.count) if offsets else None
    common = 0
    pair_rms = None
    pair_exact = False
    if offset is not None:
        a, b = max(0, offset), max(0, -offset)
        common = min(len(py)-a, len(rust)-b)
        p, r = py[a:a+common], rust[b:b+common]
        pair_exact = np.array_equal(p.view('u4'), r.view('u4'))
        pair_rms = np.sqrt(np.mean((p.astype(np.float64)-r)**2, axis=0)).tolist()
    pr, rr = regions(pw), regions(rw)
    external = [r for r in rr if r in pr]
    rust_only = [r for r in rr if r not in pr]
    python_only = [r for r in pr if r not in rr]
    # Never infer absence of defects from a splice fit. Raw global residuals are
    # retained; unexplained bad windows or non-overlap make this inconclusive.
    explained = True
    for w in (rw, pw):
        for v in w['windows']:
            if v['rms'] >= 3e-6:
                candidates = regions(w)
                if not any(r['start'] < v['end'] and r['end'] > v['start'] for r in candidates):
                    explained = False
        if any(not np.isfinite(s['piecewise_rms']) or s['piecewise_rms'] >= 3e-6 for s in w['splices']):
            explained = False
        if any(m['overlap_frames'] != len(ref) for m in w['metrics']):
            explained = False
        if any(m['repeated_128_blocks'] for m in w['metrics']):
            explained = False
    trace = verify_trace(prefix, ref)
    observer_ok = not observer['statuses'] and observer['returncode'] == 0
    full_pair = common == len(rust) and offset is not None and offset >= 0
    conclusive = (observer_ok and full_pair and explained and not python_only and
                  (pair_exact or (pw['passed'] and bool(rust_only))))
    accepted = (conclusive and pair_exact and not rust_only and trace['underruns'] == 0 and
                not trace['index_gaps'] and trace['post_initial_discontinuities'] == 0 and
                trace['silent_packets'] == 0)
    result = dict(prefix=str(prefix), observer=observer, rust=rw, python=pw, trace=trace,
                  python_minus_rust_offset=offset, common_frames=common, pair_bit_exact=pair_exact,
                  pair_residual_rms=pair_rms, offsets=sorted(set(offsets)),
                  external_regions=external if conclusive else [],
                  candidate_shared_regions=[] if conclusive else external,
                  internal_regions=rust_only if conclusive else [],
                  unclassified_rust_regions=[] if conclusive else rr,
                  python_only_regions=python_only, conclusive=conclusive, accepted=accepted,
                  internal_frames=sum(r['end']-r['start'] for r in rust_only) if conclusive else None,
                  external_frames=sum(r['end']-r['start'] for r in external) if conclusive else None)
    dest = analysis_path(prefix)
    dest.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print(json.dumps({k: result[k] for k in ('prefix', 'conclusive', 'accepted', 'internal_frames',
                     'external_frames', 'external_regions', 'internal_regions', 'common_frames',
                     'pair_bit_exact', 'pair_residual_rms')}), flush=True)
    print('rust_unfitted', rw['unfitted_rms'], 'python_unfitted', pw['unfitted_rms'],
          'buffers', trace['render_buffer_frames'], trace['capture_buffer_frames'],
          'underruns', trace['underruns'], 'artifact', dest, flush=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--period', type=int, choices=(2, 3, 4), required=True)
    parser.add_argument('--run', type=int, required=True)
    parser.add_argument('--exe', type=Path)
    parser.add_argument('--analyze-only', action='store_true')
    args = parser.parse_args()
    prefix = HERE / f'sixth-p{args.period}-{args.run:02d}'
    metadata = Path(str(prefix)+'-observer.json')
    if args.analyze_only:
        analyze(prefix, json.loads(metadata.read_text(encoding='utf-8')))
        return
    assert args.exe is not None and args.exe.is_absolute() and args.exe.is_file()
    assert not list(HERE.glob(prefix.name+'-*')), 'refusing to overwrite trial artifacts'
    # Import only after COM has been initialized on this owning thread.
    import sounddevice as sd
    blocks, statuses, callbacks = [], [], []
    ready = threading.Event()

    def capture(samples, frames, clock, status):
        blocks.append(samples.copy())
        callbacks.append((frames, clock.inputBufferAdcTime, clock.currentTime, str(status)))
        if status:
            statuses.append(str(status))
        ready.set()

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
            observer = dict(device=stream.device, dtype=stream.dtype, latency=stream.latency,
                            samplerate=stream.samplerate, channels=stream.channels,
                            blocksize=stream.blocksize, statuses=statuses, exe=str(args.exe))
            # No cargo invocation here: all compilation precedes opening capture.
            with Path(str(prefix)+'-rust.log').open('w', encoding='utf-8') as log:
                completed = subprocess.run([str(args.exe)], cwd=ROOT, env=env,
                                           stdout=log, stderr=subprocess.STDOUT, check=False)
    np.concatenate(blocks).astype('<f4').tofile(str(prefix)+'-python.f32')
    with Path(str(prefix)+'-python-callbacks.csv').open('w', newline='', encoding='utf-8') as f:
        writer = csv.writer(f)
        writer.writerow(['frames', 'adc_time', 'current_time', 'status'])
        writer.writerows(callbacks)
    observer['returncode'] = completed.returncode
    metadata.write_text(json.dumps(observer, indent=2), encoding='utf-8')
    if completed.returncode:
        raise RuntimeError(f'Rust trial failed; raw capture and log retained: {prefix}')
    analyze(prefix, observer)


if __name__ == '__main__':
    with com_mta():
        main()
