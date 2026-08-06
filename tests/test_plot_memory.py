"""``HRIR.plot()`` 메모리 잔류 회귀 테스트.

matplotlib 3D/pcolormesh figure를 파이프라인 프로세스 안에서 렌더링하면
수십만 개의 소형 객체가 glibc 힙을 조각내, ``plt.close()`` + ``gc.collect()``
후에도 수백 MB의 RSS가 프로세스 종료 시까지 잔류한다 (BRIR 생성이 끝나도
점유가 유지되는 현상). 이 테스트는 플롯 완료 + 참조 해제 후 부모 프로세스의
RSS 증가분이 임계값 아래인지 자식 프로세스에서 측정해 고정한다.
"""

import os
import subprocess
import sys

import pytest

pytestmark = pytest.mark.skipif(
    not sys.platform.startswith("linux"),
    reason="/proc 기반 RSS 측정은 Linux 전용",
)

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# 플롯 완료 후 허용되는 RSS 증가분(MB). 수정 전에는 2-스피커(figure 4장)
# 워크로드에서 ~400MB가 잔류했다. 워커 프로세스 렌더링에서는 부모에
# figure가 생성되지 않으므로 여유를 두어도 이 값 아래여야 한다.
MAX_RETAINED_MB = 150

CHILD_SCRIPT = """
import gc, sys, types
import numpy as np

def rss_mb():
    with open("/proc/self/status") as f:
        for line in f:
            if line.startswith("VmRSS:"):
                return int(line.split()[1]) / 1024.0
    raise RuntimeError("VmRSS not found")

import matplotlib
matplotlib.use("Agg")

from core.hrir import HRIR
from core.impulse_response import ImpulseResponse

fs = 48000
rng = np.random.default_rng(0)
hrir = HRIR(types.SimpleNamespace(fs=fs))
t_ir = np.arange(fs) / fs
for sp in ("FL", "FR"):
    pair = {}
    for side in ("left", "right"):
        ir = np.zeros(fs)
        ir[500] = 1.0
        ir += rng.standard_normal(fs) * np.exp(-t_ir / 0.08) * 0.03
        rec = rng.standard_normal(fs * 7) * 0.1
        pair[side] = ImpulseResponse(ir, fs, recording=rec)
    hrir.irs[sp] = pair

gc.collect()
baseline = rss_mb()

hrir.plot(dir_path=sys.argv[1])

hrir = None
gc.collect()
after = rss_mb()
print(f"RSS_DELTA_MB={after - baseline:.1f}")
"""


def test_plot_does_not_retain_memory_after_completion(tmp_path):
    """플롯 산출(PNG 저장)이 끝나고 HRIR 참조를 해제하면 프로세스 메모리
    점유가 플롯 이전 수준으로 돌아와야 한다."""
    env = dict(os.environ, MPLBACKEND="Agg")
    result = subprocess.run(
        [sys.executable, "-c", CHILD_SCRIPT, str(tmp_path)],
        capture_output=True,
        text=True,
        cwd=REPO_ROOT,
        env=env,
        timeout=600,
        check=False,
    )
    assert result.returncode == 0, f"자식 프로세스 실패:\n{result.stderr[-2000:]}"

    delta_lines = [
        line for line in result.stdout.splitlines() if line.startswith("RSS_DELTA_MB=")
    ]
    assert delta_lines, f"측정 출력이 없다:\n{result.stdout[-2000:]}"
    delta_mb = float(delta_lines[-1].split("=")[1])

    assert delta_mb < MAX_RETAINED_MB, (
        f"플롯 완료 후 {delta_mb:.0f}MB가 잔류했다 (허용 {MAX_RETAINED_MB}MB). "
        "figure 렌더링이 부모 프로세스 힙을 다시 조각내고 있는지 확인할 것."
    )

    # 플롯 산출물이 실제로 생성되었는지 확인 (빈 구현으로 통과하는 것 방지)
    pngs = [f for f in os.listdir(tmp_path) if f.endswith(".png")]
    assert len(pngs) == 4, f"PNG 4장이 생성되어야 한다: {pngs}"
