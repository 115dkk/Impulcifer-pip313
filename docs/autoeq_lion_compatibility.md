# Lion-style AutoEQ compatibility notes

이 문서는 가상 베이스 회귀를 잡기 위해 현재 버전에서 LionLion123/Impulcifer의 AutoEQ/DSP 경로로 되돌린 부분과, 기존 `autoeq-py313` 경로가 왜 출력 변조를 만들었는지 정리한다.

## 무엇이 Lion식으로 대체되었나

1. AutoEQ 구현
   - `autoeq-py313` 배포판 의존을 제거하고, Lion에서 쓰는 구 AutoEQ 계열 구현을 저장소의 `autoeq/` 패키지로 벤더링했다.
   - `FrequencyResponse.smoothen_heavy_light()`, 구형 `equalize()`, `minimum_phase_impulse_response()` 경로를 그대로 사용한다.
   - 기존 코드가 기대하던 이름을 유지하기 위해 `read_csv`, `write_csv`, `plot()`, `smoothen()` 호환 래퍼를 추가했다.
   - CSV 입출력은 표준 라이브러리 `csv`로 처리하므로 AutoEQ 패키지 설치나 `pandas` 설치 없이 동작한다.

2. Equalization worker
   - `core/parallel_workers.py`의 EQ 생성 경로를 `fr.smoothen_heavy_light()` 후 `fr.equalize(max_gain=40, treble_f_lower=10000, treble_f_upper=fs/2)`로 변경했다.
   - `autoeq-py313`식 `window_size`/`treble_window_size` 인자를 넘기는 경로는 제거했다.

3. FFT magnitude response
   - `core/utils.py:magnitude_response()`를 `scipy.fft.rfft` 기반 계산에서 Lion과 같은 `scipy.fftpack.fft` 전체 FFT 절반 슬라이스로 되돌렸다.
   - 이 변경은 주파수 bin 위치와 극저레벨 floor를 Lion과 맞추기 위한 것이다.

4. Impulse response frequency sampling
   - `core/impulse_response.py:frequency_response()`를 Lion처럼 4 Hz 목표 해상도에 대해 `round(len(f) / target_points)`로 샘플링하도록 바꿨다.
   - 특히 demo의 `room-responses.wav` 길이 21600 샘플에서는 `int()`가 step 1을 만들고 Lion의 `round()`는 step 2를 만든다. 이 차이가 room EQ FIR을 바꿔 최종 Hesuvi 저역/고역 편차로 증폭됐다.

5. Room/headphone correction 주변 DSP
   - `HRIR.crop_tails()`와 `ImpulseResponse.decay_params()`를 Lion과 맞춰 `room-responses.wav`가 21600 샘플로 정확히 나오게 했다.
   - headphone compensation은 flat zero target에 대해 `min_mean_error=False`로 보정하도록 되돌렸다.
   - 최종 normalization은 Lion처럼 모든 처리 뒤에 수행한다.

6. Virtual bass
   - 기존 per-channel low-pass/minimum-phase 재합성 대신 Lion 방식의 shared band-limited bass impulse를 사용한다.
   - 하나의 `mpbass`를 만들고 전 채널 crossover magnitude 평균으로 gain-match한 뒤, speaker/ear delay와 ILD shelf를 적용해 합성한다.
   - 이로써 채널마다 저역이 따로 부스트/감쇄되는 문제가 사라진다.

## `autoeq-py313` 경로가 변조를 만든 이유

`autoeq-py313` 자체가 단순 포팅이 아니라 API와 알고리즘이 달라진 AutoEQ 계열이었다. 이번 회귀의 핵심은 다음 조합이다.

1. smoothing 의미가 달라졌다.
   - Lion의 구 AutoEQ는 `smoothen_heavy_light()`로 light/heavy smoothing 결과를 조합한 뒤 EQ error를 만든다.
   - `autoeq-py313`에는 이 경로가 없거나 이름/동작이 달라, 현재 코드는 `window_size=1/3`, `treble_window_size=1/5` 같은 새 인자로 근사하려고 했다.
   - 이 근사는 Lion의 error curve와 같지 않아 FIR magnitude가 달라졌다.

2. equalize 보호/제한 방식이 달라졌다.
   - 구 AutoEQ의 `equalize()`는 smoothed error를 바탕으로 gain limit과 treble cutoff를 적용한다.
   - `autoeq-py313` 계열은 slope limit, protection mask, treble gain 처리, 내부 smoothing 기본값 등이 달라 같은 입력 FR에서도 다른 `equalization` curve를 만든다.

3. frequency grid가 달라졌다.
   - `magnitude_response()`를 `rfft + epsilon`으로 바꾸면서 DC/one-sided bin 산정과 noise floor가 Lion과 달라졌다.
   - `ImpulseResponse.frequency_response()`의 `int()` 샘플링은 room IR에서 Lion의 `round()`와 다른 bin subset을 고르게 했다.
   - room correction FR은 이후 minimum-phase FIR로 변환되므로 작은 grid 차이가 전체 Hesuvi 응답 차이로 커졌다.

4. headphone compensation recentering이 추가되어 있었다.
   - `min_mean_error=True`와 target 재보간은 Lion의 flat compensation과 달리 좌우 보정 기준을 다시 움직인다.
   - 이 차이는 고역 쪽 작은 편차와 채널별 tilt를 만들 수 있다.

결론적으로 `autoeq-py313`은 Python 3.13 설치 호환성을 제공하지만, Lion/원본 Impulcifer와 bit/spectral parity가 필요한 경로에서는 대체재가 아니었다. 향후 `autoeq-py313`의 최적화를 다시 도입하려면 먼저 `smoothen_heavy_light()` 호환 모드, 구 `equalize()` parity mode, frequency grid parity를 구현하고 demo 기반 회귀 테스트를 통과시켜야 한다.

## 벤더 AutoEQ의 목적과 의존성 경계

이번에 추가한 `autoeq/` 패키지의 목적은 외부 AutoEQ 배포판을 새로 설치하는 것이 아니라, Lion과 같은 수치 알고리즘을 프로젝트 안에 고정하는 것이다. Python import는 저장소 루트의 `autoeq/`를 먼저 찾으므로 `autoeq-py313`이나 Python 3.8 전용 AutoEQ wheel이 없어도 실행된다. 패키징도 `pyproject.toml`의 `autoeq/**/*.py` include로 이 코드를 함께 배포한다.

따라서 "AutoEQ 자체"는 별도 설치가 필요 없다. 필요한 것은 Impulcifer가 이미 쓰는 수치/플로팅 기반 의존성이다.

- 필수 처리 경로: `numpy`, `scipy`, `matplotlib`, `tabulate`
- CSV 입출력: 표준 라이브러리 `csv`
- 이미지 팔레트 최적화: `Pillow`를 사용한다. 이 의존성은 `core.utils`에서도 이미 `PIL.Image`를 import하므로 AutoEQ만의 새 요구사항은 아니다.
- TensorFlow 기반 parametric EQ 최적화 함수는 벤더 코드에 남아 있지만 Impulcifer의 BRIR 생성 경로에서는 호출하지 않는다. TensorFlow는 설치 요구사항에 넣지 않는다.

`autoeq/`는 ruff 검사에서 제외했다. 이유는 이 디렉터리가 일반 애플리케이션 코드가 아니라 parity 기준이 되는 벤더 수치 코드이기 때문이다. ruff의 `E721`, `F522`, `F901` 같은 알림을 자동 수정하면 현재 통과한 Lion bit parity를 다시 깨뜨릴 수 있다. 이 디렉터리를 바꿀 때는 lint 정리보다 demo parity 검증을 우선한다.

향후 최적화를 재도입하려면 이 벤더 패키지 안에 "Lion parity mode"와 "optimized mode"를 명확히 분리하는 것이 안전하다. 최적화된 AutoEQ 경로도 아래 검증을 통과해야 한다.

1. `responses.wav`, `room-responses.wav`, `headphone-responses.wav`가 Lion 대조군과 동일하거나 의도된 차이를 문서화할 것
2. `Hesuvi.wav`가 Python 3.13 Lion 대조군과 `exact_equal=True`이거나, 최적화 모드라면 허용 오차와 청각/수치 근거를 별도로 정의할 것
3. 저역 15-200 Hz와 고역 10-20 kHz의 평균/최대 편차를 PR에 기록할 것

## 검증 증거

검증 데이터:

- Demo input: `jaakkopasanen/Impulcifer`의 `data/demo`
- 기능: `--vbass --vbass_freq 250`
- Lion Python 3.8 대조군: `E:\Impulcifer\_verification\lion_control_250\hesuvi.wav`
- Lion Python 3.13 대조군: `E:\Impulcifer\_verification\lion_py313_control_250\hesuvi.wav`
- 최종 수정본: `E:\Impulcifer\_verification\current_final\hesuvi.wav`

최종 결과:

- 현재 수정본 vs Lion Python 3.13: `exact_equal=True`, `max_abs_sample_diff=0`, `all_freq_mean_abs_db=0.000000`
- 현재 수정본 vs Lion Python 3.8: `max_abs_sample_diff=7.45058059692e-09`, `all_freq_mean_abs_db=0.000023`, `low15_200_mean_abs_db=0.000010`, `high10k_20k_mean_abs_db=0.000025`
- Lion Python 3.13 vs Lion Python 3.8 자체도 같은 런타임 바닥값(`all_freq_mean_abs_db=0.000023`)을 보였으므로, Python 3.8과의 잔차는 구현 변조가 아니라 런타임/부동소수점 차이다.

중간 산출물:

- `responses.wav`: Lion과 `exact_equal=True`
- `room-responses.wav`: Lion과 `exact_equal=True`, shape `(21600, 32)`
- `headphone-responses.wav`: Lion과 `exact_equal=True`

따라서 이번 수정 후 가상 베이스 및 AutoEQ 경로의 출력은 Python 3.13에서 실행 가능한 Lion 대조군과 비트 단위로 일치한다.
