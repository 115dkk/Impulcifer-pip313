# A07 결과 — 구현 초안, 검증 미완료

**A07 합격 상태는 아닙니다.** RC 라이브러리와 TRF 포트, 기준 테스트, 벤치마크를 구현했습니다. 전체 DSP 테스트는 **208 passed / 1 failed**이며, 실패한 테스트는 `room_tuning2_matches_secs2_rc`입니다. 큰 오차를 숨기기 위해 허용치를 늘리지는 않았습니다.

SECS 소스·바이너리와 `/tmp/claude-0/`는 열지 않았습니다. 모든 명령은 포그라운드로 실행했고 종료까지 확인했습니다. 브랜치 전환·stash·commit은 하지 않았습니다. 작업 중 외부에서 `6eb256c wip(tuning2): channel correction core in progress` 커밋이 생겨, 아래 상태에는 그 이후 변경만 나타납니다.

## 1. 구현된 공개 API

`impulcifer_dsp::stages::room_tuning2`:

- `Tuning2Mode::{Zero, Mixed, Linear, Long, Ultra(f64)}`
- `Tuning2Mode::parse`, `params`
- `ModeParams { taps, peq_bands, lift_ms, phase_frac }`
- `PositionPolicy::{Symmetric, Listening { index, weight }}`
- `RcOptions`와 지정 기본값
- `BandSource`, `CorrectionBand`
- `correction_band`, `channel_level`, `analysis_len`
- 지정 필드 전체를 가진 `SpeakerRc`
- `design_rc`

구현 파일:

- `crates/impulcifer-dsp/src/stages/room_tuning2/mod.rs`
- `crates/impulcifer-dsp/src/stages/room_tuning2/primitives.rs`
- `crates/impulcifer-dsp/src/stages/room_tuning2/peq.rs`
- `crates/impulcifer-dsp/src/stages/room_tuning2/rc.rs`
- `crates/impulcifer-dsp/src/optimize.rs`

추가 사항:

- `filters::butter_bandpass`
- faer **0.24.4**, `default-features=false`, `std+linalg`만 사용
- SVD 호출에 **`Par::Seq` 명시**
- faer 라이선스 표기와 SciPy BSD-3 원문 추가
- MIMO·BRIR 적용·서비스·CLI·UI·버전·CHANGELOG는 변경하지 않음

**API 명세의 미해결점:** `channel_level`은 `f64`, `correction_band`는 `CorrectionBand`를 반환하므로 `DspError`를 반환할 수 없습니다. 현재 잘못된 입력에는 NaN을 반환하고, `design_rc`는 `DspError::InvalidArgument`로 거부합니다.

## 2. Least-squares 포트와 검증

포트한 경로:

- bounded `trf_bounds`
- Coleman–Li scaling
- exact thin-SVD trust-region solve
- 반사·경계 제한·최급강하 후보 선택
- trust-radius 갱신
- SciPy의 종료 검사와 `nfev` 계산
- 기본 상대 스텝의 bound-aware `2-point` Jacobian
- `x_scale=1`, linear loss, 세 tolerance `1e-8`

PEQ residual은 변경된 밴드 응답을 캐시하며, 전체 응답은 **밴드 순서대로 다시 곱합니다**. 기존 곱에서 밴드를 나누어 제거하지 않습니다.

최종 SciPy 1.18.1 기준:

| 문제 | 최대 x 차이 | cost 차이 | nfev Rust/SciPy | status |
|---|---:|---:|---:|---:|
| smooth curve | 5.95248e-11 | 3.90313e-18 | 8 / 8 | 1 / 1 |
| active bound | 0 | 0 | 7 / 7 | 1 / 1 |
| synthetic PEQ16 | 0.008001162 | 0.022031750 | 150 / 150 | 0 / 0 |

PEQ16의 named bounds는 실측 ×1.5보다 크게 반올림한:

```rust
const PEQ_X_MAX: f64 = 0.013;
const PEQ_COST_MAX: f64 = 0.034;
```

두 상수에 요청한 provisional 주석을 넣었습니다.

**포트의 완전한 경로 일치는 아직 입증되지 않았습니다.** 이전 synthetic curve에서는 복소수 나눗셈을 NumPy 방식으로 맞춘 뒤 Rust가 140회에서 `ftol` 종료하고 SciPy는 150회에 도달했습니다. 최종 fixture는 더 어려운 독립 곡선으로 양쪽 모두 150회에 도달하지만, 앞선 사례는 수치 경로 민감성의 미해결 증거입니다.

## 3. 발견한 차이와 원인

### 확인한 원인

1. **복소수 나눗셈의 산술 순서**
   - 일반 분모 나눗셈과 reciprocal-multiply는 반올림이 다릅니다.
   - NumPy와 무작위 복소수 10,000쌍 비교:
     - 직접 나눗셈: 5,769쌍 bit-identical
     - reciprocal-multiply: **10,000쌍 bit-identical**
   - 후자를 채택했습니다.
   - 이 변경만으로 single FR-left의 최종 최대 크기 오차가 약 **1.847 → 0.0684 dB**로 줄었지만, 다른 사례는 악화했습니다. finite-difference/TRF 경로 민감성이 큽니다.

2. **대칭 정책의 공통 onset 기준**
   - 첫 위치의 onset을 그대로 쓰면 위치 교환 시 공통 위상이 달라져 반올림 경로가 바뀔 수 있습니다.
   - `Symmetric`에서는 가장 이른 onset을 공통 기준으로 사용합니다.
   - 실제 demo FL 양쪽 위치를 교환하는 테스트가 **1e-9 dB / 1e-9 rad** 조건을 통과합니다.

3. **단일 위치 가중치의 불필요한 곱셈·나눗셈**
   - 단일 위치의 가중치를 1로 정규화했습니다.
   - 실제 demo FL-left에서 Symmetric과 Listening(5)의 응답 일치 테스트가 통과합니다.

4. **설계 곡선 골든의 정밀도**
   - `design_db`는 약 6자리 유효숫자로 저장돼 있습니다.
   - 관측 최대 차이는 약 **5.5e-6 dB**로 저장 정밀도 수준입니다.
   - 이 골든으로 **1e-9 dB** 일치를 확인할 수는 없습니다.

5. **골든 내부의 독립 스피커 불일치**
   - 동일한 측정·mode·weight·target·band 설정인데, 단독 실행과 `all/mixed`가 다릅니다:

   | 스피커 | 골든 all vs 단독 최대 크기 차이 |
   |---|---:|
   | FL | 3.21627 dB |
   | FR | 2.17019 dB |
   | FC | 0.44773 dB |
   | BL | 0.00060 dB |
   | SL | 0.00130 dB |
   | SR | 4.00761 dB |
   | BR | 0.00338 dB |

   SR의 strength도 all=0.3, 단독=0.7입니다. 원인이 블랙박스의 수치 경로인지 다른 실행 상태인지 현재 자료만으로 확정할 수 없습니다.

6. **Zero 메타데이터**
   - 명세/API: FIR 없음, 빈 `fir`.
   - 골든: `fir_taps=1`, `phase_frac_used=6`.
   - 현재 구현은 빈 FIR과 β=0을 반환하며, 테스트는 이 차이를 실패로 드러냅니다.

### 아직 원인을 끝까지 분리하지 못한 차이

- 일부 사례에서 첫 Fit부터 밴드 파라미터가 달라집니다.
- 그 차이가 두 번째 greedy Fit의 밴드 선택과 phase-strength 선택으로 증폭됩니다.
- 대표적으로 single BL-left의 두 번째 Fit은 최대 주파수 차이가 **119.39 Hz**, gain 차이가 **7.06 dB**입니다.
- 첫 Fit·실현·두 번째 Fit·touch-up을 각각 독립 입력으로 고정하는 추가 검증이 필요합니다. 모든 큰 오차를 “반올림 때문”으로 확정하지 않았습니다.

정확한 strength 불일치:

| 사례 | Rust | SECS |
|---|---:|---:|
| all/mixed/SR | 0.7 | 0.3 |
| two-main5/FR/mixed | 0.9 | 0.7 |
| two/FR/mixed | 0.7 | 0.9 |
| two/FR/ultra-60 | 0.7 | 0.9 |

Zero를 제외한 선택 β와 FIR 길이는 일치했습니다.

## 4. 전체 기준 비교

각 벡터의 대역 순서:

**15–100 / 100–500 / 500–2000 / 2000–23000 Hz**

- M: 최대 크기 오차, dB
- P: 최대 wrapped phase 오차, rad
- 모든 사례의 최종 **PEQ 개수는 일치**
- gain 오차는 모든 사례에서 **1e-8 dB 미만**
- `rc_hi`, 존재하는 `schroeder_hz` 비교는 **1e-6 Hz** 조건 통과

아래는 마지막 release 실행 결과를 소수 넷째 자리까지 줄인 표입니다.

```text
사례                              M                                     P
all/mixed/BL             .0015 .0006 .0001 .0000             .0002 .0001 .0000 .0000
all/mixed/BR             .0006 .0047 .0048 .0004             .0001 .0006 .0005 .0001
all/mixed/FC             .3824 .0273 .0621 .0037             .0475 .0070 .0069 .0014
all/mixed/FL            1.2693 3.2169 .0754 .0088             .1034 .3001 .0077 .0028
all/mixed/FR            2.4777 .7728 .0308 .0007              .3381 .0873 .0042 .0042
all/mixed/SL             .0014 .0003 .0001 .0000             .0001 .0000 .0000 .0000
all/mixed/SR            1.7667 4.0078 .5082 .0103            1.1407 2.9624 2.7444 2.7281

single/BL/left          1.1850 .2816 .0039 .0003              .1407 .0286 .0008 .0003
single/BL/right          .0040 .0210 .0057 .0004             .0007 .0021 .0008 .0002
single/BR/left           .0011 .0004 .0000 .0000             .0002 .0001 .0000 .0000
single/BR/right          .0010 .0003 .0001 .0000             .0001 .0000 .0000 .0000
single/FC/left           .0674 .0247 .0243 .0017             .0133 .0050 .0039 .0037
single/FC/right          .6383 .4119 .0122 .0003             .0804 .0374 .0002 .0001
single/FL/left           .0013 .0005 .0001 .0000             .0001 .0001 .0000 .0000
single/FL/right          .0022 .0026 .0001 .0000             .0002 .0003 .0000 .0000
single/FR/left           .0684 .0147 .0013 .0001             .0095 .0026 .0003 .0000
single/FR/right          .9486 .1164 .1179 .0057             .1280 .0174 .0150 .0015
single/SL/left           .3063 .0122 .0045 .0003             .0372 .0038 .0005 .0002
single/SL/right          .4959 .1976 .1749 .0112             .0635 .0215 .0186 .0034
single/SR/left           .0035 .0030 .0001 .0000             .0006 .0004 .0000 .0000
single/SR/right          .6362 .1291 .0079 .0005             .0706 .0125 .0010 .0001

two-main5/BL             .0061 .0345 .0033 .0003             .0009 .0055 .0011 .0002
two-main5/BR             .0001 .0006 .0001 .0000             .0000 .0001 .0000 .0000
two-main5/FC             .6357 .0235 .0045 .0004             .0700 .0039 .0010 .0000
two-main5/FL             .0273 .0047 .0011 .0001             .0031 .0004 .0002 .0000
two-main5/FR             .8161 1.9777 .5124 .0481             .5222 .9143 .7197 .7037
two-main5/SL             .2386 .0152 .0014 .0000             .0222 .0018 .0001 .0001
two-main5/SR             .0031 .0023 .0003 .0000             .0004 .0003 .0001 .0000

two/BL/linear            .0341 .0185 .0045 .0001             .0109 .0026 .0004 .0000
two/BL/long              .1776 .1586 .4064 .0493             .0388 .0464 .0553 .0119
two/BL/mixed             .0010 .0004 .0002 .0000             .0001 .0000 .0000 .0000
two/BL/ultra-60          .0170 .0038 .0003 .0000             .0025 .0005 .0001 .0000
two/BL/zero              .0009 .0034 .0002 .0000             .0001 .0004 .0000 .0000
two/BR/linear            .0007 .0037 .0048 .0004             .0001 .0006 .0004 .0001
two/BR/long              .0092 .0020 .0002 .0000             .0011 .0002 .0000 .0000
two/BR/mixed             .0010 .0041 .0053 .0004             .0001 .0006 .0006 .0001
two/BR/band30-300        .0066 .0015 .0001 .0000             .0007 .0002 .0000 .0000
two/BR/ultra-60          .0050 .0112 .0051 .0004             .0008 .0011 .0005 .0001
two/BR/zero              .0016 .0024 .0019 .0000             .0002 .0003 .0001 .0000
two/FC/linear            .1061 .0118 .0058 .0003             .0111 .0090 .0080 .0082
two/FC/long             1.3004 .0357 .0223 .0015              .1090 .0067 .0009 .0006
two/FC/mixed             .3803 .0204 .0009 .0001             .0432 .0028 .0006 .0005
two/FC/boost6            .0011 .0202 .0088 .0002             .0001 .0021 .0009 .0001
two/FC/ultra-60          .2323 .0129 .0049 .0003             .0271 .0035 .0015 .0014
two/FC/zero              .5959 .0345 .0029 .0000             .0675 .0047 .0005 .0000
two/FL/linear            .0102 .0062 .0006 .0000             .0011 .0008 .0000 .0000
two/FL/long              .0020 .0011 .0007 .0000             .0002 .0002 .0000 .0000
two/FL/mixed             .0007 .0006 .0002 .0000             .0001 .0001 .0000 .0000
two/FL/target            .1734 .0514 .0018 .0000             .0244 .0073 .0006 .0001
two/FL/volume40          .0014 .0032 .0008 .0001             .0001 .0004 .0000 .0000
two/FL/ultra-60          .0016 .0003 .0001 .0000             .0002 .0001 .0000 .0000
two/FL/zero              .0006 .0002 .0000 .0000             .0001 .0000 .0000 .0000
two/FR/linear            .2172 .3877 .1312 .0036             .0264 .0515 .0233 .0011
two/FR/long              .6281 .2141 .1032 .0026             .0941 .0256 .0088 .0015
two/FR/mixed            2.3001 .3432 .2717 .0420              .5008 .8296 .7136 .7009
two/FR/ultra-60         2.4401 1.1520 .0500 .0035             .9020 2.6324 2.6016 2.6017
two/FR/zero              .1439 .3133 .1572 .0011             .0171 .0370 .0145 .0005
two/SL/linear            .0002 .0021 .0005 .0000             .0000 .0003 .0000 .0000
two/SL/linear/target     .0258 .0078 .0004 .0000             .0035 .0007 .0001 .0000
two/SL/long              .0007 .0003 .0000 .0000             .0001 .0000 .0000 .0000
two/SL/mixed             .0006 .0010 .0003 .0000             .0001 .0001 .0000 .0000
two/SL/ultra-60          .0010 .0038 .0005 .0000             .0001 .0004 .0001 .0000
two/SL/zero              .0008 .0030 .0039 .0001             .0001 .0003 .0004 .0000
two/SR/linear            .0013 .0006 .0004 .0000             .0002 .0001 .0000 .0000
two/SR/long              .0016 .0020 .0007 .0000             .0002 .0003 .0000 .0000
two/SR/mixed             .0016 .0085 .0006 .0000             .0002 .0012 .0000 .0000
two/SR/ultra-60          .0006 .0010 .0003 .0001             .0001 .0001 .0000 .0000
two/SR/zero              .0043 .0540 .0464 .0001             .0005 .0082 .0057 .0001
```

모든 single은 mixed, 모든 two-main5도 mixed입니다. band30-300·boost6·FL target·volume40은 mixed입니다.

전체 최대값:

```text
M = [2.477683, 4.007771, 0.512392, 0.049264] dB
P = [1.140693, 2.962430, 2.744353, 2.728112] rad
```

요청한 계산식으로 나온 잠정 envelope는:

```text
M × 1.5, 올림 = [3.72, 6.02, 0.77, 0.08] dB
P × 1.5, 올림 = [1.712, 4.444, 4.117, 4.093] rad
```

**이 값을 합격 허용치로 적용하지 않았습니다.** 위상 허용치 일부가 π를 넘는 등 검사 의미가 없어지기 때문입니다. 현재 named constants는 조사 기준인 **0.01 dB / 0.001 rad**를 유지하고 테스트가 실패합니다. 요청한 최종 허용치 확정은 미완료입니다.

## 5. 시간 — FL, 두 위치, 스피커당

워밍업 후 3회 median입니다.

| 모드 | Rust | 골든 SECS | Rust/SECS |
|---|---:|---:|---:|
| zero | 2.355168 s | 1.547 s | 1.52× |
| mixed | 1.022743 s | 0.884 s | 1.16× |
| linear | 2.147108 s | 2.222 s | 0.97× |
| long | 1.676212 s | 0.732 s | 2.29× |
| ultra-60 | 2.825759 s | 1.914 s | 1.48× |

**성능 목표도 미달**입니다. 특히 long과 zero의 추가 최적화가 필요합니다.

## 6. 검증 결과

- `cargo fmt --all -- --check`: 통과
- `cargo clippy -p impulcifer-dsp --all-targets -- --no-deps -D warnings`: 통과
- `git diff --check`: 통과
- 벤치마크: 완료
- 전체 DSP suite: **208 통과, 1 실패**

전체 테스트에는 시간·디스크 절약을 위해 다음 환경 override를 사용했습니다. 따라서 요청의 기본 debug-profile 명령 그대로 실행한 결과는 아닙니다.

```bash
CARGO_PROFILE_TEST_OPT_LEVEL=2 \
CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_INCREMENTAL=0 \
cargo test -p impulcifer-dsp --no-fail-fast
```

주요 `test result:` 줄:

```text
perf_smoke:
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

room_tuning2_primitives:
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

room_tuning2_rc:
test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out

room_tuning_secs:
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

trf:
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

error: test failed, to rerun pass `-p impulcifer-dsp --test room_tuning2_rc`
error: 1 target failed
```

기존 DSP 테스트에는 실패가 없었습니다.

## 7. 최종 `git status --porcelain`

```text
 M crates/impulcifer-dsp/benches/perf.rs
 M crates/impulcifer-dsp/tests/room_tuning2_rc.rs
 M crates/impulcifer-dsp/tests/trf.rs
 M tests/migration/gen_trf_reference.py
 M tests/migration/goldens/trf_scipy_reference.json
?? crates/impulcifer-dsp/tests/room_tuning2_primitives.rs
```

앞서 설명한 외부 WIP 커밋 때문에 핵심 구현 파일은 이 목록에 없습니다.

## 8. 미완료·명세 확인 필요사항

1. **스피커 독립성과 골든 내부 불일치**
   > “The correction filter itself never depends on g_i or on the other speakers.”

   같은 입력의 단독/all 결과가 크게 다릅니다. 재생성 또는 설명이 필요합니다.

2. **Zero FIR 표현**
   > “no FIR, no lift, phase strength 0.”

   골든의 identity FIR 1탭을 빈 FIR로 정규화해 비교할지 결정이 필요합니다. β도 미사용 값의 비교 규칙이 필요합니다.

3. **설계 곡선 정밀도**
   > “expect ~1e-9 dB”

   현재 rounded golden으로는 확인할 수 없습니다. full-precision 설계 곡선이 필요합니다.

4. **오류 반환 시그니처**
   > “Reject … inputs with `DspError::InvalidArgument`.”

   `channel_level`과 `correction_band`의 고정 반환형과 충돌합니다.

5. **TRF/PEQ 정밀 경로**
   - 최종 fixture 세 개는 통과하지만 모든 PEQ의 평가 경로 일치는 미확인입니다.
   - SVD·축약 연산·초월함수·FFT의 차이 중 어떤 것이 큰 차이를 만드는지 추가 분리가 필요합니다.

6. **단계별 검증·최종 허용치·성능**
   - 첫 Fit 이후 모든 차이의 원인 규명은 미완료입니다.
   - 큰 오차를 받아들이는 허용치는 설정하지 않았습니다.
   - 다섯 모드 중 네 모드의 성능이 SECS보다 느립니다.

**따라서 이 상태를 다음 패킷의 검증된 기반으로 취급하면 안 됩니다.**
