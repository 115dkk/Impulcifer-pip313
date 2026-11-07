# SECS 2R oracle fixtures (`s2r_*`)

Virtual room tuning 2의 3.13.0 선택 세 가지는 ADR 0015에 따라 SECS 2R을 오라클로만 사용해 구현했다. 이 저장소에는 SECS 2R 코드나 픽스처 내보내기 프로그램이 없으며, 구현은 `docs/rust/ROOM_CORRECTION.md`의 명세와 이 픽스처만 사용한다.

## Encoding

배열과 복소수, 유한하지 않은 수, `.f64` 사이드카 형식은 `README-secs2.md`의 "Encoding"과 같다. `s2r_bins.json`의 저장 빈도는 `s2_bins.json`과 같다. 모든 픽스처에는 `inputs`, `outputs`, `meta`가 있으며, `meta.weights`는 오라클의 위치 가중치, `meta.libm`은 오라클이 사용한 C 수학 라이브러리다.

## Families

| 가족 | 고정한 값 |
| --- | --- |
| `libm_probe` | 400개 입력의 `ln`, `exp(x/1000)`, `log10`, `log2`, `sqrt`, `powf(0.5)`, `sin`, `cos` |
| `rolloff_*`, `rolloff_synthetic_*` | 1/3옥타브 전력 응답, 기준 레벨, 문턱, 교차점, 롤오프 |
| `clamp_*` | 롤오프 아래 PEQ 중심 이득 제한 |
| `limit_*` | 실제 PEQ·FIR 응답 제한, 줄인 밴드와 보호 컷, 최악 빈 |
| `dc_*` | FIR DC 보정 전후와 적용 여부 |
| `dip_*`, `dip_synthetic_*` | 1옥타브 포락선, 딥 깊이, 허용 보정량, 제한 전후 설계 곡선과 진단 |
| `band_set_*` | 기본 구간과 추가 구간의 정렬·병합 |
| `masks_*` | 크기 마스크, 위상 페이드, FIR 오차 구간, 위상 점수 격자, zero 모드 PEQ 도달 구간 |
| `mask_*` | 저역 보호와 긴 모드 제한, 구간 페이드를 적용한 설계 곡선 |
| `residual_*` | PEQ 잔차와 유한하지 않은 값의 `1e12` 치환 |
| `rc_*` | 선택별·스피커별 끝단 설계 곡선, PEQ, FIR, 응답, 위상 강도와 진단 |

## Gates

`room_tuning2_s2r_libm_probe`가 Rust `f64`와 오라클의 수학 함수를 비트 단위로 비교한다. 게이트는 값이 만들어진 방식에 따라 네 부류로 나뉜다.

| 부류 | 대상 | 게이트 |
| --- | --- | --- |
| `Exact` | 저장된 입력만 쓰는 밴드 집합·클램프·불연속 마스크·도달 구간, DC 적용 여부와 스칼라, `s2r_limit_*`의 결과·인덱스·최악 빈, DipLimit 빈 주파수, RcBands 진단, LongModeBoost의 상한과 피크 주파수 | 오라클 libm에서는 비트 단위로 같다. 다른 libm에서는 수치에 상대·절대 1e-9를 적용하며, `Limit`의 최악 빈·줄인 인덱스·컷 수·완료 여부는 실제 값과 기대값을 보고만 한다. |
| `Curve` | 저장된 입력으로 계산한 합성 rolloff·dip, limit PEQ 밴드 수치와 초과량, DC FIR, 잔차 배열, 연속 마스크와 위상 점수 격자 | 오라클 libm에서는 상대 1e-12와 절대 1e-12, 다른 libm에서는 상대 1e-9와 절대 1e-9를 적용한다. |
| `Rebuilt` | 데모 녹음에서 다시 만든 스펙트럼에 따른 rolloff·dip·저역 보호 마스크·끝단 RC 설계와 해당 수치 진단, LongModeBoost의 모드 T60(저율 응답에서 계산) | 모든 플랫폼에서 값마다 `오차 ≤ 1e-9 + 1e-9·|기대값|`, 상대 L2 1e-9를 적용한다. |
| `FitDependent` | 끝단 `rc_*`의 PEQ, FIR, 응답, 위상 강도와 LowProtect의 `reduced`·`cuts`·`limited` | 모든 플랫폼에서 실제 값과 기대값을 보고만 한다. |

`Rebuilt`는 libm이 같아도 비트 단위로 같지 않다. 오라클과 Rust가 녹음의 RC 윈도 스펙트럼을 서로 다른 FFT 구현으로 계산해 입력 전력부터 상대 1e-12 안팎의 차이가 생기고, 뒤 계산에서 절대 오차가 커지기 때문이다. 끝단 PEQ 정련도 `s2_*`와 마찬가지로 첫 평가 뒤 결과가 달라진다. 따라서 그 PEQ와 FIR에 의존하는 끝단 `Limit` 결정은 같은 libm에서도 `FitDependent`다. note 배열의 길이, 종류, 유무와 순서는 언제나 정확히 비교하고, payload만 위 표에 따라 판정한다.

아래 Windows 값은 2026-11-07, Windows 11 x86_64-pc-windows-msvc에서 `cargo test -p impulcifer-dsp --test golden_secs2r -- --nocapture --test-threads=1`로 측정했다. 픽스처는 Debian glibc 2.41에서 만들었으며, 이 Windows 환경에서는 `ln`, `sqrt`, `powf(0.5)`만 비트 단위로 같았다. 나머지 다섯 함수가 달라 외부 libm 게이트를 적용했다.

| 가족 | 주된 부류 | Windows 최대 절대 오차 | Windows 최대 상대 L2 오차 |
| --- | --- | ---: | ---: |
| libm probe | 함수별 비트 비교 | 다섯 함수 불일치 | 해당 없음 |
| band set | `Exact` | 0 | 0 |
| masks | `Exact` + `Curve` | 2.2737367544323206e-13 | 6.652799355587308e-17 |
| rolloff | `Curve` + `Rebuilt` | 6.821210263296962e-13 | 4.2934407425396315e-16 |
| clamp | `Exact` | 0 | 0 |
| limit | `Exact` + `Curve` + 결정 보고 | 4.440892098500626e-16 | 1.5065656150329976e-15 |
| DC correction | `Exact` + `Curve` | 1.3552527156068805e-20 | 4.851060544800653e-20 |
| dip limit | `Curve` + `Rebuilt` | 6.821210263296962e-13 | 4.360486500742148e-15 |
| low-protection design mask | `Rebuilt` | 4.263256414560601e-14 | 2.842924265171789e-15 |
| residual | `Curve` | 2.6645352591003757e-14 | 2.9169934962049308e-16 |
| RC design and complete notes | `Exact` + `Rebuilt` + `FitDependent` | 4.263256414560601e-14 | 3.2620804220457383e-15 |

같은 libm을 쓴 Ubuntu 실행에서는 FFT 차이 때문에 `Rebuilt` 값이 다음만큼 달랐다. 이 값은 1e-9 게이트 안이다.

| 가족 | glibc 최대 절대 오차 | glibc 최대 상대 L2 오차 |
| --- | ---: | ---: |
| rolloff | 4.55e-9 | 1.83e-12 |
| dip limit | 4.55e-9 | 6.43e-12 |
| low-protection design mask | 7.41e-12 | 1.62e-13 |

끝단 `rc_*`의 보고 전용 수치는 이 Windows 실행에서 다음만큼 달랐다.

| 설정 | PEQ f0 | PEQ Q | PEQ gain | FIR | 응답 dB | 위상 강도 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `off` | 2.641774393e2 | 1.234980491e1 | 3.175853158e1 | 5.054190658e-4 | 2.001949010 | 0 |
| `dip` | 3.267620189e2 | 1.776784555e1 | 1.405485902e1 | 5.991484337e-4 | 1.396361186 | 0 |
| `protect` | 6.249879281e1 | 1.205230224e1 | 3.000079814 | 6.264178161e-4 | 6.528662133e-1 | 0 |
| `bands` | 3.279458109e2 | 1.241853735e1 | 3.332511092e1 | 8.816952243e-4 | 1.947271418 | 0 |
| `all` | 1.587310403e1 | 3.164880375 | 1.035219351e1 | 1.113121823e-3 | 1.284493722 | 0 |

저역 보호 진단의 `reduced`와 `cuts`는 두 번째 PEQ를 제한할 때 만든 결과를 최종 FIR 응답 제한까지 이어서 누적한 값이며, `limited`도 최종 응답을 기준으로 한다. 오라클이 만든 데모 픽스처에서는 최종 FIR 응답 제한이 새로 줄인 밴드가 없었다. PEQ 정련 결과에 따라 이 결정이 달라지므로 `rc_*`의 세 값은 모든 플랫폼에서 `FitDependent`로 보고한다.
