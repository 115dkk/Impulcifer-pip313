"""Offline PA05 fifth-run waveform and exact submitted-byte checks."""
import argparse
import csv
import json
from pathlib import Path

import numpy as np
import soundfile as sf

from analyze_fourth import ROOT, integrity, local_offset, runs

HERE = Path(__file__).resolve().parent


def reference():
    mono, fs = sf.read(ROOT / 'data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav')
    assert fs == 48000
    ref = np.zeros((len(mono) * 7, 2))
    for seg in range(7):
        ref[seg*len(mono):(seg+1)*len(mono), seg % 2] = mono
    return ref, len(mono)


def waveform(ref, y, segment_frames):
    metrics = integrity(ref, y)
    windows, splices = [], []
    for seg in range(7):
        ch = seg % 2
        start_seg, end_seg = seg * segment_frames, (seg+1)*segment_frames
        segment_windows = []
        for start in range(start_seg, end_seg, 24000):
            end = min(start+24000, end_seg)
            if end-start < 4096:
                continue
            lag, rms = local_offset(ref, y, start, end, ch, metrics[ch]['lag_frames'])
            segment_windows.append(dict(start=start, end=end, ch=ch, lag=lag, rms=rms))
        windows.extend(segment_windows)
        # Exact two-offset splice fit within each segment. Intermediate window
        # estimates are not treated as extra slips. More than one splice per
        # segment remains an explicit limitation, exposed by piecewise RMS.
        clean = [w for w in segment_windows if w['rms'] < 3e-6]
        # A corrupted low-frequency window may suggest an arbitrary 1-2 frame
        # lag. Only clean windows can establish offsets on either side.
        if not clean:
            continue
        first, last = clean[0]['lag'], clean[-1]['lag']
        if first != last:
            lo = max(0, start_seg+max(first, last))
            hi = min(len(y), end_seg+min(first, last))
            idx = np.arange(lo, hi)
            early = (y[idx, ch] - ref[idx-first, ch])**2
            late = (y[idx, ch] - ref[idx-last, ch])**2
            cost = np.r_[0.0, np.cumsum(early)] + np.r_[np.cumsum(late[::-1])[::-1], 0.0]
            cut = lo+int(np.argmin(cost))
            splices.append(dict(segment=seg, capture_cut=cut, source_before=cut-first,
                                source_after=cut-last, missing_frames=max(0, first-last),
                                repeated_frames=max(0, last-first), before_lag=first,
                                after_lag=last, piecewise_rms=float(np.sqrt(cost.min()/len(idx)))))
    unfitted = []
    for ch, m in enumerate(metrics):
        lag, n = m['lag_frames'], m['overlap_frames']
        a, b = max(0, -lag), max(0, lag)
        unfitted.append(float(np.sqrt(np.mean((y[b:b+n, ch]-ref[a:a+n, ch])**2))))
    zeros = []
    for start, end in runs(np.all(y == 0, axis=1)):
        center = (start+end)//2
        candidates = [w for w in windows if w['rms'] < 3e-6]
        w = min(candidates or windows, key=lambda w: abs(center-(w['start']+w['end'])/2-w['lag']))
        lag = w['lag']
        lo, hi = max(start, lag), min(end, len(ref)+lag)
        if hi > lo:
            active = np.max(np.abs(ref[lo-lag:hi-lag]), axis=1) > 1e-5
            zeros.extend(dict(start=lo+a, end=lo+b) for a, b in runs(active))
    return dict(metrics=metrics, unfitted_rms=unfitted, windows=windows, splices=splices,
                active_zero_frames=sum(z['end']-z['start'] for z in zeros), zero_runs=zeros,
                missing_frames=sum(s['missing_frames'] for s in splices),
                repeated_frames=sum(s['repeated_frames'] for s in splices),
                passed=all(m['overlap_frames'] == len(ref) and m['residual_rms'] < 2e-6
                           for m in metrics) and not splices and not zeros)


def trace_files(prefix, kind):
    found = []
    for p in HERE.glob(f'{prefix}-{kind}-*.csv'):
        with p.open(newline='') as f:
            rows = list(csv.DictReader(f))
        event = 'write' if kind == 'render' else 'packet'
        if any(r['event'] == event for r in rows):
            found.append((int(p.stem.rsplit('-', 1)[1]), p, rows))
    return sorted(found)


def fnv(data):
    h = 14695981039346656037
    for b in data:
        h = ((h ^ b) * 1099511628211) & ((1 << 64)-1)
    return f'{h:016x}'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('prefix')
    parser.add_argument('--count', type=int, default=10)
    args = parser.parse_args()
    assert args.prefix.startswith('fifth-')
    ref, nseg = reference()
    source = ref.astype('<f4').tobytes()
    captures, renders = trace_files(args.prefix, 'capture'), trace_files(args.prefix, 'render')
    assert len(captures) == len(renders) == args.count
    results, hashes = [], {}
    print('| run | pass | lag ch0/ch1 | RMS fit ch0/ch1 | RMS unfitted ch0/ch1 | lost/repeated/zero | verified writes |')
    for run, (ct, rt) in enumerate(zip(captures, renders)):
        y = np.fromfile(HERE / f'{args.prefix}-play_record_7_speaker_set-{run}.f32', dtype='<f4').reshape(-1, 2).astype(np.float64)
        result = waveform(ref, y, nseg)
        writes = [r for r in rt[2] if r['event'] == 'write']
        packets = [r for r in ct[2] if r['event'] == 'packet']
        cursor = 0
        for w in writes:
            detail = dict(kv.split('=', 1) for kv in w['detail'].split())
            start, end = int(w['frame_start']), int(w['frame_end'])
            assert start == cursor and end-start == int(w['frames']) == int(detail['written'])
            assert int(detail['cursor_before']) == start and int(detail['cursor_after']) == end
            assert int(detail['requested']) == end-start and int(detail['bytes']) == (end-start)*8
            assert int(detail['after_us']) >= int(detail['before_us'])
            key = start, end
            if key not in hashes:
                hashes[key] = fnv(source[start*8:end*8])
            assert detail['payload_fnv1a'] == hashes[key], (run, w)
            assert detail['outcome'] == 'release_ok_not_hardware_consumption'
            cursor = end
        assert cursor == len(ref)
        for s in result['splices']:
            s['render_writes'] = [w for w in writes if int(w['frame_start']) < s['source_after'] and int(w['frame_end']) > s['source_before']]
            s['capture_packets'] = [p for p in packets if int(p['frame_start']) <= s['capture_cut'] < int(p['frame_end'])]
        result.update(run=run, capture_trace=str(ct[1]), render_trace=str(rt[1]),
                      verified_writes=len(writes), frames=len(y), packet_count=len(packets),
                      packet_sizes=sorted({int(p['frames']) for p in packets}),
                      buffers=sorted({int(w['buffer_size']) for w in writes}),
                      silent_packets=sum(p['silent'] == 'true' for p in packets),
                      discontinuities=sum(p['discontinuity'] == 'true' and p['initial_discontinuity'] != 'true' for p in packets),
                      index_gaps=[int(b['buffer_index'])-int(a['buffer_index'])-int(a['frames']) for a, b in zip(packets, packets[1:]) if int(b['buffer_index']) != int(a['buffer_index'])+int(a['frames'])],
                      underruns=sum(int(w['padding']) == 0 for w in writes[1:]))
        results.append(result)
        lag = '/'.join(str(m['lag_frames']) for m in result['metrics'])
        fit = '/'.join(f"{m['residual_rms']:.9g}" for m in result['metrics'])
        raw = '/'.join(f'{r:.9g}' for r in result['unfitted_rms'])
        print(f"| {run} | {result['passed']} | {lag} | {fit} | {raw} | {result['missing_frames']}/{result['repeated_frames']}/{result['active_zero_frames']} | {len(writes)} |", flush=True)
    dest = HERE / f'{args.prefix}-analysis.json'
    dest.write_text(json.dumps(results, indent=2), encoding='utf-8')
    print('artifact', dest, 'passes', sum(r['passed'] for r in results), '/', args.count)


if __name__ == '__main__':
    main()
