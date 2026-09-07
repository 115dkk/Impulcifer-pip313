"""Python core/eqapo.py:168-941 oracle exporter; p12_* fixtures, no oracle patches."""
from pathlib import Path
from dataclasses import asdict
import json
import os
import platform
import sys
import tempfile

import numpy as np
import scipy
import soundfile as sf

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from core.eqapo import parse_eqapo_config, looks_like_eqapo_config
from autoeq.frequency_response import FrequencyResponse

OUT = ROOT / 'tests/migration/goldens'
FREQUENCY = FrequencyResponse.generate_frequencies(f_min=10, f_max=24000, f_step=1.01)
WRITTEN = []


def save(name, value):
    """Serialize Python parse_eqapo_config (565-590); p12_* fixture JSON."""
    path = OUT / f'p12_{name}.json'
    data = (json.dumps(value, ensure_ascii=True, allow_nan=False, separators=(',', ':')) + '\n').encode()
    path.write_bytes(data)
    WRITTEN.append(path)


def case(name, text, base=None, texts=None, wavs=None, frequency=FREQUENCY):
    """Run actual Python parse_eqapo_config (565-590); p12_* arrays and reports."""
    result = parse_eqapo_config(text, 48000, frequency, base_dir=base)
    expected = dict(left_db=result.left_db.tolist(), right_db=result.right_db.tolist(),
                    applied_left=result.applied_left, applied_right=result.applied_right,
                    preamp_left=result.preamp_left, preamp_right=result.preamp_right,
                    applied=result.applied, bypassed=[asdict(r) for r in result.bypassed],
                    skipped=[asdict(r) for r in result.skipped], channel_split=bool(result.channel_split))
    save(name, dict(name=name, text=text, fs=48000, frequency=frequency.tolist(),
                    base_dir='/eqapo' if base else None, texts=texts or {}, wavs=wavs or {}, expected=expected))


def corpus():
    """Exercise Python handlers (306-824); p12_biquad_*, p12_condition_*, p12_reports."""
    for kind in ['PK', 'PEQ', 'Modal', 'LP', 'HP', 'LPQ', 'HPQ', 'BP', 'LS', 'HS', 'LSC', 'HSC', 'NO', 'AP']:
        case('biquad_' + kind, f'Filter 1: ON {kind} Fc 1000 Hz Gain -3 dB Q 0.71')
    for kind in ['LP', 'HP', 'BP', 'LS', 'HS', 'LSC', 'HSC', 'NO']:
        case('default_' + kind, f'Filter: ON {kind} Fc 1000 Hz Gain 4 dB')
    for kind in ['LS', 'HS', 'LSC', 'HSC']:
        for slope in [6, 12]:
            case(f'slope_{kind}_{slope}', f'Filter: ON {kind} {slope}dB Fc 1000 Hz Gain 4 dB')
    case('bandwidth', 'Filter: ON PK Fc 1000 Hz Gain 6 dB Q 2 BW Oct 1\nFilter: ON BP Fc 1500 Hz BW Oct 0.5\nFilter: ON LS Fc 1000 Hz Gain 4 dB BW Oct 1')
    case('preamp', 'Preamp: -6.4 dB\nPreamp: +2,5 dB\nPreamp: -.1ignored\nPreamp: -0')
    case('rew_frequency', 'Filter: ON PK Fc 1.250 Hz Gain -4,7 dB Q 0,70\nFilter: ON PK Fc 1 250 Hz Gain 2 dB Q 1\nFilter: ON PK Fc 1.2e3 Hz Gain 1 dB Q 2')
    case('formatting', 'Filter: ON PK Fc 1e6 Hz Gain -0 dB Q 1.23456789\nFilter: ON PK Fc 1000 Hz Gain 0.00001 dB Q 0.9999999\nFilter: ON PK Fc 1000 Hz Gain 999999e-10 dB Q 1e6')
    case('graphic_unsorted', 'GraphicEQ: 1000 3; 20 -5; 100 0; 100 4; 50 2; 999')
    case('graphic_duplicates', 'GraphicEQ: 1000 3; 20 -5; 100 0; 100 4; 0 -2; -1 -7', frequency=np.array([-1., 0., 1e-9, 10., 20., 100., 1000., 24000.]))
    case('graphic_comma', 'GraphicEQ: 20 -5,5; 100 0\nGraphicEQ: 20.0 -5,5; 100 0')
    case('graphic_single', 'GraphicEQ: 20 -2\nGraphicEQ: 0 3')
    case('scopes', 'Channel: L\nPreamp: -3\nChannel: 2\nPreamp: 1\nChannel: c lfe sl 8\nPreamp: -2\nFilter: ON LP Fc 1000 Hz\nGraphicEQ: 20 2; 1000 1\nChannel:\nPreamp: 2\nChannel: 1, r\nFilter: ON PK Fc 1000 Hz Gain -3 dB Q 1\nChannel: all\nPreamp: -1')
    case('reports', '# comment: ignored\n Device : Headphones\nStage: post-mix\nCopy: L=R\nDelay: 2ms\nMultiConvolution: ir\nVSTPlugin: foo\nEval: x\nLoudnessCorrection: 1\nFuture42: yes\nFuture 12: yes\nnot a command: no\n: no\nFilterAny: ON None')
    case('grammar', 'Filter: OFF PK Fc 1000 Hz Gain 3 dB Q 1\nFilter: ON None\nFilter: ON LSQ Fc 1000 Hz Gain 2 dB Q 1\nFilter: ON HSQ Fc 1000 Hz Gain 2 dB Q 1\nFilter: ON PK Fc 1 kHz Gain 2 dB Q 1\nFilter: ON PK Fc -1 Hz Gain 2 dB Q 1\nFilter: ON PK Fc 1e999 Hz Gain 2 dB Q 1\nFilter: ON PK Fc 1000 Hz Gain 2 dB Q -1\nFilter: ON PK Fc 1000 Hz Gain 2 dB\nFilter: ON PK Fc 1000 Hz Q 1\nFilter: ON PK Gain 2 dB Q 1\nFilter: ON AP Fc 1000 Hz\nFilter: ON pk Fc 1000 Hz\nPreamp: loud\nGraphicEQ: xyz')
    case('expressions', 'Filter: ON PK Fc `f` Hz Gain 3 dB Q 1\nPreamp: `a`\nGraphicEQ: `a`\nConvolution: `a`')
    case('iir', 'Filter: ON IIR Order 2 Coefficients 0.25 0.5 0.25 1 -0.4 0.1\nFilter: ON IIR Order 1 Coefficients 0.5 0 1 0')
    case('iir_malformed', 'Filter: ON IIR Order 0 Coefficients 1 1\nFilter: ON IIR Order 2 Coefficients 1 0 0 1 0\nFilter: ON IIR Order 1 Coefficients 0,5 0 1 0\nFilter: ON IIR Order 999999999999999999999999 Coefficients 1 1\nFilter: OFF IIR Order 1 Coefficients 1 0 1 0')
    case('iir_leading', 'Filter: ON IIR Order 1 Coefficients 0.5e 0 1 0\nFilter: ON IIR Order 1 Coefficients 1 0 1 0 trailing')
    for op, rhs in [('==',48000),('!=',48000),('<=',48000),('>=',96000),('<',96000),('>',44100)]:
        case('condition_' + {'==':'eq','!=':'ne','<=':'le','>=':'ge','<':'lt','>':'gt'}[op], f'If: (sampleRate {op} {rhs})\nPreamp: -3\nElse:\nPreamp: 2\nEndIf:')
    case('condition_elseif', 'If: sampleRate == 44100\nPreamp: -1\nElseIf: sampleRate == 48000\nPreamp: -2\nElseIf: unknown\nPreamp: -3\nElse:\nPreamp: -4\nEndIf:')
    case('condition_unknown', 'If: unknown\nIf: sampleRate == 48000\nPreamp: -1\nElseIf: unknown\nPreamp: -2\nElse:\nPreamp: -3\nEndIf:\nElseIf: unknown\nPreamp: -4\nElse:\nPreamp: -5\nEndIf:\nPreamp: -6')
    case('condition_silent', 'If: sampleRate == 44100\nIf: unknown\nPreamp: -1\nElseIf: unknown\nPreamp: -2\nElse:\nPreamp: -3\nEndIf:\nElse:\nIf: sampleRate >= 48000\nPreamp: -4\nEndIf:\nEndIf:')
    case('condition_degrade', 'If: sampleRate == 44100\nPreamp: -1\nElseIf: unknown\nFilter 1: ON None\nnot a command: no\nElse:\nPreamp: -2\nEndIf:\nPreamp: -3')
    case('condition_dangling', 'ElseIf: unknown\nElse:\nEndIf:\nIf: (sampleRate == 48000\nPreamp: -1\nElse:\nElse:\nPreamp: -2\nEndIf:\nIf: sampleRate == 48000)\nPreamp: -3')
    case('unicode_decimal', 'Preamp: ١٢.٥\nPreamp: -٢.٥\nFilter\x1f١: ON None\nFilter: ON LP Fc 1000 Hz Q\x1f1')
    case('zero_defaults', 'Filter: ON LP Fc 1000 Hz Q 0\nFilter: ON NO Fc 1000 Hz Q 0\nFilter: ON LS 0dB Fc 1000 Hz Gain 4 dB\nFilter: ON PK Fc 1000 Hz Gain 3 dB BW Oct 0\nFilter: ON HP Fc 1000 Hz Gain 55 dB')
    case('empty', '')
    case('bom', '﻿Preamp: -10\nPreamp: -1')
    case('line_endings', '# c\r\nPreamp: 1\rPreamp: 2\nPreamp: 3\vPreamp: 4\fPreamp: 5\x1cPreamp: 6\x1dPreamp: 7\x1ePreamp: 8\x85Preamp: 9 Preamp: 10   Delay: 5\r\nFilter: OFF PK\n')
    for name in ['biquad_peaking_1khz', 'iir_order2_lowpass']:
        case('real_' + name, (ROOT / 'tests/fixtures/eqapo' / f'{name}.txt').read_text(encoding='utf-8'))


def files():
    """Use real temporary files for Python Include/Convolution (827-940); p12_files_*."""
    with tempfile.TemporaryDirectory(prefix='impulcifer-p12-') as folder:
        base = Path(folder)
        texts = {'child.txt':'Channel: L\nPreamp: -3\n', 'cycle.txt':'Preamp: -1\nInclude: cycle.txt',
                 'sub/child.txt':'Preamp: -2\nInclude: ../child.txt\n',
                 'open.txt':'EndIf:\nChannel: R\nPreamp: 2\nIf: unknown',
                 'bom.txt':'﻿Preamp: -4', 'environment.txt':'Preamp: -1'}
        for i in range(10):
            texts[f'depth{i}.txt'] = f'Preamp: -1\nInclude: depth{i+1}.txt' if i < 9 else 'Preamp: -1'
        for name, text in texts.items():
            path = base / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text, encoding='utf-8', newline='')
        loader_texts = {'/eqapo/' + name: (base/name).read_bytes().decode('utf-8-sig') for name in texts}
        mono = np.zeros((64, 1)); mono[0, 0]=1; mono[20, 0]=0.5; mono[40, 0]=0.25
        stereo = np.column_stack((mono[:, 0], mono[:, 0] * 0.5))
        wavs = {}
        for name, data, fs in [('mono.wav',mono,48000),('stereo.wav',stereo,48000),('three.wav',np.column_stack((stereo,mono)),48000),('mismatch.wav',mono,44100),('tolerance.wav',mono,48001),('my ir.wav',mono,48000)]:
            sf.write(base/name, data, fs, subtype='DOUBLE')
            samples, rate = sf.read(base/name, dtype='float64', always_2d=True)
            wavs['/eqapo/' + name] = dict(fs=rate, tracks=samples.T.tolist())
        (base/'broken.wav').write_bytes(b'not a wav')
        examples = {
            'include_repeat':'Include: child.txt\nInclude: child.txt\nPreamp: -1',
            'include_cycle':'Include: cycle.txt',
            'include_depth':'Include: depth0.txt',
            'include_scope':'Channel: R\nInclude: sub/child.txt\nPreamp: -1\nChannel: ALL\nPreamp: -2',
            'include_condition_reset':'If: sampleRate == 48000\nInclude: open.txt\nPreamp: -1\nEndIf:\nPreamp: -2',
            'include_bom':'Include: bom.txt',
            'missing':'Include: absent.txt\nInclude:\nConvolution: absent.wav\nConvolution:\nConvolution: broken.wav',
            'convolution_mono':'Convolution: mono.wav',
            'convolution_stereo':'Convolution: stereo.wav',
            'convolution_three':'Convolution: three.wav',
            'convolution_tolerance':'Convolution: tolerance.wav',
            'convolution_mismatch':'Convolution: mismatch.wav',
            'convolution_quotes':'Convolution: "my ir.wav"',
            'convolution_scope':'Channel: L\nConvolution: stereo.wav\nChannel: R\nConvolution: mono.wav',
            'convolution_scope_order':'Channel: C\nConvolution: missing.wav\nConvolution: mismatch.wav\nConvolution: mono.wav',
            'include_no_expansion':'Include: $P12_EQAPO_CHILD',
            'environment':'Convolution: ${P12_EQAPO_IR}',
        }
        old = {key:os.environ.get(key) for key in ['P12_EQAPO_CHILD','P12_EQAPO_IR']}
        os.environ['P12_EQAPO_CHILD']='environment.txt'
        os.environ['P12_EQAPO_IR']='mono.wav'
        try:
            for name, text in examples.items():
                case('files_' + name, text, str(base), loader_texts, wavs)
        finally:
            for key,value in old.items():
                if value is None: os.environ.pop(key,None)
                else: os.environ[key]=value
        case('files_no_base', 'Include: child.txt\nConvolution: mono.wav', texts=loader_texts, wavs=wavs)


def main():
    """Export core/eqapo.py (168-941); p12_manifest and p12_detection."""
    corpus()
    files()
    texts = ['', 'frequency,raw,error\n20,0,1', '20 0\n40 1', '# Preamp: -3', 'Preamp: -3',
             'Filter1: ON None', 'Filter 12: OFF', 'Filter: ON None', 'FilterThing: ON None',
             'filter: ON None', '﻿Preamp: -3', 'Channel: L', 'Device: X', 'ElseIf: x',
             'Future: x', 'GraphicEQ: 20 1', '#x Preamp: -1', 'x: y\rPreamp: -1',
             'Convolution: missing', '  # Filter: ON None', 'Filter  123: OFF', 'Filter ١: OFF']
    save('detection', [dict(text=t,expected=looks_like_eqapo_config(t)) for t in texts])
    names = [p.name for p in WRITTEN]
    errors = []
    for fs, text in [(48000, 'Preamp:\x1f-٢.٥'),
                     (0, 'Filter: ON PK Fc 1000 Hz Gain 3 dB Q 1'),
                     (48000, 'Filter: ON LS 1000dB Fc 1000 Hz Gain 40 dB'),
                     (48000, 'Filter: ON PK Fc 1000 Hz Gain 20000 dB Q 1'),
                     (48000, 'Filter: ON PK Fc 1000 Hz Gain -20000 dB Q 1'),
                     (48000, 'Filter: ON PK Fc 1000 Hz Gain 2 dB BW Oct 100000')]:
        try:
            parse_eqapo_config(text, fs, np.array([100.]))
        except (ValueError, OverflowError, ZeroDivisionError) as error:
            errors.append(dict(fs=fs, text=text, exception=type(error).__name__))
        else:
            raise AssertionError(f'Expected Python scalar exception: {text}')
    save('arithmetic', errors)
    save('manifest', dict(files=names, python=platform.python_version(), numpy=np.__version__, scipy=scipy.__version__, source='core/eqapo.py:168-941'))
    size = sum(p.stat().st_size for p in WRITTEN)
    assert size < 12 * 1024 * 1024
    print(f'P12 export: {len(names)-1} parser cases, {len(texts)} detection cases, {len(WRITTEN)} files, {size} bytes')
    print(f'Python {platform.python_version()}, NumPy {np.__version__}, SciPy {scipy.__version__}; allow_nan=False; oracle unmodified')


if __name__ == '__main__':
    main()
