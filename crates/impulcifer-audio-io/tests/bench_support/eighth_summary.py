"""Summarize PA07 evidence without replaying, filtering, or replacing trials."""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
OPS = ['enumerate_devices', 'open_close_session', 'play_record_headphones_sweep',
       'play_record_headphones_sweep_overhead', 'play_record_7_speaker_set',
       'play_record_7_speaker_set_overhead', 'first_sample_latency', 'capture_loop_cpu']


def table(name):
    data = {}
    for line in (HERE / f'eighth-{name}.log').read_text(encoding='utf-8').splitlines():
        if not line.startswith('|'):
            continue
        fields = [p.strip() for p in line.split('|')[1:-1]]
        try:
            data[fields[0].replace('enumerate_backend', 'enumerate_devices')] = float(fields[2])
        except ValueError:
            pass
    return data


def main():
    rust = table('rust-observed')
    print('RATIOS')
    for mode in ('explicit', 'production'):
        normal, ft = table(f'python-{mode}'), table(f'python-ft-{mode}')
        print(mode)
        for op in OPS:
            print(f'| {op} | {normal[op] / rust[op]:.6f} / {ft[op] / rust[op]:.6f} |')
    evidence = []
    for period, first in ((2, 300), (3, 310), (4, 320)):
        for suffix in ('headphones', 'seven'):
            group = []
            for run in range(first, first + 10):
                path = HERE / f'sixth-p{period}-{run}-{suffix}-analysis.json'
                a = json.loads(path.read_text(encoding='utf-8'))
                t = a['trace']
                entry = dict(period=period, run=run, op=suffix,
                             buffers=t['render_buffer_frames'], underruns=t['underruns'],
                             discontinuities=t['post_initial_discontinuities'],
                             gaps=t['index_gaps'], silent=t['silent_packets'],
                             strict_clean=bool(a.get('strict_clean', False)),
                             conclusive=a['conclusive'], accepted=a['accepted'],
                             observer_statuses=a['observer']['statuses'],
                             python_source=a.get('python_source'), rust_source=a.get('rust_source'),
                             external_regions=a.get('external_regions'),
                             python_only_regions=a.get('python_only_regions'),
                             unclassified_rust_regions=a.get('unclassified_rust_regions'),
                             analysis_error=a.get('analysis_error'),
                             pair_bit_exact=a['pair_bit_exact'])
                group.append(entry)
            evidence.extend(group)
            print('PERIOD', period, suffix,
                  'U', sum(x['underruns'] for x in group),
                  'D', sum(x['discontinuities'] for x in group),
                  'G', sum(sum(x['gaps']) for x in group),
                  'S', sum(x['silent'] for x in group),
                  'PASS', sum(x['strict_clean'] for x in group),
                  'SOURCE_OBSERVER', sum(bool(x['python_source'] and x['python_source']['passed']) for x in group),
                  'STATUS_NONEMPTY', sum(bool(x['observer_statuses']) for x in group))
            for x in group:
                print(f"| {period} | {suffix} {x['run']} | {x['underruns']} | {x['discontinuities']} | {sum(x['gaps'])} | {x['strict_clean']} | source={x['python_source']} conclusive={x['conclusive']} external={x['external_regions']} python_only={x['python_only_regions']} unclassified={x['unclassified_rust_regions']} error={x['analysis_error']}")
    with (HERE / 'eighth-evidence.json').open('x', encoding='utf-8') as f:
        json.dump(evidence, f, indent=2)


if __name__ == '__main__':
    main()
