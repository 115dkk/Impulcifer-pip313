# ADR 0019: SOFA 파일 내보내기

- **상태:** 결정됨 (2026-11-14)
- **결정자:** 유지보수자 (115dkk)
- **관련:** ADR 0007(슬롯의 방향), ADR 0013(재지 않은 스피커 채우기), ADR 0016·0017·0018(LFE)

## 맥락

맥 테스터가 SOFA 내보내기를 요청했다. SOFA(AES69)는 임펄스 응답을 방향과 함께 담는 표준 파일이다. `hesuvi.wav` 같은 채널 순서 파일은 재생기가 채널마다 방향을 미리 알아야 하지만, SOFA는 방향이 파일 안에 있어서 바이노럴 렌더러가 그 방향으로 소리를 놓을 수 있다.

2026-11-14에 조사하고 시험한 결과는 다음과 같다.

- **재생기가 읽는 규약:** libmysofa 기반 재생기, 3D Tune-In, Steam Audio는 `SimpleFreeFieldHRIR`만 읽는다. libmysofa는 방 종류를 검사하지 않아서 이 규약에 BRIR을 넣어도 읽는다. 방에서 잰 응답을 위한 `SingleRoomSRIR`는 읽는 재생기를 찾지 못했다.
- **방 종류:** AES69의 규칙(pyfar `sofar` 1.3.0의 검사)은 `SimpleFreeFieldHRIR`의 `RoomType`을 "free field"로 강제한다. "reverberant"로 저장하면 규약 위반이고, 방 설명도 따로 요구한다.
- **파일 형식:** SOFA는 netCDF-4(HDF5) 파일이다. libmysofa는 그룹의 자식 목록을 조밀 링크 저장소(fractal heap)에서만 읽고, 객체 헤더에 바로 기록된 링크 메시지(6번)를 만나면 지원하지 않는 형식으로 거부한다.
  - 순수 Rust 라이브러리 `netcdf-writer` 0.13.0과 그 바탕인 `hdf5-writer`는 링크를 객체 헤더에 바로 기록한다. 그렇게 만든 파일은 pyfar 검사는 통과했지만 libmysofa(와 그 Rust 이식판 `sofar` 0.4.0)는 열지 못했다.
  - `netcdf` 크레이트의 정적 빌드(netCDF-C와 HDF5)는 호환이 확실하지만, 모든 빌드와 pip의 sdist 설치에 cmake와 C++ 컴파일러가 필요해진다.

## 결정

1. **선택은 둘이고 기본은 끈다.** 확장 설정 `sofa`와 `sofa_split_headphone_eq`(둘 다 기본 false)를 둔다. CLI는 `--sofa`, `--sofa_split_headphone_eq`다. 기본값에서는 모든 출력이 바이트 단위로 지금과 같다.
2. **규약은 `SimpleFreeFieldHRIR` 1.0(SOFA 2.1)이다.** `RoomType`은 규약대로 "free field"로 두고, 방에서 잰 BRIR이라는 사실은 `Comment` 속성에 기록한다.
3. **`brir.sofa`는 `hrir.wav`와 같은 BRIR이다.** 헤드폰 보정과 EQ가 들어 있고, 출력 샘플레이트를 따른다.
   - 스피커도 `hrir.wav`와 같다. 잰 스피커에, 재지 않은 스피커 채우기(ADR 0013)를 켰으면 채운 스피커도 들어간다.
   - LFE는 방향이 없으므로 넣지 않는다.
4. **위치는 슬롯의 공칭 방향이다.** 방위각과 고도는 `SLOTS`의 값(ITU 규약, 왼쪽이 양수)을 그대로 쓴다. SOFA의 구면 좌표와 방향이 같다. 거리는 측정에서 알 수 없으므로 2 m로 고정한다. 청취자, 두 귀, 방출기의 위치는 규약의 기본값이다.
5. **헤드폰 보정을 따로 내보내는 선택.** `sofa_split_headphone_eq`를 켜면 `brir.sofa`와 함께 다음 두 파일을 저장한다.
   - `brir-no-headphone-eq.sofa`: 헤드폰 보정만 뺀 BRIR.
   - `headphone-eq.wav`: 귀마다 하나인 최소 위상 FIR(두 트랙)을 32비트 부동소수점 WAV로 저장한다. 올리는 EQ는 샘플이 1을 넘어서(데모에서 약 1.96) 다른 WAV처럼 32비트 정수로 저장하면 잘린다. 이득을 바꾸지 않고 FIR을 그대로 남기려고 부동소수점을 쓴다.
   - 보정 곡선은 룸 보정, 헤드폰 보정, 사용자 EQ의 합에서 목표를 뺀 곡선 하나다. 그래서 분리판은 헤드폰 항만 빼고 곡선을 다시 계산하고, 헤드폰 EQ는 헤드폰 항만으로 같은 다듬기와 이득 한계를 거쳐 만든다.
   - EQ 뒤의 단계는 이렇게 맞춘다. 감쇠는 같은 설정으로 건다. 좌우 균형은 최종본에서 구한 보정 필터를 그대로 건다. 정규화(그리고 리샘플 뒤의 정규화)는 최종본의 이득을 그대로 건다. 그래서 분리판에 헤드폰 EQ를 걸면 최종본과 레벨이 같다.
   - 곡선의 다듬기와 이득 한계는 비선형이고 감쇠는 응답에서 다시 재므로, 그 결과는 최종본과 근사로만 같다. 허용 오차는 `docs/rust/SOFA.md`에 둔다.
   - 헤드폰 보정을 쓰지 않는 작업이면 분리 파일을 저장하지 않고 작업 기록에 알린다.
6. **작성기는 직접 만든다.** `impulcifer-io`에 SOFA 파일만 만드는 HDF5 작성기를 순수 Rust로 둔다. netCDF-C가 만드는 배치(조밀 링크 저장소, 차원 스케일, 전역 힙의 `DIMENSION_LIST`)를 따른다. 새 의존성은 없고, 테스트에만 libmysofa 이식판 `sofar`를 쓴다.
7. **출력은 결정적이다.** 같은 입력이면 같은 바이트가 나온다. 날짜 속성은 환경 변수 `SOURCE_DATE_EPOCH`가 있으면 그 시각, 없으면 현재 UTC 시각이다. 앱 버전은 넣지 않는다. 3.21.0은 `APIVersion`과 `ApplicationVersion`에 앱 버전을 넣었고, 그래서 버전만 올린 3.21.1 PR에서 brir-3x의 SOFA 해시가 달라졌다. 3.21.1부터 `APIName`은 `Impulcifer SOFA writer`, `APIVersion`은 작성기 형식 버전(`SOFA_WRITER_VERSION`, 저장하는 바이트가 바뀔 때만 올림)이고, 선택 속성 `ApplicationVersion`은 두지 않는다.
8. **검증.** `cargo test`는 libmysofa 이식판으로 파일을 연다. CI(`brir-3x.yml`)는 pyfar `sofar`의 규약 검사와 원본 libmysofa로 데모 출력을 열고, `.sofa` 파일도 해시해 기준과 비교한다. 이식판은 원본보다 너그럽다. 첫 작성기가 낸 파일을 이식판과 pyfar는 열었지만 원본 libmysofa 1.3.2는 거부했다. 원인은 둘이었다. 64바이트가 넘는 전역 속성(`Comment`)을 객체 헤더에 바로 둔 것, 그리고 전역 힙을 64 KiB 너머에 둔 것이다(원본은 그 위치를 16비트로 다룬다). 그래서 전역 속성도 조밀 저장소에 두고 전역 힙을 데이터 앞에 둔다. 작성기를 고칠 때는 원본 libmysofa(`mysofa2json -c`)로 확인한다.

## 결과

- SOFA를 읽는 재생기에서 개인 BRIR을 쓸 수 있다. 방향은 공칭값이라 실제 스피커의 각도와 다를 수 있다.
- 헤드폰 EQ를 재생 쪽에서 따로 걸고 싶은 사람은 분리판과 `headphone-eq.wav`를 쓴다.
- 다시 제안하지 말 것: `netcdf-writer`나 `hdf5-writer`로 저장하기(libmysofa가 열지 못한다), `RoomType`을 "reverberant"로 두기(규약 위반), LFE를 SOFA에 넣기.
- 남은 일: `SingleRoomSRIR`는 읽는 재생기가 생기면 다시 본다. 머리 돌려 측정에서 실제로 돈 각도를 위치로 쓰는 일은 이 결정에 들어 있지 않다.
