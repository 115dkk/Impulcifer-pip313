"""Read all retained PA05b results; never replace failed trials."""
import csv
import json
from pathlib import Path
import statistics

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]


def table(path):
    rows = {}
    for line in path.read_text(encoding='utf-8').splitlines():
        if line.startswith('|'):
            parts = [p.strip() for p in line.split('|')[1:-1]]
            if len(parts) == 4:
                try:
                    rows[parts[0]] = (float(parts[2]), float(parts[3]))
                except ValueError:
                    pass
    return rows


def strict(d):
    # The legacy waveform.passed fits gain and has a stereo-sparse RMS
    # threshold inappropriate for the duplicated headphones signal. The
    # PA05b check instead uses the full unfitted <=4e-6 bound on BOTH channels.
    return bool(d.get('accepted') and d.get('external_frames') == 0
                and d.get('rust_source', {}).get('passed')
                and d.get('python_source', {}).get('passed')
                and not d.get('unclassified_rust_regions')
                and not d.get('python_only_regions')
                and not any(m['repeated_128_blocks'] for w in ('rust', 'python')
                            for m in d[w]['metrics']))


def main():
    trials = []
    stages = {}
    print('| period | run | waveform | underruns | discontinuities | silent | gap frames | observer statuses | strict | shared frames | conclusive |')
    for period in (2, 3, 4):
        for waveform, numbers in (('headphones', range(150, 160)), ('seven', range(160, 170))):
            for run in numbers:
                trials.append((period, run, waveform))
    trials += [(4, run, waveform) for waveform in ('headphones', 'seven') for run in range(200, 210)]
    results = []
    for period, run, waveform in trials:
        prefix = f'sixth-p{period}-{run}-{waveform}'
        d = json.loads((HERE / f'{prefix}-analysis.json').read_text(encoding='utf-8'))
        t = d['trace']
        row = dict(period=period, run=run, waveform=waveform, strict=strict(d),
                   underruns=t['underruns'], discontinuities=t['post_initial_discontinuities'],
                   silent=t['silent_packets'], gap_frames=sum(abs(g) for g in t['index_gaps']),
                   observer_statuses=d['observer']['statuses'], shared=d['external_frames'],
                   conclusive=d['conclusive'], analysis_error=d.get('analysis_error'),
                   source=d.get('rust_source'), observer_source=d.get('python_source'))
        print(f"| {period} | {run} | {waveform} | {row['underruns']} | {row['discontinuities']} | {row['silent']} | {row['gap_frames']} | {row['observer_statuses']} | {row['strict']} | {row['shared']} | {row['conclusive']} |")
        for direction in ('render', 'capture'):
            with Path(t[f'{direction}_trace']).open(newline='') as file:
                events = list(csv.DictReader(file))
            promotion = [e for e in events if e['event'] == 'mmcss_promote']
            demotion = [e for e in events if e['event'] == 'mmcss_demote']
            assert len(promotion) == len(demotion) == 1
            assert promotion[0]['detail'] == demotion[0]['detail'] == 'ok'
            for event in events:
                if event['category'] == 'open':
                    stages.setdefault(event['event'], []).append(int(event['duration_us']))
        results.append(row)
    print('\nGROUPS')
    for period in (2, 3, 4):
        for waveform in ('headphones', 'seven'):
            group = [r for r in results if r['period'] == period and r['waveform'] == waveform and r['run'] < 200]
            print(period, waveform, sum(r['strict'] for r in group), '/', len(group),
                  'underruns', sum(r['underruns'] for r in group))
    for waveform in ('headphones', 'seven'):
        group = [r for r in results if r['waveform'] == waveform and r['run'] >= 200]
        print('final', waveform, sum(r['strict'] for r in group), '/', len(group))
    print('MMCSS 160 promoted sessions / 160 successful same-thread demotions')
    print('OPEN STAGE MEDIAN MICROSECONDS', {k: statistics.median(v) for k, v in stages.items()})
    rust = table(HERE / 'seventh-rust-observed.log')
    ratios = {}
    for name in ('python-explicit', 'python-ft-explicit', 'python-production', 'python-ft-production'):
        rows = table(HERE / f'seventh-{name}.log')
        ratios[name] = {key: value[0] / rust['enumerate_backend' if key == 'enumerate_devices' else key][0]
                        for key, value in rows.items()}
        print('\nRATIOS', name)
        for key, ratio in ratios[name].items():
            print(f'| {key} | {ratio:.6f} |')
    print('\nRAW TABLES')
    for name in ('rust-standalone', 'rust-observed', 'sys-bench', 'python-explicit',
                 'python-ft-explicit', 'python-production', 'python-ft-production'):
        print('\n', name)
        for line in (HERE / f'seventh-{name}.log').read_text(encoding='utf-8').splitlines():
            if line.startswith('|') or line.startswith('EXIT_CODE='):
                print(line)
    destination = HERE / 'seventh-evidence.json'
    if destination.exists():
        raise FileExistsError(destination)
    destination.write_text(json.dumps(dict(trials=results, ratios=ratios,
                                          stage_median_us={k: statistics.median(v) for k, v in stages.items()}),
                                     indent=2), encoding='utf-8')


if __name__ == '__main__':
    main()
