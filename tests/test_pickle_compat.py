"""Pins the ``__main__`` pickle contract for the bundled demo test signal.

The demo estimator pickle (``data/sweep-*.pkl``) was created by running
``impulcifer.py`` directly, so it stores its class reference as
``__main__.ImpulseResponseEstimator``. Whenever the CLI is the entry point,
``__main__`` *is* the impulcifer module — the class must therefore stay
resolvable as an attribute of that module (discovered the hard way in audit
#138 C1-2 when the seemingly-unused import was dropped).
"""

import pickle
import sys
from pathlib import Path

import pytest


def test_impulcifer_module_exports_estimator_class():
    import impulcifer
    from core.impulse_response_estimator import ImpulseResponseEstimator

    assert impulcifer.ImpulseResponseEstimator is ImpulseResponseEstimator


def test_demo_pickle_resolves_via_main_namespace(monkeypatch):
    pkl = Path(__file__).parent.parent / 'data' / 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.pkl'
    if not pkl.is_file():
        pytest.skip('demo pickle not present')

    import impulcifer

    monkeypatch.setitem(sys.modules, '__main__', impulcifer)
    with open(pkl, 'rb') as fh:
        estimator = pickle.load(fh)
    assert type(estimator).__name__ == 'ImpulseResponseEstimator'
