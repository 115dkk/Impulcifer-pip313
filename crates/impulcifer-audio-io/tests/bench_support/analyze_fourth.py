"""Offline PA05 artifact analysis; never opens an audio device."""
import csv
import json
from pathlib import Path
import sys

import numpy as np
import soundfile as sf
from scipy.signal import correlate

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / 'tests/migration'))
from bench_oracle_impulcifer_audio_io import integrity  # noqa: E402

HERE = Path(__file__).resolve().parent
PREFIX = 'fourth-astra-integrity'


def runs(mask):
    edges = np.diff(np.r_[False, mask, False].astype(np.int8))
    return list(zip(np.flatnonzero(edges == 1).tolist(), np.flatnonzero(edges == -1).tolist()))


def traces(kind):
    result = []
    for path in HERE.glob(f'{PREFIX}-{kind}-*.csv'):
        rows = list(csv.DictReader(path.open(newline='')))
        if any(r['event'] == ('packet' if kind == 'capture' else 'write') for r in rows):
            pid, seq = map(int, path.stem.rsplit('-', 2)[-2:])
            result.append((pid, seq, path, rows))
    result.sort(key=lambda item: item[:2])
    assert len(result) == 10 and len({v[0] for v in result}) == 1
    return result


def local_offset(ref, captured, start, end, ch, expected, radius=2048):
    lo = max(0, start + expected - radius)
    hi = min(len(captured), end + expected + radius)
    a = ref[start:end, ch]
    b = captured[lo:hi, ch]
    corr = correlate(b, a, mode='valid', method='fft')
    energy = np.r_[0.0, np.cumsum(b*b)]
    squared_error = energy[len(a):] - energy[:-len(a)] - 2*corr + np.dot(a, a)
    lag = lo + int(np.argmin(squared_error)) - start
    actual = captured[start+lag:end+lag, ch]
    return lag, float(np.sqrt(np.mean((actual-a)**2)))


def main():
    mono, fs = sf.read(ROOT / 'data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav')
    ref = np.zeros((len(mono)*7, 2))
    for seg in range(7):
        ref[seg*len(mono):(seg+1)*len(mono), seg % 2] = mono
    captures, renders = traces('capture'), traces('render')
    result = []
    print('| run | trace capture/render seq | lag ch0/ch1 | overlap ch0/ch1 | gain ch0/ch1 | RMS fit ch0/ch1 | active both-zero frames | changed window lags | packet index gaps | render underruns |')
    for run, (ct, rt) in enumerate(zip(captures, renders)):
        y = np.fromfile(HERE / f'{PREFIX}-play_record_7_speaker_set-{run}.f32', dtype='<f4').reshape(-1, 2).astype(np.float64)
        assert len(y) == 2078890
        metrics = integrity(ref, y)
        windows = []
        for seg in range(7):
            # Each search uses the actual reference channel, never the quiet channel.
            expected = metrics[seg % 2]['lag_frames']
            for start in range(seg*len(mono), (seg+1)*len(mono), 24000):
                end = min(start+24000, (seg+1)*len(mono))
                if end-start < 4096:
                    continue
                lag, rms = local_offset(ref, y, start, end, seg % 2, expected)
                windows.append({'reference_start': start, 'reference_end': end, 'channel': seg % 2,
                                'lag': lag, 'unfitted_rms': rms})
        packet_rows = [r for r in ct[3] if r['event'] == 'packet']
        deliveries = [r for r in ct[3] if r['event'] == 'delivery']
        writes = [r for r in rt[3] if r['event'] == 'write']
        raw_zero_runs = runs(np.all(y == 0, axis=1))
        zeros = []
        for start, end in raw_zero_runs:
            center = (start+end)//2
            window = min(windows, key=lambda w: abs(center-(w['reference_start']+w['reference_end'])/2-w['lag']))
            lag = window['lag']
            lo, hi = max(start, lag), min(end, len(ref)+lag)
            if hi <= lo:
                continue
            # Zero reference samples (including alternating silent channel) are not dropouts.
            active = np.max(np.abs(ref[lo-lag:hi-lag]), axis=1) > 1e-5
            for a, b in runs(active):
                zstart, zend = lo+a, lo+b
                packets = [p for p in packet_rows if int(p['frame_start']) < zend and int(p['frame_end']) > zstart]
                zeros.append({'capture_start': zstart, 'capture_end': zend, 'local_lag': lag,
                              'reference_start': zstart-lag, 'reference_end': zend-lag,
                              'packets': packets})
        index_gaps = []
        for a, b in zip(packet_rows, packet_rows[1:]):
            gap = int(b['buffer_index']) - int(a['buffer_index']) - int(a['frames'])
            if gap:
                index_gaps.append({'before': a['ordinal'], 'after': b['ordinal'], 'gap': gap})
        for a, b in zip(deliveries, deliveries[1:]):
            assert a['delivery_end'] == b['delivery_start']
        for a, b in zip(writes, writes[1:]):
            assert a['frame_end'] == b['frame_start']
        assert int(deliveries[-1]['delivery_end']) == len(y)
        assert sum(int(w['frames']) for w in writes) == len(ref)
        first, last = packet_rows[0], packet_rows[-1]
        device_hz = (int(last['buffer_index'])-int(first['buffer_index'])) / ((int(last['buffer_timestamp'])-int(first['buffer_timestamp'])) / 1e7)
        render_hz = (int(writes[-1]['frame_end'])-int(writes[0]['frame_end'])) / ((int(writes[-1]['elapsed_us'])-int(writes[0]['elapsed_us'])) / 1e6)
        unfitted = []
        for ch, m in enumerate(metrics):
            lag = m['lag_frames']
            a, b = max(0, -lag), max(0, lag)
            n = m['overlap_frames']
            unfitted.append(float(np.sqrt(np.mean((y[b:b+n, ch]-ref[a:a+n, ch])**2))))
        # Refine single first-segment clock slips found by the window search.
        # This is diagnostic segmentation, never a replacement for full-overlap RMS.
        splices = []
        global_lag = metrics[0]['lag_frames']
        initial_lag = windows[0]['lag']
        delta = initial_lag - global_lag
        if delta > 0:
            lo, hi = initial_lag, len(mono) + global_lag
            indices = np.arange(lo, hi)
            early = (y[indices, 0] - ref[indices-initial_lag, 0])**2
            late = (y[indices, 0] - ref[indices-global_lag, 0])**2
            cost = np.r_[0.0, np.cumsum(early)] + np.r_[np.cumsum(late[::-1])[::-1], 0.0]
            cut = lo + int(np.argmin(cost))
            before, after = cut-initial_lag, cut-global_lag
            splices.append({'capture_cut': cut, 'missing_reference_start': before,
                            'missing_reference_end': after, 'missing_frames': delta,
                            'piecewise_unfitted_rms': float(np.sqrt(cost.min()/len(indices))),
                            'capture_packets': [p for p in packet_rows
                                                if int(p['frame_start']) < cut+1 and int(p['frame_end']) > cut-1],
                            'render_writes': [w for w in writes
                                              if int(w['frame_start']) < after and int(w['frame_end']) > before]})
        entry = {'run': run, 'missing_frame_splices': splices, 'capture_trace': str(ct[2]), 'render_trace': str(rt[2]), 'metrics': metrics,
                 'unfitted_rms': unfitted, 'windows': windows, 'internal_zero_runs': zeros,
                 'active_both_zero_frames': sum(z['capture_end']-z['capture_start'] for z in zeros),
                 'index_gaps': index_gaps, 'device_capture_hz': device_hz,
                 'render_submission_hz_approx': render_hz, 'packet_count': len(packet_rows),
                 'packet_sizes': sorted({int(p['frames']) for p in packet_rows}),
                 'raw_zero_total_including_leading_tail_reference': sum(int(p['raw_zero_frames']) for p in packet_rows),
                 'silent_fill_frames': sum(int(p['silent_fill_frames']) for p in packet_rows),
                 'discontinuity_after_initial': sum(p['discontinuity']=='true' and p['initial_discontinuity']!='true' for p in packet_rows),
                 'queue_max_samples': max(int(p['queue_after']) for p in packet_rows),
                 'render_buffer': sorted({int(w['buffer_size']) for w in writes}),
                 'render_underruns': sum(int(w['padding']) == 0 for w in writes[1:]),
                 'render_timeouts': sum(r['event']=='wait' and r['detail']=='timeout' for r in rt[3])}
        result.append(entry)
        changes = sorted({w['lag'] for w in windows})
        gains = '/'.join(format(m['gain'], '.9f') for m in metrics)
        residuals = '/'.join(format(m['residual_rms'], '.9g') for m in metrics)
        print(f"| {run} | {ct[1]}/{rt[1]} | {'/'.join(str(m['lag_frames']) for m in metrics)} | {'/'.join(str(m['overlap_frames']) for m in metrics)} | {gains} | {residuals} | {entry['active_both_zero_frames']} | {changes} | {index_gaps} | {entry['render_underruns']} |")
    destination = HERE / 'fourth-astra-analysis.json'
    destination.write_text(json.dumps(result, indent=2), encoding='utf-8')
    print('Detailed zero-run packet mapping, all windows and clocks:', destination)


if __name__ == '__main__':
    main()
