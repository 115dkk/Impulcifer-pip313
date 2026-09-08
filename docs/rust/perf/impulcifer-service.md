# PA04: impulcifer-service 성능 감사

2026-09-08. **2차 감사에서 9개 연산 모두 성능 기준을 충족했다.** ASTRA가 허용된 스윕 감지·번역 구현을 최적화하고 두 Python 인터프리터로 전후 비교를 마쳤다. 부모 세션이 디프와 원문 로그를 검토하고 fmt·clippy·service/jobs/policy 테스트를 직접 재실행하여 통과했다. 상세 결과는 아래 **8. 2차 감사**에 있다. `features.toml`은 허용 범위 밖이므로 변경하지 않았다.

아래 1~7절은 첫 감사 당시의 측정과 판정을 보존한 기록이다. 당시 두 연산은 성능 미달이었으며, 미완료 판정은 8절의 결과로 갱신한다.

## 1. 환경

- CPU는 Intel Core i5-12600KF, 물리 10코어·논리 16코어다.
- OS는 Windows 11, 10.0.22621, x86_64다.
- Rust는 `rustc 1.97.0 (2d8144b78 2026-07-07)`이며 `cargo bench`의 release 프로파일로 측정했다.
- 일반 Python은 `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`, CPython 3.14.5, MSC v.1944 64 bit다.
- Free-threaded Python은 `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`, CPython 3.14.7 free-threading build, MSC v.1944 64 bit다. 서비스 모듈을 임포트한 뒤에도 `sys._is_gil_enabled()`는 `False`였다.
- 두 Python 모두 NumPy 2.5.3, SciPy 1.18.1이다.
- `numpy.show_config()`의 BLAS/LAPACK은 scipy-openblas, OpenBLAS 0.3.34.106.0이다. 설정은 `USE64BITINT DYNAMIC_ARCH NO_AFFINITY`, Haswell, `MAX_THREADS=24`다.
- FFT는 NumPy/SciPy 기본 pocketfft 계열이며 SciPy 호출 모듈은 각 인터프리터의 `Lib/site-packages/scipy/fft/_basic_backend.py`다. 별도 FFT 백엔드를 설치하거나 선택하지 않았다.
- `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `NUMEXPR_NUM_THREADS`, `RAYON_NUM_THREADS`는 모두 미설정이다. 스레드 수를 1로 제한하지 않았다.
- 다른 워커가 작업 중인 같은 머신에서 순차 측정했다. 프로세스 격리나 머신 독점은 하지 않았다. 반복 측정 사이에 시간 차이가 있으므로 작은 차이를 확정적인 개선으로 해석하지 않는다.

## 2. 측정 방법

Rust는 실제 `ImpulciferService::call`, Python은 실제 `ImpulciferApplicationService`의 메서드를 호출한다. 저장소의 Python 클래스 이름은 작업서의 축약 이름과 다르다. pywebview와 Tauri를 IPC 비교에 넣지 않았다. `catalog_translate`만 지정대로 `Catalog::translate`와 `loc.get(...).format(**args)`를 직접 호출한다.

모든 연산은 **3배치 워밍업 후 11배치 측정**했다. 표에는 배치 내 평균의 중앙값과 최솟값을 기록한다. 배치당 호출 수는 bootstrap·설정 조회·녹음 파일명 처리 200회, 언어 변경 100쌍, 스윕 감지 20회, 번역 100,000회다. 작업 관련 세 연산은 배치당 5개 작업을 실행하므로 각 연산에 실제 작업 70개가 포함된다. 반복 수를 줄인 연산은 없다.

시간 단위는 ms이며 호출당, 언어 변경 한 쌍당 또는 작업당 값이다. `job_event_emit`의 작업당 이벤트 수는 10,000개, `poll_job_drain`은 2,000개다. 요청과 로그 payload는 두 언어에서 동일하다. 녹음 파일명 API의 실제 시그니처에 맞춰 demo 디렉터리와 `FL,FR.wav`를 두 인자로 전달한다.

- 설정 파일은 임시 디렉터리에 생성한다. Python은 localization singleton을 임포트하기 전에 `HOME`과 `USERPROFILE`을 임시 프로필로 바꾸고 마지막에 복구한다. `set_language_round_trip`은 한국어와 영어를 차례로 저장하며 실제 파일 쓰기를 포함한다.
- BRIR 입력은 두 벤치에서 동일한 manifest로 복사한다. 복사·취소·작업 스레드 join·임시 디렉터리 삭제는 시간 측정에서 제외한다. 사용자의 demo 입력이나 설정을 덮어쓰지 않는다.
- `start_brir_to_first_event`는 작업 시작부터 첫 비어 있지 않은 `poll_job` 응답까지 잰다. 첫 이벤트는 실제 API가 생성하는 `status/running`이다. DSP 전체 실행 시간을 이 연산에 포함하지 않는다.
- 실제 `poll_job`에는 페이지 크기 인자가 없고 이벤트 보관 한도는 2,000개다. 로그 2,000개를 모두 발행한 후 조회하면 초기 상태 이벤트 때문에 일부 기록을 잃는다. 따라서 작업 스레드가 로그 500개씩 발행하고 호출자의 조회 완료를 기다리게 했다. 네 번의 실제 `poll_job(after_seq)` 호출 시간을 합산하며 발행·동기화·검증은 제외한다. 로그 2,000개의 연속 순번, 인덱스, 메시지, `next_seq`를 검증한다. 마지막 상태 이벤트는 별도 조회로 확인한다.
- 이벤트 발행은 실제 작업 스레드 안에서 측정하며 로그 payload 생성과 보관 한도 처리를 포함한다. 완료 후 최종 순번 10,002, 보관 이벤트 2,000개, 마지막 로그 인덱스 9,999를 검증한다.
- Python bootstrap의 DSP 사전 준비 작업은 fixture 준비 때 끝낸다. 두 언어 모두 워밍업 후 정상 상태를 비교한다.

### Rust BEFORE 원문

ASTRA 워커가 최적화 전에 기록한 `target/pa04-before.log`의 행이다.

```text
| op | size | rust median ms | rust min ms |
| bootstrap | 200/batch; per unit | 0.562671000 | 0.554706000 |
| get_ui_settings | 200/batch; per unit | 0.454287000 | 0.449462500 |
| set_language_round_trip | 100/batch; per unit | 2.851902000 | 2.740955000 |
| resolve_recording_paths | 200/batch; per unit | 0.001321000 | 0.001253500 |
| start_brir_to_first_event | 5/batch; per unit | 0.585620000 | 0.553800000 |
| poll_job_drain | 5/batch; per unit | 4.513260000 | 4.262800000 |
| job_event_emit | 5/batch; per unit | 2.771040000 | 2.709360000 |
| detect_sweep | 20/batch; per unit | 12.051685000 | 11.925545000 |
| catalog_translate | 100000/batch; per unit | 0.000477907 | 0.000471403 |
```

### Rust AFTER 원문

부모 세션에서 최종 코드를 직접 재측정한 `target/pa04-parent-rust.log`의 행이다. 워커의 더 빠른 측정값으로 대체하지 않았다.

```text
| op | size | rust median ms | rust min ms |
| bootstrap | 200/batch; per unit | 0.312903500 | 0.253554000 |
| get_ui_settings | 200/batch; per unit | 0.163412500 | 0.149922000 |
| set_language_round_trip | 100/batch; per unit | 2.073810000 | 2.008763000 |
| resolve_recording_paths | 200/batch; per unit | 0.001191500 | 0.001187000 |
| start_brir_to_first_event | 5/batch; per unit | 0.454380000 | 0.339680000 |
| poll_job_drain | 5/batch; per unit | 1.845620000 | 1.689280000 |
| job_event_emit | 5/batch; per unit | 6.138100000 | 5.556500000 |
| detect_sweep | 20/batch; per unit | 21.515630000 | 17.349550000 |
| catalog_translate | 100000/batch; per unit | 0.000533148 | 0.000474156 |
```

### CPython 3.14.5 원문

부모 세션의 `target/pa04-parent-python.log`다.

```text
| op | size | python median ms | python min ms |
| bootstrap | 200/batch; per unit | 1.178179000 | 1.039997500 |
| get_ui_settings | 200/batch; per unit | 0.796799000 | 0.556640500 |
| set_language_round_trip | 100/batch; per unit | 3.660083000 | 3.405379000 |
| resolve_recording_paths | 200/batch; per unit | 0.002882500 | 0.002805000 |
| start_brir_to_first_event | 5/batch; per unit | 0.529380000 | 0.494639997 |
| poll_job_drain | 5/batch; per unit | 11.192720002 | 10.100260001 |
| job_event_emit | 5/batch; per unit | 48.188879999 | 45.654359995 |
| detect_sweep | 20/batch; per unit | 0.763025000 | 0.696375001 |
| catalog_translate | 100000/batch; per unit | 0.000477610 | 0.000456290 |
```

### CPython 3.14.7 free-threaded 원문

부모 세션의 `target/pa04-parent-python-ft.log`다.

```text
| op | size | python median ms | python min ms |
| bootstrap | 200/batch; per unit | 1.101411500 | 1.077206000 |
| get_ui_settings | 200/batch; per unit | 0.436440500 | 0.430321500 |
| set_language_round_trip | 100/batch; per unit | 3.421022000 | 3.269721000 |
| resolve_recording_paths | 200/batch; per unit | 0.003015500 | 0.002954500 |
| start_brir_to_first_event | 5/batch; per unit | 0.544199999 | 0.519739999 |
| poll_job_drain | 5/batch; per unit | 5.165960005 | 5.083139997 |
| job_event_emit | 5/batch; per unit | 46.668760001 | 46.253500000 |
| detect_sweep | 20/batch; per unit | 0.718290000 | 0.686210000 |
| catalog_translate | 100000/batch; per unit | 0.000511325 | 0.000491865 |
```

### 전후 비율

`ratio = Python median / Rust median`. 아래 전후 비율 모두 위 최종 Python 측정값을 분자로 사용한다. BEFORE와 AFTER는 같은 머신에서 다른 시점에 측정했다.

| 연산 | BEFORE 3.14 | BEFORE 3.14t | AFTER 3.14 | AFTER 3.14t |
|---|---:|---:|---:|---:|
| bootstrap | 2.094 | 1.957 | 3.765 | 3.520 |
| get_ui_settings | 1.754 | 0.961 | 4.876 | 2.671 |
| set_language_round_trip | 1.283 | 1.200 | 1.765 | 1.650 |
| resolve_recording_paths | 2.182 | 2.283 | 2.419 | 2.531 |
| start_brir_to_first_event | 0.904 | 0.929 | 1.165 | 1.198 |
| poll_job_drain | 2.480 | 1.145 | 6.064 | 2.799 |
| job_event_emit | 17.390 | 16.842 | 7.851 | 7.603 |
| detect_sweep | 0.063 | 0.060 | **0.035** | **0.033** |
| catalog_translate | 0.999 | 1.070 | **0.896** | **0.959** |

## 3. 조사와 최적화

### 내장 카탈로그의 반복 파싱

`settings.rs`에서 설정 payload를 만들 때마다 영어와 선택 언어 JSON을 파싱했다. 내장 자산은 실행 중 바뀌지 않으므로 언어별 `OnceLock` 9개에 파싱 결과를 저장했다. 호출자에게는 독립된 소유 복사본을 반환한다. 새 의존성은 없다.

캐시만 적용한 중간 측정(`target/pa04-cache.log`)에서 설정 조회는 0.454287 → 0.234928 ms, 언어 변경 한 쌍은 2.851902 → 2.202696 ms였다. 최종 설정 조회는 0.163413 ms다. `cached_catalogues_match_parsing_and_are_independent`는 9개 언어 모두 기존 파싱 결과와 같고 반환값을 비워도 캐시를 변경하지 않는지 확인한다.

설정 파일 재조회와 파일 쓰기 동안의 mutex는 그대로 유지했다. 외부에서 저장한 설정을 다시 읽고 기존 키를 보존하는 계약이 있으므로 디스크 조회를 생략하거나 잠금을 무조건 풀지 않았다.

### IPC Value 복사

`lib.rs`에서 성공 응답을 생성할 때 완성된 `Value`를 다시 직렬화하던 처리를 소유권 이동으로 바꿨다. `poll_job`도 조회 결과의 이벤트 payload를 응답에 옮긴다. 이벤트 journal의 보관·필터·오류 정책은 그대로다.

`poll_job_drain`은 BEFORE 4.513260 ms에서 최종 1.845620 ms로 줄었다. 워커의 앞선 AFTER 측정은 1.087100 ms였지만 이 보고서의 최종 판정에는 부모 세션 재측정값을 썼다. 두 측정 모두 최적화 전보다 빠르다.

`StartBrir`도 UI payload의 카탈로그를 복사하지 않고 작업으로 옮긴다. 시작 시간은 0.585620 → 0.454380 ms다. 최종 비율은 1.5 미만이다. 설정 파일 조회·payload 생성·요청 검증·스레드 생성·첫 poll에 드는 비용이 남아 있다. 설정 재조회 생략이나 첫 이벤트의 정의 변경은 하지 않았다.

### 작업 정리용 join

`impulcifer-jobs/src/registry.rs`에 작업 핸들을 보관하고 `pub fn join(&self, job_id: &str) -> Result<(), ErrorCode>`를 추가했다. 기존 공개 시그니처를 바꾸지 않았다. 취소 요청만 보낸 채 임시 입력을 삭제하지 않도록 벤치에서 사용하며 측정 시간에는 포함하지 않는다. journal mutex를 풀고 join한다. 자기 자신에 대한 join은 거부한다. 단일 정리 호출자가 핸들을 가져가는 형태이며 여러 호출자에게 동시 종료 대기를 보장하는 API는 아니다.

새 테스트는 없는 작업, 자기 자신 join 거부, 작업 정리 후 종료, 종료 뒤 반복 호출을 확인한다. 이벤트 발행 방식 자체는 최적화하지 않았다. 최종 발행 시간은 이전 측정보다 증가했지만 두 Python보다 빠르다. 다른 작업이 진행 중인 머신에서 얻은 결과만으로 증가 원인을 확정하지 않는다.

### 수치 회귀

DSP 수치 경로·합산 순서·골든·허용 오차는 변경하지 않았다. 새 부동소수점 오차를 계산하지 않았으며 기존 서비스의 demo 기본값/vbass, 플롯, 출력, 녹음 등의 골든 테스트가 통과했다. 각 최적화 직후 서비스·정책·jobs 테스트를 실행했고, 부모 세션에서도 다시 통과했다.

## 4. Python보다 느린 연산

### detect_sweep

`crates/impulcifer-service/src/brir/sweep_grid.rs:118`에서 WAV 전체를 디코딩하고 샘플 수를 얻는다. Python은 정상 길이 녹음에서 `soundfile.info()`로 헤더를 읽어 길이를 계산하고, 길이만으로 판단하지 못할 때 파형을 읽는다. 단순히 디렉터리를 한 번 더 걷는 문제보다 파일 전체 디코딩 비용이 크다.

최종 Rust 21.515630 ms, Python 0.763025 / 0.718290 ms다. 올바른 수정은 WAV 메타데이터를 먼저 확인하고 필요한 때만 디코딩하는 것이다. 하지만 `brir/sweep_grid.rs`는 사용자가 지정한 수정 허용 목록 밖이다. 다른 파일에 별도 감지 구현을 복제하거나 변경 감지 없는 캐시를 추가해 측정을 통과시키지 않았다.

### catalog_translate

`crates/impulcifer-service/src/brir/mod.rs:73–88`은 원문 소유 복사, 인자별 문자열 변환, `format!`으로 자리표시자 생성, 반복 `replace`를 한다. Python의 C 구현 format보다 할당과 문자열 순회가 많다.

최종 Rust 0.000533148 ms, Python 0.000477610 / 0.000511325 ms다. 워커의 앞선 측정에서도 일반 Python 기준은 미달했다. `brir/mod.rs` 역시 수정 허용 목록 밖이므로 변경하지 않았다. 번역 키 하나를 특별 취급하거나 벤치만 다른 번역 구현을 호출하게 하지 않았다.

## 5. 변경 파일

- `crates/impulcifer-service/benches/perf.rs`: 기본 실행을 PA04로 연결했다. 기존 PA03 전체 파이프라인 벤치는 `--pipeline`으로 유지한다.
- `crates/impulcifer-service/tests/bench_support/service.rs`: 9개 연산, 임시 입력, 측정·검증 공통 코드다.
- `crates/impulcifer-service/tests/perf_smoke.rs`: PA04 테스트를 추가하고 PA03 테스트는 유지했다.
- `tests/migration/bench_oracle_impulcifer_service.py`: 동일 작업량의 실제 Python 서비스 벤치다.
- `crates/impulcifer-service/src/settings.rs`: 내장 카탈로그 캐시와 회귀 테스트다.
- `crates/impulcifer-service/src/lib.rs`: 성공 응답·이벤트 payload·BRIR 카탈로그의 소유권 이동이다. 다른 워커의 updater 변경은 보존했다.
- `crates/impulcifer-jobs/src/registry.rs`: 작업 정리용 join과 테스트다.
- `docs/rust/perf/impulcifer-service.md`: 이 보고서다.

Cargo.toml에는 PA03의 `[[bench]] name = "perf", harness = false`가 이미 있어 별도 항목을 추가하지 않았다. 기존 다른 워커의 의존성 수정은 보존했다. Cargo.lock을 손으로 수정하지 않았다.

## 6. 검증과 명령

등록부에서 인용할 테스트 이름은 **`impulcifer-service::bench_smoke_impulcifer_service`**다. 이벤트 수를 줄여 9개 연산을 모두 한 번씩 실행한다. 성능 미달이 남아 있으므로 이 테스트가 통과해도 성능 등록부를 implemented로 바꾸면 안 된다.

부모 세션이 아래 명령을 모두 포그라운드로 실행했다. 셸에서 `&&`로 연결하여 앞 명령의 실패를 숨기지 않았다.

```bash
cargo fmt -p impulcifer-service -p impulcifer-jobs -- --check
cargo clippy -p impulcifer-service -p impulcifer-jobs --all-targets -- --no-deps -D warnings
cargo test -p impulcifer-service -p impulcifer-jobs -p impulcifer-policy
cargo bench -p impulcifer-service --bench perf > target/pa04-parent-rust.log 2>&1
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py > target/pa04-parent-python.log 2>&1
/c/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py > target/pa04-parent-python-ft.log 2>&1
py -3.14 -m py_compile tests/migration/bench_oracle_impulcifer_service.py
ruff check tests/migration/bench_oracle_impulcifer_service.py --output-format=concise
```

검증 출력의 집계는 service **102 passed, 0 failed, 2 ignored**, jobs **13 passed, 0 failed**, policy **6 passed, 0 failed**다. 서비스에서 제외한 2개는 CABLE-A 실기 녹음과 설치된 Velopack의 실서비스 피드 테스트다. fmt·clippy·Python 구문 검사·ruff는 통과했다.

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.87s
Finished `test` profile [unoptimized + debuginfo] target(s) in 11.13s

Running tests\perf_smoke.rs
running 2 tests
test bench_smoke_impulcifer_service ... ok
test bench_smoke_pipeline_demo ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests\gates.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

All checks passed!
```

워커도 동일한 벤치·검증 명령을 `--manifest-path E:/Impulcifer/Cargo.toml`을 붙여 각 크레이트별로 실행했다. 카탈로그 캐시 직후의 로그는 `target/pa04-cache-{service,policy,jobs}-tests.log`, 소유권 이동 직후는 `target/pa04-move-{service,policy,jobs}-tests.log`다. 중간 검증에서는 다른 워커가 수정 중인 sys-win 코드의 E0658 때문에 실패했다. 해당 워커가 수정한 뒤 재실행하여 통과했다. PA04에서 sys-win을 변경하지 않았다.

초기 Python 원문과 환경 전체는 `target/pa04-python.log`, `target/pa04-python-ft.log`, 중간 캐시 측정은 `target/pa04-cache.log`, 워커 최종 Rust 측정은 `target/pa04-retry-final.log`에 있다. target 로그는 로컬 증거이며 저장소에 포함하지 않는다. 재현에 필요한 벤치 소스와 주요 결과는 이 보고서에 남겼다.

## 7. 미완료

- 성능 실패 두 건은 해당 구현 파일의 수정 허용이 필요하다. 그 전까지 PA04는 완료가 아니다.
- **Rust app round trip, no opponent**: 워커가 기존 스모크 드라이버로 한 번 시도했으나 WebView2 CDP 포트가 30초 안에 열리지 않아 200회 bootstrap 왕복을 측정하지 못했다. 앱과 임시 프로필을 정리했다는 워커 보고를 받았다. 부모 세션에서는 이 선택 검증을 반복하지 않았다. 성공 측정값은 없으며 Rust/Python 성능 판정에 넣지 않았다.
- PA04는 M2/M5 전체 감사가 아니므로 최대 상주 메모리를 측정하지 않았다.
- 다른 워커의 update, apps, audio-io, sys-win 변경은 수정하지 않았다. 커밋·푸시·CI 실행은 요청받지 않아 수행하지 않았다.

## 8. 2차 감사 (2026-09-08)

### 환경과 방법

1절의 동일 머신·인터프리터·라이브러리를 사용했다. 일반 Python 경로는 `C:/Users/32170336/AppData/Local/Programs/Python/Python314/python.exe`(3.14.5), FT는 `C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe`(3.14.7)다. 둘 다 NumPy 2.5.3, SciPy 1.18.1이며 FT는 서비스 임포트 뒤에도 GIL이 비활성 상태였다. `numpy.show_config()`의 OpenBLAS는 0.3.34.106.0, `USE64BITINT DYNAMIC_ARCH NO_AFFINITY`다. 기본 pocketfft를 사용하고 스레드 환경 변수는 지정하지 않았다. 전체 설정은 `target/pa04-second-config-python.log`, `target/pa04-second-config-python-ft.log`에 보관했다.

모든 명령은 포그라운드로 실행했다. 2절과 같은 작업량으로 3배치 워밍업 뒤 11배치를 측정했다. 작업 관련 연산은 배치마다 5회 실행했다. 아래 ms는 호출당·언어 변경 한 쌍당·작업당 시간이다. 임시 입력은 작업 종료를 join한 뒤 삭제한다. 다른 워커와 머신을 공유했으며 독점 실행은 아니다.

### 2차 BEFORE 원문

로그는 `target/pa04-second-before-{rust,python,python-ft}.log`다.

#### Rust

```text
| op | size | rust median ms | rust min ms |
| bootstrap | 200/batch; per unit | 0.250896500 | 0.241891500 |
| get_ui_settings | 200/batch; per unit | 0.152049500 | 0.146045000 |
| set_language_round_trip | 100/batch; per unit | 1.952760000 | 1.855079000 |
| resolve_recording_paths | 200/batch; per unit | 0.001195000 | 0.001185500 |
| start_brir_to_first_event | 5/batch; per unit | 0.321540000 | 0.307860000 |
| poll_job_drain | 5/batch; per unit | 0.878000000 | 0.790900000 |
| job_event_emit | 5/batch; per unit | 2.843600000 | 2.725460000 |
| detect_sweep | 20/batch; per unit | 13.203190000 | 12.463785000 |
| catalog_translate | 100000/batch; per unit | 0.000482714 | 0.000469235 |
```

#### CPython 3.14.5

```text
| op | size | python median ms | python min ms |
| bootstrap | 200/batch; per unit | 1.112545000 | 1.056918000 |
| get_ui_settings | 200/batch; per unit | 0.453651500 | 0.436177000 |
| set_language_round_trip | 100/batch; per unit | 3.948974000 | 3.538539000 |
| resolve_recording_paths | 200/batch; per unit | 0.002921500 | 0.002854000 |
| start_brir_to_first_event | 5/batch; per unit | 0.491299998 | 0.451179998 |
| poll_job_drain | 5/batch; per unit | 4.716720007 | 4.611279993 |
| job_event_emit | 5/batch; per unit | 44.026559999 | 43.114700000 |
| detect_sweep | 20/batch; per unit | 0.679625000 | 0.656230000 |
| catalog_translate | 100000/batch; per unit | 0.000445817 | 0.000435389 |
```

#### CPython 3.14.7 free-threaded

```text
| op | size | python median ms | python min ms |
| bootstrap | 200/batch; per unit | 1.087376000 | 1.071131500 |
| get_ui_settings | 200/batch; per unit | 0.436693500 | 0.425221000 |
| set_language_round_trip | 100/batch; per unit | 3.425116000 | 3.257190000 |
| resolve_recording_paths | 200/batch; per unit | 0.003093000 | 0.003002000 |
| start_brir_to_first_event | 5/batch; per unit | 0.537399997 | 0.503580002 |
| poll_job_drain | 5/batch; per unit | 5.179760006 | 5.099780002 |
| job_event_emit | 5/batch; per unit | 47.062979999 | 46.185379999 |
| detect_sweep | 20/batch; per unit | 0.717885000 | 0.688170001 |
| catalog_translate | 100000/batch; per unit | 0.000500061 | 0.000488503 |
```

### 2차 AFTER 원문

로그는 `target/pa04-second-final-{rust,python,python-ft}.log`다. 부모 세션에서 여섯 로그의 원문 행을 확인했다.

#### Rust

```text
| op | size | rust median ms | rust min ms |
| bootstrap | 200/batch; per unit | 0.254561500 | 0.251191000 |
| get_ui_settings | 200/batch; per unit | 0.152443500 | 0.149791500 |
| set_language_round_trip | 100/batch; per unit | 2.073037000 | 1.949460000 |
| resolve_recording_paths | 200/batch; per unit | 0.001266000 | 0.001202000 |
| start_brir_to_first_event | 5/batch; per unit | 0.319240000 | 0.309980000 |
| poll_job_drain | 5/batch; per unit | 1.052300000 | 1.025700000 |
| job_event_emit | 5/batch; per unit | 2.816680000 | 2.730960000 |
| detect_sweep | 20/batch; per unit | 0.281940000 | 0.272635000 |
| catalog_translate | 100000/batch; per unit | 0.000159515 | 0.000158017 |
```

#### CPython 3.14.5

```text
| op | size | python median ms | python min ms |
| bootstrap | 200/batch; per unit | 1.056128000 | 1.008284000 |
| get_ui_settings | 200/batch; per unit | 0.377525000 | 0.371006000 |
| set_language_round_trip | 100/batch; per unit | 3.177987000 | 3.059407000 |
| resolve_recording_paths | 200/batch; per unit | 0.002878500 | 0.002816500 |
| start_brir_to_first_event | 5/batch; per unit | 0.483500000 | 0.452200000 |
| poll_job_drain | 5/batch; per unit | 4.692460003 | 4.538479992 |
| job_event_emit | 5/batch; per unit | 43.925420006 | 43.227359996 |
| detect_sweep | 20/batch; per unit | 0.746400000 | 0.672965000 |
| catalog_translate | 100000/batch; per unit | 0.000445671 | 0.000437092 |
```

#### CPython 3.14.7 free-threaded

```text
| op | size | python median ms | python min ms |
| bootstrap | 200/batch; per unit | 1.098530500 | 1.081044500 |
| get_ui_settings | 200/batch; per unit | 0.405825000 | 0.386744000 |
| set_language_round_trip | 100/batch; per unit | 3.223068000 | 3.147594000 |
| resolve_recording_paths | 200/batch; per unit | 0.006487000 | 0.003096500 |
| start_brir_to_first_event | 5/batch; per unit | 0.524259999 | 0.501299999 |
| poll_job_drain | 5/batch; per unit | 5.123900011 | 5.031160003 |
| job_event_emit | 5/batch; per unit | 46.695579996 | 46.130780000 |
| detect_sweep | 20/batch; per unit | 0.823010000 | 0.738740000 |
| catalog_translate | 100000/batch; per unit | 0.000497418 | 0.000490986 |
```

### 전후 비율과 판정

각 시점의 `Python median / Rust median`이다. **9개 연산 모두 두 인터프리터 대비 1.0 이상이다.** FT의 파일명 처리 중앙값과 최솟값 차이가 커, 해당 5.124배를 안정적인 차이라고 단정하지 않는다. 더 빠른 배치로 바꾸어 보고하지 않았다.

| 연산 | BEFORE 3.14 | BEFORE 3.14t | AFTER 3.14 | AFTER 3.14t |
|---|---:|---:|---:|---:|
| bootstrap | 4.434 | 4.334 | 4.149 | 4.315 |
| get_ui_settings | 2.984 | 2.872 | 2.476 | 2.662 |
| set_language_round_trip | 2.022 | 1.754 | 1.533 | 1.555 |
| resolve_recording_paths | 2.445 | 2.588 | 2.274 | 5.124 |
| start_brir_to_first_event | 1.528 | 1.671 | 1.515 | 1.642 |
| poll_job_drain | 5.372 | 5.899 | 4.459 | 4.869 |
| job_event_emit | 15.483 | 16.550 | 15.595 | 16.578 |
| detect_sweep | 0.051 | 0.054 | 2.647 | 2.919 |
| catalog_translate | 0.924 | 1.036 | 2.794 | 3.118 |

### 최적화와 동작 보존

**스윕 감지.** 정상 demo도 WAV 전체를 디코딩하던 처리를 바꿨다. `brir/sweep_grid.rs`의 내부 헤더 판독기는 기존 IO 판독기와 같은 RIFF/RF64·extensible·컨테이너 경계 검증을 수행한다. 프레임 수로 추정한 길이가 양수이고 격자 편차가 0.15 이하이면 샘플은 디코딩하지 않는다. 그 외에는 기존 envelope 처리를 호출한다. Python envelope는 모든 프레임·채널을 사용하므로 임의로 앞부분만 읽지 않았다. 시간은 13.203190 → 0.281940ms로 줄어 46.83배 빨라졌다.

IO 크레이트에는 공개 메타데이터 판독기가 없고 해당 크레이트의 변경은 허용되지 않았다. 그 때문에 서비스에 컨테이너 검증 코드가 중복된다. 헤더 변조·잘림·RIFF/RF64·extensible 입력에 대해 기존 디코더와 수락/거절 및 오류 문자열을 비교하는 테스트를 추가했다. 향후 IO 판독 규칙을 바꾸면 두 구현을 함께 검토해야 한다. 정상 길이 입력의 디코딩 생략, off-grid 입력의 전체 디코딩, 혼합 샘플레이트도 검증했다.

**번역.** `brir/mod.rs`에서 일반 스칼라 인자는 원문의 리터럴 구간과 자리표시자를 한 번 순회하며 결과에 직접 기록한다. 숫자는 기존 `serde_json::Value` 표기를 유지한다. 0.000482714 → 0.000159515ms로 줄어 3.03배 빨라졌다. 치환 결과가 뒤 인자의 자리표시자를 만들거나 중첩 괄호가 있는 입력은 기존 순차 치환을 유지한다. 인자 이름·값의 괄호, 배열·객체도 이 예외 처리 대상이다. 특정 번역 키를 특별 취급하거나 결과를 캐시하지 않았다. 9개 언어의 문자열과 길이 0~6의 괄호·문자 조합을 기존 치환 결과와 비교했다. README 바이트 골든도 통과했다.

**이벤트 발행.** 같은 전체 payload(`level`, `message`, `index`)의 전후 측정은 2.843600 → 2.816680ms다. 첫 감사의 BEFORE 2.771040ms와도 약 1.65% 차이다. 별도 진단에서도 2.818420ms로 재현했다. 6.138100ms 증가를 지속적인 회귀로 확인하지 못했으며 당시 부하나 스케줄링 때문이라고 확정할 자료도 없다. `join`은 발행 타이머 밖에 있고, 생산 코드의 이벤트 처리나 jobs 구현을 이번에 변경하지 않았다.

`--emit-diagnostic`는 두 payload의 발행과 join을 따로 재며, 3회 워밍업·11회 측정·배치당 5개 작업을 사용한다. index-only 결과를 기존 workload의 성능 개선으로 계산하지 않았다. 로그는 `target/pa04-second-emit-diagnostic.log`다.

```text
| full_payload_emit | 5/batch; per unit | 2.818420000 | 2.723960000 |
| full_payload_join | 5/batch; per unit | 0.031920000 | 0.026940000 |
| index_only_emit | 5/batch; per unit | 1.371600000 | 1.264940000 |
| index_only_join | 5/batch; per unit | 0.029400000 | 0.024840000 |
```

최종 비율은 전부 1.5 이상이다. 중간 언어 변경 측정에서는 1.458/1.471이었다. 파일 재조회·JSON 저장·ko/en 카탈로그의 독립 payload 생성에 시간이 든다. 외부 설정 키 보존과 실제 쓰기 계약 때문에 이 작업을 생략하지 않았다. 첫 이벤트 연산에도 설정 payload 생성·요청 검증·스레드 시작·첫 poll 비용이 남는다. 전체 DSP 실행 시간으로 해석하면 안 된다.

### 검증

ASTRA가 각 최적화 뒤 서비스·정책 테스트를 실행했고, 마지막에는 jobs까지 검증했다. 부모 세션은 디프를 검토한 뒤 다음 명령을 포그라운드에서 직접 재실행하여 통과했다.

```bash
cargo fmt -p impulcifer-service -p impulcifer-jobs -- --check && cargo clippy -p impulcifer-service -p impulcifer-jobs --all-targets -- --no-deps -D warnings && cargo test -p impulcifer-service -p impulcifer-jobs -p impulcifer-policy
```

부모 세션의 합계는 service **106 passed, 0 failed, 2 ignored**, jobs **13 passed**, policy **6 passed**로 총 **125 passed, 0 failed, 2 ignored**다. 워커의 최종 집계와 같다. 현재 작업 트리의 recovery 등 다른 기능 테스트까지 부모 세션에서 통과했다. 제외된 둘은 CABLE-A 실기와 설치된 Velopack 실서비스 피드 테스트다.

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.42s
Finished `test` profile [unoptimized + debuginfo] target(s) in 0.41s
Running tests\perf_smoke.rs
running 2 tests
test bench_smoke_impulcifer_service ... ok
test bench_smoke_pipeline_demo ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests\brir_outputs.rs
test demo_readme_korean_bytes_match_python ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests\demo_parity.rs
test demo_brir_matches_python_within_budget ... ok
test demo_vbass_matches_python_within_budget ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Running tests\gates.rs
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

DSP 합산 순서나 수치 알고리즘, 골든, 허용 오차는 바꾸지 않았다. `target/pa04-second-golden-errors.log`에서 확인한 출력별 최대 절대 오차는 아래와 같다. 이는 현재 출력과 기존 Python 골든의 차이이며 이번 최적화가 새로 만든 오차라는 뜻은 아니다. 해당 로그의 debug 프로파일 실행 시간은 성능 표에 사용하지 않는다.

| 시나리오 | 출력 | 트랙 수 | 최대 절대 오차 |
|---|---|---:|---:|
| default | hesuvi.wav | 14 | 9.750947356e-7 |
| default | hrir.wav | 16 | 9.750947356e-7 |
| vbass | hesuvi.wav | 14 | 4.656612873e-8 |
| vbass | hrir.wav | 16 | 4.656612873e-8 |

중간 실패도 숨기지 않았다. 새 테스트 모듈 위치 때문에 clippy `items_after_test_module`가 실패하여 위치를 수정했다. envelope 테스트의 최초 입력은 길이 추정만으로 통과했으므로 off-grid를 검증하도록 수정하고 Python 값도 확인했다. 다른 워커의 sys-win 수정 중 컴파일 실패와 updater 테스트의 `Checksum file fetch failed: timeout: global`도 있었으나 해당 파일은 수정하지 않았고 전체 재실행에서 통과했다.

### 실행 명령과 로그

아래 ASTRA 명령은 모두 `--manifest-path E:/Impulcifer/Cargo.toml`을 사용했다. 벤치와 Python은 BEFORE/AFTER 각각 실행했다. 출력은 해당 target 로그로 리다이렉트하되 명령이 끝날 때까지 포그라운드에서 기다리고 종료 코드를 확인했다.

```text
cargo bench --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-service --bench perf
cargo bench --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-service --bench perf -- --emit-diagnostic
py -3.14 E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py
C:/Users/32170336/AppData/Local/impulcifer-bench/py314t/Scripts/python.exe E:/Impulcifer/tests/migration/bench_oracle_impulcifer_service.py
cargo fmt --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-service -p impulcifer-jobs -- --check
cargo clippy --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-service -p impulcifer-jobs --all-targets -- --no-deps -D warnings
cargo test --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-service
cargo test --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-policy
cargo test --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-service -p impulcifer-jobs -p impulcifer-policy
cargo test --manifest-path E:/Impulcifer/Cargo.toml -p impulcifer-service --test demo_parity -- --nocapture
```

최적화 직후의 로그는 `target/pa04-second-sweep-{service,policy}-tests.log`, `target/pa04-second-translate-{service,policy}-tests.log`다. ASTRA 최종 로그는 `target/pa04-second-final-tests-retry2.log`, `target/pa04-second-last-fmt.log`, `target/pa04-second-last-clippy.log`다.

### 이번 변경 파일과 남은 사항

- `crates/impulcifer-service/src/brir/sweep_grid.rs`: 헤더 우선 감지와 회귀 테스트.
- `crates/impulcifer-service/src/brir/mod.rs`: 단일 순회 번역과 기존 치환 결과 비교 테스트.
- `crates/impulcifer-service/tests/bench_support/service.rs`: payload별 이벤트 발행·join 진단. 기본 workload는 유지.
- `crates/impulcifer-service/benches/perf.rs`: 선택 인자 `--emit-diagnostic`. PA03와 기본 PA04는 유지.
- `docs/rust/perf/impulcifer-service.md`: 부모 세션이 기존 기록을 보존하고 2차 감사 결과 추가.

공개 시그니처를 변경하지 않았고 새 의존성·unsafe·SIMD·f32 수치 처리를 추가하지 않았다. 등록부가 인용할 테스트는 기존 **`impulcifer-service::bench_smoke_impulcifer_service`**다. `features.toml`은 수정하지 않았다.

**Rust app round trip, no opponent**는 성공 측정값이 없다. 기존 스모크 드라이버와 첫 감사의 CDP 실패 기록을 확인했으며 이번에는 실행을 반복하지 않았다. 선택 항목이므로 성능 판정에는 넣지 않았다. M2/M5 전체 감사가 아니므로 최대 상주 메모리는 측정하지 않았다. 다른 워커의 파일과 사용자의 기존 변경은 보존했으며 커밋·푸시·CI는 실행하지 않았다.
