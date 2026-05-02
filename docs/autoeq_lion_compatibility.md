# Lion-style AutoEQ compatibility notes

이 문서는 가상 베이스 회귀를 잡기 위해 Impulcifer의 AutoEQ/DSP 경로에서 무엇을 LionLion123/Impulcifer 방식으로 되돌렸는지, 그리고 기존 `autoeq-py313` 경로가 왜 Hesuvi 출력을 변조했는지 정리한다.

## 무엇을 Lion식으로 대체했나

1. AutoEQ 구현
   - 외부 `autoeq-py313` 의존을 제거하고, Lion과 같은 구 AutoEQ 계열의 `FrequencyResponse` 구현을 저장소의 `autoeq/` 패키지로 벤더링했다.
   - `smoothen_heavy_light()`, 구형 `equalize()`, `minimum_phase_impulse_response()` 경로를 Impulcifer equalization worker가 직접 사용한다.
   - 기존 코드 호환을 위해 `read_csv`, `write_csv`, `plot()`, `smoothen()` 이름만 wrapper로 제공한다. 수치 경로는 Lion parity를 우선한다.
   - CSV 입출력은 표준 라이브러리 `csv`로 처리한다. 따라서 AutoEQ 패키지나 `pandas`를 따로 설치하지 않아도 동작한다.

2. Equalization worker
   - `core/parallel_workers.py`의 EQ 생성은 `fr.smoothen_heavy_light()` 이후 `fr.equalize(max_gain=40, treble_f_lower=10000, treble_f_upper=fs/2)`를 호출한다.
   - `autoeq-py313`식 smoothing 인자를 넘기던 근사 경로는 제거했다. 같은 입력 FR이면 Lion과 같은 error/equalization curve가 만들어져야 한다.

3. DSP 보조 경로
   - `core/utils.py:magnitude_response()`는 Lion과 같은 `scipy.fftpack.fft` 전체 FFT 후 half slice 경로를 사용한다.
   - `core/impulse_response.py:frequency_response()`의 주파수 샘플링은 Lion처럼 `round(len(f) / target_points)` 기반으로 맞췄다.
   - headphone compensation은 flat zero target에 대해 `min_mean_error=False`를 사용한다.
   - 최종 normalization은 Lion처럼 모든 처리 이후 한 번 수행한다.

4. Virtual bass
   - per-channel low-pass/minimum-phase 합성 대신 shared band-limited bass impulse를 만든다.
   - 채널마다 crossover magnitude 평균으로 gain match를 하고, speaker/ear delay 및 ILD shelf를 적용한다.
   - 이 변경으로 채널별 저역이 임의로 boost 또는 cut되는 회귀가 사라졌다.

## 왜 `autoeq-py313`이 변개를 만들었나

`autoeq-py313`은 단순 Python 3.13 포팅이 아니라 AutoEQ 알고리즘/API가 달라진 구현이었다. 변조의 핵심은 다음이다.

1. Smoothing 경로 차이
   - Lion은 `smoothen_heavy_light()`로 light/heavy smoothing 결과를 조합해 error curve를 만든다.
   - `autoeq-py313`에는 이 경로가 없거나 동작이 달라, `window_size`, `treble_window_size` 인자로 근사하면 같은 FIR magnitude가 나오지 않는다.

2. Equalization 제한 방식 차이
   - Lion의 구 `equalize()`는 smoothed error를 직접 뒤집고 gain limit 및 treble transition을 적용한다.
   - `autoeq-py313` 계열은 slope limit, protection mask, treble gain 처리, 내부 smoothing 기본값이 달라 같은 입력에서도 다른 `equalization` curve를 만든다.

3. Frequency grid 차이
   - `rfft + epsilon` 기반 magnitude response와 Lion식 full FFT half slice는 DC/one-sided bin 처리와 noise floor가 다르다.
   - room correction FR은 이후 minimum-phase FIR로 변환되므로 작은 grid 차이가 Hesuvi의 고역/저역 응답 차이로 커진다.

4. Headphone compensation recentering
   - `min_mean_error=True`식 recentering은 Lion의 flat compensation과 달리 좌우 보정 기준을 다시 움직인다.
   - 이 차이는 고역 tilt와 채널별 level 차이로 보일 수 있다.

결론적으로 `autoeq-py313`은 설치 호환성을 제공했지만, Lion/원본 Impulcifer와 bit/spectral parity가 필요한 BRIR 생성 경로에서는 대체재가 아니었다.

## 현재 AutoEQ 의존성 경계

`autoeq/`는 프로젝트 내부 패키지다. 별도 AutoEQ wheel 설치가 필요 없고, `pyproject.toml`에서도 외부 AutoEQ 의존성은 제거되어 있다. `impulcifer.py --info`에도 `autoeq-py313`은 dependency로 표시하지 않는다.

필수 런타임은 Impulcifer 자체가 쓰는 `numpy`, `scipy`, `matplotlib`, `tabulate`, `Pillow` 범위에 있다. `Pillow`는 `core.utils`에서도 PNG 최적화에 쓰이므로 AutoEQ만의 추가 의존성이 아니다.

구식 의존성 정리는 다음과 같이 했다.

- `pandas`: CSV 입출력을 표준 `csv`로 대체했다.
- TensorFlow v1: parametric/fixed-band EQ 최적화의 활성 경로를 `scipy.optimize.least_squares`로 대체했다.
- Pillow old constant: `Image.ADAPTIVE` 직접 사용을 `Image.Palette.ADAPTIVE` 우선 fallback으로 바꿨다.
- 오래된 matplotlib API: 현재 활성 AutoEQ/Impulcifer 경로에는 `matplotlib.mlab` 또는 `specgram` 의존이 없다.

## 최적화와 병렬 처리

AutoEQ 내부에는 출력 parity가 흔들리지 않는 범위의 최적화를 재도입했다.

- `FrequencyResponse.equalize()`의 per-bin gain clipping은 NumPy vectorized 계산으로 바꿨다.
- kink smoothing의 제거 대상 처리는 리스트 membership 반복 대신 boolean mask를 사용한다.
- biquad 최적화는 TensorFlow graph 생성 없이 SciPy least-squares로 실행한다.

Impulcifer의 channel/speaker 단위 EQ 처리는 기존처럼 `core.parallel_utils.parallel_map()`을 통해 병렬 실행된다.

- free-threaded Python: `ThreadPoolExecutor`를 사용한다.
- 일반 GIL Python: `ProcessPoolExecutor`를 사용한다.
- 일반 GIL Python에서 process worker가 실패하면 thread fallback으로 CPU-bound 작업을 계속하지 않고 예외를 올린다. free-threaded 빌드에서만 thread fallback이 허용된다.

AutoEQ 내부에서 다시 중첩 process pool을 열지는 않는다. 현재 병렬 단위는 Impulcifer worker level이며, 이 편이 Windows spawn 비용과 pickle 비용을 가장 덜 만든다.

## Ruff 정책

`autoeq/`는 더 이상 ruff 제외 대상이 아니다. 이번 수정에서 ruff가 잡는 실제 문제를 코드로 고쳤다.

- `type(x) == list`류 비교를 `isinstance()`로 변경했다.
- `raise NotImplemented(...)`를 `raise NotImplementedError(...)`로 고쳤다.
- `.format()`의 미사용 인자를 제거했다.
- 사용하지 않는 변수와 구식 상수 사용을 정리했다.

`_verification/`은 대조군 산출물과 대용량 demo output을 담는 작업 폴더라 ruff 제외를 유지한다.

## 검증 증거

검증 입력:

- Demo input: `jaakkopasanen/Impulcifer`의 `data/demo`
- 기능: `--vbass --vbass_freq 250`
- Candidate: `E:\Impulcifer\_verification\current_autoeq_opt\hesuvi.wav`
- Lion Python 3.13 대조군: `E:\Impulcifer\_verification\lion_py313_control_250\hesuvi.wav`
- Lion Python 3.8 대조군: `E:\Impulcifer\_verification\lion_control_250\hesuvi.wav`

최신 결과:

- Candidate vs Lion Python 3.13: `exact_equal=True`, `max_abs_sample_diff=0`, `all_freq_mean_abs_db=0.000000`, `low15_200_mean_abs_db=0.000000`, `high10k_20k_mean_abs_db=0.000000`
- Candidate vs Lion Python 3.8: `max_abs_sample_diff=7.45058059692e-09`, `all_freq_mean_abs_db=0.000023`, `low15_200_mean_abs_db=0.000010`, `high10k_20k_mean_abs_db=0.000025`

Python 3.8 대조군과의 미세 차이는 Lion Python 3.13 대조군과 Lion Python 3.8 대조군 사이에서도 같은 크기로 나타나는 런타임 부동소수 차이다. Python 3.13에서 실행 가능한 Lion 대조군과는 bit 단위로 일치한다.
