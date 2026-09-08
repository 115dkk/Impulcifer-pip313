"""Offline, append-only paired diagnostic for the two saved sixth p4 trials.

No audio API is imported. Coordinates are zero-based, half-open stereo frames.
Exact stereo anchors discover offsets; EVERY frame is subsequently checked at
all discovered offsets. Long exact runs establish local mappings, not global
correlation peaks. Source checks use no gain fit and retain all residuals.
"""
import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy.signal import correlate

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]


def runs(mask):
    edges = np.diff(np.r_[False, mask, False].astype(np.int8))
    return list(zip(np.flatnonzero(edges == 1).tolist(), np.flatnonzero(edges == -1).tolist()))


def stats(diff):
    return dict(frames=len(diff), rms=np.sqrt(np.mean(diff * diff, axis=0)).tolist(),
                max_abs=np.max(np.abs(diff), axis=0).tolist(),
                min=np.min(diff, axis=0).tolist(), max=np.max(diff, axis=0).tolist())


def reference():
    mono, fs = sf.read(ROOT / 'data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav')
    assert fs == 48000
    ref = np.zeros((len(mono) * 7, 2))
    for seg in range(7):
        ref[seg * len(mono):(seg + 1) * len(mono), seg % 2] = mono
    return ref, len(mono)


def source_alignment(ref, rust):
    # Initial search is restricted to the first sweep, avoiding identical later
    # sweeps. SSE includes BOTH channels and energy, unlike raw correlation.
    start, size = 48000, 8192
    a, b = ref[start:start + size], rust[:120000].astype(float)
    corr = sum(correlate(b[:, c], a[:, c], mode='valid', method='fft') for c in (0, 1))
    energy = np.r_[0., np.cumsum(np.sum(b * b, axis=1))]
    costs = energy[size:] - energy[:-size] - 2 * corr + np.sum(a * a)
    lag = int(np.argmin(costs)) - start
    assert 0 <= lag and lag + len(ref) <= len(rust)
    aligned = rust[lag:lag + len(ref)].astype(float)
    diff = aligned - ref
    checks = []
    # Adjacent windows cover the entire source, including the final short one.
    # Exhaustive +/-2048 lags detect local sweep/period aliases. Exact RMS is
    # recomputed for the best and runner-up to avoid FFT cancellation artifacts.
    for start in range(0, len(ref), 8192):
        end = min(start + 8192, len(ref))
        a = ref[start:end]
        lo, hi = max(0, start + lag - 2048), min(len(rust), end + lag + 2048)
        b = rust[lo:hi].astype(float)
        size = len(a)
        corr = sum(correlate(b[:, c], a[:, c], mode='valid', method='fft') for c in (0, 1))
        energy = np.r_[0., np.cumsum(np.sum(b * b, axis=1))]
        cost = energy[size:] - energy[:-size] - 2 * corr + np.sum(a * a)
        candidates = np.argsort(cost)[:2]
        fits = []
        for i in candidates:
            d = b[i:i + size] - a
            fits.append(dict(lag=int(lo + i - start), **stats(d)))
        checks.append(dict(start=start, end=end, best=fits[0], runner_up=fits[1]))
    return lag, dict(lag=lag, source_frames=len(ref), **stats(diff), windows=checks,
                     local_best_lags=sorted({w['best']['lag'] for w in checks}),
                     active_zero_regions=runs(np.all(aligned == 0, axis=1) &
                                              (np.max(np.abs(ref), axis=1) > 1e-5)),
                     above_4e_6_regions=runs(np.any(np.abs(diff) > 4e-6, axis=1)),
                     quantized_on_2pow_minus18=bool(np.all(aligned * 2**18 == np.round(aligned * 2**18))))


def analyze(run, ref, nseg):
    prefix = f'sixth-p4-{run:02d}'
    rpath = HERE / f'{prefix}-play_record_7_speaker_set-0.f32'
    ppath = HERE / f'{prefix}-python.f32'
    rb, pb = rpath.read_bytes(), ppath.read_bytes()
    rust = np.frombuffer(rb, dtype='<f4').reshape(-1, 2)
    py = np.frombuffer(pb, dtype='<f4').reshape(-1, 2)
    lag, source = source_alignment(ref, rust)
    offsets, anchors = set(), []
    for start in range(0, len(rust) - 128 + 1, 2048):
        needle = rb[start * 8:(start + 128) * 8]
        if not np.any(rust[start:start + 128]):
            continue
        lo, hi = max(0, start - 65536) * 8, min(len(pb), (start + 100000) * 8)
        matches = []
        pos = pb.find(needle, lo, hi)
        while pos >= 0:
            if pos % 8 == 0:
                matches.append(pos // 8 - start)
            pos = pb.find(needle, pos + 1, hi)
        anchors.append(dict(rust_start=start, offsets=matches))
        offsets.update(matches)
    # Dense reverse anchors catch short islands between multiple deletions;
    # forward 2048-stride anchors alone can skip a 1056-frame surviving island.
    # Search both channels and collect ALL matches in the local neighborhood.
    reverse_anchors = []
    for start in range(0, len(py) - 128 + 1, 128):
        if not np.any(py[start:start + 128]):
            continue
        needle = pb[start * 8:(start + 128) * 8]
        lo, hi = max(0, start - 100000) * 8, min(len(rb), (start + 65536) * 8)
        matches = []
        pos = rb.find(needle, lo, hi)
        while pos >= 0:
            if pos % 8 == 0:
                matches.append(start - pos // 8)
            pos = rb.find(needle, pos + 1, hi)
        if matches:
            offsets.update(matches)
        else:
            reverse_anchors.append(dict(python_start=start, offsets=matches))
    # Retain every mismatch, even for windows that established an offset.
    coverage = np.zeros(len(rust), dtype=bool)
    mappings = []
    raw_offsets = []
    for offset in sorted(offsets):
        lo, hi = max(0, -offset), min(len(rust), len(py) - offset)
        eq = np.all(rust[lo:hi].view('<u4') == py[lo + offset:hi + offset].view('<u4'), axis=1)
        exact = [(lo + a, lo + b) for a, b in runs(eq) if b - a >= 128]
        raw_offsets.append(dict(offset=offset, overlap_frames=hi-lo,
                                all_overlap_residual=stats(rust[lo:hi].astype(float) - py[lo+offset:hi+offset]),
                                exact_runs_ge128=exact))
        for a, b in exact:
            # Entirely silent matches do not independently establish alignment.
            if not np.any(rust[a:b]):
                continue
            coverage[a:b] = True
            mappings.append(dict(rust_start=a, rust_end=b, python_start=a+offset,
                                 python_end=b+offset, offset=offset,
                                 source_start=a-lag, source_end=b-lag))
    mappings.sort(key=lambda m: (m['rust_start'], m['rust_end']))
    missing = runs(~coverage[lag:lag+len(ref)])
    gaps = []
    for a, b in missing:
        before = [m for m in mappings if m['rust_end'] == a+lag]
        after = [m for m in mappings if m['rust_start'] == b+lag]
        gaps.append(dict(source_start=a, source_end=b, frames=b-a,
                         rust_start=a+lag, rust_end=b+lag, before=before, after=after,
                         rust_vs_source=stats(rust[a+lag:b+lag].astype(float)-ref[a:b]),
                         channels_with_nonzero_source=np.any(ref[a:b] != 0, axis=0).tolist()))
    pcoverage = np.zeros(len(py), dtype=bool)
    for m in mappings:
        pcoverage[m['python_start']:m['python_end']] = True
    pactive = np.any(py != 0, axis=1)
    ractive = np.any(rust != 0, axis=1)
    # Validate a complete ordered deletion-only explanation; no unmatched
    # nonzero observer sample, hidden island, overlap, or repeat is permitted.
    chain_ok = bool(mappings and mappings[0]['rust_start'] == 0 and
                    mappings[-1]['rust_end'] == len(rust) and
                    all(a['python_end'] == b['python_start'] and
                        a['rust_end'] < b['rust_start'] for a, b in zip(mappings, mappings[1:])) and
                    not np.any(~pcoverage & pactive))
    # Identical sweeps are intentional in this source. Use their same-channel
    # counterparts ONLY as a quantization/content check, never as a temporal
    # alignment anchor or a substitute contemporaneous observation.
    counterpart_checks = []
    aligned = rust[lag:lag+len(ref)]
    for gap in gaps:
        a, b = gap['source_start'], gap['source_end']
        seg, local = divmod(a, nseg)
        alternatives = []
        for other in range(seg % 2, 7, 2):
            aa, bb = other*nseg+local, other*nseg+local+b-a
            if other == seg or bb > (other+1)*nseg:
                continue
            if np.all(coverage[aa+lag:bb+lag]) and np.array_equal(ref[a:b], ref[aa:bb]):
                alternatives.append(dict(source_start=aa, source_end=bb,
                                         both_channels_bit_exact=bool(np.array_equal(
                                             aligned[a:b].view('<u4'), aligned[aa:bb].view('<u4')))))
        counterpart_checks.append(dict(source_start=a, source_end=b, alternatives=alternatives))
    counterpart_ok = all(any(a['both_channels_bit_exact'] for a in c['alternatives'])
                         for c in counterpart_checks)
    boundaries = []
    for gap in gaps:
        # Compare adjacent source windows with +/-1 frame alternatives. Correct
        # matching is required on both sides, not just at one sweep phase.
        for a, b in ((gap['source_start']-1024, gap['source_start']),
                     (gap['source_end'], gap['source_end']+1024)):
            fits = []
            for shift in (-1, 0, 1):
                fits.append(dict(shift=shift, **stats(rust[a+lag+shift:b+lag+shift].astype(float)-ref[a:b])))
            boundaries.append(dict(source_start=a, source_end=b, fits=fits))
    historical = json.loads((HERE / f'{prefix}-analysis-v2.json').read_text())
    index = json.loads((HERE / 'sixth-validation-index.json').read_text())
    prior = next(v for v in index if v['period'] == 4 and v['run'] == run)
    hashes = {p.name: hashlib.sha256(data).hexdigest() for p, data in ((rpath, rb), (ppath, pb))}
    assert hashes == prior['sha256'], 'Historical capture hash changed'
    result = dict(trial=prefix, coordinates='zero-based half-open stereo frames',
                  sha256=hashes, segment_frames=nseg, rust_frames=len(rust), python_frames=len(py),
                  source=source, historical_trace=historical['trace'],
                  observer=json.loads((HERE / f'{prefix}-observer.json').read_text()),
                  anchor_search=dict(window_frames=128, stride=2048, radius_before=65536,
                                     radius_after=100000, anchors=anchors),
                  raw_offset_checks=raw_offsets, exact_piecewise_mappings=mappings,
                  exact_local_rust_coverage_frames=int(coverage.sum()),
                  exact_local_source_coverage_frames=int(coverage[lag:lag+len(ref)].sum()),
                  unmatched_rust_regions=runs(~coverage),
                  unmatched_rust_nonzero_frames=int(np.sum(~coverage & ractive)),
                  unmatched_python_nonzero_regions=runs(~pcoverage & pactive),
                  python_unmapped_regions=runs(~pcoverage),
                  missing_observer_candidates=gaps,
                  reverse_anchor_unmatched_windows=reverse_anchors,
                  ordered_deletion_only_chain_verified=chain_ok,
                  counterpart_quantization_checks=counterpart_checks,
                  counterpart_quantization_verified=counterpart_ok,
                  adjacent_source_alias_checks=boundaries,
                  pair_residual_on_all_mapped_frames=dict(rms=[0.0, 0.0], max_abs=[0.0, 0.0]),
                  observed_signal_counts=dict(observer_only_missing_frames=sum(b-a for a,b in missing),
                                              rust_only_missing_frames=0 if chain_ok and not source['above_4e_6_regions'] else None,
                                              shared_missing_frames=0 if chain_ok and not source['above_4e_6_regions'] else None),
                  accepted=False, internal_frames=None, external_frames=None,
                  note='Complete saved-signal deletion attribution is separate from strict contemporaneous '
                       'internal/external proof. Observer gaps have source and same-channel repeated-sweep '
                       'content corroboration, but no contemporaneous independent samples. Thus no strict '
                       'paired pass and no unqualified causal internal/external counts are assigned.')
    assert chain_ok and counterpart_ok, 'Unresolved paired mapping or counterpart content'
    assert source['local_best_lags'] == [lag], 'Local source alignment differs'
    assert not source['above_4e_6_regions'] and not source['active_zero_regions']
    assert sum(w['end']-w['start'] for w in source['windows']) == len(ref)
    assert sum(m['rust_end']-m['rust_start'] for m in mappings) + sum(b-a for a,b in missing) == len(rust)
    assert all(sum(f['rms'][c]**2 for c in (0, 1)) >
               sum(w['fits'][1]['rms'][c]**2 for c in (0, 1))
               for w in boundaries for f in (w['fits'][0], w['fits'][2]))
    result['validation_assertions_passed'] = True
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', default='sixth-followup-local.json')
    args = parser.parse_args()
    dest = HERE / args.output
    assert dest.parent == HERE and dest.name.startswith('sixth-followup') and dest.suffix == '.json'
    assert not dest.exists(), 'Refusing to overwrite historical or follow-up evidence'
    ref, nseg = reference()
    results = [analyze(run, ref, nseg) for run in (2, 7)]
    with dest.open('x', encoding='utf-8') as f:
        json.dump(results, f, indent=2, allow_nan=False)
    for r in results:
        print(r['trial'], 'source', {k:v for k,v in r['source'].items() if k != 'windows'})
        print('local coverage', r['exact_local_rust_coverage_frames'], '/', r['rust_frames'],
              'source', r['exact_local_source_coverage_frames'], '/', len(ref))
        print('mappings', json.dumps(r['exact_piecewise_mappings']))
        print('gaps', json.dumps(r['missing_observer_candidates']))
        print('unmatched observer nonzero', r['unmatched_python_nonzero_regions'])
    print('artifact', dest)


if __name__ == '__main__':
    main()
