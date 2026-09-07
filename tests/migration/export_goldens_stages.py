"""P10 oracle. Execute real 2.x DSP in foreground on temporary demo copies only."""
from pathlib import Path
from collections import Counter
import contextlib
import hashlib
import json
import os
import re
import shutil
import sys
import tempfile
from unittest.mock import patch, MagicMock

import numpy as np
import soundfile as sf

import export_goldens_fr as common
from export_goldens import plain
from export_goldens_brir import summary

ROOT = common.ROOT
OUT = common.OUT
sys.path.insert(0, str(ROOT))
from core import pipeline_stages as stages, room_correction as room
from core.constants import SPEAKER_NAMES, IPSILATERAL_PAIRS, HEXADECAGONAL_TRACK_ORDER, HESUVI_TRACK_ORDER, speaker_side
from core.hrir import HRIR
from core.impulse_response import ImpulseResponse
from core.impulse_response_estimator import ImpulseResponseEstimator
from core.pipeline import ProcessingConfig, BRIRPipeline
from core.parallel_workers import process_equalization_worker
from core.virtual_bass import apply_virtual_bass_to_hrir
from core.microphone_deviation_correction import MicrophoneMatchingCorrector, apply_microphone_deviation_correction_to_hrir
from core.eqapo import looks_like_eqapo_config
from autoeq.frequency_response import FrequencyResponse as FR

WRITTEN = {}
BLOBS = {}


def write(path, data):
    """P10 fixture encoding (Python sources recorded by save); p10_* fixtures."""
    assert path.name.startswith('p10_')
    if not path.exists() or path.read_bytes() != data:
        path.write_bytes(data)
    WRITTEN[path.name] = len(data)


def encode(name, value):
    """P05/P09 f64 encoding; core/pipeline.py:424-972; all p10 fixtures."""
    if isinstance(value, np.ndarray):
        value = np.asarray(value, dtype='<f8')
        if value.size > 4096:
            data = value.tobytes()
            digest = hashlib.sha256(data).hexdigest()
            if digest not in BLOBS:
                file = f'p10_{name}.f64'
                write(OUT / file, data)
                BLOBS[digest] = file
            return {**plain(summary(value)), 'file': BLOBS[digest]}
        return plain(value)
    if isinstance(value, dict):
        return {k: encode(f'{name}_{k}', v) for k, v in value.items()}
    if isinstance(value, (tuple, list)):
        return [encode(f'{name}_{i}', v) for i, v in enumerate(value)]
    return plain(value)


def save(name, inputs, outputs, source='core/pipeline.py:424-972'):
    """Save oracle outputs and provenance; core/pipeline.py:424-972; p10_* fixtures."""
    data = json.dumps(encode(name, dict(inputs=inputs, outputs=outputs, meta={**common.META, 'source': source})), separators=(',', ':'), allow_nan=False).encode() + b'\n'
    assert len(data) < 200000, (name, len(data))
    write(OUT / f'p10_{name}.json', data)
    return f'p10_{name}.json'


def snapshot(name, hrir, full=()):
    """Actual per-ear state; core/hrir.py:368-938; p10_*_tracks."""
    files = []
    for speaker, pair in hrir.irs.items():
        for side, ir in pair.items():
            result = dict(summary(ir.data), speaker=speaker, side=side, peak_index=ir.peak_index())
            if speaker in full:
                result['samples'] = ir.data
            files.append(save(f'{name}_{speaker}_{side}', {}, result))
    return files


def room_state(name, frs):
    """Room FR errors and targets; core/room_correction.py:185-292; p10_room."""
    return [save(f'{name}_{speaker}_{side}', {}, dict(speaker=speaker, side=side, frequency=fr.frequency, error=fr.error, target=fr.target)) for speaker, pair in (frs or {}).items() for side, fr in pair.items()]


def table(config):
    """Actual 26-row table; core/pipeline.py:424-470; p10_stage_table."""
    rows = [[method.__name__.removeprefix('_stage_'), bool(enabled), steps] for enabled, steps, method in BRIRPipeline(config)._stage_table()]
    return dict(rows=rows, total_steps=sum(n for _, enabled, n in rows if enabled))


def readme_numbers(hrir, applied_gain=0.0, fs=None):
    """Capture write_readme locals, not a second formula; pipeline_stages.py:525-687; p10_readme."""
    rows = []
    seen = set()
    captured = {}
    def trace(frame, event, arg):
        """Trace actual numeric locals; pipeline_stages.py:631,687; p10_readme."""
        if frame.f_code is stages.write_readme.__code__:
            if event == 'line' and frame.f_lineno == 631:
                v = frame.f_locals
                key = (v['speaker'], v['side'])
                if key not in seen:
                    seen.add(key)
                    rows.append(dict(speaker=v['speaker'], side=v['side'], pnr_db=v['pnr_val'], itd_us=v['current_itd'], length_ms=v['length_ms'], reverb=v['current_ir_rt_name'], reverb_ms=v['rt_val_ms']))
            if event == 'return':
                captured['reverb_header'] = frame.f_locals['final_rt_name']
        return trace
    with tempfile.TemporaryDirectory(prefix='impulcifer-p10-readme-') as tmp:
        sys.settrace(trace)
        try:
            stages.write_readme(str(Path(tmp)/'README.md'), hrir, fs, hrir.estimator, applied_gain)
        finally:
            sys.settrace(None)
    return dict(rows=rows, **captured, reflections=hrir.calculate_reflection_levels(), fs=hrir.fs if fs is None else fs, applied_gain=applied_gain)


def crop(hrir):
    """Actual crop/align sequence; core/pipeline.py:593-609; p10_default_crop."""
    hrir.crop_heads(1.0)
    hrir.align_ipsilateral_all(list(IPSILATERAL_PAIRS), segment_ms=30)
    hrir.align_onset_groups_peak_leftref()
    hrir.crop_tails()


def eq_firs(hrir, frs, hp, target):
    """Call actual worker serially; core/parallel_workers.py:69-131; p10_eq_firs."""
    return [(s, side, process_equalization_worker((s, side, frs, hp[0], hp[1], None, None, target, target.frequency, hrir.fs))[2]) for s, pair in hrir.irs.items() for side in pair]


def apply_eq(hrir, firs):
    """Actual full convolutions; core/pipeline.py:698-699; p10_default_equalize."""
    for s, side, fir in firs:
        hrir.irs[s][side].equalize(fir)


def normalize(hrir, target=None):
    """Actual normalize; core/pipeline.py:733-743; p10_default_normalize."""
    return hrir.normalize(peak_target=-0.1 if target is None else None, avg_target=target)


def final(name, hrir, full=()):
    """Actual write_wav routing captured before I/O; core/hrir.py:427-474; p10_default_final."""
    layouts = {}
    for label, order in [('hrir', HEXADECAGONAL_TRACK_ORDER), ('hesuvi', HESUVI_TRACK_ORDER)]:
        captured = []
        with patch('core.hrir.write_wav', lambda path, fs, data, **kw: captured.append(np.array(data))):
            hrir.write_wav('unused.wav', track_order=order, trim_extensions=True)
        tracks = captured[0]
        layouts[label] = [save(f'{name}_{label}_{i}', {}, dict(name=order[i], **summary(x))) for i, x in enumerate(tracks)]
    return dict(tracks=snapshot(name, hrir, full), layouts=layouts)


def resolution_cases(tmp):
    """Execute Python file resolution on real paths; pipeline_stages.py:369-418; p10_headphone_resolution."""
    root = tmp/'resolution'
    root.mkdir()
    external = tmp/'external'
    external.mkdir()
    cases = []
    def run(label, request, files, dirs=()):
        """Resolve with Python and convert cwd/root context; pipeline_stages.py:369-418; p10_headphone_resolution."""
        case = root/label
        case.mkdir()
        for d in dirs:
            (case/d).mkdir(parents=True, exist_ok=True)
        for f in files:
            path = case/f
            path.parent.mkdir(parents=True, exist_ok=True)
            path.touch()
        ext = external/f'{label}.wav'
        if request == '@external':
            ext.touch()
            requested = str(ext)
        else:
            requested = request
        found = []
        class StopResolution(Exception):
            pass
        class Capture:
            def __init__(self, estimator):
                """Intercept only post-resolution ingestion; pipeline_stages.py:421; p10_headphone_resolution."""
            def open_recording(self, path, **kwargs):
                """Record selected real path; pipeline_stages.py:421; p10_headphone_resolution."""
                found.append(path)
                raise StopResolution
        old = os.getcwd()
        os.chdir(case)
        try:
            with patch.object(stages, 'HRIR', Capture):
                try:
                    stages.headphone_compensation(None, str(case), requested)
                except StopResolution:
                    pass
        finally:
            os.chdir(old)
        def virtual(path):
            """Normalize caller snapshot; pipeline_stages.py:370-405; p10_headphone_resolution."""
            p = Path(path)
            if not p.is_absolute():
                return p.as_posix()
            if p.is_relative_to(case):
                return p.relative_to(case).as_posix()
            return '/external/'+p.name
        listing = [d.rstrip('/')+'/' for d in dirs]+list(files)
        if request == '@external':
            listing.append(virtual(ext))
        cases.append(dict(requested=virtual(requested) if requested is not None else None, listing=listing, result=virtual(found[0]) if found else None))
    run('default', None, ['headphones.wav'])
    run('none_missing', None, [])
    run('explicit', 'custom.wav', ['custom.wav', 'headphones.wav'])
    run('relative', './nested/../nested/custom.wav', ['nested/custom.wav'])
    run('missing', 'missing.wav', ['headphones.wav'])
    run('missing_all', 'missing.wav', [])
    run('directory_named', 'hpdir', ['hpdir/hp.wav', 'hpdir/headphone.wav', 'headphones.wav'], ['hpdir'])
    run('directory_first', 'hpdir', ['hpdir/z.WAV', 'hpdir/a.wav'], ['hpdir'])
    # Preserve the actual filesystem's enumeration, not creation order.
    first = cases[-1]
    first['listing'] = ['hpdir/'] + ['hpdir/'+p for p in os.listdir(root/'directory_first'/'hpdir')]
    run('empty', 'hpdir', ['headphones.wav'], ['hpdir'])
    run('external', '@external', [])
    save('headphone_resolution', {}, cases, 'core/pipeline_stages.py:369-418')


def eq_cases(tmp, estimator):
    """Execute CSV parsing and precedence; pipeline_stages.py:183-350; p10_eq_files."""
    cases = []
    for label, text in [('gain', 'frequency,raw\n20.0,3.0\n1000.0,0.0\n24000.0,-2.0\n'), ('error', 'frequency,raw,error\n20.0,3.0,-1.0\n1000.0,0.0,0.5\n24000.0,-2.0,1.0\n')]:
        path = tmp/f'{label}.csv'
        path.write_text(text)
        fr, _ = stages._read_eq_settings(str(path), estimator)
        cases.append(dict(name=label, text=text, raw=fr.raw, error=fr.error))
    texts = ['# Filter 1: ON PK', 'Filter1: ON PK', 'Filter 12: ON PK', 'filter 1: ON PK', 'Preamp: -3 dB', 'If: sampleRate > 48000', 'frequency,raw\n20.0,3.0', 'Else:', 'Unknown: x']
    save('eq_files', {}, dict(csv=cases, detection=[dict(text=t, expected=looks_like_eqapo_config(t)) for t in texts]), 'core/pipeline_stages.py:183-350; core/eqapo.py:168-189')
    directory = tmp/'eq'
    directory.mkdir()
    for name, gain in [('eq', 1), ('eq-left', 2), ('eq-right', 3)]:
        (directory/f'{name}.csv').write_text(f'20.0,{gain}.0\n24000.0,{gain}.0\n')
    l, r = stages.equalization(estimator, str(directory))
    save('eq_precedence', {}, dict(left=l.error, right=r.error), 'core/pipeline_stages.py:294-334')


def inspect_previous_fixtures():
    """Inspect owned files before regeneration; export protocol; p10_manifest."""
    previous = {}
    for path in sorted(OUT.glob('p10_*')):
        assert path.is_file() and not path.is_symlink(), path
        assert path.suffix in ('.json', '.f64'), path
        data = path.read_bytes()
        if path.suffix == '.json':
            record = json.loads(data)
            assert {'inputs', 'outputs', 'meta'} <= record.keys(), path
        else:
            assert len(data) % 8 == 0, path
        previous[path.name] = len(data)
    print(f'Inspected {len(previous)} existing P10 files, {sum(previous.values())} bytes; no deletion.')
    return previous


def optional_outputs(tmp, estimator):
    """Actual output stages with synthetic extended speakers; pipeline.py:873-972; p10_outputs."""
    names = ['FL','FR','FC','BL','BR','SL','SR','TFL','TFR','TSL','TSR','TBL','TBR']
    cases = []
    track_files = {}
    for count in (7, 8, 9, 10, 13):
        for compact in (False, True):
            label = f'outputs_{count}_{int(compact)}'
            h = HRIR(estimator)
            specs = []
            # Reversed insertion order deliberately differs from output layout order.
            for index, speaker in reversed(list(enumerate(names[:count]))):
                h.irs[speaker] = {}
                for ear, side in enumerate(('left', 'right')):
                    scale = (index + 1) * (ear + 2) / 32.0
                    silent = speaker == 'TFR' and side == 'left'
                    x = np.array([(((i * 37) % 101) - 50) / 50000.0 for i in range(8192)])
                    x[48] += 1.0
                    x *= 0.0 if silent else scale
                    h.irs[speaker][side] = ImpulseResponse(x, estimator.fs)
                    specs.append(dict(speaker=speaker, side=side, scale=scale, silent=silent))
            cfg = ProcessingConfig(do_room_correction=False, do_headphone_compensation=False,
                                   do_equalization=False, output_truehd_layouts=True,
                                   jamesdsp=True, hangloose=True, remove_silent_channels=compact)
            p = BRIRPipeline(cfg)
            p.hrir, p.estimator, p.dir_path, p.logger = h, estimator, str(tmp/label), MagicMock()
            p._stage_crop_and_align()
            p._stage_normalize()
            captured = []
            compact_names = {}
            def capture(path, fs, data, **kwargs):
                """Capture pre-quantization matrices; core/hrir.py:474; p10_outputs."""
                captured.append((Path(path).relative_to(p.dir_path).as_posix(), fs, np.array(data)))
            def capture_sf(path, data, fs, **kwargs):
                """Capture compact matrices before PCM conversion; core/hrir.py:468; p10_outputs."""
                capture(path, fs, data.T)
            def capture_names(path, order):
                """Capture actual compact routing metadata; core/hrir.py:469; p10_outputs."""
                compact_names[Path(path).name] = order
            with patch('core.hrir.write_wav', capture), patch('soundfile.write', capture_sf), patch('core.brir_layout.append_track_names', capture_names):
                p._stage_write_brirs()
                p._stage_truehd_layouts()
                p._stage_jamesdsp()
                p._stage_hangloose()
            matrices = []
            for path, fs, tracks in captured:
                refs = []
                for i, x in enumerate(tracks):
                    digest = hashlib.sha256(x.tobytes()).hexdigest()
                    if digest not in track_files:
                        track_files[digest] = save(f'{label}_{len(matrices)}_{i}', {}, dict(samples=x, **summary(x)), 'core/pipeline.py:873-972')
                    refs.append(track_files[digest])
                matrices.append(dict(path=path, fs=fs, tracks=refs))
            cases.append(dict(specs=specs, compact=compact, count=count, matrices=matrices,
                              compact_names=compact_names, applied_gain=p.applied_gain))
    return save('outputs', {}, cases, 'core/pipeline.py:593-609,733-743,873-972')


def main():
    """P10 scenario families; core/pipeline.py:424-972; p10_manifest."""
    OUT.mkdir(exist_ok=True)
    previous = inspect_previous_fixtures()
    manifest = {}
    with tempfile.TemporaryDirectory(prefix='impulcifer-p10-') as temporary:
        tmp = Path(temporary)
        demo = tmp/'demo'
        shutil.copytree(ROOT/'data/demo', demo)
        estimator = ImpulseResponseEstimator.from_wav(str(ROOT/'data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        files = [n for n in os.listdir(demo) if re.fullmatch(r'[A-Z]{2,3}(,[A-Z]{2,3})*\.wav', n)]
        room_files = [n for n in os.listdir(demo) if re.fullmatch(r'room-[A-Z]{2,3}(,[A-Z]{2,3})*(-(left|right))?\.wav', n)]
        save('protocol', {}, dict(recordings=files, room_recordings=room_files, sweep='data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
        hrir = stages.open_binaural_measurements(estimator, str(demo))
        rir, frs = room.room_correction(estimator, str(demo))
        manifest['room'] = room_state('room', frs)
        manifest['room_responses'] = snapshot('room_responses', rir)
        captured_room = []
        with patch('core.hrir.write_wav', lambda path, fs, data, **kw: captured_room.append(np.array(data))):
            rir.write_wav('unused.wav')
        manifest['room_response_tracks'] = [save(f'room_response_track_{i}', {}, summary(x), 'core/room_correction.py:127-132') for i, x in enumerate(captured_room[0])]
        hp = stages.headphone_compensation(estimator, str(demo))
        save('headphone', {}, dict(left=hp[0].error, right=hp[1].error), 'core/pipeline_stages.py:424-438')
        target = stages.create_target(estimator, 0, 105, .76, 0)
        save('target', {}, target.raw, 'core/pipeline_stages.py:485-501')
        crop(hrir)
        manifest['crop'] = snapshot('default_crop', hrir)
        cropped = hrir.copy()
        firs = eq_firs(hrir, frs, hp, target)
        manifest['firs'] = [save(f'eq_fir_{s}_{side}', {}, dict(speaker=s, side=side, fir=fir), 'core/parallel_workers.py:69-131') for s, side, fir in firs]
        apply_eq(hrir, firs)
        manifest['equalize'] = snapshot('default_equalize', hrir, ('FL',))
        equalized = hrir.copy()
        gain = normalize(hrir)
        manifest['normalize'] = snapshot('default_normalize', hrir)
        manifest['default'] = final('default_final', hrir)
        save('readme', {}, readme_numbers(hrir, gain), 'core/pipeline_stages.py:525-687')
        save('gain', {}, gain)
        tables = [dict(config=c, **table(ProcessingConfig(**c))) for c in [{}, {'vbass':True}, {'microphone_deviation_correction':True}, {'microphone_deviation_correction':True, 'do_headphone_compensation':False}, {'fs':44100}, {'plot':True,'interactive_plots':True,'jamesdsp':True,'hangloose':True,'output_truehd_layouts':True,'decay':{'FL':.3},'channel_balance':'mids'}]]
        save('stage_table', {}, tables)
        bass = cropped.copy()
        apply_virtual_bass_to_hrir(bass, crossover_freq=250)
        manifest['vbass'] = snapshot('vbass', bass, ('FL','FC'))
        apply_eq(bass, firs)
        normalize(bass)
        manifest['vbass_final'] = final('vbass_final', bass)
        manifest['channel_balance'] = {}
        for method in ['trend','left','right','avg','min','mids','3']:
            cb = equalized.copy()
            left = ImpulseResponse(np.mean([cb.irs[s]['left'].data for s in ['FL','FR']],axis=0), cb.fs).frequency_response()
            right = ImpulseResponse(np.mean([cb.irs[s]['right'].data for s in ['FL','FR']],axis=0), cb.fs).frequency_response()
            pair = cb.channel_balance_firs(left, right, method)
            save(f'channel_balance_{method}_firs', {}, dict(left=pair[0],right=pair[1]), 'core/hrir.py:674-783')
            cb.correct_channel_balance(method)
            manifest['channel_balance'][method] = snapshot(f'channel_balance_{method}',cb)
        mic = cropped.copy()
        matching = MicrophoneMatchingCorrector(mic.fs)
        for s,p in mic.irs.items():
            matching.collect_speaker(s,p['left'].data,p['right'].data,p['left'].peak_index(),p['right'].peak_index())
        mismatch = matching.estimate_interaural_mismatch()
        pair = matching.design_correction_filters()
        save('mic_deviation', {}, dict(mismatch_db=mismatch,left=pair[0],right=pair[1],summary=matching.get_analysis_summary()), 'core/microphone_deviation_correction.py:57-292')
        apply_microphone_deviation_correction_to_hrir(mic)
        manifest['mic'] = snapshot('mic',mic)
        dec = equalized.copy()
        params = {}
        for s in ['FL','FR']:
            params[s] = {}
            for side,ir in dec.irs[s].items():
                params[s][side] = ir.decay_adjustment_params(.3)
                ir.adjust_decay(.3)
        save('decay_params', {}, params, 'core/pipeline.py:702-724')
        manifest['decay'] = snapshot('decay',dec,('FL',))
        res = hrir.copy()
        res.resample(44100)
        second = normalize(res)
        save('resample_gain', {}, second, 'core/pipeline.py:859-871')
        manifest['resample'] = final('resample_final',res,('FL',))
        manifest['options'] = {}
        for key,value in [('fr_combination_method','conservative'),('specific_limit',0),('generic_limit',0),('target_level',-12),('tilt',-1.0),('bass_boost_gain',4)]:
            cfg = ProcessingConfig(**{key:value})
            _, option_frs = room.room_correction(estimator,str(demo),fr_combination_method=cfg.fr_combination_method,specific_limit=cfg.specific_limit,generic_limit=cfg.generic_limit)
            option_target = stages.create_target(estimator,cfg.bass_boost_gain,cfg.bass_boost_fc,cfg.bass_boost_q,cfg.tilt)
            option = cropped.copy()
            apply_eq(option,eq_firs(option,option_frs,hp,option_target))
            option_gain=normalize(option,cfg.target_level)
            manifest['options'][key] = dict(value=value,room=room_state(f'option_{key}_room',option_frs),target=save(f'option_{key}_target',{},option_target.raw),gain=option_gain,tracks=snapshot(f'option_{key}',option.subset(['FL'])))
        tracks, fs = sf.read(demo/'FL,FR.wav',always_2d=True)
        n = 2*fs+len(estimator)+2*fs
        generic_tracks = np.stack([tracks[:n,0],np.roll(tracks[:n,0],17)])
        sf.write(demo/'room.wav',generic_tracks.T,fs,subtype='DOUBLE')
        generic_target=room.open_room_target(estimator,str(demo))
        calibration=room.open_mic_calibration(estimator,str(demo))
        for columns in (1, 2):
            recording = generic_tracks if columns == 1 else np.concatenate(
                [generic_tracks, -0.5 * np.roll(generic_tracks[:, 2*fs:], 31, axis=1)], axis=1)
            sf.write(demo/'room.wav', recording.T, fs, subtype='DOUBLE')
            generic, boundaries = [], []
            def trace_generic(frame, event, arg):
                """Capture actual split locals; core/room_correction.py:350-386; p10_generic_room."""
                if frame.f_code is room._open_generic_room_measurement.__code__ and event == 'line' and frame.f_lineno == 377:
                    v=frame.f_locals
                    generic.append(v['ir'])
                    boundaries.append([v['start'],v['end']])
                return trace_generic
            sys.settrace(trace_generic)
            try:
                room._open_generic_room_measurement(estimator,room.discover_room_measurements(str(demo)),calibration,generic_target)
            finally:
                sys.settrace(None)
            segments = [save(f'generic_segments_{columns}_{i}', {}, dict(recording=summary(ir.recording), ir=summary(ir.data), peak_index=ir.peak_index()), 'core/room_correction.py:368-377') for i, ir in enumerate(generic)]
            save(f'generic_split_{columns}', {}, dict(columns=columns, boundaries=boundaries, segments=segments), 'core/room_correction.py:368-377')
            for method, limit in [('average', 300), ('conservative', 300), ('average', 0)]:
                fr=room.calculate_generic_room_correction(generic,generic_target,calibration,method,limit)
                label = method if columns == 1 and limit == 300 else f'{columns}_{method}_{limit}'
                save(f'generic_room_{label}',{},dict(raw=fr.raw,error=fr.error,error_smoothed=fr.error_smoothed,target=fr.target,boundaries=boundaries,lengths=[len(ir.data) for ir in generic]),'core/room_correction.py:231-386')
            if columns == 1:
                # Exercise the real generic-limit option after room.wav exists, including fallback copies.
                _, unlimited = room.room_correction(estimator, str(demo), generic_limit=0)
                manifest['generic_limit_room'] = room_state('generic_limit_room', unlimited)
        manifest['outputs'] = optional_outputs(tmp, estimator)
        resolution_cases(tmp)
        eq_cases(tmp,estimator)
    save('manifest',{},manifest)
    stale = sorted(previous.keys() - WRITTEN.keys())
    size=sum(p.stat().st_size for p in OUT.glob('p10_*'))
    assert size<25_000_000,size
    print(f'P10: {len(WRITTEN)} generated files, {size} total bytes; {len(stale)} retained previous files; demo unchanged.')


if __name__ == '__main__':
    main()
