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
| `lf_weight`, `lf_pull`, `lf_prep_*`, `lf_excess_*` | 저역 도착 맞춤의 가중치, 실현 상한, 스피커별 준비 곡선과 후보별 전체 위상 결합 |
| `decay_cut`, `mode_t60_48k_*`, `refine_*`, `mode_decay_*` | 모드 감쇠 컷, 전체 샘플레이트 T60, 공진 정련, 스피커별 컷 설계 |
| `hf_region_*`, `realize_*` | 고역 위상 보정 구간과 실현 함수의 저역 상한·고역 위상 보정 결과 |
| `rc_*` | 선택별·스피커별 끝단 설계 곡선, PEQ, FIR, 응답, 위상 강도와 진단 |
| `band_sos`, `band_t60_*` | 엄격한 옥타브와 너그러운 1/3옥타브 대역 필터, 교차점과 감쇠 시간 |
| `upper_demo_*`, `upper_rule` | 엄격한 값, 너그러운 대역 근거와 선택, 전이 배수를 적용한 상한 |
| `hp8` | 8차 버터워스 하이패스 계수와 복소 응답 |
| `pair_phase_*` | 스피커 짝의 대역별 위상과 상쇄량, 보고할 짝의 순서와 여섯 쌍 제한 |
| `rc_*_upper_mixed` | 너그러운 상한과 2배 전이를 쓴 끝단 RC 설계 및 진단 |
| `mimo_early_samples`, `mimo_seat_change` | MIMO 초기 구간의 표본 변환과 귀별 에너지 변화 규칙 |
| `mimo_normal_*` | 여러 초기 구간에서 만든 MIMO 정규 방정식과 우변 |
| `mimo_design_*` | 초기 구간·귀별 가드 조합별 MIMO 풀이, 모든 가드 시도, 서포트와 귀 기록 |
| `mimo_guard_*` | 합산 가드와 귀별 가드의 합성 사례, 귀별 대역 에너지 |

## Gates

`room_tuning2_s2r_libm_probe`가 Rust `f64`와 오라클의 수학 함수를 비트 단위로 비교한다. 게이트는 값이 만들어진 방식에 따라 네 부류로 나뉜다.

| 부류 | 대상 | 게이트 |
| --- | --- | --- |
| `Exact` | 저장된 입력만 쓰는 밴드 집합·클램프·불연속 마스크·도달 구간, DC 적용 여부와 스칼라, `s2r_limit_*`의 결과·인덱스·최악 빈, DipLimit 빈 주파수, RcBands 진단, LongModeBoost의 상한과 피크 주파수 | 오라클 libm에서는 비트 단위로 같다. 다른 libm에서는 수치에 상대·절대 1e-9를 적용하며, `Limit`의 최악 빈·줄인 인덱스·컷 수·완료 여부는 실제 값과 기대값을 보고만 한다. |
| `Curve` | 저장된 입력으로 계산한 합성 rolloff·dip, limit PEQ 밴드 수치와 초과량, DC FIR, 잔차 배열, 연속 마스크와 위상 점수 격자 | 오라클 libm에서는 상대 1e-12와 절대 1e-12, 다른 libm에서는 상대 1e-9와 절대 1e-9를 적용한다. |
| `Rebuilt` | 데모 녹음에서 다시 만든 스펙트럼에 따른 rolloff·dip·저역 보호 마스크·끝단 RC 설계와 해당 수치 진단, LongModeBoost의 모드 T60(저율 응답에서 계산) | 모든 플랫폼에서 값마다 `오차 ≤ 1e-9 + 1e-9·|기대값|`, 상대 L2 1e-9를 적용한다. |
| `FitDependent` | 끝단 `rc_*`의 PEQ, FIR, 응답, 위상 강도와 LowProtect의 `reduced`·`cuts`·`limited` | 모든 플랫폼에서 실제 값과 기대값을 보고만 한다. |

`Rebuilt`는 libm이 같아도 비트 단위로 같지 않다. 오라클과 Rust가 녹음의 RC 윈도 스펙트럼을 서로 다른 FFT 구현으로 계산해 입력 전력부터 상대 1e-12 안팎의 차이가 생기고, 뒤 계산에서 절대 오차가 커지기 때문이다. 끝단 PEQ 정련도 `s2_*`와 마찬가지로 첫 평가 뒤 결과가 달라진다. 따라서 그 PEQ와 FIR에 의존하는 끝단 `Limit` 결정은 같은 libm에서도 `FitDependent`다. note 배열의 길이, 종류, 유무와 순서는 언제나 정확히 비교하고, payload만 위 표에 따라 판정한다.

다시 만든 `s2r_refine_*`와 `s2r_mode_decay_*`의 `curvature`·`offset`에는 값별 `Rebuilt` 한도만 적용하고 상대 L2 한도에서는 제외한다. glibc에서 `offset`의 상대 오차는 1.8067888727235077e-9였지만 절대 오차는 약 5.6e-13이었다. 거의 같은 두 dB 차이를 `curvature`로 나누므로 조건수가 커지는 수치이며, 값별 `오차 ≤ 1e-9 + 1e-9·|기대값|` 판정에는 충분히 들어온다. 저장 입력을 쓰는 `s2r_refine_resonance`는 계속 `Exact`로 판정한다.

아래 Windows 값은 2026-11-08, Windows 11 x86_64-pc-windows-msvc에서 `cargo test -p impulcifer-dsp --test golden_secs2r -- --nocapture --test-threads=1`로 측정했다. 픽스처는 Debian glibc 2.41에서 만들었으며, 이 Windows 환경에서는 `ln`, `sqrt`, `powf(0.5)`만 비트 단위로 같았다. 나머지 다섯 함수가 달라 외부 libm 게이트를 적용했다.

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
| LF weight and pull | `Exact` | 1.1102230246251565e-16 | 2.4207429354183342e-17 |
| mode-decay cut and HF range | `Exact` | 1.1102230246251565e-16 | 2.9336934266632966e-17 |
| full-rate T60 and resonance refinement | `Exact` + `Curve` + `Rebuilt` | 1.95110594347625e-12 | 3.8149630433217217e-12 |
| LF preparation | `Rebuilt` | 6.821210263296962e-13 | 7.390250486780205e-14 |
| LF excess recombination | `Exact` + `Curve` | 0 | 0 |
| realization synthetic cases | `Exact` + `Curve` | 7.105427357601002e-15 | 7.072561793909911e-15 |
| mode-decay design | `Rebuilt` | 2.3386093062072177e-10 | 9.129288844769646e-12 |
| RC design and complete notes | `Exact` + `Rebuilt` + `FitDependent` | 1.939670646322611e-10 | 5.335245331134957e-12 |
| upper-limit band SOS | `Curve` | 4.440892098500626e-16 | 1.2109958378534601e-16 |
| upper-limit band T60 (synthetic and demo) | `Exact` + `Rebuilt` | 0 | 0 |
| upper-limit rule and demo evidence | `Exact` + `Rebuilt` | 0 | 0 |
| HP8 | `Exact` | 0 | 0 |
| pair phase | `Exact` | 1.9984014443252818e-15 | 5.287077574998131e-18 |
| upper-limit RC design | `Rebuilt` + `FitDependent` | 4.263256414560601e-14 | 1.2185139258003184e-14 |
| MIMO early samples and seat change | `Exact` | 0 | 0 |
| MIMO normal equations | `Curve` | 4.440892098500626e-15 | 3.7305743002381116e-15 |
| MIMO guard evaluation and band energies | `Curve` + exact decisions | 2.509104035652854e-13 (guard), 1.7763568394002505e-15 (energy) | 4.080288248210585e-14 (guard), 1.7666563505222917e-16 (energy) |
| MIMO design | `Curve` + exact decisions | 3.771757661752417e-8 (ratio), 1.021405182655144e-12 (support) | 1.3473578830673903e-9 (ratio), 1.1873105152459935e-12 (support) |

상한 대역 SOS는 Impulcifer가 3.5.0부터 엄격한 추정에 쓰던 Butterworth 포트를 그대로 사용한다. 기본 출력의 바이트를 지켜야 하므로 오라클의 설계 연산으로 바꾸지 않는다. Debian glibc 2.41에서도 일부 계수가 1~2 ULP 달랐고, 최대 절대 오차는 4.440892098500626e-16, 최대 상대 L2 오차는 1.2109958378534601e-16이었다. 따라서 SOS 계수는 `Curve`로 판정하며, 같은 필터로 얻은 감쇠 교차 인덱스는 오라클과 정확히 같아 계속 exact로 검사한다.

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

MIMO 가드 합성 사례(`mimo_guard_*`)는 늦은 부분 전체에 같은 배율을 곱하므로 모든 대역이 같은 dB만큼 바뀐다. 그래서 기록되는 대역은 반올림 끝자리로 정해지고, 오라클끼리도 libm에 따라 다르다(같은 `one_ear_worse`의 귀별 가드에서 glibc는 77.0~80.0 Hz, msvcrt는 31.7~40.0 Hz를 기록했다). 시험은 오라클이 저장한 귀·대역별 에너지로 후보값을 다시 만든다. 최댓값이 다른 후보보다 1e-9 dB 넘게 크면 기록 대역과 귀를 정확히 비교하고, 그 안에서 여럿이 같으면 Impulcifer와 오라클의 선택이 둘 다 그 후보 안에 있는지 본다. 동점이 없는 실제 입력(`mimo_design_*` 70개)에서는 모든 시도의 대역·모드·귀를 정확히 비교한다. 대역 에너지는 libm과 무관한 |rfft|² 합이므로 모든 플랫폼에서 상대 1e-12로 비교한다. SECS 2는 대역 가드가 이미 넘친 시도에서 꼬리 가드를 계산하지 않아 `s2_mimo_*`의 `guard_tail`이 `guard_band_db`보다 짧을 수 있다. 묶음 4 가족은 오라클이 모든 시도에서 두 가드를 다 계산한 값이다.

## Regeneration

픽스처는 유지보수자와 명세를 쓰는 주 세션만 다시 만든다. 옵션을 구현하는 사람은 이 절차를 돌리지 않고, 내보내기 프로그램을 열지도 않는다.

1. **입력.** 이 저장소에서 `cargo run --release -p impulcifer-dsp --example secs2_inputs -- <inputs>`를 돌려 데모의 자르지 않은 무지향 귀 위치 IR을 `<inputs>/omni/`에, 정렬된 귀속 BRIR을 `<inputs>/brir/`에 쓴다(`s2_*`와 같은 입력. 묶음 5의 극성 가족은 `brir/`의 앞 4096표본을 쓰고 그 사본을 `s2r_polarity_demo_base`에 둔다).
2. **오라클.** SECS 2R 06183335(v0.55.0) 체크아웃에 내보내기 디프(기능 플래그 `oracle-fixtures`, 예제 `impulcifer_fixtures`)를 적용한다. 이 디프는 GPL 저장소의 코드이므로 이 저장소에 두지 않고 SECS 2R 작업 사본에 둔다.
3. **내보내기.** Debian glibc(WSL Debian, glibc 2.41에서 확인)에서 `S2R_BINS=<이 저장소>/tests/migration/goldens/s2_bins.json`과 `S2R_GOLDENS=<이 저장소>/tests/migration/goldens`(묶음 4의 MIMO 가족이 읽는 `s2_mimo_*` 입력, 비우면 `S2R_BINS`의 폴더)를 주고 `cargo run --release -p secs-engine --features oracle-fixtures --example impulcifer_fixtures -- <inputs> <out>`를 두 번 돌린다. 두 번째 실행은 파일을 하나도 바꾸지 않아야 한다. 내보내기는 자기가 쓰지 않은 `s2r_*` 파일이 `<out>`에 있으면 실패하므로 빈 폴더나 앞선 출력 폴더를 쓴다. glibc에서 내는 까닭은 CI의 ubuntu 러너가 같은 libm을 써서 `Exact`·`Curve` 게이트가 그곳에서 비트 단위로 맞기 때문이다.
4. **반영.** `<out>`의 파일을 `tests/migration/goldens/`에 그대로 복사하고, 앞선 묶음의 파일이 바이트 단위로 그대로인지 `git diff --stat`으로 확인한다.
