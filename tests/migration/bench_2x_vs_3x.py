"""Wall time and process-tree memory of 2.x and 3.x, for the README comparison.

Linux only: memory is read from /proc. Two subcommands, each printing one JSON line.

  run LABEL -- CMD...
      Run CMD to completion. Reports wall time, the peak of the tree's summed PSS
      and RSS (sampled every 20 ms) and the largest single-process VmHWM.
  gui LABEL DISPLAY SETTLE_S SAMPLE_S SHOT.png -- CMD...
      Start a GUI app on an X display, wait SETTLE_S, then sample the tree once a
      second for SAMPLE_S. Reports the median of the summed PSS, the per-process
      PSS/RSS/anonymous PSS of the last sample, and saves a screenshot (ImageMagick
      ``import``) to confirm the window rendered. The app is then terminated.

PSS splits pages shared between processes, so the sum over a tree does not count
the pages a forked worker shares with its parent twice; the RSS sum does. The
procedure and the results are in docs/rust/perf/2x-vs-3x-linux.md.
"""
import json
import os
import signal
import statistics
import subprocess
import sys
import time


def _children():
    kids = {}
    for name in os.listdir("/proc"):
        if not name.isdigit():
            continue
        try:
            with open(f"/proc/{name}/stat") as f:
                stat = f.read()
        except OSError:
            continue
        ppid = int(stat[stat.rfind(")") + 2:].split()[1])
        kids.setdefault(ppid, []).append(int(name))
    return kids


def _tree(root):
    kids = _children()
    out, stack = [], [root]
    while stack:
        pid = stack.pop()
        out.append(pid)
        stack.extend(kids.get(pid, []))
    return out


def _kib(path, key):
    try:
        with open(path) as f:
            for line in f:
                if line.startswith(key):
                    return int(line.split()[1])
    except OSError:
        return None
    return None


def _comm(pid):
    try:
        with open(f"/proc/{pid}/comm") as f:
            return f.read().strip()
    except OSError:
        return "?"


def _mib(kib):
    return round(kib / 1024, 1)


def run(label, cmd):
    start = time.perf_counter()
    proc = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    peak_pss = peak_rss = procs = 0
    hwm = {}
    while proc.poll() is None:
        pids = _tree(proc.pid)
        pss = rss = 0
        for pid in pids:
            pss += _kib(f"/proc/{pid}/smaps_rollup", "Pss:") or 0
            rss += _kib(f"/proc/{pid}/status", "VmRSS:") or 0
            high = _kib(f"/proc/{pid}/status", "VmHWM:")
            if high:
                hwm[pid] = max(hwm.get(pid, 0), high)
        peak_pss, peak_rss = max(peak_pss, pss), max(peak_rss, rss)
        procs = max(procs, len(pids))
        time.sleep(0.02)
    result = {
        "label": label,
        "rc": proc.returncode,
        "wall_s": round(time.perf_counter() - start, 3),
        "peak_tree_pss_mib": _mib(peak_pss),
        "peak_tree_rss_mib": _mib(peak_rss),
        "max_single_hwm_mib": _mib(max(hwm.values(), default=0)),
        "max_procs": procs,
    }
    if proc.returncode:
        result["stderr_tail"] = proc.stderr.read().decode(errors="replace")[-1500:]
    print(json.dumps(result), flush=True)


def _snapshot(root):
    rows = []
    for pid in _tree(root):
        pss = _kib(f"/proc/{pid}/smaps_rollup", "Pss:")
        if pss is None:
            continue
        rss = _kib(f"/proc/{pid}/status", "VmRSS:") or 0
        anon = _kib(f"/proc/{pid}/smaps_rollup", "Pss_Anon:") or 0
        rows.append((_comm(pid), pss, rss, anon))
    return rows


def gui(label, display, settle, sample, shot, cmd):
    env = dict(os.environ, DISPLAY=display)
    with open(f"{label}.log", "w") as log:
        proc = subprocess.Popen(cmd, env=env, stdout=log, stderr=subprocess.STDOUT,
                                start_new_session=True)
    time.sleep(float(settle))
    totals, last = [], []
    end = time.perf_counter() + float(sample)
    while time.perf_counter() < end and proc.poll() is None:
        last = _snapshot(proc.pid)
        totals.append(sum(row[1] for row in last))
        time.sleep(1)
    subprocess.run(["import", "-display", display, "-window", "root", shot], check=False)
    alive = proc.poll() is None
    if alive:
        os.killpg(proc.pid, signal.SIGTERM)
        try:
            proc.wait(10)
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
    print(json.dumps({
        "label": label,
        "alive_at_end": alive,
        "steady_tree_pss_mib": _mib(statistics.median(totals)) if totals else None,
        "procs": [(name, _mib(pss), _mib(rss), _mib(anon)) for name, pss, rss, anon in last],
    }), flush=True)


def main(argv):
    cmd = argv[argv.index("--") + 1:]
    if argv[0] == "run":
        run(argv[1], cmd)
    elif argv[0] == "gui":
        gui(*argv[1:6], cmd)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
