# 무음 채널과 HeSuVi 호환성

2026-09-05, Impulcifer `5fdf2995793a73c652a516f7fbbf9696d8c6221d` 기준 조사.

## 결론

기본 동작에서는 HeSuVi용 첫 14채널을 고정 배치로 유지한다. 그 뒤에 연속해서 나오는
무음 확장 스피커 쌍은 자동으로 제거한다. 앞쪽 빈자리를 지우거나 유효
확장 채널의 위치를 앞으로 옮기지 않는다. 생성과 복원에 같은 규칙을 적용한다.

| 입력 응답 | hesuvi.wav | hrir.wav |
|---|---:|---:|
| FL/FR만 있음 | 14 | 16 |
| 기본 7스피커만 있음 | 14 | 16 |
| TFL까지 있음, 이후 무음 | 20 | 22 |
| TFR까지 있음, 이후 무음 | 22 | 24 |
| TBR 응답이 있음 | 30 | 32 |

마지막 스피커의 한쪽 귀만 무음이면 그 쌍을 보존한다. 모든 샘플이 정확히
0인 쌍만 제거하므로 낮은 레벨의 응답이나 잔향을 문턱값으로 잘라내지 않는다.

## 확인한 코드

- [HeSuVi 공식 소스 unit1.pas](https://sourceforge.net/projects/hesuvi/files/source/unit1.pas/download)
  - `TForm1.SAVEFile`(2636행 부근)은 선택한 WAV를 `Convolution:` 명령에 넘긴다.
  - `TForm1.SAVEVol`(2748행 부근)는 정해진 가상 채널들을 좌우로 합산한다.
  - 측정 WAV에서 비어 있는 스피커를 찾아 채널 배치를 수정하는 경로가 없다.
- [HeSuVi 공식 배포본 2.0.0.1](https://sourceforge.net/projects/hesuvi/files/HeSuVi_2.0.0.1.exe/download)의
  `HeSuVi/hesuvi.txt`(39–47행)는 컨볼루션 대상으로 14개 가상 채널을 선택한다.
  설치 프로그램은 실행하지 않고 7z 내용만 추출했다.
- [EqualizerAPO-XT ConvolutionFilter.cpp](https://github.com/115dkk/EqualizerAPO-XT/blob/master/filters/ConvolutionFilter.cpp)
  - `initializeFilters`의 `irChannel = i % ir->channels`는 파일 채널 수가
    선택 채널 수보다 작으면 IR 채널을 순환 적용한다.
  - `distinctIrChannels = min(channelCount, ir->channels)` 및 초기화 루프는
    선택한 채널 수를 초과하는 뒤쪽 IR을 HeSuVi의 14개 채널에 적용하지 않는다.
- Impulcifer의 `core/constants.py`에 HeSuVi 30채널과 HRIR 32채널 배치가 있고,
  `HRIR.write_wav`는 없는 응답을 0으로 채웠다. 와이드·상단 스피커용 16개
  추가 채널이 항상 생성되던 이유다. 원본 `core/brir_recovery.py`도
  최대 채널 수만 허용하고 복원 출력에 전부 0을 채웠다.

## 왜 FL/FR만 있어도 앞 14채널이 필요한가

HeSuVi 배포본의 선택 순서는 다음과 같다. WAV 번호는 1부터 센다.

| WAV 번호 | 응답 | 가상 채널 |
|---:|---|---|
| 1, 2 | FL-left, FL-right | L0, R1 |
| 3, 4 | SL-left, SL-right | SL0, SR1 |
| 5, 6 | BL-left, BL-right | RL0, RR1 |
| 7 | FC-left | C0 |
| 8, 9 | FR-right, FR-left | R0, L1 |
| 10, 11 | SR-right, SR-left | SR0, SL1 |
| 12, 13 | BR-right, BR-left | RR0, RL1 |
| 14 | FC-right | C1 |

FL/FR만 있는 비대칭 측정은 1·2·8·9번만 유효하다. 중간 빈자리를 빼서
4채널로 만들면 FR 응답이 SL 위치로 이동한다. 9번 뒤를 잘라도 해결되지
않는다. 10번부터 첫 IR을 반복해서 적용하므로 SR·BR·FC에 잘못된 응답이
생긴다. 뒤쪽 무음 슬롯이 그 스피커들을 실제로 무음으로 만드는 역할을 한다.

좌우 응답이 정확히 대칭인 특수한 7채널 IR은 순환 적용을 이용할 수 있지만,
실측 FL/FR의 동일성을 가정할 수 없다. 기본 출력에는 대칭 변환이나 14채널 미만 출력을 적용하지 않는다.
사용자가 별도 옵션을 켜면 호환성 경고와 함께 중간 무음도 제거할 수 있다. `test_hesuvi_stereo_routing_requires_all_fourteen_slots`
는 서로 다른 FL/FR 응답으로 1~13채널 절단과 4채널 재배치가 오작동하고,
30→14채널 정리는 동일한 채널별 출력을 내는지 검증한다.

## 선택 기능: 중간 무음 채널도 제거

생성 고급 옵션과 출력 복원에 `중간 무음 채널도 제거`를 제공한다.
기본값은 꺼짐이며 CLI에서는 `--remove_silent_channels`로 켠다.
화면에 고정 채널 배치를 쓰는 HeSuVi 등과 호환되지 않을 수 있다는 경고를
표시하고, 처리 로그에도 경고를 남긴다.

켜면 중간 빈자리와 개별 무음 귀 채널까지 제거한다. 남은 채널의 상대 순서와
샘플은 그대로 유지한다. FL/FR의 네 응답만 있으면 두 출력 모두 4채널이다.
모든 채널이 무음이면 채널 없는 WAV를 만들지 않고 오류를 표시한다.
JamesDSP·Hangloose·TrueHD 전용 출력은 해당 형식의 배치를 유지한다.

축소 WAV 내부에 `ICHL` RIFF 청크를 넣는다. UTF-8 JSON 형식으로
`{"version":1,"tracks":["FL-left",...]}`를 저장한다. 일반 WAV 리더는 이를
건너뛰며, Impulcifer 복원은 이 정보를 채널 수 추정보다 먼저 확인한다.
축소 결과가 우연히 14/16채널이어도 원래 스피커를 혼동하지 않는다.
중복·알 수 없는 채널 이름, 채널 수 불일치, 잘못된 버전·JSON은 거부한다.
HeSuVi는 이 채널 정보를 읽지 않으므로 메타데이터가 호환성을 보장하지 않는다.

복원 시 옵션을 끄면 생략했던 중간 채널을 다시 0으로 채워 호환 배치를 만든다.
기존 파일은 보존하므로, 예를 들어 축소 hesuvi.wav만 있는 폴더에서 복원하면
호환 hrir.wav가 생기고 원본 hesuvi.wav는 그대로 남는다. 호환 hesuvi.wav까지
필요하면 새 hrir.wav를 별도 폴더에서 복원하면 된다.
WAV를 이동하거나 이름을 바꿔도 내부 정보는 유지된다. 단, 복원 파일명은
hrir.wav/hesuvi.wav 규칙을 따른다. 오디오 편집기가 청크를 제거하면 채널
이름을 잃으므로, 편집 후 메타데이터 없는 축소 파일의 배치를 추정하지 말아야 한다.

## 복원 및 다른 출력

메타데이터 없는 `hesuvi.wav`는 14~30채널, `hrir.wav`는 16~32채널의 짝수 채널 수를 허용한다.
고정 배치의 앞부분으로 읽고 생략한 뒷부분은 0으로 해석한다. 기존 파일과
새 파일을 함께 발견해도 응답을 채널 이름별로 비교한다. 이미 있는 파일을
덮어쓰지 않는 복원 규칙은 유지한다.

HRIR의 앞 16채널에는 LFE용 두 무음 슬롯이 포함된다. 이를 제거하면 이후
BL/BR/SL/SR 위치가 달라지므로 보존한다. JamesDSP, Hangloose, TrueHD 전용
출력의 별도 고정 배치는 변경하지 않는다. HeSuVi 자체가 사용하지 않는
확장 채널이라도 실제 응답이 있으면 다른 용도와 복원을 위해 보존한다.

## 검증 기준

파일 헤더와 채널 수가 바뀌므로 WAV 파일 전체 SHA-256은 의도적으로 달라진다.
데모 무결성 테스트는 PCM_32 샘플을 원래 30채널 배치로 무음 패딩한 다음,
샘플 수·샘플레이트와 모든 PCM 샘플의 SHA-256을 비교한다. 남아 있는 신호는
1비트도 달라지면 실패하고, 유효한 확장 채널을 제거해도 실패한다.
Windows HeSuVi 실기 청취 테스트는 이 Linux 환경에서 수행하지 않았다.
