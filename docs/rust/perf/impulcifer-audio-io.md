# PA05: audio-io and sys-win measurement-session overhead
2026-09-08 재측정과 M5 판정은 [PA06 출시 전 성능 감사](release.md)를 참고한다.

## Eighth run (PA07), 2026-09-12: alpha.2 재측정, 기본 4주기 유지

**작은 버퍼는 이번에도 통과하지 못했다. `SHARED_BUFFER_PERIODS = 4`를 그대로 둔다.** 2·3주기는 두 작업 각각 10회 모두 underrun이 있었다. 4주기의 underrun·시작 이후 discontinuity·packet gap은 모두 0이지만, 독립 동시 녹음까지 요구한 엄격한 통과 수는 headphones **7/10**, seven **3/10**이다. 따라서 4주기의 무손실을 보장하거나 이번 측정으로 성능 감사를 통과했다고 주장하지 않는다.

명시적 Python 스트림에 대한 headphones wall 비율은 일반/FT **0.997704 / 0.997552**다. Rust는 각각 **14.2454 / 15.1930 ms** 더 걸렸다. Rust 시간을 Python 시간으로 나누면 **0.2301 / 0.2454%** 더 길다. PA06·PA05b의 약 0.25~0.33% 지연보다 커지지 않았지만, 여전히 `Python/Rust >= 1.0` 조건에는 미달한다. 소유자가 이미 받아들인 M5 예외와 등록부는 변경하지 않았다. 이번 작업은 최적화가 아닌 재측정이다.

### 1. 환경과 측정 방법

- 측정 커밋은 `a623d0b2a71e7bf46422ba251f17acfa4fc5d5c2`, 브랜치는 `claude/rust-alpha2-followups`, 워크스페이스는 `3.0.0-alpha.2`다. 시작 시 작업 트리는 깨끗했다. 최종 확인에서 HEAD는 `0f30ce394b4c9a370beae02b9ded43b250fdb18c`로 바뀌어 있었다(21:57:40의 CI 수정 커밋). 이 세션에서 커밋하지 않았으며, 두 SHA 사이의 변경은 `.github/workflows/release-3x.yml`과 `rust.yml`뿐이다. 오디오 두 크레이트·Cargo.toml·Cargo.lock은 동일하므로 측정한 오디오 소스는 바뀌지 않았다. Rust/Cargo 1.97.0, `x86_64-pc-windows-msvc`, LLVM 22.1.6, release/bench 프로필로 실행했다.
- Windows 11 Education 10.0.22621, 논리 16코어를 확인했다. CPU 모델 Intel Core i5-12600KF와 물리 10코어는 기존 PA06 기록을 따른다. 이번 `Get-CimInstance Win32_Processor` 확인은 도구의 승인 요구로 실행하지 못했으며, 모델명을 새로 조회했다고 주장하지 않는다.
- **다른 프로세스는 조회하지 않았다(other processes were not queried). 호출자가 다른 워커와 빌드를 실행하지 않아 머신을 조용히 유지했다(caller kept the machine quiet).** 모든 명령과 그 자식은 포그라운드에서 종료를 기다렸으며 사용자 프로세스를 종료하지 않았다. 오라클의 `--environment-only`는 프로세스 목록을 조회하므로 사용하지 않았다. CPU 계측은 요청한 방식대로 벤치 자신의 user+kernel 카운터와 ACK로 받은 Rust 자식 PID의 카운터만 읽었다.
- 일반 Python은 `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`, CPython 3.14.5, GIL 활성이다. 지정된 `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`는 디렉터리부터 없어 실행이 `WinError 2`로 실패했다. 설치된 `C:/Users/32170336/AppData/Roaming/uv/python/cpython-3.14+freethreaded-windows-x86_64-none/python.exe`는 CPython 3.14.7t지만 NumPy가 없어 환경 확인이 실패했다.
- 그래서 git에서 무시하는 `crates/impulcifer-audio-io/tests/bench_support/eighth-py314t/`에 위 3.14.7t로 가상환경을 만들었다. 실제 FT 실행 파일은 `E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth-py314t/Scripts/python.exe`이며 GIL 비활성을 확인했다. **기존 venv 자체를 재현한 것은 아니다.** NumPy 2.5.3, SciPy 1.18.1, sounddevice 0.5.6은 두 인터프리터와 이전 측정이 같다. 나머지 설치 버전은 `eighth-install-ft.log`에 보존했다.
- PortAudio V19.7.0-devel, NumPy OpenBLAS ILP64 0.3.34.106.0/pocketfft, SciPy OpenBLAS LP64 0.3.31.dev/duccfft다. OMP/MKL/OPENBLAS/NUMEXPR/BLIS/RAYON 스레드 변수와 RUSTFLAGS는 미설정이며 스레드를 1로 고정하지 않았다. 환경 원문은 `eighth-environment.log`, `eighth-environment-ft-local.log`에 있다.
- CABLE-A Input(8ch)의 앞 두 채널 → CABLE-A Output(2ch), WASAPI 48000 Hz stereo float32다. Rust `auto`는 exclusive 거절 후 shared auto-convert로 열었고 실제 모드를 로그에 확인했다. Python은 기존 2.x shared 정책이다. Realphones System-Wide는 이 머신에서 exclusive float32 render를 받는 유일한 엔드포인트지만 exclusive capture 장치는 없다. **완전한 duplex exclusive 측정은 불가능하므로 실행하지 않았다.** 기존 하드웨어 조사 결과이며 이번에 장치 전체를 다시 프로브하지 않았다.
- PA06과 같은 하네스를 수정 없이 사용했다. 워밍업 3회, headphones 295270프레임 5회, seven 2066890프레임 3회, 첫 전달 10회, 열거·duplex open/close 20 calls/batch를 따른다. 신호의 길이는 각각 6151.458333 / 43060.208333 ms다. 벤치의 기존 짧은 연산 반복 수까지 유지했으므로 일반 PA 템플릿의 11회 규칙으로 바꾸지 않았다.
- 성능 측정에는 trace가 없다. Wall은 준비·파일 읽기·열기·재생/캡처·drain·정지·join을 포함하고 결과 WAV 쓰기는 제외한다. 모든 비율의 Rust 분모는 **동일한 `eighth-rust-observed.log`의 전체 실행**으로 고정했다. 별도 standalone 표에서 유리한 행을 골라 쓰지 않았다. CPU는 headphones 5회의 ACK observer 결과이며 Windows 카운터는 15.625 ms 단위다.
- 무결성 시험만 12000프레임 tail, CSV trace와 독립 `sd.InputStream` 관찰자를 사용했다. 현재 빌드의 실행 파일 `target/release/deps/perf-2ad73f81dcfefbde.exe`를 직접 실행했으므로 녹음 중 cargo 빌드는 없었다. `--period`가 전달한 환경 변수와 실제 버퍼를 trace로 확인했다.
- 캡처·로그·가상환경·Python 임시 WAV는 모두 `crates/impulcifer-audio-io/tests/bench_support/` 아래에 있다. `eighth_run.py`가 TEMP/TMP/TMPDIR도 그 아래로 지정한다. 추적 후보는 작은 Python 실행기 세 파일과 이 보고서뿐이다.

### 2. 회귀 표

비율은 Python median / Rust median, 각 셀은 일반 / FT 순서다. PA06b는 `release.md` 2.4절의 마지막 M5 재측정이다. 열거·open/close·wall의 새 비율은 세 과거 측정 모두와 비교해 5% 이내다. Overhead·latency·CPU에서 5%를 넘긴 변화는 바로 아래에 따로 적었다.

#### 명시적 InputStream/OutputStream

| 연산 | PA06 | PA05b | PA06b | PA07 |
|---|---:|---:|---:|---:|
| 캐시 열거 | 75.650558 / 74.828996 | 78.547893 / 76.911877 | 77.003846 / 78.626923 | 76.643678 / 76.850575 |
| duplex open/close | 1.273951 / 1.292799 | 1.363745 / 1.245404 | 1.280979 / 1.291691 | 1.330552 / 1.268311 |
| headphones wall | 0.996738 / 0.996549 | 0.997478 / 0.997627 | 0.997030 / 0.997440 | 0.997704 / 0.997552 |
| headphones overhead | 0.660448 / 0.640820 | 0.723585 / 0.739944 | 0.668897 / 0.714574 | 0.734988 / 0.717360 |
| seven wall | 0.999532 / 0.999568 | 0.999639 / 0.999685 | 0.999621 / 0.999555 | 0.999670 / 0.999633 |
| seven overhead | 0.719489 / 0.740656 | 0.777739 / 0.805918 | 0.771701 / 0.732015 | 0.787922 / 0.764235 |
| 첫 샘플 전달 | 0.869886 / 0.890895 | 0.764189 / 0.711882 | 0.809602 / 0.845734 | 0.689251 / 0.707475 |
| CPU | 2.333333 / 2.000000 | 3.000000 / 2.500000 | 2.000000 / 1.666667 | 1.500000 / 2.500000 |

#### 실제 core.recorder.play_and_record

PA06 production 비율은 `release.md` 5.2절의 정확한 분수로 계산했다.

| 연산 | PA06 | PA05b | PA06b | PA07 |
|---|---:|---:|---:|---:|
| 캐시 열거 | 2.0007÷0.0269 / 2.0817÷0.0269 | 77.068966 / 77.486590 | 77.100000 / 77.957692 | 75.693487 / 79.532567 |
| duplex open/close | 230.2767÷179.0243 / 233.1205÷179.0243 | 1.273735 / 1.338634 | 1.267459 / 1.302388 | 1.268761 / 1.274920 |
| headphones wall | 6202.1309÷6211.1332 / 6204.2565÷6211.1332 | 0.998808 / 0.999423 | 0.999481 / 0.999594 | 0.999919 / 0.999473 |
| headphones overhead | 50.672567÷59.674867 / 52.798167÷59.674867 | 0.869391 / 0.936721 | 0.942106 / 0.954757 | 0.990676 / 0.939130 |
| seven wall | 43129.7026÷43132.0996 / 43128.7581÷43132.0996 | 1.000164 / 1.000020 | 0.999955 / 1.000210 | 1.000082 / 1.000088 |
| seven overhead | 69.494267÷71.891267 / 68.549767÷71.891267 | 1.100785 / 1.012603 | 0.972617 / 1.126174 | 1.053016 / 1.056558 |
| 첫 샘플 전달 | 34.4411÷25.69865 / 36.2067÷25.69865 | 0.739952 / 1.131744 | 1.270103 / 1.265757 | 1.119690 / 1.058512 |
| CPU | 62.5÷46.875 / 93.75÷46.875 | 2.500000 / 3.000000 | 2.333333 / 2.000000 | 3.000000 / 2.500000 |

첫 샘플 행은 Rust 입력 start 호출부터 첫 nonempty application read 반환, Python 입력 start부터 첫 callback이다. Production 표에서도 별도 짧은 입력 시험이며 소스 신호의 실제 도착 지연이 아니다. Production convenience-stream의 기존 무결성 제약은 해소했다고 주장하지 않는다.

#### 5%를 넘긴 비율 변화와 원인 검토

아래 백분율은 `(PA07 비율 / 과거 비율 - 1) × 100`이다. 적지 않은 조합은 5% 이내다. 원인은 같은 커밋의 반복 측정과 코드 확인으로 추정했으며 과거 커밋을 다시 빌드한 인과 검증은 아니다.

| 방식·연산 | 5% 초과 변화 | 원인 검토 |
|---|---|---|
| 명시적 headphones overhead | PA06 일반 +11.29%, FT +11.94%; PA06b 일반 +9.88% | Rust overhead가 PA06 59.674867 → 이번 53.753867 ms로 줄었다. 고정 재생 길이를 빼고 남은 수십 ms에서 시작·drain의 엔진 주기 차이가 비율에 크게 반영된다. 정책 분기 추가 때문에 빨라졌다는 근거는 없다. |
| 명시적 seven overhead | PA06 일반 +9.51%; PA05b FT -5.17% | PA06보다 Rust overhead는 줄었고, PA05b FT Python overhead도 56.491667 → 51.280767 ms로 줄어 비교 기준에 따라 부호가 다르다. 시작·정지 시점 변동을 우선 의심한다. |
| 명시적 첫 전달 | PA06 일반 -20.77%, FT -20.59%; PA05b 일반 -9.81%; PA06b 일반 -14.87%, FT -16.35% | Rust는 PA06 25.698650 → 31.633750 ms, Python은 이번 21.803600 / 22.380100 ms다. Rust 원시 10회가 17.4817~34.5929 ms에 분포하며 엔진 이벤트와 시작 위상에 따라 약 한 주기 차이가 난다. share preference는 start 타이머보다 앞에서 처리하므로 새 분기의 직접 실행 시간이 이 지연은 아니다. |
| 명시적 CPU | PA06 일반 -35.71%, FT +25.00%; PA05b 일반 -50.00%; PA06b 일반 -25.00%, FT +50.00% | Rust 중앙값은 PA05b와 같은 31.25 ms, PA06b 46.875 ms보다 한 틱 적다. 일반 Python은 PA05b 93.75 → 46.875 ms, FT는 78.125 ms로 같다. 15.625 ms 카운터와 작은 표본에서 비율이 크게 변하며 계산량 회귀라고 단정하지 않는다. |
| Production headphones overhead | PA06 일반 +16.67%, FT +6.14%; PA05b 일반 +13.95%; PA06b 일반 +5.16% | 같은 Rust 분모 감소에 더해 Python 일반 overhead가 PA05b 49.249167 → 53.252667 ms로 늘었다. 실제 녹음 함수의 시작·정지 스케줄 차이가 우선 의심된다. |
| Production seven overhead | PA06 일반 +8.93%, FT +10.81%; PA06b 일반 +8.27%, FT -6.18% | 이번 Python 70.658167 / 70.895867 ms 대 Rust 67.100767 ms다. PA06b FT의 80.693567 ms보다 Python 시간이 줄었으므로 FT 비율은 감소했다. 긴 재생 시간을 뺀 잔여 시간의 변동이며 새로운 DSP 작업은 없다. |
| Production 첫 전달 | PA06 일반 -16.45%, FT -24.87%; PA05b 일반 +51.32%, FT -6.47%; PA06b 일반 -11.84%, FT -16.37% | Python 일반은 PA05b 22.965950 → 35.420000 ms다. 명시적 방식의 약 22 ms와도 다르고 한 엔진 주기를 넘는 차이가 있으므로 start/callback 시점 차이를 우선 의심한다. 하네스 정의상 production 신호 도착 지연이나 share 분기 비용으로 단정할 수 없다. |
| Production CPU | PA06 일반 +125.00%, FT +25.00%; PA05b 일반 +20.00%, FT -16.67%; PA06b 일반 +28.57%, FT +25.00% | 이번 Python 93.75 / 78.125 ms, Rust 31.25 ms다. 위와 같은 카운터 단위와 작은 표본의 제한이 있으며 FT 환경을 새로 설치한 차이도 남아 있다. |

먼저 지정된 정책 코드를 확인했다. `policy.rs`의 `open_*_with_preference`는 `Auto`에서 기존 `open_*_with_policy`로 전달한다. `session.rs`는 이 함수를 입력·출력 세션을 열 때 호출하며 패킷마다 호출하지 않는다. `cached_backend.rs`의 `selectable_share_modes`는 내부 백엔드로 단순 전달한다. **기존 duplex open/close 벤치는 여전히 `open_*_with_policy`를 직접 호출하므로 새 preference 분기를 측정하지 않는다.** Sweep 세션은 새 preference 함수를 거친다. 이번 open/close 비율의 변화는 세 과거 결과 모두 5% 이내이며, 정책 변경을 회귀 원인으로 확정할 근거는 없다. 같은 소스의 standalone/observer open 중앙값도 176.1741 / 187.9374 ms로 달랐다. 고정 shared/exclusive 동작은 실기 테스트로 확인했지만 별도의 속도 표는 만들지 않았다.

1.5 미만인 wall·overhead·첫 전달은 오디오 엔진에 맞춰 기다리는 시간과 stream 열기·drain 비용을 포함한다. 재생 길이 자체는 바꿀 수 없고, 작은 버퍼는 아래 무결성 시험에서 실패했다. 이번에는 코드를 최적화하거나 trace 대기 시간을 임의로 줄이지 않았다.

#### sys-win fresh COM 열거

| 값 | PA06 | PA05b | PA06b | PA07 |
|---|---:|---:|---:|---:|
| Rust median, 20 calls/batch, ms | 269.343700 | 272.007700 | 267.252500 | 220.397000 |
| Python 명시적 일반/FT 캐시 ÷ Rust fresh COM | 0.007555 / 0.007473 | 2.0501÷272.0077 / 2.0074÷272.0077 | 2.0021÷267.2525 / 2.0443÷267.2525 | 2.0004÷220.397 / 2.0058÷220.397 |

Rust fresh COM 시간은 PA05b보다 18.97%, PA06b보다 17.53% 줄었다. 이 분수도 5% 넘게 바뀌지만 **PortAudio 캐시와 fresh COM 열거는 다른 작업이므로 성능 통과 비율로 쓰지 않는다.** 엔드포인트 열거와 COM 상태·시스템 타이밍 차이를 먼저 의심한다. share preference를 실행하지 않는 연산이며, 정확한 원인은 재현 시험으로 분리하지 않았다.

### 3. 주기별 무결성

D는 첫 패킷을 제외한 discontinuity 횟수, G는 packet index gap의 프레임 합계다. 각 행은 10회 합계다. 모든 실행의 SILENT 패킷 수는 0, 독립 observer callback status는 빈 목록이었다. 그것만으로 무결성을 인정하지 않는다. 원본 전체를 고정 stereo offset에서 gain fit 없이 최대 오차 4e-6 이하로 비교하고, 두 녹음 일치·카운터 0·손실/반복 없음·분석 확정을 모두 요구한 기존 `strict_clean`을 그대로 사용했다. 기존 `accepted=true`는 공통 손실도 허용하므로 승인 근거가 아니다.

| 주기 | 실제 버퍼 | 신호 / run | underrun | D | G | 엄격한 동시 관찰 통과 |
|---:|---:|---|---:|---:|---:|---:|
| 2 | 1056 | headphones 300~309 | 15 | 2 | 768 | 0/10 |
| 2 | 1056 | seven 300~309 | 12 | 3 | 1152 | 0/10 |
| 3 | 1440 | headphones 310~319 | 16 | 0 | 0 | 0/10 |
| 3 | 1440 | seven 310~319 | 14 | 2 | 0 | 0/10 |
| 4 | 1920 | headphones 320~329 | 0 | 0 | 0 | 7/10 |
| 4 | 1920 | seven 320~329 | 0 | 0 | 0 | 3/10 |

독립 Python 녹음의 원본 전체 비교만 통과한 수는 차례로 0/10, 0/10, 5/10, 5/10, 7/10, 3/10이다. 3주기 일부는 파형이 맞아도 underrun 카운터가 0이 아니므로 탈락이다. 60회 명령은 모두 종료 코드 0으로 끝났지만 이는 **측정과 분석이 종료됐다는 뜻**이며 무결성 통과를 뜻하지 않는다. 분석 예외를 보존하는 기존 하네스 동작도 그대로다.

실패를 특정할 수 있도록 실행별 카운터를 남긴다. U의 열은 run 오름차순이다.

| 주기·작업 | run | U 순서 | D/G가 0이 아닌 run |
|---|---|---|---|
| 2 headphones | 300~309 | 1,1,2,1,1,2,2,2,2,1 | 302에서 D=2, G=768 |
| 2 seven | 300~309 | 1,1,1,2,1,1,1,2,1,1 | 302에서 D=3, G=1152 |
| 3 headphones | 310~319 | 2,2,1,1,1,2,2,2,2,1 | 없음 |
| 3 seven | 310~319 | 1,2,2,1,1,2,2,1,1,1 | 314에서 D=2, G=0 |
| 4 두 작업 | 320~329 | 모두 0 | 없음 |

2주기 300부터 두 작업 모두 초기 `[672,1056)`의 384프레임이 두 녹음에 공통으로 0이 됐다. 다른 run도 각 분석 JSON에 zero·missing 영역과 원본 최대 오차를 보존했다. 3주기 headphones 310·311·317·318은 초기 zero 영역, 316은 `[29280,29760)` 공통 480프레임 손실이 있다. Seven 310·311·315·316에는 각각 공통 480프레임 손실이 있고, 314는 `operands could not be broadcast together with shapes (0,) (22359,)`로 분석을 확정하지 못했다. 이 실패들을 재시도로 바꾸지 않았다.

4주기의 실패 10회는 다음과 같다.

| 작업·run | 실패한 검증 |
|---|---|
| headphones 322 | Rust 원본 비교는 통과, Python 녹음만 `[111,464)` zero 후보. Pair 불일치, 미확정이며 Python 최대 오차 0.0091330. |
| headphones 327 | Pair 불일치와 공통 비교 길이 부족. Python에 768프레임 missing 후보, Rust에 초기 zero 후보가 있어 미확정. |
| headphones 328 | 두 녹음이 비트 단위로 같지만 `[72960,73440)`의 공통 480프레임 손실. |
| seven 320 | `operands could not be broadcast together with shapes (0,) (23079,)`로 분석 실패. Pair도 불일치이므로 통과로 세지 않음. |
| seven 321 | `[224160,224640)` 공통 480프레임 손실. |
| seven 324 | Pair는 같지만 원본 정렬의 두 채널 lag가 7728/7248로 달라 단일 stereo offset 검증 실패. 미확정. |
| seven 326 | Rust 원본 비교는 통과, Python에 384프레임 missing 후보 3곳. Pair 불일치, 미확정. |
| seven 327 | `[1587360,1587840)` 공통 480프레임 손실. |
| seven 328 | `[408000,408480)` 공통 480프레임 손실. |
| seven 329 | `[37440,37920)` 공통 480프레임 손실. |

공통 손실 시험에서는 source 전체의 연속 render 제출과 payload checksum 검증을 통과했고 두 독립 녹음이 같았다. 이때 손실을 Rust 캡처만의 결함으로 단정할 수 없다. Windows shared engine과 VB-Cable 중 어디서 생겼는지, 관찰자 자체에 왜 누락이 있었는지는 이번에 해결하지 못했다. PA05b 최종 6/10·8/10 대비 이번 7/10·3/10을 그대로 기록한다. Seven의 엄격한 통과 수 감소는 확인했지만 이 작은 표본만으로 alpha.2 코드 회귀라고 확정하지 않는다.

60개 개별 분석의 요약은 `eighth-evidence.json`, 원문 표는 `eighth-summary.log`다. 각 캡처·trace·observer metadata는 `sixth-p{2|3|4}-{run}-{headphones|seven}-*`이며 어떤 실패 자료도 삭제하거나 덮어쓰지 않았다.

### 4. 기본값 판정과 변경 파일

`SHARED_BUFFER_PERIODS`는 **4**를 유지한다. 2·3주기가 두 작업 모두 10/10을 충족하지 못했으므로 기본값을 변경하지 않는다. `IMPULCIFER_PA05_SHARED_PERIODS=2|3|4`도 그대로이며 4주기는 계속 선택할 수 있다. 기본값 변경 후의 회귀 표 재측정은 조건이 성립하지 않아 실행하지 않았다. `HARDWARE.md`도 변경 조건에 해당하지 않아 수정하지 않았다.

Rust 코드·상수·공개 API·의존성·Cargo.lock·Python production 트리·features.toml·CHANGELOG.md는 그대로다. 직접 작성한 Rust unsafe는 0이며 수치 계산·골든 허용 오차도 바꾸지 않았다. 변경 파일은 이 보고서와 `eighth_run.py`(동기 실행/로그/임시 폴더), `eighth_series.py`(유한한 순차 실행), `eighth_summary.py`(기존 결과 집계)뿐이다. 기존 측정·무결성 하네스의 판정은 변경하지 않았다.

### 5. 명령과 검증 원문

작업 디렉터리는 `E:/Impulcifer`다. 모든 실제 하위 명령 argv, 시작/종료 시각, 전체 stdout/stderr, 종료 코드는 `eighth-<name>.log`에 있다. `eighth_run.py <name> <command...>`는 명령을 동기 실행하고 종료 뒤 출력한다. Python 임시 파일을 측정 폴더로 제한하는 것 외에 benchmark 환경 변수는 설정하지 않으며, 기존 `IMPULCIFER_PA05*` 또는 CI 변수가 있으면 거부한다. 프로세스 목록 조회나 종료 함수는 없다.

검증·성능 측정은 다음 명령을 각각 위 logger로 감싸 실행했다. FT는 명시된 대체 venv를 사용했다.

```powershell
cargo fmt --all -- --check
cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-service --test recording -- --ignored recording_virtual_cable_end_to_end
cargo test -p impulcifer-audio-io --test hardware -- --ignored
cargo test -p impulcifer-policy
cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --observe-rust cargo bench -p impulcifer-audio-io --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
& E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth-py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
& E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth-py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
```

무결성은 다음 템플릿에서 P=2/R=300..309, P=3/R=310..319, P=4/R=320..329, OP는 두 작업을 모두 실행했다. 실제 실행 순서는 각 run의 headphones 다음 seven이다. 2주기 300은 직접 logger로 두 번 호출했고 나머지는 아래 순차 실행기를 썼다.

```powershell
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/sixth_paired.py --period <P> --run <R> --op <play_record_headphones_sweep|play_record_7_speaker_set> --exe E:/Impulcifer/target/release/deps/perf-2ad73f81dcfefbde.exe
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth_series.py --period 2 --first 301 --last 309 --exe E:/Impulcifer/target/release/deps/perf-2ad73f81dcfefbde.exe
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth_series.py --period 3 --first 310 --last 314 --exe E:/Impulcifer/target/release/deps/perf-2ad73f81dcfefbde.exe
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth_series.py --period 3 --first 315 --last 319 --exe E:/Impulcifer/target/release/deps/perf-2ad73f81dcfefbde.exe
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth_series.py --period 4 --first 320 --last 324 --exe E:/Impulcifer/target/release/deps/perf-2ad73f81dcfefbde.exe
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth_series.py --period 4 --first 325 --last 329 --exe E:/Impulcifer/target/release/deps/perf-2ad73f81dcfefbde.exe
```

가상환경을 만들고 환경 정보를 수집할 때 실행한 명령은 다음과 같다.

```powershell
py -0p
uv python list --only-installed
uv venv --python C:/Users/32170336/AppData/Roaming/uv/python/cpython-3.14+freethreaded-windows-x86_64-none/python.exe E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth-py314t
uv pip install --python E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth-py314t/Scripts/python.exe -r E:/Impulcifer/requirements.txt numpy==2.5.3 scipy==1.18.1 sounddevice==0.5.6 psutil
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/seventh_environment.py
& E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth-py314t/Scripts/python.exe E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/seventh_environment.py
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/eighth_summary.py
git status --porcelain
```

실행된 테스트 묶음의 `test result:` 원문이다. 0개 실행한 doc-test 행도 포함했다.

```text
# eighth-tests.log
 test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
 test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
 test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
# eighth-recording.log
 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 16 filtered out; finished in 23.31s
# eighth-hardware.log
 test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.33s
# eighth-policy.log
 test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.64s
 test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

`bench_smoke_impulcifer_audio_io`, `bench_smoke_impulcifer_sys_win`, 고정 share preference 실기 및 service 녹음 실기를 통과했다. fmt·clippy와 위 벤치/오라클도 종료 코드 0이다. 커밋·푸시·CI는 요청하지 않아 실행하지 않았다.

### 6. 측정 표 원문

#### Rust, CPU observer (`eighth-rust-observed.log`)

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.026100 | 0.025600 |
| open_close_session | 20 calls/batch | 187.937400 | 183.092400 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6205.212200 | 6204.213300 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 53.753867 | 52.754967 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43127.309100 | 43126.145700 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 67.100767 | 65.937367 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 31.633750 | 17.481700 |
| capture_loop_cpu | process user+kernel; 5 runs | 31.250000 | 15.625000 |

#### Rust, standalone (`eighth-rust-standalone.log`)

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.028000 | 0.027800 |
| open_close_session | 20 calls/batch | 176.174100 | 173.262500 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6206.191100 | 6198.755200 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 54.732767 | 47.296867 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43126.163500 | 43119.080800 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 65.955167 | 58.872467 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 31.818950 | 19.966700 |

#### sys-win (`eighth-sys.log`)

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 220.397000 | 218.171000 |

#### Python 3.14.5, 명시적 (`eighth-python-explicit.log`)

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.000400 | 1.954800 |
| open_close_session | 20 calls/batch | 250.060400 | 237.875300 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6190.966800 | 6188.602900 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 39.508467 | 37.144567 |
| capture_loop_cpu | process user+kernel; 5 runs | 46.875000 | 31.250000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43113.078500 | 43111.451200 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 52.870167 | 51.242867 |
| first_sample_latency | input start to nonempty callback; 10 runs | 21.803600 | 13.301500 |

#### Python 3.14.7t, 명시적 (`eighth-python-ft-explicit.log`)

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.005800 | 1.994700 |
| open_close_session | 20 calls/batch | 238.363000 | 233.724100 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6190.019200 | 6189.548900 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 38.560867 | 38.090567 |
| capture_loop_cpu | process user+kernel; 5 runs | 78.125000 | 46.875000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43111.489100 | 43102.685000 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 51.280767 | 42.476667 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.380100 | 19.735100 |

#### Python 3.14.5, production (`eighth-python-production.log`)

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 1.975600 | 1.935100 |
| open_close_session | 20 calls/batch | 238.447700 | 235.873200 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6204.711000 | 6201.858400 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 53.252667 | 50.400067 |
| capture_loop_cpu | process user+kernel; 5 runs | 93.750000 | 46.875000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43130.866500 | 43126.417700 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 70.658167 | 66.209367 |
| first_sample_latency | input start to nonempty callback; 10 runs | 35.420000 | 22.123000 |

#### Python 3.14.7t, production (`eighth-python-ft-production.log`)

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.075800 | 2.066500 |
| open_close_session | 20 calls/batch | 239.605100 | 236.782800 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6201.940200 | 6193.544100 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 50.481867 | 42.085767 |
| capture_loop_cpu | process user+kernel; 5 runs | 78.125000 | 46.875000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43131.104200 | 43128.296600 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 70.895867 | 68.088267 |
| first_sample_latency | input start to nonempty callback; 10 runs | 33.484700 | 21.693500 |

최종 `git status --porcelain` 원문이다.

```text
 M docs/rust/perf/impulcifer-audio-io.md
?? crates/impulcifer-audio-io/tests/bench_support/eighth_run.py
?? crates/impulcifer-audio-io/tests/bench_support/eighth_series.py
?? crates/impulcifer-audio-io/tests/bench_support/eighth_summary.py
```

하네스 세 파일은 각각 1970 / 1320 / 3677바이트다. 캡처와 trace를 포함한 이번 trial 자료 1,297,451,232바이트는 git에서 무시되며 위 상태에 나타나지 않는다. 별도 대조에서 일곱 로그의 원문 표 55행이 보고서에 모두 있는지, 중복 없는 60회와 각 명령 종료 코드 0, Python 세 파일의 구문을 확인했다. `git diff --check`와 설치된 `ruff check`는 통과했다. 먼저 시도한 `py -3.14 -m ruff`는 해당 Python에 ruff 모듈이 없어 실패했으므로 이를 통과로 기록하지 않는다.

### 7. 미완료·제한

지정된 기존 FT venv에서의 실행은 디렉터리 부재로 수행하지 못했으며 같은 3.14.7t와 핵심 라이브러리 버전의 대체 환경으로 완료했다. 완전한 exclusive duplex 실기는 장치가 없어 미실행이다. 작은 버퍼 승인은 실패했으며 4주기도 엄격한 10/10은 실패했다. 공통 손실·관찰자 불일치·분석 미확정의 원인을 해결하지 않았고, 성능 표의 1.0 미달 연산을 최적화하지 않았다. 이들은 재측정 결과와 다음 조사 대상으로 기록하며 통과로 처리하지 않는다.

## Seventh run (PA05b), 2026-09-08: audit NOT PASSED

MMCSS 승격을 추가했지만 작은 버퍼는 무결성 기준을 통과하지 못했다. 기본 버퍼는 **4주기, 1920프레임, 40 ms를 유지**한다. 4주기도 엄격한 독립 동시 관찰에서 10/10 통과하지 못했으므로 승인된 설정이라고 주장하지 않는다. 명시적 Python 스트림에 대한 wall time, overhead, 첫 샘플 전달 비율은 두 인터프리터 모두 1.0 미만이다. PA05의 0.2% 예외를 적용하지 않았다. M5 미통과이며 `features.toml`은 변경하지 않았다.

### 1. 환경과 절차 위반

- Intel Core i5-12600KF, 물리 10코어 / 논리 16코어(기존 PA06 하드웨어 기록), Windows 11 Education 10.0.22621, x86-64. Rust/Cargo 1.97.0, MSVC, release/bench 프로필.
- 일반 CPython 3.14.5: `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`.
- CPython 3.14.7 free-threaded, GIL 비활성: `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`.
- 두 환경 모두 NumPy 2.5.3, SciPy 1.18.1, sounddevice 0.5.6, PortAudio V19.7.0-devel. NumPy OpenBLAS ILP64/pocketfft, SciPy OpenBLAS LP64/duccfft. 환경 원문은 `seventh-environment.log`, `seventh-environment-ft.log`에 있다. OMP/MKL/OPENBLAS/NUMEXPR 스레드 변수는 미설정이다.
- CABLE-A Input → CABLE-A Output, WASAPI, 48000 Hz, stereo float32. Exclusive 거절 후 기존 shared auto-convert 폴백을 측정했다. f32 전송·골든·공개 시그니처는 그대로다.
- **caller kept machine quiet.** 다른 워커나 빌드를 동시에 실행하지 않았고 모든 명령은 포그라운드에서 종료했다. 단, 워커가 기존 Python 오라클의 `--environment-only`를 호출하면서 **금지된 프로세스 조회를 한 번 실행했다.** 프로세스를 종료하거나 종료를 요청하지 않았다. 따라서 요청한 `other processes were not queried` 문구는 사실과 달라 사용할 수 없다. 이후 환경 수집에는 프로세스를 조회하지 않는 `seventh_environment.py`를 사용했다.
- 모든 측정 자료와 로그는 `crates/impulcifer-audio-io/tests/bench_support/` 안에 있으며 git에서 무시한다. 아래 파일명은 이 디렉터리를 기준으로 한다. 캡처는 추적 파일에 추가하지 않았다.

워밍업 3회, headphones 5회, seven-segment 3회, 첫 전달 10회, 열거·open/close는 20 calls/batch를 유지했다. Headphones는 295270프레임(6.151458333333초), seven-segment는 2066890프레임(43.060208333333초)이다. 진단 캡처는 끝부분 검증을 위해 12000프레임을 추가한다. Wall에는 읽기·준비·해석·세션 열기·재생/캡처·정지·해제·join을 포함하고 출력 파일 쓰기는 제외한다. 성능 측정에서는 trace를 끈다. CPU는 ACK observer로 같은 headphones 구간의 Rust user+kernel을 측정하며 Windows의 15.625 ms 카운터 단위를 유지한다.

### 2. 구현과 API 확인

Windows 전용 의존성 `audio_thread_priority = { version = "0.37.0", default-features = false }`를 추가했다. 기본 `dbus` 기능을 끄며 Windows 의존성에 D-Bus를 추가하지 않는다. Cargo가 생성한 lock 변경은 새 의존성과 그 전이 의존성 `mach2 0.4.3`, 기존 mach2 버전 구분뿐이다.

확인한 공개 함수는 다음과 같다.

```rust
pub fn promote_current_thread_to_real_time(
    audio_buffer_frames: u32,
    audio_samplerate_hz: u32,
) -> Result<RtPriorityHandle, AudioThreadPriorityError>
// demote_current_thread_from_real_time consumes RtPriorityHandle
// and returns Result<(), AudioThreadPriorityError>.
```

[공개 API](https://docs.rs/audio_thread_priority/0.37.0/audio_thread_priority/), [승격 함수](https://docs.rs/audio_thread_priority/0.37.0/audio_thread_priority/fn.promote_current_thread_to_real_time.html), [Windows 구현](https://docs.rs/crate/audio_thread_priority/0.37.0/source/src/rt_win.rs)을 확인했다. **문서 설명은 Pro Audio지만 0.37.0의 Windows 구현은 `Audio` 작업을 등록한다.** 버퍼·샘플레이트 인자는 Windows 구현에서 사용하지 않는다. 별도 `AvSetMmThreadPriority` 호출도 없다. 이번 결과는 Pro Audio 승격 실험이 아니다. 허용 범위를 벗어나 의존성을 패치하거나 직접 Win32 unsafe를 추가하지 않았다.

`RealtimeGuard`는 세션이 소유하는 `Rc`와 thread-local `Weak`를 사용한다. 같은 스레드의 두 세션은 한 등록을 공유하며 마지막 소유자가 사라질 때 같은 스레드에서 해제한다. 승격 실패는 세션 열기 오류로 반환한다. 해제 실패는 trace와 stderr에 남긴다. 오류·unwind·중복 세션 수명 테스트 2개를 추가했고 성공한 80회 진단의 render/capture trace에서 승격 160회와 해제 성공 160회를 확인했다. `Rc`의 비-Send 성질로 스레드 간 이동을 막는다.

기존 하네스 `--period`는 컴파일된 설정만 검사했다. 반복 측정을 위해 비공개 환경 변수 `IMPULCIFER_PA05_SHARED_PERIODS=2|3|4`를 추가했고 미설정 시 4를 사용한다. 다른 값은 오류다. 공개 Rust API는 추가하지 않았다. `sixth_paired.py`는 headphones 선택, 신호별 파일 이름, 각 동시 녹음 전체를 한 고정 오프셋에서 원본과 비교하는 판정을 추가했다. 이 검사는 gain fit 없이 두 채널 최대 오차 4e-6 이하를 요구한다(기존 CABLE-A 양자화 기준). 골든 허용 오차를 바꾼 것이 아니다. 공통 외부 손실을 허용하던 `accepted=true`만으로 통과시키지 않는다.

직접 작성한 **unsafe는 0**이며 `#![forbid(unsafe_code)]`와 `unsafe-budget.toml`은 그대로다. 의존성 내부 Win32 호출은 해당 크레이트가 구현한다. `HARDWARE.md`는 기본 버퍼가 바뀌지 않아 수정하지 않았다. CHANGELOG·서비스·Python 오라클 및 production 트리는 변경하지 않았다.

### 3. 주기별 무결성

아래 카운터는 각 10회의 합계다. D는 첫 패킷을 제외한 discontinuity 수, G는 packet index gap 프레임 수다. 모든 그룹의 SILENT 패킷 수는 0이며 독립 Python observer의 callback status 목록은 비어 있었다. **빈 status 목록은 파형 무결성을 보장하지 않는다.** 통과 수는 원본과 두 동시 녹음의 온전한 일치, 카운터 0, 누락·반복·미분류 구간 없음까지 요구한다.

| 주기 | 실제 버퍼 | 신호 / run | underrun | D | G | 엄격한 동시 관찰 통과 |
|---:|---:|---|---:|---:|---:|---:|
| 2 | 1056 | headphones 150~159 | 16 | 2 | 768 | 0/10 |
| 2 | 1056 | seven 160~169 | 16 | 8 | 3072 | 0/10 |
| 3 | 1440 | headphones 150~159 | 13 | 1 | 0 | 0/10 |
| 3 | 1440 | seven 160~169 | 14 | 0 | 0 | 0/10 |
| 4 | 1920 | headphones 150~159 | 0 | 5 | 0 | 6/10 |
| 4 | 1920 | seven 160~169 | 0 | 0 | 0 | 7/10 |
| 4, 최종 | 1920 | headphones 200~209 | 0 | 1 | 0 | 6/10 |
| 4, 최종 | 1920 | seven 200~209 | 0 | 0 | 0 | 8/10 |

최종 headphones 202·203에는 각각 공통 손실 480프레임이 있고 200·204는 분석을 확정하지 못했다. 최종 seven 206에는 공통 손실 353프레임이 있으며 205는 미확정이다. 손실의 원인이 Windows 엔진인지 VB-Cable인지 확정하지 못했다. 4주기를 유지하는 결정은 작은 버퍼를 승인하지 않았다는 뜻이며 4주기의 무손실 보장이 아니다.

80회 개별 행, 공통 손실·분석 확정 여부, 소스 최대 오차는 `seventh-evidence.log`와 `seventh-evidence.json`에 보존했다. 각 원본은 `sixth-p{period}-{run}-{headphones|seven}-*`다. 2주기 seven 150~154를 처음 시도했을 때 기존 산출물 충돌로 **종료 코드 1**을 반환했다. 재생 전 덮어쓰기 방지 검사에서 중단됐으며 로그를 삭제하지 않았다. 새 번호 160~169로 측정했다. 이 다섯 명령을 성공으로 세지 않는다. 최종 200~209는 별도 검증이며 이전 실패를 교체하지 않는다.

### 4. PA06 전후 비율

비율은 Python median / Rust median이며 일반 / FT 순서다. 이후 비율의 Rust 분모는 CPU ACK observer를 붙인 전체 실행 `seventh-rust-observed.log`로 고정했다. 별도 standalone 실행도 아래에 보존하며 유리한 행만 골라 섞지 않는다. Open/close는 기존 **duplex `sd.Stream`** 비교다.

| 연산 | PA06 일반 / FT | 이후 명시적 스트림 일반 / FT | 판정 |
|---|---:|---:|---|
| 캐시 열거 | 75.650558 / 74.828996 | 78.547893 / 76.911877 | 통과 |
| duplex open/close | 1.273951 / 1.292799 | 1.363745 / 1.245404 | 통과 |
| headphones wall | 0.996738 / 0.996549 | 0.997478 / 0.997627 | 미달 |
| headphones overhead | 0.660448 / 0.640820 | 0.723585 / 0.739944 | 미달 |
| seven wall | 0.999532 / 0.999568 | 0.999639 / 0.999685 | 미달 |
| seven overhead | 0.719489 / 0.740656 | 0.777739 / 0.805918 | 미달 |
| 첫 샘플 전달 | 0.869886 / 0.890895 | 0.764189 / 0.711882 | 미달 |
| CPU | 2.333333 / 2.000000 | 3.000000 / 2.500000 | 통과, 카운터 정밀도 제한 |

실제 `core.recorder.play_and_record`를 쓰는 production 실행도 두 인터프리터로 완료했다. 첫 샘플 행은 그 실행에서도 별도 입력 시작→첫 callback 측정이다. production 재생 자체의 첫 소스 도착 지연으로 해석하지 않는다. 기존 convenience-stream 특성과 파형 무결성은 위 sixth-run의 제한이 계속 적용된다.

| 연산 | 이후 production 일반 / FT |
|---|---:|
| 캐시 열거 | 77.068966 / 77.486590 |
| duplex open/close | 1.273735 / 1.338634 |
| headphones wall | 0.998808 / 0.999423 |
| headphones overhead | 0.869391 / 0.936721 |
| seven wall | 1.000164 / 1.000020 |
| seven overhead | 1.100785 / 1.012603 |
| 첫 샘플 전달 | 0.739952 / 1.131744 |
| CPU | 2.500000 / 3.000000 |

#### Rust 원문 표 (CPU observer 포함)

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.026100 | 0.026000 |
| open_close_session | 20 calls/batch | 178.484200 | 175.320900 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6208.106200 | 6204.961300 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 56.647867 | 53.502967 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43130.304400 | 43129.595200 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 70.096067 | 69.386867 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 31.037100 | 20.105700 |
| capture_loop_cpu | process user+kernel; 5 runs | 31.250000 | 15.625000 |

#### Rust 원문 표 (standalone)

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.026800 | 0.026100 |
| open_close_session | 20 calls/batch | 176.419600 | 175.520600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6208.774500 | 6206.400700 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 57.316167 | 54.942367 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43128.979100 | 43119.491200 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 68.770767 | 59.282867 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 31.175650 | 19.893100 |

sys-win fresh COM 열거는 20 calls/batch 중앙값 **272.007700 ms**, 최소 **270.334700 ms**다. Python PortAudio의 캐시 조회와 작업이 다르므로 같은 연산의 비율로 판정하지 않는다.

#### Python 3.14.5 원문 표 (명시적 스트림)

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.050100 | 2.025900 |
| open_close_session | 20 calls/batch | 243.406900 | 241.085100 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6192.447900 | 6188.740800 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 40.989567 | 37.282467 |
| capture_loop_cpu | process user+kernel; 5 runs | 93.750000 | 78.125000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43114.724800 | 43105.355600 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 54.516467 | 45.147267 |
| first_sample_latency | input start to nonempty callback; 10 runs | 23.718200 | 17.174400 |

#### Python 3.14.7t 원문 표 (명시적 스트림)

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.007400 | 1.983300 |
| open_close_session | 20 calls/batch | 222.284900 | 220.148400 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6193.374600 | 6189.124300 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 41.916267 | 37.665967 |
| capture_loop_cpu | process user+kernel; 5 runs | 78.125000 | 31.250000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43116.700000 | 43112.304700 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 56.491667 | 52.096367 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.094750 | 13.434700 |

#### Python 3.14.5 원문 표 (production)

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.011500 | 1.995500 |
| open_close_session | 20 calls/batch | 227.341600 | 226.420600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6200.707500 | 6199.466500 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 49.249167 | 48.008167 |
| capture_loop_cpu | process user+kernel; 5 runs | 78.125000 | 62.500000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43137.369000 | 43128.033400 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 77.160667 | 67.825067 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.965950 | 20.969800 |

#### Python 3.14.7t 원문 표 (production)

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.022400 | 2.018000 |
| open_close_session | 20 calls/batch | 238.925100 | 228.275300 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6204.521600 | 6201.772300 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 53.063267 | 50.313967 |
| capture_loop_cpu | process user+kernel; 5 runs | 93.750000 | 62.500000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43131.187800 | 43128.724200 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 70.979467 | 68.515867 |
| first_sample_latency | input start to nonempty callback; 10 runs | 35.126050 | 23.390700 |

### 5. 미달 연산의 프로파일과 남은 작업

- 승격/해제 중앙값은 각각 **106.5 / 9 μs**였다. `initialize_client` 중앙값 **7296.5 μs**, enumerator **899 μs**이며 두 세션은 동시에 열기 때문에 이 중앙값들을 더해 duplex wall time으로 해석하면 안 된다. Open/close 비율은 1.5 미만이지만 1.0 이상이다.
- 첫 캡처 패킷→호출자 전달은 33표본 중앙값 **1 μs**, 최대 **2 μs**다. 첫 전달의 약 31 ms는 이 복사 단계의 CPU 비용 때문이라고 볼 수 없다. 기존 capture worker가 직접 애플리케이션 전달을 처리하므로 제거할 추가 전달 스레드는 없다. 입력 시작·엔진 패킷 도착·이벤트 대기에 대한 추가 조사가 필요하다.
- 첫 render buffer는 **이미 Start 전에 제출**한다. preroll→Start 중앙값은 **375.5 μs**다. 같은 최적화를 다시 추가하지 않았다.
- 최종 4주기 seven 20회에서 마지막 write→MMCSS 해제 중앙값은 **41.330 ms**(최소 40.678, 최대 42.200 ms)다. 이 구간은 drain과 정지·해제 비용을 함께 포함한다. 전부 불필요한 대기라고 단정할 수 없다. 프레임 수에 정확히 맞춘 drain 단축은 구현하지 못했다.
- 승격만으로 작은 버퍼의 underrun을 해결하지 못했다. 4주기의 공통 손실·관찰자 불일치·미확정 원인도 해결하지 못했다. `Audio`와 `Pro Audio` 차이의 영향은 측정하지 않았다.

근거는 `seventh-profile-summary.json`, `seventh-latency-profile-*.csv`, 80회 원본 trace다. 골든이나 수치 합산 순서는 바뀌지 않았으며 recording golden 테스트는 그대로 통과했다. 새 골든 최대 오차 측정이나 전체 pipeline 감사 완료를 주장하지 않는다.

### 6. 실행 명령과 검증

작업 디렉터리는 `E:/Impulcifer`다. `seventh_run.py <name> <command...>`는 해당 명령을 동기 실행하고 `seventh-<name>.log`에 argv·전체 출력·종료 코드를 보존한다. 성공 로그를 덮어쓰지 않는다. 아래 벤치·오라클·검증 명령은 모두 종료 코드 0이다. 부모 세션은 fmt·clippy·두 오디오 크레이트 테스트·service recording·CABLE-A 실기·policy를 별도로 재실행했다(`seventh-parent-*.log`).

```powershell
cargo bench -p impulcifer-audio-io --bench perf --no-run
cargo fmt --all -- --check
cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win -p impulcifer-service --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-service recording
cargo test -p impulcifer-service --test recording -- --ignored recording_virtual_cable_end_to_end
cargo test -p impulcifer-policy
cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --observe-rust cargo bench -p impulcifer-audio-io --bench perf
```

무결성 명령은 다음 템플릿의 모든 조합으로 실행했다. P=2/3/4, headphones N=150~159, seven N=160~169이며 최종 P=4는 두 신호 모두 N=200~209다. 실행 파일은 이 빌드의 해시이며 다른 빌드에서 그대로 사용하면 안 된다. `--period`는 이제 하네스가 자식에게 전달하는 환경 변수로 실제 설정한다.

```powershell
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/sixth_paired.py --period <P> --run <N> --op <play_record_headphones_sweep|play_record_7_speaker_set> --exe E:/Impulcifer/target/release/deps/perf-8808d997687b49b0.exe
py -3.14 E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/seventh_environment.py
& C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/seventh_environment.py
```

일반 환경 수집은 처음에 오라클의 `--environment-only`를 사용했다(위 프로세스 조회 위반). 위 별도 환경 스크립트는 FT 수집에 사용했다. 명령별 실제 argv는 각 로그 첫 줄에 있다. 순차 실행기 `seventh_series.py`는 부모 검토에서 실패 종료 코드를 반환하도록 수정하고 실행 파일 인자를 추가했다. 측정 코드는 바꾸지 않았으므로 재생을 다시 하거나 기존 결과를 교체하지 않았다.

부모 검증의 실행된 테스트 묶음 원문은 다음과 같다. 0개 실행된 필터 대상·doc-test 행은 로그에 보존했다.

```text
# audio-io/sys-win: 41 passed, 5 hardware ignored
 test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
 test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
 test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
# service recording filter
 test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.00s
 test result: ok. 9 passed; 0 failed; 1 ignored; 0 measured; 4 filtered out; finished in 0.67s
# CABLE-A integration
 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 23.27s
# policy
 test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.75s
```

`bench_smoke_impulcifer_audio_io`와 `bench_smoke_impulcifer_sys_win`은 통과했다. fmt·clippy도 부모 재실행에서 종료 코드 0이다. 커밋·푸시·CI 실행은 요청하지 않아 수행하지 않았다. Rust 검증 통과는 성능·무결성 기준 통과와 다르다.

최종 변경 파일은 아래 `git status --porcelain`과 같다. 대용량 산출물은 없다.

```text
 M Cargo.lock
 M crates/impulcifer-audio-io/tests/bench_support/analyze_fifth.py
 M crates/impulcifer-audio-io/tests/bench_support/sixth_paired.py
 M crates/impulcifer-sys-win/Cargo.toml
 M crates/impulcifer-sys-win/src/lib.rs
 M docs/rust/perf/impulcifer-audio-io.md
?? crates/impulcifer-audio-io/tests/bench_support/seventh_environment.py
?? crates/impulcifer-audio-io/tests/bench_support/seventh_evidence.py
?? crates/impulcifer-audio-io/tests/bench_support/seventh_run.py
?? crates/impulcifer-audio-io/tests/bench_support/seventh_series.py
```

## Sixth run, 2026-09-08: audit NOT PASSED

### Verdict

- Cached enumeration passes: Python/Rust **75.83 / 78.43**. Requested duplex open/close comparator passes: **1.283 / 1.275**.
- Seven-segment wall time passes the new 0.2% allowance: Rust is **0.028383% / 0.040157%** slower. Headphones wall time **fails**: **0.268353% / 0.245488%** slower. CPU passes: **2.0 / 2.5**.
- Keep **four shared periods, 1920 frames, 40 ms**. Three periods (1440 frames) and two requested periods (actually 1056 frames) each reported render underruns in **all ten trials**.
- Four periods: zero render underruns in ten trials. Eight trials have complete independent paired verification, including two externally lost regions totaling **912 frames**. Two further trials have **Python-observer-only deletions**, not Rust deletions; full Rust/source residual matches the clean quantization baseline. Strict contemporaneous paired coverage is missing in those observer gaps, so **10/10 paired acceptance is not claimed**.
- No safe thread-priority API was found in the allowed libraries. No priority change, unsafe, dependency, public API or registry-status change. Hardware integration and all requested local gates pass. Performance acceptance remains incomplete; passing tests is not a performance pass.

This section supersedes the fifth-run verdict below. Historical data and failed analyses are retained. This run does not establish CPU load as the cause of any loss, nor distinguish the Windows engine from VB-Cable.

### Environment and measurement contract

Same dedicated CABLE-A endpoints: output `CABLE-A Input (VB-Audio Cable A)`, input `CABLE-A Output (VB-Audio Cable A)`, WASAPI, 48000 Hz, stereo float32 transport. Exclusive is refused; memoized shared auto-convert fallback is used. CPU: Intel Core i5-12600KF (Family 6 Model 151), 10 physical/16 logical cores; Windows 11 build 22621; Rust/Cargo 1.97.0. CPython 3.14.5 at `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`; free-threaded 3.14.7 at `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`. Both have NumPy 2.5.3, SciPy 1.18.1, sounddevice 0.5.6, PortAudio V19.7.0-devel. NumPy OpenBLAS ILP64/pocketfft and SciPy OpenBLAS LP64/duccfft configuration output is retained in the sixth environment logs. OMP/MKL/OPENBLAS/NUMEXPR thread variables were unset; no affinity restriction or installation.

Observed audio-related processes: Discord 3128/13112/17004/17824/19580/20044, HLConvolverHost 71740, audiodg 113280. No user process was terminated or asked to close. Commands and agents ran foreground. Independent Python callbacks capture while a synchronous, prebuilt Rust child plays; compilation is not performed during those captures. The machine was not isolated from unrelated applications. The working tree acquired unrelated changes during this session; absence of other system activity was not independently established.

Three warmups; five measured runs/batches, except seven segments use three and first-data/integrity ten. Enumeration/open-close batches contain 20 operations. Wall spans include read/preparation, resolution, open, playback/capture, stop/drop/join; exclude file writes. CPU is user+kernel during the same five headphones runs, observed with psutil, with Windows 15.625 ms counter granularity retained. Timing traces are disabled. All final Rust timing runs report zero render underruns; their aggregate discontinuity boolean is true and does not distinguish the initial flag. Python timing callback status lists are empty. Integrity traces exclude the first discontinuity from abnormal counts.

Headphones: 295270 frames, 6.151458333333 s, bundled mono sweep duplicated to stereo. Seven alternating-channel segments: 2066890 frames, 43.060208333333 s. Timed capture length equals playback length; diagnostic capture adds 12000 frames (250 ms), yielding 2078890 frames. Full trailing-source integrity is established only by the diagnostic captures. Python timing uses explicit `InputStream`/`OutputStream`, **not** `core.recorder.play_and_record`. The unchanged 2.x COM and sounddevice global convenience-stream observations remain documented below. Open/close retains the user's separate **duplex `sd.Stream`** comparator. A worker briefly changed that comparator to two streams; parent restored duplex and uses the original sixth-final logs below. The later `sixth-explicit-open-*` logs and their ratios are retained but **not the acceptance comparator**.

### Final timing output, verbatim

Artifacts below are relative to `crates/impulcifer-audio-io/tests/bench_support/`. The raw captures (`.f32`, `.csv`, `.wav`, `.json`, logs) are not committed; the directory's `.gitignore` keeps them local and only the harness (`mod.rs`) and the analysis scripts are tracked. Primary logs are `sixth-final-rust.log`, `sixth-final-python314.log`, `sixth-final-python314t.log`. Note that `sixth-final-evidence.log` was produced before parent review and substitutes the subsequently rejected two-stream open/close comparator; use the primary logs for that row.

#### Rust

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.026300 | 0.026100 |
| open_close_session | 20 calls/batch | 179.141300 | 175.896800 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6206.751900 | 6200.810200 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 55.293567 | 49.351867 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43132.911500 | 43128.949500 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 72.703167 | 68.741167 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 31.216850 | 20.019100 |
| capture_loop_cpu | process user+kernel; 5 runs | 31.250000 | 15.625000 |

#### CPython 3.14.5

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 1.994300 | 1.983400 |
| open_close_session | 20 calls/batch | 229.870800 | 228.093100 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6190.140500 | 6187.856700 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 38.682167 | 36.398367 |
| capture_loop_cpu | process user+kernel; 5 runs | 62.500000 | 46.875000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43120.672500 | 43115.736500 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 60.464167 | 55.528167 |
| first_sample_latency | input start to nonempty callback; 10 runs | 21.811900 | 15.056400 |

#### CPython 3.14.7t

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.062800 | 2.033900 |
| open_close_session | 20 calls/batch | 228.423800 | 221.445100 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6191.552400 | 6188.091100 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 40.094067 | 36.632767 |
| capture_loop_cpu | process user+kernel; 5 runs | 78.125000 | 31.250000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43115.597600 | 43113.316600 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 55.389267 | 53.108267 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.912350 | 14.235000 |

#### Ratios and remaining costs

Ratio = Python median / Rust median. Excess percentage = `(Rust/Python - 1) * 100`, not `1-ratio`.

| Operation | Python/Rust | Python-t/Rust | Verdict |
|---|---:|---:|---|
| Cached enumeration | 75.828897 | 78.433460 | Pass |
| Duplex open/close | 1.283181 | 1.275104 | Pass |
| Headphones wall | 0.997324 | 0.997551 | Fail, excess 0.268353% / 0.245488% |
| Headphones overhead | 0.699578 | 0.725113 | Diagnostic |
| Seven-segment wall | 0.999716 | 0.999599 | Pass within 0.2%, excess 0.028383% / 0.040157% |
| Seven-segment overhead | 0.831658 | 0.761855 | Diagnostic |
| First 480-frame delivery | 0.698722 | 0.733974 | Diagnostic |
| Process CPU | 2.000000 | 2.500000 | Pass |

Direct fresh sys-win enumeration is 273.570900 median / 267.333800 minimum ms per 20 calls; this is unequal work versus PortAudio's cached enumeration and remains diagnostic. Four-period buffering means a requested 40 ms buffer, not a measured decomposition of all 55–73 ms overhead. The retained concurrent initialization still costs about 8.96 ms per pair; its detailed fifth-run profile remains below, and no new per-stage profile is claimed. Reducing buffering did not pass the underrun gate, so these remaining costs were not hidden by retaining a failing smaller buffer. No sixth-run speedup is claimed for instrumentation alone.

First-data compares **input-start invocation to first 480-frame application delivery**, not playback-start to callback or physical ADC latency. Rust 1024-frame diagnostic median/min: 41.529900/39.392500 ms; Python cumulative >=1024: 42.730900/41.427400; Python-t: 44.132450/42.842400. The original playback-start latency remains unmeasured. Python explicit timing uses automatic blocksize; first callback was 480 frames and reported per-direction latency 22 ms. The independent observer instead explicitly requests 480-frame callbacks.

### impulcifer-sys-win: settings and priority

Only opt-in negotiated-format/period trace details and the explanatory buffer comment were retained from this run's source edits. `SHARED_BUFFER_PERIODS` was tried at 3, then 2, then restored to 4. Actual negotiated buffers were 1440, **1056**, and 1920 frames respectively; the two-period request was 20 ms but the backend reported a 22 ms buffer. All capture packet sizes were 480 frames. The negotiated period trace records 100000 hns (10 ms), requested duration, actual buffer size and transport format. There is no public API change and no added allocation/checksum work in trace-disabled timing.

[wasapi 0.24 documentation](https://docs.rs/wasapi/0.24.0/wasapi/) and installed source, plus [std::thread](https://doc.rust-lang.org/std/thread/index.html), exposed no safe MMCSS or thread-priority setter. None was added. No new native crate or unsafe was permitted. Priority stayed unchanged throughout; its effect on external loss was **not measured**.

### impulcifer-audio-io: independent capture and classification

Every trial opens an independent Python WASAPI InputStream before synchronously launching the prebuilt Rust benchmark. Capture artifacts, callback metadata and in-memory render/capture traces are retained. All **126605** render writes across 30 trials matched immutable source float32 checksums and contiguously submitted all 2066890 source frames per trial. Successful release does not prove engine consumption.

In the tables, `M[a,b)` is missing source frames and `Z[a,b)` is an active zero interval, zero-based half-open stereo frame coordinates. External frame counts combine those kinds; they do not count duplicated channels. `U` is unresolved, never zero. `O` means an observer-only loss diagnosed by the follow-up; strict contemporaneous coverage is unavailable. The original analyzer supports limited splices and failed on some complex observer recordings; original errors and later analyses are retained, not replaced by clean reruns. Smaller-buffer unresolved trials were not accepted even though underruns already disqualified the setting.

#### Three requested periods, 1440-frame buffer

| Run | Internal frames | External frames | Render underruns | Shared source regions / limitation |
|---:|---:|---:|---:|---|
| 0 | 0 | 480 | 2 | M[126240,126720) |
| 1 | 0 | 480 | 2 | M[142560,143040) |
| 2 | 0 | 480 | 2 | M[1317120,1317600) |
| 3 | 0 | 0 | 1 | None |
| 4 | 0 | 0 | 1 | None |
| 5 | 0 | 0 | 1 | None |
| 6 | 0 | 0 | 1 | None |
| 7 | U | U | 1 | Observer alignment failed; post-initial flag 1 |
| 8 | U | U | 2 | Observer alignment failed; post-initial flags 3 |
| 9 | 0 | 480 | 2 | M[1982880,1983360) |

All ten fail the underrun gate. Confirmed shared loss totals 1920 frames; not an exhaustive total including unresolved runs.

#### Two requested periods, actual 1056-frame buffer

| Run | Internal frames | External frames | Render underruns | Shared source regions / limitation |
|---:|---:|---:|---:|---|
| 0 | 0 | 384 | 1 | Z[672,1056) |
| 1 | 0 | 1306 | 1 | M[1304736,1305216), M[1585488,1585872); Z[672,1056), Z[2066832,2066890) |
| 2 | U | U | 1 | Incomplete alignment; 4608 index-gap frames, 12 post-initial flags |
| 3 | 0 | 384 | 1 | Z[672,1056) |
| 4 | U | U | 1 | Alignment failed; 8832 index-gap frames, 23 post-initial flags |
| 5 | 0 | 384 | 1 | Z[672,1056) |
| 6 | 0 | 768 | 2 | Z[288,672), Z[1728,2112) |
| 7 | 0 | 1354 | 2 | M[401472,401952); Z[288,672), Z[1728,2112), Z[2066784,2066890) |
| 8 | U | U | 1 | Unexplained residual; candidates retained |
| 9 | 0 | 442 | 1 | Z[672,1056), Z[2066832,2066890) |

All ten fail the underrun gate. Run 1 also has one post-initial discontinuity and a 384-frame packet-index gap. Confirmed shared missing/zero frames total 5022, excluding unresolved runs. Shared zeros here do not establish a VB-Cable-only fault: render underruns coexist.

#### Four periods retained, 1920-frame buffer

| Run | Internal frames | External frames | Observer-only frames | Render underruns | Result / source region |
|---:|---:|---:|---:|---:|---|
| 0 | 0 | 0 | 0 | 0 | Complete paired verification |
| 1 | 0 | 432 | 0 | 0 | Complete paired verification; M[1322880,1323312) |
| 2 | O | O | 4992 | 0 | Full source match; incomplete contemporaneous observer coverage |
| 3 | 0 | 0 | 0 | 0 | Complete paired verification |
| 4 | 0 | 0 | 0 | 0 | Complete paired verification |
| 5 | 0 | 0 | 0 | 0 | Complete paired verification |
| 6 | 0 | 0 | 0 | 0 | Complete paired verification |
| 7 | O | O | 384 | 0 | Full source match; observer M[1080384,1080768) |
| 8 | 0 | 480 | 0 | 0 | Complete paired verification; M[25920,26400) |
| 9 | 0 | 0 | 0 | 0 | Complete paired verification |

No four-period run had a post-initial capture discontinuity, SILENT packet, packet-index gap or render underrun. Complete paired trials overlap all 2078890 captured frames and are bit-identical between observers, residual 0/0. Clean source-relative unfitted RMS is 1.6122093906661403e-6 / 1.396214288536708e-6. Run 1 has 0.44431154814465135 / 0.3511580109200847; run 8 has 0.02102263844585834 / 1.396214288536708e-6. These nonzero source-relative residuals are retained: external classification is not a claim that the waveform is intact.

Run 8's lost source [25920,26400) was submitted with padding 1440, free/requested/written 480, FNV `93b7a18661d7803a`; capture packet 58 had no SILENT/discontinuity flag. Combined with the independently matched loss this excludes a Rust-capture-only defect for that event. It does not identify the common engine/cable culprit.

### Follow-up on four-period observer failures

`sixth_followup_analysis.py` accounts for both channels with exact local byte matching, then checks the entire Rust source interval at one fixed offset, no gain fit. Parent independently reran it. Trial 2 has **13 observer-only 384-frame deletions** at the following source intervals:

```text
[146208,146592)   [939648,940032)   [979968,980352)
[1260768,1261152) [1291008,1291392) [1341408,1341792)
[1342848,1343232) [1347168,1347552) [1348608,1348992)
[1485408,1485792) [1882848,1883232) [1913088,1913472)
[1914528,1914912)
```

Trial 7 has one observer-only deletion [1080384,1080768). There are no unexplained nonzero observer frames. Trial 2 has 14 exact paired segments covering 2073898 capture frames / 2061898 source frames; trial 7 has two segments covering 2078506 capture frames / 2066506 source frames. Every mapped stereo sample is bit-identical. The full 2066890-frame Rust/source interval matches at fixed lags 1728 and 2112 respectively: unfitted RMS 1.6122093906661403e-6 / 1.396214288536708e-6, maximum absolute error 3.8142316043376923e-6. All source windows agree on the lag; there are no active zeros or samples above 4e-6. Captured values lie on the 2^-18 quantization grid. Samples unavailable in the observer match quantized samples from independently observed same-channel repeated sweep segments, but those are not contemporaneous observations.

Thus **no observable Rust source loss is found in these two trials**. The missing contemporaneous observer evidence is not relabeled an external Rust loss or an unconditional strict paired pass. The helper retains `accepted=false` and null strict internal/external counts. Single-global-offset residuals, rather than just favorable local fits, remain in the follow-up JSON. The audit still fails headphones wall time independently of this classification limitation.

### Verification and reproducibility

Parent reran the restored duplex oracle with `--op open_close_session --explicit-streams` in both interpreters, three warmups and five 20-call batches. Both exited 0; median/min were 231.531300/227.470500 ms (3.14.5), 231.935400/226.080100 ms (3.14.7t). These confirm restoration and are not substituted selectively into the final timing set above.

The parent independently ran the following foreground chain after restoring the duplex comparator; all commands exited 0. Two crates: **39 passed, 5 hardware ignored**; policy: **6 passed**; service recording: **13 passed, 1 hardware ignored**; separately opted-in CABLE-A test: **1 passed**, 23.28 s, 878540 frames, 48000 Hz/stereo and two detected segments. fmt and clippy passed. Python syntax checks and Ruff passed. No golden tolerance changed. These are local gates, not a claim of CI completion; no commit or push was requested.

```sh
cargo fmt -p impulcifer-audio-io -p impulcifer-sys-win -- --check
cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-policy
cargo test -p impulcifer-service --test recording
cargo test -p impulcifer-service --test recording recording_virtual_cable_end_to_end -- --ignored --exact --nocapture
py -3.14 -m py_compile tests/migration/bench_oracle_impulcifer_audio_io.py crates/impulcifer-audio-io/tests/bench_support/sixth_paired.py crates/impulcifer-audio-io/tests/bench_support/sixth_followup_analysis.py
ruff check tests/migration/bench_oracle_impulcifer_audio_io.py crates/impulcifer-audio-io/tests/bench_support/sixth_paired.py crates/impulcifer-audio-io/tests/bench_support/sixth_followup_analysis.py
py -3.14 -B E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/sixth_followup_analysis.py --output sixth-followup-parent.json
git diff --check
```

Hardware/timing invocation pattern (Git Bash, repository root). Artifact names must be fresh. The executable path shown is the measured build, not a stable Cargo hash; rebuild first and use Cargo's resulting executable. `--period` validates the compiled setting, it does not change it.

```sh
H=E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support
O=E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
cargo bench -p impulcifer-audio-io --bench perf --no-run
# Run after each chosen compile-time setting, N=0..9, P=3 then 2 then 4:
py -3.14 "$H/sixth_paired.py" --period "$P" --run "$N" --exe E:/Impulcifer/target/release/deps/perf-b4dd78d0010aed28.exe
PYTHONIOENCODING=utf-8 py -3.14 "$O" --observe-rust cargo bench -p impulcifer-audio-io --bench perf
PYTHONIOENCODING=utf-8 py -3.14 "$O" --explicit-streams
PYTHONIOENCODING=utf-8 C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe "$O" --explicit-streams
cargo bench -p impulcifer-sys-win --bench perf
```

Trace/checksum and artifact SHA-256 index: `sixth-validation-index.json`; per-trial originals: `sixth-p{3,2,4}-{00..09}-*`; follow-up: `sixth-followup-verified.json/.log` and independently regenerated `sixth-followup-parent.json`. Original analysis errors and intermediate revisions remain. Exact run parameters, callback metadata, raw logs and commands appear in the corresponding artifacts. The final-evidence summary predates the duplex restoration and follow-up; this section states both corrections explicitly.

Retained changed files for the sixth run: `crates/impulcifer-sys-win/src/lib.rs` (private opt-in trace and buffer comment), `crates/impulcifer-audio-io/tests/bench_support/mod.rs` (fresh fixture names), new `sixth_paired.py` / `sixth_followup_analysis.py` and sixth artifacts in that directory, `tests/migration/bench_oracle_impulcifer_audio_io.py` (duplex restored), this report and `docs/rust/HARDWARE.md`. Previous dirty changes elsewhere were not reverted. Smoke hooks remain `bench_smoke_impulcifer_audio_io` and `bench_smoke_impulcifer_sys_win`. No features registry, unsafe budget, public API, Python production recorder, frontend, service implementation or release configuration was changed by this audit. No version bump, changelog or unrelated README change was made under the packet's restricted allowlist.

**Remaining work:** headphones wall excess must fall to <=0.2% against both Python environments without introducing underruns, and strict contemporaneous paired verification must handle the independent observer's missing data. Three-/two-period complex unresolved classifications are also retained as incomplete diagnostic work; they are not accepted settings.

## Fifth run, 2026-09-08: measured, acceptance NOT PASSED

**INCOMPLETE.** Concurrent initialization passes the open/close performance target against both interpreters. Waveform integrity still fails: 8/10 captures pass before and 8/10 after concurrency. Headphones/seven-segment overhead and first 480-frame delivery remain slower than both Python environments. Four-period buffering remains; three- and two-period trials were withheld because neither integrity series passed. No registry status was changed. This section supersedes the fourth-run measurements below, which remain historical evidence.

### Environment and measurement boundaries

Intel Core i5-12600KF, 10 physical/16 logical cores; Windows 11 Education build 22621; Rust/Cargo 1.97.0, LLVM 22.1.6, x86_64-pc-windows-msvc. CPython 3.14.5: `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`. CPython 3.14.7 free-threaded: `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`. Both: NumPy 2.5.3, SciPy 1.18.1, sounddevice 0.5.6, PortAudio V19.7.0-devel. NumPy OpenBLAS 0.3.34.106.0 ILP64/pocketfft; SciPy OpenBLAS 0.3.31.dev LP64/duccfft. Thread variables OMP/MKL/OPENBLAS/NUMEXPR were unset; no affinity/thread pinning or installation. Full interpreter paths and `numpy.show_config()` output are in `fifth-env314.log` and `fifth-env314t.log` under `crates/impulcifer-audio-io/tests/bench_support/` (all artifact filenames below are relative to that directory unless stated otherwise).

Observed audio processes: Discord PIDs 3128, 13112, 17004, 17824, 19580, 20044; HLConvolverHost 71740; audiodg 113280. No application was terminated or asked to close. Every command and worker ran foreground. The independent observer runs a synchronous Rust subprocess while its Python input callback captures the same playback; no detached task was used.

Both implementations use CABLE-A Input for output, CABLE-A Output for input, WASAPI, 48 kHz, two channels, existing float32 transport. Python selects devices within the WASAPI host API, not WDM-KS. Rust's first exclusive float32 attempt is refused on this pair and its memoized shared auto-convert fallback is measured thereafter. Rust endpoint buffers are 1920 frames (four 480-frame periods), capture packets 480 frames. Python explicit timing streams use blocksize 0 (first callback observed at 480 frames), reported latency 22 ms per direction. No measured Python callback status reported an xrun/overflow.

Three warmups precede five measured batches/runs, except seven-segment timing uses three and latency/integrity uses ten. Enumeration/open-close batches contain 20 calls/pairs. Wall time includes file read, preparation, device resolution, stream open, playback/capture, stop, destruction and joins; excludes output file writes. Trace is disabled during performance measurements. CPU is process user+kernel over the same five headphones spans, using the acknowledged psutil observer for Rust. Counter granularity is 15.625 ms; even a zero minimum is retained, not clamped or interpreted as zero actual CPU consumption.

Headphones playback is the bundled 295270-frame mono sweep duplicated to stereo (6.151458333333 s). Seven segments are the 2066890-frame alternating-channel stereo fixture (43.060208333333 s), not a call requesting seven channels from the stereo generator. Timed captures contain exactly these frame counts. Diagnostic captures include a 250 ms tail (2078890 frames), allowing all 2066890 reference frames/channel to overlap despite leading latency. Fixed-length timed capture does not establish full trailing-signal integrity.

**The Python timing columns are explicit `sd.InputStream`/`sd.OutputStream`, NOT `core.recorder.play_and_record`.** Open/close retains the requested duplex `sd.Stream`, while Rust opens its two thread-affine sessions concurrently and includes thread creation, destruction and joins. The unchanged 2.x function needs COM initialized on its calling thread on this machine. Separately, sounddevice `play()` closes an active `rec()` convenience stream through its global callback state. These are observations about 2.x, not changes made here; fourth-run reproduction and results remain below.

### Verbatim before/after timing output

Raw source logs: `fifth-before-rust.log`, `fifth-before-python314.log`, `fifth-before-python314t.log`, `fifth-after-rust.log`, `fifth-after-python314.log`, `fifth-after-python314t.log`. `fifth-evidence-tables.log` consolidates the printed rows. The rows are reproduced verbatim below; separator lines are added for Markdown rendering.

#### Before: Rust

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.025800 | 0.025800 |
| open_close_session | 20 calls/batch | 263.047100 | 256.314600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6206.777600 | 6205.276900 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 55.319267 | 53.818567 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43127.596700 | 43124.587500 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 67.388367 | 64.379167 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 40.727300 | 30.501700 |
| capture_loop_cpu | process user+kernel; 5 runs | 46.875000 | 0.000000 |

#### Before: CPython 3.14.5

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.051300 | 2.003300 |
| open_close_session | 20 calls/batch | 227.259000 | 222.255900 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6192.239300 | 6178.796900 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 40.780967 | 27.338567 |
| capture_loop_cpu | process user+kernel; 5 runs | 46.875000 | 15.625000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43116.420300 | 43114.204000 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 56.211967 | 53.995667 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.370600 | 21.447400 |

#### Before: CPython 3.14.7t

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.329000 | 2.242000 |
| open_close_session | 20 calls/batch | 249.068600 | 239.423300 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6192.307600 | 6188.185600 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 40.849267 | 36.727267 |
| capture_loop_cpu | process user+kernel; 5 runs | 62.500000 | 31.250000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43115.306600 | 43113.335700 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 55.098267 | 53.127367 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.053600 | 19.670400 |

#### After: Rust

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.027700 | 0.027100 |
| open_close_session | 20 calls/batch | 175.201400 | 174.527100 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6209.260900 | 6205.120600 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 57.802567 | 53.662267 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43129.164200 | 43128.047800 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 68.955867 | 67.839467 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 31.345300 | 20.391800 |
| capture_loop_cpu | process user+kernel; 5 runs | 31.250000 | 31.250000 |

#### After: CPython 3.14.5

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.021500 | 2.011300 |
| open_close_session | 20 calls/batch | 227.347500 | 222.604300 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6189.820200 | 6188.647700 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 38.361867 | 37.189367 |
| capture_loop_cpu | process user+kernel; 5 runs | 62.500000 | 31.250000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43116.680800 | 43115.047400 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 56.472467 | 54.839067 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.111850 | 13.065400 |

#### After: CPython 3.14.7t

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.057100 | 2.004100 |
| open_close_session | 20 calls/batch | 230.917800 | 229.477600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6187.896900 | 6184.659300 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 36.438567 | 33.200967 |
| capture_loop_cpu | process user+kernel; 5 runs | 46.875000 | 46.875000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43111.996400 | 43104.440800 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 51.788067 | 44.232467 |
| first_sample_latency | input start to nonempty callback; 10 runs | 21.843600 | 17.227300 |

Direct fresh sys-win enumeration (`fifth-sys-bench.log`):

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 267.525100 | 265.979800 |

#### Ratios and latency qualification

Ratio = Python median / Rust median. Cached backend enumeration is DTO cloning, not complete service JSON assembly. Direct sys-win fresh enumeration versus PortAudio's cached enumeration has unequal caching boundaries; its after ratios 0.007556 / 0.007689 are not an equal-work performance result.

| Operation | Before Python/Rust | Before Python-t/Rust | After Python/Rust | After Python-t/Rust |
|---|---:|---:|---:|---:|
| Cached enumeration | 79.507752 | 90.271318 | 72.978339 | 74.263538 |
| Open/close | 0.863948 | 0.946859 | 1.297635 | 1.318013 |
| Headphones wall | 0.997658 | 0.997669 | 0.996869 | 0.996559 |
| Headphones overhead | 0.737193 | 0.738427 | 0.663671 | 0.630397 |
| Seven-segment wall | 0.999741 | 0.999715 | 0.999711 | 0.999602 |
| Seven-segment overhead | 0.834149 | 0.817623 | 0.818965 | 0.751032 |
| First delivery | 0.549278* | 0.541494* | 0.705428 | 0.696870 |
| Process CPU | 1.000000 | 1.333333 | 2.000000 | 1.500000 |

*Before compares Rust's 1024-frame application read to Python's first 480-frame callback and is not a parity result. After uses a 480-frame Rust read and the observed 480-frame Python callback, both timed from input-start invocation. This still measures application delivery, not hardware arrival or the requested common playback-start boundary. Actual playback-start latency remains unmeasured. Rust's after auxiliary 1024-frame median/min is 42.362150/40.169700 ms (`fifth-after-latency1024.log`); Python reaching at least 1024 frames is 41.866900/41.166400 ms and Python-t 42.865950/41.604300 ms.

### impulcifer-sys-win: render proof and independent capture

The private, opt-in trace now fingerprints the exact scratch bytes passed to safe `AudioRenderClient::write_to_device`. It records timestamps immediately before/after that call, padding/free-space snapshot, requested frames, successful submitted count, source cursor before/after and payload FNV-1a. Logs stay in memory until session destruction. On an error the actual written count is labeled unknown and the application cursor is not advanced. Trace-disabled performance runs do not compute checksums.

The inspected [wasapi 0.24 API](https://docs.rs/wasapi/0.24.0/wasapi/struct.AudioRenderClient.html) and installed source validate the byte length, acquire the endpoint buffer, copy the requested bytes and release the requested count on success. The safe wrapper does not expose separate internal GetBuffer/ReleaseBuffer timestamps. These records bracket the complete safe write, not those internal calls independently. Successful release is not proof of subsequent engine/hardware consumption.

Offline checking verified every submitted byte checksum against the immutable source and contiguous coverage of all 2066890 source frames: 42613 writes before concurrency, 42687 after, and 4300 for simultaneous capture. **No skipped source cursor or incorrect render payload was found.** There is no demonstrated accounting defect to fix; success-only source advancement remains.

For a failing playback, a second independent Python `sd.InputStream` recorded the same cable concurrently. Python and Rust both lose source frames `[103200,103680)`. Python capture starts earlier; after a 25440-frame offset the entire 2078890-frame common interval is bit-exact, maximum difference 0. The parent independently loaded both raw files and verified this equality, not just the worker's summary. Python callback statuses were empty.

The write covering that lost region (`fifth-final-evidence.log`) is:

```text
ordinal=208 buffer_size=1920 padding=1440 free=480
before_us=2094375 after_us=2094377 requested=480 written=480
cursor_before=103200 cursor_after=103680
payload_fnv1a=5d567107839964bf bytes=3840
outcome=release_ok_not_hardware_consumption
```

The checksum matches those exact source frames. The Rust splice occurs at capture frame 105744 in packet 220, range `[105600,106080)`, index 131040, with no SILENT/discontinuity flag; queue samples before/after 256/1216. Both captures have unfitted RMS `0.1871900279405533 / 1.396214288536708e-6`. Independent captures sharing the loss exclude a defect unique to Rust's capture queue for this observed run. They locate the loss in their common audio processing, but **do not distinguish Windows audio processing from VB-Cable** or establish causes for every other failed run.

### impulcifer-audio-io: concurrent initialization and open/close profile

`src/session.rs` opens output on its owning scoped worker while input opens on the other worker. Output still waits for the existing input-start permit before playback. `InputReady` precedes `OutputStarted`; cancellation/error/panic handling joins both workers and drops sessions on their owning threads. No COM/session object crosses threads, no initialized session pooling, no deferred destruction, no public signature change. A fake-backend handshake proves both opens enter before either can complete and checks success, input-open failure, cancellation and output-open panic cleanup. Existing ordering tests now distinguish output initialization from actual playback.

Paired open/close improves from 263.047100 to 175.201400 ms per 20 pairs (13.152355 to 8.760070 ms/pair). Ratios exceed 1.0 against both Python environments. Same-thread enumerator reuse remains implemented, but separate owning threads cannot share that thread-local lease. The new benchmark includes spawn/drop/join; it does not claim an enumerator cache hit on each newly created thread.

Post-change profile (`fifth-open-profile.log`, `fifth-evidence-tables.log`) covers 320 successful sessions. Per-direction stage medians cannot be added to obtain concurrent pair wall time.

| Stage | Samples | median us | min us | mean us |
|---|---:|---:|---:|---:|
| com_init | 320 | 11.000 | 0 | 8.491 |
| enumerator | 320 | 875.500 | 770 | 889.041 |
| device_lookup | 320 | 88.500 | 67 | 90.756 |
| get_iaudioclient | 320 | 239.500 | 201 | 246.353 |
| mixformat_validation | 320 | 228.500 | 69 | 194.406 |
| get_device_period | 320 | 383.500 | 184 | 322.228 |
| initialize_client | 320 | 5497.000 | 3414 | 4904.600 |
| device_release | 320 | 7.000 | 4 | 6.938 |
| service_acquisition | 320 | 3.000 | 2 | 3.134 |
| get_buffer_size | 320 | 0.000 | 0 | 0.006 |
| event_creation | 320 | 34.000 | 21 | 34.375 |
| allocation | 320 | 1.000 | 0 | 2.122 |
| capture_service_release | 160 | 1.000 | 0 | 1.156 |
| audio_client_release | 320 | 676.500 | 483 | 657.100 |
| enumerator_release | 320 | 0.000 | 0 | 0.000 |
| com_uninit | 320 | 15.000 | 1 | 17.628 |
| render_service_release | 160 | 1.000 | 0 | 0.963 |

Initialization is still the largest measured open/close stage, now overlapped across directions. Playback overhead did not improve. Four-period buffering, event delivery and stream startup/drain remain costs; this run does not quantitatively attribute the entire remaining overhead to any one of them. The previously reviewed render conversion, capture allocation/queue logic and COM enumeration did not establish the observed loss's cause. No safe MMCSS call was added; no native dependency or unsafe was introduced.

### Ten-run waveform integrity before and after concurrency

Every capture has 2078890 frames and 4332 packets of 480 frames; every comparison overlaps all 2066890 reference frames/channel. All twenty runs have zero post-initial discontinuities, SILENT packets, packet-index gaps, reported render underruns, and exact repeated 128-frame blocks. The latter is not an exhaustive repetition test. The analyzer estimates at most one splice per segment from clean local-offset windows; complex multiple splices could require further analysis. Global fitted and unfitted RMS remain reported, so a residual cannot be concealed by excluding splice regions.

Clean fitted RMS is `1.61220319e-6 / 1.39620892e-6`; unfitted RMS is `1.61220939e-6 / 1.39621429e-6`. The before/after tables describe an initialization optimization, **not a successful loss repair**.

#### Before concurrency (`fifth-baseline-analysis.log`)

| Run | Lag ch0/ch1 | Fitted RMS ch0/ch1 | Unfitted RMS ch0/ch1 | Lost/repeated/zero frames | Verified writes | Result |
|---|---|---|---|---|---:|---|
| 0 | 960/960 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4257 | Pass |
| 1 | 1008/1008 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4252 | Pass |
| 2 | 1488/1488 | 0.391354538/0.326005266 | 0.428180984/0.353475204 | 480/0/0 | 4300 | Fail |
| 3 | 960/960 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4247 | Pass |
| 4 | 1104/1104 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4244 | Pass |
| 5 | 1008/1008 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4231 | Pass |
| 6 | 1152/1152 | 0.282812507/1.39620892e-6 | 0.294532263/1.39621429e-6 | 480/0/0 | 4300 | Fail |
| 7 | 1152/1152 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4289 | Pass |
| 8 | 1248/1248 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4250 | Pass |
| 9 | 1056/1056 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4243 | Pass |

#### After concurrency (`fifth-concurrent-analysis-final.log`)

| Run | Lag ch0/ch1 | Fitted RMS ch0/ch1 | Unfitted RMS ch0/ch1 | Lost/repeated/zero frames | Verified writes | Result |
|---|---|---|---|---|---:|---|
| 0 | 1200/1200 | 0.0013236144/1.39620892e-6 | 0.00132363772/1.39621429e-6 | 0/0/1409 | 4256 | Fail |
| 1 | 1008/1008 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4300 | Pass |
| 2 | 1056/1056 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4299 | Pass |
| 3 | 960/960 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4251 | Pass |
| 4 | 1248/1248 | 0.426036895/0.330926965 | 0.477711154/0.360025696 | 240/0/0 | 4300 | Fail |
| 5 | 1056/1056 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4239 | Pass |
| 6 | 912/912 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4244 | Pass |
| 7 | 576/576 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4250 | Pass |
| 8 | 816/816 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4248 | Pass |
| 9 | 864/864 | 1.61220319e-6/1.39620892e-6 | 1.61220939e-6/1.39621429e-6 | 0/0/0 | 4300 | Pass |

After run 0's active zero interval is capture `[1311,2720)`, aligned source `[111,1520)`. Packets 2–5 carry it; all flags are false, queue before/after 0/960 samples. Raw zero frames in those packets are 240,480,480,320 (1520 total, including subthreshold reference samples; 1409 satisfy the active threshold). No synthetic SILENT fill occurred. Source coverage is the initial 1920-frame prefill, checksum `6990f84311618cba`, requested/written 1920, cursor 0→1920, padding 0, successful release. Full missing-region packet/write mappings are in `fifth-baseline-analysis.json` and `fifth-concurrent-analysis.json`; paired evidence is in `fifth-paired-1-analysis.json` and `fifth-paired-1-comparison.json`.

### Verification, commands, and failures retained

The parent reviewed concurrent startup/cleanup, trace accounting, paired-capture helper and offline analyzer, checked saved timing/integrity logs, then independently ran the following foreground chain. All commands exited 0:

```sh
cargo fmt -p impulcifer-audio-io -p impulcifer-sys-win -- --check
cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-policy
cargo test -p impulcifer-service --test recording
cargo test -p impulcifer-service --test recording recording_virtual_cable_end_to_end -- --ignored --exact --nocapture
py -3.14 -m py_compile tests/migration/bench_oracle_impulcifer_audio_io.py crates/impulcifer-audio-io/tests/bench_support/analyze_fifth.py crates/impulcifer-audio-io/tests/bench_support/fifth_paired.py
ruff check tests/migration/bench_oracle_impulcifer_audio_io.py crates/impulcifer-audio-io/tests/bench_support/analyze_fifth.py crates/impulcifer-audio-io/tests/bench_support/fifth_paired.py
git diff --check
```

Parent Rust output totals: audio-io 11 unit + 14 fake/smoke, sys-win 13 unit + 1 smoke, policy 6, service recording 13, explicit CABLE-A 1 = **59 passed, zero failures**. Five other hardware tests remain ignored. The service CABLE-A test recorded 878540 frames, 48000 Hz/stereo, detected two segments, and passed in 23.29 s. This integration pass does not override the detailed waveform failures. Golden tests pass unchanged; no golden tolerances or numeric algorithms were changed, and no new per-golden max-error export was calculated. Python compilation and ruff passed; diff check reported only line-ending warnings.

Independent parent paired-file verification (exit 0):

```sh
py -3.14 -c "import numpy as n; from pathlib import Path; p=Path('E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/fifth-paired-1'); a=n.fromfile(str(p)+'-python.f32',dtype='<f4').reshape(-1,2); b=n.fromfile(str(p)+'-play_record_7_speaker_set-0.f32',dtype='<f4').reshape(-1,2); a=a[25440:25440+len(b)]; print('frames',len(b),'bit_exact',n.array_equal(a.view('u4'),b.view('u4')),'max_difference',n.max(n.abs(a.astype('f8')-b))); assert n.array_equal(a.view('u4'),b.view('u4'))"
```

```text
frames 2078890 bit_exact True max_difference 0.0
All checks passed!
```

Measurement entrypoints (worker ran foreground before/after, retaining separate `fifth-*` output logs):

```sh
PYTHONIOENCODING=utf-8 py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --observe-rust cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
PYTHONIOENCODING=utf-8 py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
PYTHONIOENCODING=utf-8 C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
IMPULCIFER_PA05_OP=profile_open cargo bench -p impulcifer-audio-io --bench perf
py -3.14 crates/impulcifer-audio-io/tests/bench_support/fifth_paired.py 1
py -3.14 crates/impulcifer-audio-io/tests/bench_support/analyze_fifth.py fifth-baseline
py -3.14 crates/impulcifer-audio-io/tests/bench_support/analyze_fifth.py fifth-concurrent
py -3.14 crates/impulcifer-audio-io/tests/bench_support/analyze_fifth.py fifth-paired-1 --count 1
```

Ten-run integrity commands set `IMPULCIFER_PA05_OP=play_record_7_speaker_set`, `IMPULCIFER_PA05_INTEGRITY=1`, `IMPULCIFER_PA05_TAIL=1`, and both `IMPULCIFER_PA05_CAPTURE_PREFIX`/`IMPULCIFER_PA05_TRACE_PREFIX` to the absolute `.../tests/bench_support/fifth-baseline` or `fifth-concurrent` prefix before running the audio-io bench. The paired helper sets a fresh `fifth-paired-1` prefix and one integrity repetition. Full before/after measurement logs, trace CSVs, raw captures, environment, final gates and six passing synthetic analyzer checks remain local evidence. No command timed out or remained running in this fifth run.

Earlier failures are not counted as passes: paired attempt 0 failed COM initialization before playback; an initial new test lacked `RefUnwindSafe`; its next run exposed a test-only poisoned synchronization lock. These were corrected and rerun. The initial analyzer mistook a corrupted low-frequency window for a two-frame slip; clean-window offset selection corrected that report and all final series were reanalyzed. Earlier logs are retained. The initial parent PowerShell process query was rejected; it was not retried verbatim or delegated as a permission bypass.

### Files changed and remaining work

Fifth-run implementation files:

- `crates/impulcifer-sys-win/src/lib.rs`: private exact render-payload tracing; no demonstrated cursor repair.
- `crates/impulcifer-audio-io/src/session.rs`: concurrent initialization with capture-start permit preserved.
- `crates/impulcifer-audio-io/tests/session_fake.rs`: concurrency/cleanup test and ordering assertions.
- `crates/impulcifer-audio-io/tests/bench_support/mod.rs`: concurrent paired open/close, 480-frame latency workload and preserved artifact names.
- `crates/impulcifer-audio-io/tests/bench_support/analyze_fifth.py`: offline payload/continuity/waveform validation.
- `crates/impulcifer-audio-io/tests/bench_support/fifth_paired.py`: independent foreground simultaneous Python capture.
- `docs/rust/perf/impulcifer-audio-io.md`: parent-written fifth-run report.

Smoke names remain `bench_smoke_impulcifer_audio_io` and `bench_smoke_impulcifer_sys_win`. Other dirty tree files predate or belong to other workers; no features.toml, unsafe budget, production Python, service/update/apps, golden fixtures, version, README, CHANGELOG, commit or push was changed by PA05 fifth-run work. No new dependency or unsafe was introduced.

**Still required before acceptance:** establish and repair/otherwise resolve active waveform loss with 10/10 passing runs, then test smaller buffers at three and two periods, improve both sweep overhead ratios and 480-frame delivery latency, and measure a common playback-start latency if that original metric is required. No claim of completed performance audit or crate acceptance is made.

## Fourth run, 2026-09-08: measured, acceptance NOT PASSED

**INCOMPLETE.** Open/close still loses to both interpreters. Seven of ten diagnostic captures pass; three lose exactly 480 active frames. All ten have zero internal both-channel zero-fill, zero post-first discontinuities and zero reported render underruns. These counters do not establish waveform integrity. Actual first-buffer latency on a common playback-start boundary remains unmeasured. No registry entry was changed. The third and second runs below are historical, superseded where this section records new observations.

### Environment and method

Intel Core i5-12600KF, 10 physical / 16 logical cores; Windows 11 Education build 22621; Rust 1.97.0 x86_64-pc-windows-msvc, LLVM 22.1.6. CPython 3.14.5 is `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`; CPython 3.14.7 free-threaded (GIL disabled) is `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`. Both use NumPy 2.5.3, SciPy 1.18.1, sounddevice 0.5.6, PortAudio `V19.7.0-devel, revision unknown`. NumPy OpenBLAS 0.3.34.106.0 ILP64 / pocketfft; SciPy OpenBLAS 0.3.31.dev LP64 / duccfft. OMP/MKL/OPENBLAS/NUMEXPR thread variables were unset; no thread pinning or installation.

Observed processes: Discord PIDs 3128, 13112, 17004, 17824, 19580, 20044; HLConvolverHost 71740; audiodg 113280. No user application was closed, killed or requested to close. All commands and workers ran foreground. One combined Python invocation exceeded the 600-second tool limit (exit 143): the first interpreter finished, the second produced no log. It was not counted as completed; subsequent per-operation foreground commands completed its measurements. No process-termination command was issued.

Both implementations selected WASAPI host 2, output 35 `CABLE-A Input (VB-Audio Cable A)`, input 39 `CABLE-A Output (VB-Audio Cable A)`, 48 kHz, stereo float32 transport. CABLE-A rejects the requested exclusive float32 format; Rust used shared auto-convert after its exclusive-first refusal memo. Python uses the existing recorder WASAPI settings, shared mode. Windows display names omit the space before `(`. Python stream metadata: blocksize 0, reported latency 22 ms in each direction, first input callback 480 frames; no measured callback statuses reported xruns/overflows. Rust diagnostic buffers were 1920 frames, packets 480 frames.

Three warmups precede five 20-call enumeration/open-close batches, five headphones measurements, three seven-segment timings and ten latency measurements. CPU is process user+kernel over the same five measured headphones runs, observed with psutil (Rust via the synchronous acknowledged observer). Resolution is 15.625 ms; the resulting CPU ratios are coarse observations, not precise speedups. Wall spans include file read, sample preparation, device resolution, session open, playback/capture and stop/close/destruction; no file writes. COM setup is outside Python timing. Trace is disabled for performance timings; ten separate integrity captures enable buffered trace and a 250 ms tail.

Headphones: bundled 295270-frame sweep duplicated to stereo, 6.151458333333 s. Seven segments: 2066890 frames, 43.060208333333 s, alternating stereo file fixture. This is not a seven-speaker call to the stereo generator. Timed Rust captures contain exactly these frame counts; diagnostic captures contain 2078890 frames including the tail. Fixed-length production capture starts before playback and can truncate its end by the leading latency; the diagnostic tail ensures full reference overlap.

### Verbatim current Rust tables

From `tests/bench_support/fourth-astra-rust-timing.log` in audio-io (parent checked rows against the saved log):

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.026600 | 0.025900 |
| open_close_session | 20 calls/batch | 268.094200 | 260.033600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6207.706700 | 6204.875200 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 56.248367 | 53.416867 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43128.910500 | 43128.575800 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 68.702167 | 68.367467 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 42.042050 | 39.642500 |
| capture_loop_cpu | process user+kernel; 5 runs | 31.250000 | 15.625000 |

Direct sys-win enumeration (fresh, not cached):

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 268.834600 | 263.997200 |

### Python COM and production recorder

The oracle now balances successful `CoInitializeEx(None, 0)` calls with `CoUninitialize`. Initializing only after importing the production stack failed with `0x80010106` on both interpreters. Initializing the calling thread before imports succeeded (outer S_OK, nested S_FALSE). The actual unchanged `core.recorder.play_and_record` then completed in both interpreters. This does not resolve its global sounddevice convenience-stream race.

The minimal reproduction opens `sd.rec` then starts `sd.play` on the pinned WASAPI endpoints. Both interpreters print:

```text
before sd.play True False
after sd.play False True capture_shape (4800, 2)
```

Installed sounddevice `start_stream()` calls global `stop()` and replaces `_last_callback`; `stop()` closes the previous stream. The oracle exposes `--reproduce-convenience-race` to reproduce this independently. Production captures had observed residual RMS as high as 0.439931 (CPython full run), 0.427824 (free-threaded headphones), 0.500673 (free-threaded seven segments). Completion/frame count is not a correctness pass. `core/recorder.py` was not modified.

Verbatim production CPython 3.14.5 rows:

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.043000 | 2.001300 |
| open_close_session | 20 calls/batch | 247.258200 | 226.579700 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6204.042600 | 6201.611100 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 52.584267 | 50.152767 |
| capture_loop_cpu | process user+kernel; 5 runs | 125.000000 | 46.875000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43130.004800 | 43128.904100 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 69.796467 | 68.695767 |
| first_sample_latency | input start to nonempty callback; 10 runs | 23.254150 | 21.924400 |

Verbatim production CPython 3.14.7t per-operation reruns:

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| play_record_headphones_sweep | 295270 frames; 5 runs | 6204.519300 | 6201.166300 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 53.060967 | 49.707967 |
| capture_loop_cpu | process user+kernel; 5 runs | 156.250000 | 109.375000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43130.439100 | 43128.902200 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 70.230767 | 68.693867 |
| first_sample_latency | input start to nonempty callback; 10 runs | 35.437300 | 22.420400 |

### Authorized explicit-stream comparison (NOT the 2.x function)

`--explicit-streams` uses independent `InputStream` / `OutputStream` callbacks, with COM on the thread that creates the streams. Input starts before output creation; no preopened-stream shortcut or convenience `sd.rec`/`sd.play`. The benchmark allocates a fixed-length capture and waits for both finished callbacks before closing both streams. The open/close operation remains the requested duplex `sd.Stream`, versus two separate Rust sessions. Results cannot establish production-recorder parity or waveform correctness merely from absent callback flags.

Verbatim CPython 3.14.5:

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.015000 | 1.975600 |
| open_close_session | 20 calls/batch | 226.566100 | 223.782300 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6190.708100 | 6183.252900 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 39.249767 | 31.794567 |
| capture_loop_cpu | process user+kernel; 5 runs | 62.500000 | 31.250000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43113.205400 | 43111.320700 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 52.997067 | 51.112367 |
| first_sample_latency | input start to nonempty callback; 10 runs | 23.335050 | 18.031200 |

Verbatim CPython 3.14.7t:

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.100200 | 2.031100 |
| open_close_session | 20 calls/batch | 242.819400 | 238.016600 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6190.036100 | 6188.761000 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 38.577767 | 37.302667 |
| capture_loop_cpu | process user+kernel; 5 runs | 62.500000 | 31.250000 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43114.770300 | 43110.213400 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 54.561967 | 50.005067 |
| first_sample_latency | input start to nonempty callback; 10 runs | 22.693950 | 15.001400 |

### Before/after and ratios

Ratio = explicit Python median / current Rust median. Previous production comparisons failed before completing, so no production before/after sweep ratio exists. Enumeration is backend DTO cloning, not full service JSON assembly. sys-win fresh enumeration versus PortAudio cache is not an equal caching boundary.

| Operation | Third-run Rust median ms | Fourth-run Rust median ms | Python/Rust | Python-t/Rust | Assessment |
|---|---:|---:|---:|---:|---|
| Cached enumeration / 20 | 0.026700 | 0.026600 | 75.751880 | 78.954887 | Backend scope only |
| Direct sys-win enumeration / 20 | 278.228300 | 268.834600 | 0.007495 | 0.007812 | Fresh versus cached |
| Open/close / 20 pairs | 282.279500 | 268.094200 | 0.845099 | 0.905724 | Fails |
| Headphones wall | 6209.446500 | 6207.706700 | 0.997262 | 0.997153 | Fails |
| Headphones overhead | 57.988167 | 56.248367 | 0.697794 | 0.685847 | Fails |
| Seven-segment wall | 43146.423100 | 43128.910500 | 0.999636 | 0.999672 | Fails |
| Seven-segment overhead | 86.214767 | 68.702167 | 0.771403 | 0.794181 | Fails |
| Process CPU | 62.500000 | 31.250000 | 2.000000 | 2.000000 | Coarse counter |
| First application read | 42.672850 | 42.042050 | N/A | N/A | Not same callback boundary |

Python's first callback supplies 480 frames whereas Rust returns the first nonempty application read (1024 frames). Python reaching at least 1024 captured frames had median/min 42.766000/42.004400 ms and 42.966150/41.594300 ms; auxiliary ratios are 1.017220/1.021980. Neither is the requested common playback-start-to-first-buffer metric. No negative overhead occurred; no counter or duration was clamped.

### impulcifer-sys-win: measured cause, changes and open/close profile

Device lookup already used `DeviceEnumerator::get_device(id)`. Fourth-run changes cache immutable render buffer size and use a single padding snapshot instead of redundant buffer/padding queries. A thread-local enumerator lease reuses only the enumerator while same-thread session lifetimes overlap; the last lease releases it before COM uninitialization. Initialized clients are not pooled, and teardown remains inside open/close timing. Production playback/capture use separate workers, so same-thread enumeration reuse is principally relevant to the requested same-thread open/close benchmark; it is not evidence of the same saving in production.

Private opt-in `IMPULCIFER_PA05_TRACE_PREFIX` collects open/drop stages, render writes/waits, and capture packet/queue/delivery records in memory and writes CSV after session resources are released. No public diagnostics API, unsafe, new native crate or precision change. Transport bit-preservation tests remain exact, with zero-bit conversion error; no golden tolerance changed. No new per-golden numeric-error table was computed. No safe MMCSS API was found in wasapi 0.24; none was added. Four-period buffering, full prefill and event-driven draining predate this run.

Shared-only 20-pair measurements: Daybreak before median/min 279.151500/267.451700 ms; after 260.832300/259.439900 ms. Later Astra shared-only median/min 257.484500/252.781900 ms. These are not policy-open timings (268.094200/260.033600 ms). Memoized exclusive rejection measured 0.004500/0.004200 ms per 20 calls; it is not an actual exclusive driver probe.

Current trace profile, 320 successful sessions. Trace-enabled runs are separate from performance timings. The median of a sum need not equal the sum of medians, and these rows include both input and output sessions; do not treat their sum as one exact pair's wall time.

| Stage | Samples | median us | min us | mean us |
|---|---:|---:|---:|---:|
| com_init | 320 | 5.0 | 0 | 7.243750 |
| enumerator | 320 | 317.5 | 0 | 344.631250 |
| device_lookup | 320 | 72.0 | 51 | 75.168750 |
| get_iaudioclient | 320 | 205.0 | 178 | 212.903125 |
| mixformat_validation | 320 | 219.0 | 67 | 170.275000 |
| get_device_period | 320 | 347.0 | 160 | 277.818750 |
| initialize_client | 320 | 5364.5 | 3345 | 4836.953125 |
| device_release | 320 | 7.0 | 4 | 7.346875 |
| service_acquisition | 320 | 3.0 | 2 | 3.293750 |
| get_buffer_size | 320 | 0.0 | 0 | 0.006250 |
| event_creation | 320 | 37.0 | 25 | 37.946875 |
| allocation | 320 | 4.0 | 0 | 4.525000 |
| capture_service_release | 160 | 2.0 | 1 | 2.218750 |
| audio_client_release | 320 | 669.5 | 553 | 685.337500 |
| enumerator_release | 160 | 0.0 | 0 | 0.012500 |
| com_uninit | 320 | 13.0 | 0 | 17.709375 |
| render_service_release | 160 | 1.0 | 0 | 0.925000 |

Initialization dominates, followed by release and endpoint setup. No remaining safely removable duplicate initialization was established. Profiling did not justify stream pooling, deferred destruction, removing format validation or changing other devices' buffer policy. Shared buffering and initialization contribute to sweep overhead; no speculative change was made to meet a numerical threshold.

### impulcifer-audio-io: 10-run waveform evidence

The bench diagnostic count is ten; an offline analyzer maps trace PID/sequence to captures and checks actual reference channel activity, local lags, full overlap, fitted and unfitted residuals, zero runs, repeats and packet positions. Quiet alternating channels and reference zeros are not counted as corruption. PID 391284; exclusive-refusal trace files are excluded because they contain no packets/writes.

All captures contain 2078890 frames, all reference comparisons cover 2066890 frames/channel, every run has 4332 packets of 480 frames. All have zero post-first discontinuities, zero SILENT packets, zero active both-channel zero runs, zero packet-index gaps, zero reported render underruns and zero exact repeated 128-frame blocks. This last test is not an exhaustive repetition detector.

| Run | Capture/render sequence | Lag ch0/ch1 | RMS fit ch0/ch1 | Missing active frames | Integrity |
|---|---|---|---|---:|---|
| 0 | 1/3 | 1056/1056 | 1.61220319e-6 / 1.39620892e-6 | 0 | Pass |
| 1 | 4/5 | 1584/1584 | 1.61220319e-6 / 1.39620892e-6 | 0 | Pass |
| 2 | 6/7 | 1104/1104 | 1.61220319e-6 / 1.39620892e-6 | 0 | Pass |
| 3 | 8/9 | 1008/1008 | 1.61220319e-6 / 1.39620892e-6 | 0 | Pass |
| 4 | 10/11 | 1056/1056 | 0.316242930 / 1.39620892e-6 | 480 | Fail |
| 5 | 12/13 | 1200/1200 | 0.192350985 / 1.39620892e-6 | 480 | Fail |
| 6 | 14/15 | 1248/1248 | 0.0504252552 / 1.39620892e-6 | 480 | Fail |
| 7 | 16/17 | 1248/1248 | 1.61220319e-6 / 1.39620892e-6 | 0 | Pass |
| 8 | 18/19 | 1440/1440 | 1.61220319e-6 / 1.39620892e-6 | 0 | Pass |
| 9 | 20/21 | 1296/1296 | 1.61220319e-6 / 1.39620892e-6 | 0 | Pass |

Clean gains are 1.000000008 in both channels. Failed ch0 gains are 0.800535883, 0.931070793, 0.995257416. Their unfitted ch0 RMS values are 0.333319036, 0.195763905, 0.050487394; gain fitting must not conceal the error.

| Run | Missing reference range | Capture splice | Capture packet / range | Render write / range |
|---|---|---:|---|---|
| 4 | [266400,266880) | 267936 | 558 / [267840,268320) | 549 / [266400,266880) |
| 5 | [111840,112320) | 113520 | 236 / [113280,113760) | 227 / [111840,112320) |
| 6 | [49440,49920) | 51168 | 106 / [50880,51360) | 96 / [49440,49920) |

Each cited capture packet: 480 frames, flags all false, raw_zero_frames=0, silent_fill_frames=0, queue_before=0, queue_after=960 interleaved samples. Each render write: 480 frames, buffer_size=1920, padding=1440, free=480, submitted. Local lag changes 1536->1056, 1680->1200 and 1728->1248 respectively. Coarse windows straddling the splice produce intermediate lag estimates; the subwindow check locates the 480-frame discontinuity. Aligning before/after separately produces approximately 2.13e-6 unfitted RMS, but excluding the missing frames is not an integrity pass.

Render timeout counts: 162,149,174,159,193,171,177,191,147,149. Capture index/timestamp rate is 47999.396–48001.157 Hz. Render submission rate 48021.783–48032.303 frames/s includes buffering and is NOT a device-clock measurement.

Before this ten-run series, Daybreak's traced three-run series had two clean captures and one with reported 960 per-channel active zeros. Per-channel zeros with a globally wrong lag can misclassify shifted waveform regions; those preliminary counts are not directly comparable to the final actual-channel/local-lag test. Historical third-run failure (737 ch0 zero samples) remains evidence of corruption, not a proven number of engine zero-fill frames. Before/after reported underruns remain zero; final ten-run failures are frame deletion, not SILENT zero-fill.

A final Daybreak review checked the exact wasapi 0.24 `read_from_device` and `write_to_device` implementation. Successful writes validate byte length and copy/release the complete requested frame count. Rust submission slices are contiguous and immutable; capture queue copies retain partial packets. Splices occur 96,240,288 frames inside capture packets, not at queue packet boundaries. This argues against whole-packet queue loss and supports a loss after application render submission, but does not prove whether Windows or VB-Cable caused it. The original render traces contain ranges, not submitted-byte checksums. No source defect was established; no speculative patch or additional claim of acceptance was made. Next useful evidence is an opt-in per-write byte fingerprint on a failing run plus engine/cable observation, without changing user device settings.

### Verification and exact commands

The parent reviewed the production changes, oracle, saved table rows and ten-run analysis, then independently ran all these commands in the foreground after worker completion:

```sh
cargo fmt -p impulcifer-audio-io -p impulcifer-sys-win -- --check
cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-policy
cargo test -p impulcifer-service --test recording
cargo test -p impulcifer-service --test recording recording_virtual_cable_end_to_end -- --ignored --exact --nocapture
```

All passed: audio-io 11 unit + 13 fake/smoke, sys-win 13 unit + 1 smoke, policy 6, service recording 13, explicit service CABLE-A test 1 = **58 tests passed, zero failures**. Five other hardware tests remain ignored in this parent run. CABLE-A output was 878540 frames, 48000 Hz/stereo, detected two sweep segments. This integration pass does not override the ten-run waveform failures. No workspace/CI/PR acceptance claim.

Worker measurement commands (working directory E:/Impulcifer; each invoked foreground, sequential hardware use):

```sh
PYTHONIOENCODING=utf-8 py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --observe-rust cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
IMPULCIFER_PA05_OP=profile_open cargo bench -p impulcifer-audio-io --bench perf
PYTHONIOENCODING=utf-8 py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py
PYTHONIOENCODING=utf-8 py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
PYTHONIOENCODING=utf-8 C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe tests/migration/bench_oracle_impulcifer_audio_io.py --explicit-streams
PYTHONIOENCODING=utf-8 C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe tests/migration/bench_oracle_impulcifer_audio_io.py --op play_record_headphones_sweep
PYTHONIOENCODING=utf-8 C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe tests/migration/bench_oracle_impulcifer_audio_io.py --op play_record_7_speaker_set
PYTHONIOENCODING=utf-8 C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe tests/migration/bench_oracle_impulcifer_audio_io.py --op first_sample_latency
```

Integrity uses `IMPULCIFER_PA05_OP=play_record_7_speaker_set IMPULCIFER_PA05_INTEGRITY=1 IMPULCIFER_PA05_TAIL=1`, with fresh absolute `IMPULCIFER_PA05_CAPTURE_PREFIX` and `IMPULCIFER_PA05_TRACE_PREFIX` under audio-io/tests/bench_support, then `cargo bench -p impulcifer-audio-io --bench perf`. The original raw logs and exact artifact names are retained as `fourth-astra-*`; `analyze_fourth.py` writes `fourth-astra-analysis.json`. Environment and race were invoked with `--environment-only` and `--reproduce-convenience-race` on both interpreters. Do not overwrite historical capture prefixes on rerun.

Workers additionally passed Python compilation, installed `ruff` CLI lint, synthetic analyzer checks and sys-win library tests. `py -3.14 -m ruff` failed because that interpreter lacks the ruff module; the installed CLI then found import/f-string issues, fixed before its passing rerun. Early new fake-packet tests had incorrect frame/channel counts, fixed and all gates rerun. Three overly specific `--exact` test calls ran zero tests and were not counted; all 13 sys-win library tests were subsequently run. These earlier failures were not hidden.

### Files and remaining work

Fourth-run product change: `crates/impulcifer-sys-win/src/lib.rs` (private trace, render-query reduction, overlapping-lifetime enumerator reuse, two tests). Bench change: `crates/impulcifer-audio-io/tests/bench_support/mod.rs` (ten integrity runs and truthful diagnostic labels). Oracle change: `tests/migration/bench_oracle_impulcifer_audio_io.py` (COM, explicit alternative and race reproduction). New offline analyzer: `crates/impulcifer-audio-io/tests/bench_support/analyze_fourth.py`. This report was updated by the parent; raw CSV/f32/log/JSON experiment files remain local evidence, not golden fixtures or intended commits.

Smoke names remain `bench_smoke_impulcifer_audio_io` and `bench_smoke_impulcifer_sys_win`. No features.toml, unsafe budget, production Python, service/update/apps, golden/exporter, version, README, CHANGELOG, commit or push was changed for this run. Source f32 remains the existing transport, not a new DSP precision choice.

Still required: eliminate the reproducible 480-frame losses in 10/10 runs, reach open/close and sweep-overhead ratios >=1 for both interpreters, obtain an equal first-buffer measurement boundary, and measure complete service enumeration if claiming its parity. The explicit-stream timing is not a repair of the 2.x recorder. Both crates remain performance-audit incomplete.

## Third run, 2026-09-08: final working-tree assessment

**INCOMPLETE / NOT PASSED.** The default backend now caches enumeration and exclusive-format refusals. Shared WASAPI uses event waits, reusable transport buffers and device-period-based buffering. However, the final parent measurement still loses to both Python interpreters for open/close, the unchanged Python production recorder cannot start capture on its recording thread, and the final Rust seven-segment integrity check fails in one of three captures. Passing existing integration tests does not override these failures. No feature registry entry was changed.

The sections headed “Archived second run” below are historical evidence. Their statements about unchanged production code, forbidden sys-win edits and unresolved Python diagnosis describe that earlier run only; this third-run section supersedes them.

### Environment and execution

Same CABLE-A endpoints and 48,000 Hz / two-channel transport as the archived environment. Intel Core i5-12600KF, 10 physical / 16 logical cores; Windows 11 Education build 22621; Rust/Cargo 1.97.0, LLVM 22.1.6. Worker observations reported Discord (six processes), HLConvolverHost and audiodg running. These observations are not proof of endpoint ownership. No user process was stopped and no request to close applications was made. Commands, including worker agents and the synchronous CPU-observer child, ran in the foreground; none were detached.

- CPython 3.14.5: `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe` (`py -3.14`).
- CPython 3.14.7 free-threaded: `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`.
- Both: NumPy 2.5.3, SciPy 1.18.1, sounddevice 0.5.6. PortAudio reports `V19.7.0-devel, revision unknown`.
- NumPy OpenBLAS 0.3.34.106.0 ILP64 / pocketfft; SciPy OpenBLAS 0.3.31.dev LP64 / duccfft.
- OMP_NUM_THREADS, MKL_NUM_THREADS, OPENBLAS_NUM_THREADS and NUMEXPR_NUM_THREADS unset. No thread pinning or installations.
- WASAPI host index 2, input index 39 (`CABLE-A Output(VB-Audio Cable A)`), output index 35 (`CABLE-A Input(VB-Audio Cable A)`). Both channels are explicitly requested. No default-device substitution.

### Method and remaining scope limitations

Three warmups, then five 20-call batches for enumeration and open/close, five headphones runs, three seven-segment runs, five CPU observations during measured headphones sessions and ten latency runs. Rust uses release `cargo bench`. CPU observation reads the Rust process user+kernel totals through psutil with an acknowledged pipe; it is synchronous and waits for the child to exit.

The final Rust wall span includes file read, sample preparation, endpoint resolution, open, playback/capture and stop/join; file writes are outside the span. Python calls the real `core.recorder.play_and_record` with file reading enabled, real blocking `sd.rec` on its recording thread, real blocking `sd.play`, and the real join; only final `write_wav` is replaced with in-memory collection. There are still language-specific bookkeeping differences, and no successful sweep ratio can be established.

Headphones is the bundled 295,270-frame mono sweep duplicated to stereo (6.151458333333 s, transport FNV-1a `d0345121ff98253d`). Seven segments are an explicit stereo file accepted by service speakers file mode, 2,066,890 frames / 43.060208333333 s / hash `4aa88e1cfdf037c1`. It is not a seven-speaker request to the stereo generator. Latency uses 4,800 frames, hash `2a96093a100aaf11`.

`enumerate_backend` measures the backend used by the service, not the complete `list_audio_devices` JSON assembly and filtering. Python enumeration includes `query_devices()` and the recorder host API name list. Its speed ratio is therefore supplementary, not proof of complete service-operation parity. Final `first_sample_latency` measures input-session start entry to the first nonempty 1,024-frame application read return; it is not an OS capture callback timestamp. This requested callback comparison remains incomplete.

### Final parent rerun: verbatim Rust tables

These rows were independently rerun after the temporary public diagnostics API was removed. They are the current code's results, not the earlier worker results.

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 0.026700 | 0.025400 |
| open_close_session | 20 calls/batch | 282.279500 | 271.942100 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6209.446500 | 6205.708300 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 57.988167 | 54.249967 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43146.423100 | 43129.046600 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 86.214767 | 68.838267 |
| first_sample_latency | input session start call to first nonempty application read return; 10 runs | 42.672850 | 40.901200 |
| capture_loop_cpu | process user+kernel; 5 runs | 62.500000 | 0.000000 |

Cold enumeration was 20.863100 ms; warm per-call median is 0.001335 ms. CPU observations were 78.125, 78.125, 15.625, 0.000 and 62.500 ms. The zero observation is a coarse process-counter delta, **not proof that capture consumed no CPU**. Counter granularity and observer bookkeeping prevent a fine-grained CPU speedup claim.

Raw `impulcifer-sys-win` remains uncached to preserve its Copy unit-struct public API. The application cache lives in audio-io.

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 278.228300 | 275.329300 |

### Final parent rerun: verbatim Python tables

CPython 3.14.5:

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.080400 | 2.027400 |
| open_close_session | 20 calls/batch | 246.917600 | 236.802200 |

CPython 3.14.7 free-threaded:

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.025600 | 1.958800 |
| open_close_session | 20 calls/batch | 251.819300 | 237.764400 |

Both full oracle invocations failed during the first headphones warmup with zero input callbacks. Seven-segment and latency were also attempted separately by workers and failed. There are no Python sweep/CPU/latency measurements to compare.

| Operation | Second-run Rust median ms | Final Rust median ms | Final Python/Rust | Final Python-t/Rust | Assessment |
|---|---:|---:|---:|---:|---|
| Warm backend enumeration, 20 | 284.886700 | 0.026700 | 77.917603 | 75.865169 | Cached backend only; complete service op unmeasured |
| Raw sys-win enumeration, 20 | 273.753800 | 278.228300 | 0.007477 | 0.007280 | Slower; fresh versus cached enumeration |
| Open/close, 20 pairs | 325.701600 | 282.279500 | 0.874727 | 0.892092 | Fails both comparisons |
| Headphones wall | 6189.703100 | 6209.446500 | unavailable | unavailable | Before omitted file preparation |
| Headphones overhead | 38.244767 | 57.988167 | unavailable | unavailable | Python capture fails |
| Seven-segment wall | 43100.063300 | 43146.423100 | unavailable | unavailable | Before omitted file preparation |
| Seven-segment overhead | 39.854967 | 86.214767 | unavailable | unavailable | Python capture fails |
| Capture CPU | 93.750000 | 62.500000 | unavailable | unavailable | Different span; coarse counter |
| First nonempty application delivery | 40.994550 | 42.672850 | unavailable | unavailable | Not requested OS callback metric |

All final timed Rust headphones captured/drained 295,270 frames, seven-segment sessions 2,066,890 frames. Render underruns were zero in every measured session. Capture aggregate discontinuity remained true, silent false. These facts alone do not establish integrity.

### impulcifer-audio-io changes

`CachedBackend` is an additive decorator returned by `default_backend()`. It caches DTOs per instance for at most two seconds, ages from enumeration entry, clones results, and returns refresh errors rather than indefinitely serving an expired snapshot. It does not retain COM objects. Refusals are keyed by endpoint ID, host API, direction, rate and channel count. Only `UnsupportedFormat` is memoized. Explicit exclusive calls still fail as exclusive; the unchanged policy functions then report actual shared mode. Tests cover TTL boundaries, refresh failure, clone independence, instance isolation, format/direction keys, transient errors and mode metadata.

No service source edits were needed: the service retains the default backend instance. CPAL transport itself was not changed. Fake-backend smoke tests still exercise every benchmark operation at small sizes; Windows hardware is not required by that smoke test.

### impulcifer-sys-win changes and profiling

- Shared streams use existing safe wasapi event handles with bounded 10 ms wait timeouts. Capture drains available packets before returning a full destination, with bounded backlog and explicit error instead of silent data loss.
- Render waits after priming. Scratch bytes and capture deque capacity are reused; packet conversion no longer allocates a temporary Vec; queue draining uses slice copies.
- Shared buffer duration follows four default device periods. The instrumented worker build measured 1,920 frames in each direction, versus 1,056 earlier. This reduces starvation at the cost of buffering; it did not eliminate all waveform failures.
- Mix format and initialization reuse one client; the unused shared `IsFormatSupported` query was removed. Exclusive format-mask probes preserve transient HRESULTs rather than permanently memoizing them as format rejection.
- No unsafe, SIMD, new native dependency, transport precision change or existing public signature change. A worker initially added public transport diagnostics; the parent rejected and removed that API and all its production bookkeeping before final verification and timing.
- Consequently final public APIs do not expose actual driver buffer sizes, packet timestamps or first-packet-excluded discontinuity counts through the session trait. Earlier instrumented observations are archived below, not represented as final API measurements.
- Safe device notifications exist but require separate registration lifetime/thread handling; the cache uses the authorized two-second TTL instead. No safe MMCSS facility was established in wasapi 0.24; none was added. Exclusive streams retain their previous polling implementation, as CABLE-A rejects exclusive float32.

Profiling was code inspection and narrowed release runs, not sampled attribution. A worker obtained open medians Rust 267.835900 / Python 329.370200 / Python-t 273.628600 ms (ratios 1.229746 / 1.021628), but the final parent rerun above failed both comparisons. The favorable earlier sample is not the verdict. Client initialization, two separate Rust sessions versus Python duplex Stream, and environmental variation remain. No algebraic subtraction of medians is treated as an API cost measurement.

Transport tests preserve exact f32 bit patterns, including signed zero and NaN payload, with no arithmetic reordering. Conversion error in these tests is zero bits. Goldens and tolerances were unchanged and all applicable golden tests passed; no new per-golden numerical error maximum was calculated. Hardware residuals below are a separate failure, not a golden-tolerance adjustment.

### Capture integrity: before, optimized diagnostic build, final code

Diagnostic captures use an extra 250 ms tail, outside the timed production workload. Cross-correlation finds an offset per channel, then least-squares gain and residuals are calculated in float64. Active-reference zero runs and exact repeated 128-frame blocks are checked. Gain fitting can conceal a uniform gain defect, so gain is also recorded. This is not an exhaustive inserted/deleted-frame detector.

The follow-up worker measured three headphones captures before additional draining/buffering fixes, then three after. Full reference overlap was available; these are not tail-truncation counts.

| Capture variant/run | Active zero frames | Residual RMS | Render underruns |
|---|---:|---:|---:|
| Before follow-up / 0 | 1623 | 0.049088397 | 1 |
| Before follow-up / 1 | 502 | 0.435407009 | 1 |
| Before follow-up / 2 | 384 | 0.000243222 | 1 |
| Four-period buffer / 0 | 0 | 0.000002133 | 0 |
| Four-period buffer / 1 | 0 | 0.000002133 | 0 |
| Four-period buffer / 2 | 0 | 0.000002133 | 0 |

In earlier polling captures, first-packet-excluded discontinuity counts were 81,15,5,2,4 and render underruns 60,9,10,2,5. After event waiting, reported discontinuities beyond the first reached zero but waveform gaps persisted. The four-period worker seven-segment captures had ch0 zero-frame counts 449,449,0, all with reported underruns and discontinuities-after-first zero. The two gaps were at aligned `[111,560)`. No masking of initial discontinuity in the production CaptureRead API was performed.

The parent independently captured **three final-code seven-segment sessions after removing diagnostics**, each with 2,078,890 captured frames (including the tail). All had zero reported render underruns, aggregate capture discontinuity true, silent false. Full reference overlap was confirmed in every channel.

| Final capture | Offset frames | ch0 active zero frames / runs >=16 | ch0 residual RMS | ch1 residual RMS | max residual ch0 / ch1 |
|---|---:|---|---:|---:|---|
| 0 | 1632 | 737 / 3 | 0.326074843 | 0.004898486 | 1.786301396 / 0.645267487 |
| 1 | 1392 | 0 / 0 | 0.000001612 | 0.000001396 | 0.000003823 / 0.000003823 |
| 2 | 1392 | 0 / 0 | 0.000001612 | 0.000001396 | 0.000003823 / 0.000003823 |

Capture 0 ch0 gain was 0.786313251; the other fitted gains were approximately 1.000000008. Maximum zero-run length was 480 frames; no exact repeated 128-frame block was found. **Capture 0 fails integrity despite zero reported underruns.** The source of this intermittent corruption remains unresolved; no claim is made that it must be the wrapper rather than the engine/virtual endpoint.

Raw diagnostic captures and fixtures remain under `crates/impulcifer-audio-io/tests/bench_support/`: worker `before-*`, `after-*`, `final-*`, `tail-*`, `followup-*`, and parent `parent-final-play_record_7_speaker_set-{0,1,2}.f32`. They are untracked experiment artifacts, not golden fixtures and not intended for committing. Existing captures were preserved, not overwritten or removed.

### Python production blocker, now reproduced

The oracle pins WASAPI device indices not just defaults but at `sd.rec`, `sd.play` and stream constructors, and restores patches/exception hooks. It now surfaces the real recording-thread exception instead of allowing the production function's successful return to look like a capture success. Both final parent oracle runs still ended with:

```text
core/recorder.py:200 -> sd.rec(... blocking=True)
sounddevice.py:2671 -> self.stream.start()
PortAudioError: Error starting stream ... [PaErrorCode -9999]
RuntimeError: production recorder worker failed
input device=39, output device=35, input_callbacks=0, captured_frames=None
```

The worker ran controlled 4,800-frame input-only tests with the same device. Main-thread recording succeeded. A plain Python worker thread failed twice; initializing COM MTA on that worker with `CoInitializeEx(None, 0)` made both counterpart attempts succeed. This was a diagnostic experiment only; native COM calls were not added to the oracle or repository. The WDM-KS last-host-error text already existed before recording, persisted after a successful main-thread capture, and was unchanged after failure. It is stale error context, not evidence that device 39 changed host APIs.

There is also an independent convenience-API conflict: installed sounddevice.py uses global `_last_callback` (line 104), invokes `stop()` in `start_stream` (2664), and replaces `_last_callback` (2673); `stop` closes that previous stream (415–419). In three controlled tests, starting `sd.play` changed the previously active `sd.rec` stream from `(active=True, closed=False)` to `(False, True)`. In production, recorder.py starts its recording thread at 514–519, invokes blocking play at 539 and joins at 568. Fixing COM alone would not remove this shared-state conflict.

`core/recorder.py` is outside this run's write allowlist, and its blocking/join contract must be retained. It was not modified, and a replacement `playrec` or hand-written input stream was not substituted to manufacture passing oracle numbers. Production Python repair requires a separately authorized change.

### Final verification and reproducible commands

The parent independently ran these commands after the API cleanup, all foreground:

```sh
cargo fmt -p impulcifer-audio-io -p impulcifer-sys-win -- --check
cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-audio-io -p impulcifer-sys-win
cargo test -p impulcifer-policy
cargo test -p impulcifer-service --test recording
cargo test -p impulcifer-service --test recording recording_virtual_cable_end_to_end -- --ignored --exact --nocapture
cargo test -p impulcifer-audio-io --test hardware wasapi_session_virtual_cable -- --ignored --exact --nocapture
PYTHONIOENCODING=utf-8 py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --observe-rust cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
PYTHONIOENCODING=utf-8 py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py
PYTHONIOENCODING=utf-8 C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe tests/migration/bench_oracle_impulcifer_audio_io.py
IMPULCIFER_PA05_OP=play_record_7_speaker_set IMPULCIFER_PA05_INTEGRITY=1 IMPULCIFER_PA05_TAIL=1 IMPULCIFER_PA05_CAPTURE_PREFIX=E:/Impulcifer/crates/impulcifer-audio-io/tests/bench_support/parent-final cargo bench -p impulcifer-audio-io --bench perf
```

Format and clippy passed. Test output counts: audio-io 11 unit + 13 fake/smoke passed; sys-win 11 unit + 1 enumeration smoke passed; policy 6 passed; service recording 13 passed. Default test runs ignored five audio hardware tests and one service hardware test. The two CABLE-A-specific tests were then explicitly run and passed. Parent total is **57 passed**, counting those two explicit hardware runs, and **four other hardware tests remain unrun** (including CPAL and default-device tests).

Parent service hardware output: 878,540 frames, 48 kHz stereo, sweep detection found two segments. Parent WASAPI test: 96,000 captured / 48,000 submitted and drained frames, left RMS `-inf`, right RMS `-26.037964063675236 dBFS`, underruns 0. A worker's earlier run of this unchanged WASAPI hardware test failed with left RMS `-51.725489620444726 dBFS` against `< -60`; its retry passed. The later passes do not erase that intermittent failure.

Both final Rust benchmarks exited 0. Both Python full benchmarks exited 1. Workers also ran scoped Python compile/ruff and diff checks. No full workspace test/PR/CI claim is made.

### Files and outstanding work

Production additions are `crates/impulcifer-audio-io/src/cached_backend.rs`, its registration/default-backend wiring in `src/lib.rs`, and optimizations in `crates/impulcifer-sys-win/src/lib.rs`. Bench registrations, both benches, both bench-support modules, audio-io fake/smoke tests, sys-win enumeration smoke and `tests/migration/bench_oracle_impulcifer_audio_io.py` retain and extend the earlier scaffold. This report was updated by the parent. Existing workspace dev dependency `impulcifer-io` remains the WAV reader; no new external crate was installed. Other workers' service/apps/jobs/lockfile changes are not attributed to PA05.

Registry test names remain `bench_smoke_impulcifer_audio_io` and `bench_smoke_impulcifer_sys_win`. No features.toml, unsafe budget, golden/exporter, frontend, production Python, README, CHANGELOG, version, commit or push change was made for PA05.

Still required: fix and authorize the Python production recorder, eliminate/reliably diagnose waveform corruption, achieve open/close parity on repeat measurements, measure the complete service enumeration operation, obtain callback/buffer diagnostics without changing the forbidden public API, and rerun complete valid comparisons. Neither crate is declared performance-audited.

API references checked by workers: [StreamMode](https://docs.rs/wasapi/0.24.0/wasapi/enum.StreamMode.html), [Handle](https://docs.rs/wasapi/0.24.0/wasapi/struct.Handle.html), [AudioClient](https://docs.rs/wasapi/0.24.0/wasapi/struct.AudioClient.html), [DeviceEnumerator](https://docs.rs/wasapi/0.24.0/wasapi/struct.DeviceEnumerator.html), [WasapiError](https://docs.rs/wasapi/0.24.0/wasapi/enum.WasapiError.html).

## Archived second run

Date: 2026-09-08, second run. **Status: audit incomplete and performance gate not passed.** Rust hardware measurements finished. The Python production recorder failed to start capture on both tested interpreters. Session open/close is slower in Rust: Python/Rust = **0.694415** on CPython 3.14.5. Missing measurements are not zero and are not passing results. No registry entry, production source, public API or golden tolerance was changed.

The first run's requirement to close other audio applications was withdrawn. This run removed that obsolete prerequisite and measured with applications running. No application was closed or terminated, and no request to close one was made.

## 1. Environment

- CPU: Intel Core i5-12600KF, 10 physical cores / 16 logical processors.
- OS: Windows 11 Education, build 22621.
- Rust: rustc 1.97.0 / Cargo 1.97.0, x86_64-pc-windows-msvc, LLVM 22.1.6.
- CPython: `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`, 3.14.5.
- Free-threaded CPython: `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`, 3.14.7, GIL disabled. The recorder uses a recording thread, so both interpreters were attempted.
- Both interpreters: NumPy 2.5.3, SciPy 1.18.1, sounddevice 0.5.6.
- NumPy BLAS: OpenBLAS 0.3.34.106.0, ILP64. SciPy BLAS: OpenBLAS 0.3.31.dev, LP64. NumPy FFT: pocketfft; installed SciPy FFT: duccfft.
- `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `NUMEXPR_NUM_THREADS`: unset. No thread pinning or artificial single-thread limit.
- Read-only process observations during the worker's run: Discord PIDs 3128, 13112, 17004, 17824, 19580, 20044; HLConvolverHost PID 71740; audiodg PID 113280. These are process observations, not endpoint-ownership measurements.
- Output: `CABLE-A Input (VB-Audio Cable A)`; input: `CABLE-A Output (VB-Audio Cable A)`; Windows WASAPI, 48,000 Hz, two stream channels. Windows reported names without the space before `(`. Selection allows only that whitespace difference, requires the full name and appropriate channel direction, and refuses ambiguous matches. No default-device substitution.
- Python selected WASAPI host API index 2, input device 39 and output device 35. Input advertises two channels; output advertises eight. Both streams use two channels.

No new external dependency or tool was installed. The existing workspace `impulcifer-io` crate was added as an audio-io dev dependency to read the bundled WAV.

## 2. Method and workload boundaries

Three warmups preceded each operation. Measured repetitions follow the hardware packet: 20-call enumeration/open-close batches repeated five times; headphones five times; seven segments three times; latency ten times. Process CPU was measured during the same five measured headphones sessions. All commands were foreground, including the synchronous Python parent observing a Rust child. The parent services an acknowledged pipe protocol and waits for the child to exit; it does not detach or leave a process running.

| Workload | Frames | Duration seconds | Matching Rust/Python float32 transport FNV-1a |
|---|---:|---:|---|
| Headphones, bundled mono sweep duplicated to stereo | 295270 | 6.151458333333 | `d0345121ff98253d` |
| Seven consecutive stereo segments | 2066890 | 43.060208333333 | `4aa88e1cfdf037c1` |
| Latency fixture | 4800 | 0.100000000000 | `2a96093a100aaf11` |

The source is `data/sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav`, mono PCM32. Transport conversion uses the existing production float32 format; no DSP precision or production format changed.

**Scope qualifications**

- `enumerate_backend` measures raw production backend enumeration. It does not include service `list_audio_devices` host-list construction, filtering, default-index lookup and JSON response. The requested complete service operation remains unmeasured. The enumeration ratios below are supplementary, not parity results.
- The seven-segment fixture alternates left/right output using seven complete bundled sweeps. Both implementations use matching transport samples. It is an explicit benchmark file, not a seven-speaker request to the stereo generator. The actual generators accept at most two stereo speakers and add silence. Consequently this is not verified as the requested complete service speakers-mode workload; it must not be represented as such.
- Rust wall timing wraps production `session::play_and_record`, excluding fixture preparation and file saving. Python timing wraps the complete `core.recorder.play_and_record`, including loading, device resolution and saving. These boundaries must be aligned before a successful future sweep comparison. No sweep ratio is claimed here.
- Rust `first_sample_latency` is capture-session start entry to first nonempty `read_into` return. It is not the inaccessible WASAPI buffer callback timestamp. Python's intended boundary is `sd.play` entry to the first nonempty input callback. The recording thread starts before `sd.play`, so this is a different boundary and can in principle yield a negative value. No callback ratio is claimed.
- Bench-only Rust observation takes a mutex and timestamp after each read. Python observation wraps stream callbacks. Observer overhead is included; no uninstrumented timing delta was established.

## 3. Verbatim measured tables

These are the worker's release/CPython benchmark rows, preserved verbatim. The parent reviewed the implementation and independently reran correctness gates and the failing Python headphones operation; it did not repeat all long Rust measurements.

### impulcifer-audio-io, Rust release

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 284.886700 | 273.871300 |
| open_close_session | 20 calls/batch | 325.701600 | 320.520500 |
| play_record_headphones_sweep | 295270 frames; 5 runs | 6189.703100 | 6187.748800 |
| play_record_headphones_sweep_overhead | wall minus playback duration | 38.244767 | 36.290467 |
| play_record_7_speaker_set | 2066890 frames; 3 runs | 43100.063300 | 43093.487000 |
| play_record_7_speaker_set_overhead | wall minus playback duration | 39.854967 | 33.278667 |
| first_sample_latency | start to nonempty read delivery; 10 runs | 40.994550 | 40.204100 |
| capture_loop_cpu | process user+kernel; 5 runs | 93.750000 | 78.125000 |

### impulcifer-sys-win, Rust release

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| enumerate_backend | 20 calls/batch | 273.753800 | 269.345500 |

Production sys-win capture/render sessions are also exercised by the audio-io table. Those sessions are not separately measured direct sys-win session operations.

### Python 3.14.5

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.094200 | 2.033800 |
| open_close_session | 20 calls/batch | 226.172200 | 221.754200 |

### Python 3.14.7 free-threaded

| op | size | python median ms | python min ms |
|---|---|---:|---:|
| enumerate_devices | 20 calls/batch | 2.118800 | 2.016200 |
| open_close_session | 20 calls/batch | 238.331000 | 225.851200 |

Python sweeps, capture CPU and first-callback results are unavailable because capture failed during warmup.

### Comparison and before/after status

| Comparison | Python 3.14.5 / Rust | Python 3.14.7t / Rust | Before | After |
|---|---:|---:|---|---|
| open_close_session | 0.694415 | 0.731746 | Measured; below 1.0 | No production optimization; no after measurement |
| Python enumeration / audio backend enumeration | 0.007351 | 0.007437 | Supplementary, unequal scope | No production optimization; no after measurement |
| Python enumeration / sys-win backend enumeration | 0.007650 | 0.007740 | Supplementary, unequal scope | No production optimization; no after measurement |
| Headphones overhead | unavailable | unavailable | Python capture failed | Not measured |
| Seven-segment overhead | unavailable | unavailable | Python capture failed; service workload not verified | Not measured |
| Capture CPU | unavailable | unavailable | Python capture failed | Not measured |
| First captured callback | unavailable | unavailable | Rust callback unobservable; Python capture failed | Not measured |

Rust open/close follows exclusive-first policy; Python `sd.Stream` follows the 2.x shared policy with per-direction WASAPI settings. This compares existing policies rather than identical opening attempts. Rust is slower for the requested policy workload, but the ratio is not a comparison of equal numbers of low-level API calls.

## 4. Raw measured samples

Milliseconds, in execution order, excluding warmups.

```text
Rust audio enumeration:
287.839600, 290.942200, 277.244800, 284.886700, 273.871300
Rust sys-win enumeration:
269.345500, 279.864700, 276.018900, 272.978800, 273.753800
Rust open/close:
323.141800, 330.625200, 329.487200, 320.520500, 325.701600
Rust headphones wall:
6191.277000, 6189.703100, 6189.340400, 6192.040200, 6187.748800
Rust headphones overhead:
39.818667, 38.244767, 37.882067, 40.581867, 36.290467
Rust headphones CPU:
93.750000, 109.375000, 109.375000, 78.125000, 78.125000
Rust seven-segment wall:
43100.063300, 43093.487000, 43104.018300
Rust seven-segment overhead:
39.854967, 33.278667, 43.809967
Rust first nonempty delivery:
41.088100, 42.437200, 40.901000, 40.306200, 41.538700,
40.638300, 42.538800, 40.756900, 40.204100, 41.902200
Python 3.14.5 enumeration:
2.238000, 2.036900, 2.094200, 2.033800, 2.153300
Python 3.14.5 open/close:
231.653800, 226.172200, 221.754200, 225.429800, 229.328200
Python 3.14.7t enumeration:
2.046100, 2.118800, 2.243000, 2.016200, 2.144300
Python 3.14.7t open/close:
246.855400, 244.980200, 238.331000, 228.738300, 225.851200
```

## 5. impulcifer-audio-io diagnostics and smoke coverage

Bench helpers wrap the unchanged production backend and recording session. They check captured/drained lengths and timestamp only actual nonempty deliveries, rather than entry to a polling call.

Both input and output actually rejected exclusive float32 with `Could not find a compatible format`, then opened with `SharedAutoConvert`. This is a fresh PA05 observation, not an inference from the older HARDWARE.md run.

- All measured headphones sessions captured and drained 295270 frames; all seven-segment sessions captured and drained 2066890 frames.
- Headphones reported render underruns `1, 1, 1, 2, 1`; seven-segment sessions reported `1, 2, 1`.
- Capture discontinuity was `true` in every measured run. Capture silent flag was `false`.
- First delivered capture block: 1024 frames. Application reads: 289 per headphones session, 2019 per seven-segment session; no empty deliveries.
- These are the backend's flags/counters. Exact underlying xrun timing and packet-level discontinuity counts are not exposed, so the flags do not prove audio integrity. Exact frame counts alone do not prove an undamaged capture.
- Actual driver buffer sizes are unavailable through the existing session trait. The 1024-frame application delivery must not be relabeled as a negotiated driver buffer size.
- CPU is psutil process user+kernel time read by a synchronous parent at acknowledged session boundaries. Compilation, fixture loading and warmups are excluded; boundary pipe bookkeeping is included. Observed CPU values have 15.625 ms granularity.

`bench_smoke_impulcifer_audio_io` uses the fake backend to exercise enumeration, refusal of a fake/default endpoint as CABLE-A, policy opening, tiny one/seven-segment recordings, wall/overhead calculation, first nonempty delivery and error propagation. CPU boundary helpers run with observation disabled; CI does not test the real psutil/pipe exchange. `bench_playback_set_preserves_transport_and_segment_routing` checks sample preservation and alternating channel routing. These tests do not validate real hardware callbacks or the full service workload.

## 6. impulcifer-sys-win and profiling

The standalone bench measures real endpoint enumeration. `bench_smoke_impulcifer_sys_win` is Windows-only, accepts an empty endpoint list and propagates actual enumeration errors. No sys-win source, public API or unsafe budget changed.

All available ratios below 1.5 were investigated through code inspection and narrower measurements. Verbatim Rust release profiling rows:

| op | size | rust median ms | rust min ms |
|---|---|---:|---:|
| profile_shared_pair | 20 calls/batch | 331.878400 | 324.908600 |
| profile_exclusive_rejections | 20 calls/batch | 80.551800 | 78.932500 |

Python's separate 20-call medians were 1.778400 ms for `query_devices` and 0.227400 ms for `query_hostapis`.

- Enumeration: sys-win performs fresh COM/device enumeration and mix-format queries. PortAudio queries its cached device information. The markedly different costs therefore include different freshness semantics; caching Rust enumeration would need an explicit invalidation policy.
- Session opening: Rust performs exclusive rejection attempts and separately initializes capture/render clients. Python opens a shared duplex Stream. Shared-only measurements taken later were slower than the earlier exclusive-first batch, demonstrating variability; subtracting these medians would not be a valid cost decomposition.
- Capture implementation inspection found packet-conversion allocation, VecDeque copies and 1 ms polling in sys-win, plus 5 ms coordination polling in audio-io. These are implementation observations, not sampled CPU attribution. No measured lock-contention percentage or sample-conversion speedup is claimed.
- No source optimization was made. The identified backend operations are in read-only sys-win code. Skipping exclusive attempts or returning stale devices from audio-io would change semantics. Neither was done.

There is no new numerical maximum error to report: production computations and summation order did not change. Correctness tests and policy tests pass unchanged. A passing correctness suite is not a passing performance audit.

## 7. Python recorder failure

Both CPython versions failed during the first headphones warmup. Separate seven-segment and latency attempts also failed. The worker reproduced the issue with uninstrumented `core.recorder.play_and_record`; it printed completion after the recording thread failed but produced no capture file. Input-only streams and `sd.rec` without simultaneous playback succeeded. The underlying simultaneous-playback/capture failure has not been conclusively diagnosed.

The parent independently reran:

```sh
PYTHONIOENCODING=utf-8 py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py --op play_record_headphones_sweep
```

It exited 1. Relevant output:

```text
Exception in thread Thread-1 (record_target):
  File "E:\Impulcifer\core\recorder.py", line 200, in record_target
    recording = _sounddevice().rec(length, samplerate=fs, channels=channels, blocking=True)
sounddevice.PortAudioError: Error starting stream: Unanticipated host error [PaErrorCode -9999]: 'WdmSyncIoctl: DeviceIoControl GLE = 0x00000490 (prop_set = {8C134960-51AD-11CF-878A-94F801C10000}, prop_id = 10)' [Windows WDM-KS error 0]
AssertionError: ... 'input_callbacks': 0, 'output_callbacks': 617, ... 'captured_frames': None
```

Observed stream metadata in that parent run: output device 35 and input device 39, float32, two channels, 48000 Hz, requested blocksize 0 (host-selected), reported latency 0.022 seconds on each stream. No input callback arrived. The error's WDM-KS wording does not establish device substitution: observed stream device identities were WASAPI.

The oracle refuses to report successful timing when the recording thread fails. No production `sd.play(blocking=True)` or `Thread.join()` behavior was changed, and no replacement recorder was substituted to produce a passing result.

## 8. Commands and verification

All commands were foreground. Bench entry points are:

```sh
cargo bench -p impulcifer-audio-io --bench perf
cargo bench -p impulcifer-sys-win --bench perf
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_audio_io.py
```

The worker reported both Rust benches successful and both default Python oracle runs failed. Its complete shell transcript is not embedded in this report; the following commands reproduce per-operation selection and the synchronous CPU measurement used for the reported results (they are not a claim that every line below was independently rerun by the parent):

```sh
IMPULCIFER_PA05_OP=play_record_headphones_sweep py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --observe-rust cargo bench -p impulcifer-audio-io --bench perf
IMPULCIFER_PA05_OP=play_record_7_speaker_set cargo bench -p impulcifer-audio-io --bench perf
IMPULCIFER_PA05_OP=first_sample_latency cargo bench -p impulcifer-audio-io --bench perf
IMPULCIFER_PA05_OP=profile_open cargo bench -p impulcifer-audio-io --bench perf
py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --environment-only
py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --op play_record_7_speaker_set
py -3.14 tests/migration/bench_oracle_impulcifer_audio_io.py --op first_sample_latency
```

The parent independently ran this exact foreground validation chain after reading the helper, Python oracle and changed test code:

```sh
cargo fmt -p impulcifer-audio-io -p impulcifer-sys-win -- --check && cargo clippy -p impulcifer-audio-io -p impulcifer-sys-win --all-targets -- --no-deps -D warnings && cargo test -p impulcifer-audio-io && cargo test -p impulcifer-sys-win && cargo test -p impulcifer-policy
```

Formatting and clippy passed. Test output, in order:

```text
# audio-io unit tests
 test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
# audio-io hardware tests
 test result: ok. 0 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.00s
# audio-io fake sessions and benchmark properties
 test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.25s
# sys-win unit tests
 test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
# sys-win enumeration smoke
 test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.24s
# sys-win hardware tests
 test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
# policy tests
 test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.62s
```

Total: **35 passed, 5 hardware tests ignored, 0 failed**. Doctests contained no tests. Hardware tests above were not forcibly enabled because some target default devices; the explicit CABLE-A benches performed this audit's hardware runs. The worker also reported Python AST syntax and scoped diff checks passed.

## 9. Files changed

PA05 files relative to repository HEAD, including the retained first-run scaffolding:

- `crates/impulcifer-audio-io/Cargo.toml` (bench registration and existing workspace dev dependency)
- `crates/impulcifer-audio-io/benches/perf.rs`
- `crates/impulcifer-audio-io/tests/bench_support/mod.rs`
- `crates/impulcifer-audio-io/tests/session_fake.rs`
- `crates/impulcifer-sys-win/Cargo.toml` (bench registration)
- `crates/impulcifer-sys-win/benches/perf.rs`
- `crates/impulcifer-sys-win/tests/bench_support/mod.rs`
- `crates/impulcifer-sys-win/tests/bench_smoke.rs`
- `tests/migration/bench_oracle_impulcifer_audio_io.py`
- `docs/rust/perf/impulcifer-audio-io.md`

Concurrent changes in apps, service, jobs and Cargo.lock were preserved and are not attributed to PA05. No manual lockfile edit, commit, push, stash or revert. No frontend wiring, catalog, README, release version, CHANGELOG or feature-registry modification was made under this packet's restricted allowed-file list.

## 10. Remaining work

1. Diagnose the production Python simultaneous capture/playback failure without changing the required recorder semantics or stopping other applications. Successful playback-only timings are not an oracle.
2. Measure complete `list_audio_devices` and verify the exact requested service speakers-mode workload, rather than promoting supplementary backend/fixture measurements to those operations.
3. Align Rust/Python wall and CPU boundaries, and obtain an agreed first-buffer observation boundary. Actual Rust callback timestamps and driver-buffer diagnostics are unavailable under the present public API restriction.
4. Investigate reported Rust underruns/discontinuities and validate capture content; frame counts alone are insufficient.
5. Optimize the slower session operation only within an explicitly authorized source scope, then repeat same-machine comparisons and unchanged goldens. The sys-win source restriction remains in force.
6. Record all missing Python samples and valid ratios. Neither crate is performance-audited until every required operation has a valid ratio of at least 1.0.
