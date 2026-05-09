import argparse
import csv
from pathlib import Path

import numpy as np
import soundfile as sf


def read_wav(path):
    data, fs = sf.read(path, always_2d=True)
    return data.astype(np.float64), fs


def db(x):
    return 20.0 * np.log10(np.maximum(x, 1e-20))


def channel_name(i):
    names = [
        "FL-left", "FL-right", "SL-left", "SL-right", "BL-left", "BL-right", "FC-left",
        "FR-right", "FR-left", "SR-right", "SR-left", "BR-right", "BR-left", "FC-right",
        "WL-left", "WL-right", "WR-left", "WR-right", "TFL-left", "TFL-right",
        "TFR-left", "TFR-right", "TSL-left", "TSL-right", "TSR-left", "TSR-right",
        "TBL-left", "TBL-right", "TBR-left", "TBR-right",
    ]
    return names[i] if i < len(names) else f"ch{i + 1}"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("candidate")
    parser.add_argument("reference")
    parser.add_argument("--csv-out")
    args = parser.parse_args()

    cand, fs_c = read_wav(args.candidate)
    ref, fs_r = read_wav(args.reference)
    print(f"candidate={args.candidate}")
    print(f"reference={args.reference}")
    print(f"fs candidate/reference: {fs_c}/{fs_r}")
    print(f"shape candidate/reference: {cand.shape}/{ref.shape}")

    n = min(len(cand), len(ref))
    ch = min(cand.shape[1], ref.shape[1])
    cand2 = cand[:n, :ch]
    ref2 = ref[:n, :ch]
    diff = cand2 - ref2
    print(f"exact_equal={np.array_equal(cand, ref)}")
    print(f"max_abs_sample_diff={np.max(np.abs(diff)):.12g}")
    print(f"rms_sample_diff={np.sqrt(np.mean(diff ** 2)):.12g}")

    f_min = 15
    f_max = min(20000, fs_c // 2, fs_r // 2)
    n_fft = 1
    target_fft = max(fs_c, n)
    while n_fft < target_fft:
        n_fft *= 2
    freqs = np.fft.rfftfreq(n_fft, 1.0 / fs_c)
    bins = np.array([np.argmin(np.abs(freqs - f)) for f in range(f_min, f_max + 1)])

    rows = []
    all_abs = []
    low_abs = []
    high_abs = []
    for c in range(ch):
        c_mag = np.abs(np.fft.rfft(cand2[:, c], n=n_fft))[bins]
        r_mag = np.abs(np.fft.rfft(ref2[:, c], n=n_fft))[bins]
        delta_db = db(c_mag) - db(r_mag)
        all_abs.extend(np.abs(delta_db))
        low = delta_db[: 200 - f_min + 1]
        high = delta_db[10000 - f_min :]
        low_abs.extend(np.abs(low))
        high_abs.extend(np.abs(high))
        max_i = int(np.argmax(np.abs(delta_db)))
        low_mean = float(np.mean(low))
        low_min = float(np.min(low))
        low_max = float(np.max(low))
        print(
            f"{c + 1:02d} {channel_name(c):8s} "
            f"mean_abs={np.mean(np.abs(delta_db)):.3f}dB "
            f"max_abs={np.max(np.abs(delta_db)):.3f}dB@{f_min + max_i}Hz "
            f"low15_200(mean/min/max)={low_mean:.3f}/{low_min:.3f}/{low_max:.3f}dB "
            f"high10k_20k_mean_abs={np.mean(np.abs(high)):.3f}dB"
        )
        if args.csv_out:
            for i, f in enumerate(range(f_min, f_max + 1)):
                rows.append([c + 1, channel_name(c), f, float(delta_db[i])])

    print(f"all_freq_mean_abs_db={np.mean(all_abs):.6f}")
    print(f"all_freq_max_abs_db={np.max(all_abs):.6f}")
    print(f"low15_200_mean_abs_db={np.mean(low_abs):.6f}")
    print(f"high10k_20k_mean_abs_db={np.mean(high_abs):.6f}")

    if args.csv_out:
        out = Path(args.csv_out)
        out.parent.mkdir(parents=True, exist_ok=True)
        with out.open("w", newline="", encoding="utf-8") as f:
            writer = csv.writer(f)
            writer.writerow(["channel_index", "channel", "frequency_hz", "candidate_minus_reference_db"])
            writer.writerows(rows)
        print(f"csv_out={out}")


if __name__ == "__main__":
    main()
