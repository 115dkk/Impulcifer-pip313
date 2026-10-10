# ADR 0007: 몰입형 포맷은 위치로 정한 스피커 슬롯 하나에 대응시키고, 포맷마다 공식 순서 파일을 따로 쓴다

- **상태:** 결정됨 (2026-10-10)
- **결정자:** 유지보수자 (115dkk)
- **선행:** ADR 0002(3.x 재작성), ADR 0004(룸 EQ v2, 확장 필드 방식)

## 맥락

3.x는 스피커 15개(FL FR FC BL BR SL SR WL WR TFL TFR TSL TSR TBL TBR)만 안다. 녹음 파일 이름(`FL,FR.wav`), hrir.wav·hesuvi.wav의 트랙 순서, 출력 복구, 녹음 레이아웃이 모두 이 목록에 묶여 있다. 한국 BRIR 사용자 모임이 21방향(7.1 + Auro 높이 다섯 + Atmos 천장 여섯 + 천정 + 와이드)을 측정했지만, Impulcifer가 아는 15개만 처리하고 Auro 높이 스피커와 천정 스피커는 따로 빼 둘 수밖에 없었다. 유지보수자는 Dolby Atmos 24.1.10, DTS:X Pro 30.2, NHK 22.2, Auro-3D 13.1의 스윕 재생·녹음·BRIR 합성·복구를 지원하고, 측정이 이 포맷이면 hrir.wav·hesuvi.wav 말고 그 포맷의 공식 채널 순서로 나열한 WAV를 하나 더 쓰기로 했다.

포맷마다 같은 라벨이 다른 위치를 가리키는 것이 이 작업이 틀어지기 쉬운 이유다.

- 22.2의 FL/FR은 ±60°이고 ±30° 쌍은 FLc/FRc다. Impulcifer의 FL(±30°)이 아니라 WL에 해당한다.
- DTS의 Lb/Rb는 귀 아래 앞쪽(low front)이고, Auro의 Lb/Rb는 귀 높이 뒤쪽(back)이다.
- Dolby의 Lrs1은 Lrs보다 앞(약 120°)이고 Lrs2는 뒤(약 145°)다.
- FFmpeg의 `22.2` 레이아웃은 SMPTE 순서와 10~21번이 다르다.

트랙 하나가 다른 스피커나 반대쪽 귀로 가도 소리만으로는 거의 알아채지 못하므로, 대응 규칙을 데이터와 테스트로 고정해야 한다.

## 결정

1. **슬롯은 위치로 정하고, 포맷 채널은 슬롯 하나에 대응한다.** 슬롯마다 기준 방향(방위각, 고도, 방위각은 ITU처럼 왼쪽이 +)과 층을 둔다. 층은 고도가 아니라 역할로 정한다. 귀 높이(Ear), 벽면 높이층(Height, 22.2·Auro의 높이 고리, DTS High, Dolby Lfh/Lrh), 천장(Top, Dolby Ltf/Ltm/Ltr, DTS Ltf/Ltr, 천정), 바닥(Down)으로 나눈다. 포맷 채널은 같은 층에서 기준 방향이 가장 가까운 슬롯에 대응한다. 그 규칙과 다르게 대응하는 것은 아래 예외표에 출처와 함께 적은 다섯 건뿐이며, 테스트가 규칙과 예외표를 그대로 계산한다.

   | 포맷 채널 | 슬롯 | 규칙상 슬롯 | 근거 |
   | --- | --- | --- | --- |
   | 22.2 TpSiL/TpSiR | TSL/TSR | 높이층의 HBL | Dolby Studio Certification Guide 표 1(Ltm ↔ TpSiL), DTS TS 103 491 표 7-27·7-28(22.2의 TpSiL을 Lhs로, Lhs는 Ltm을 겸함) |
   | DTS Lhs/Rhs | TSL/TSR | 높이층의 HBL | DTS TS 103 491 표 7-28 주석(Lhs는 Left Top in middle을 겸함), Trinnov 배치 안내서 표(Ltm ↔ Lhs) |
   | Auro Ls/Rs (110°) | SL/SR | SL2/SR2 | Trinnov 표(Dolby Ls ↔ Auro Ls ↔ DTS Lss), Auro 설치 지침 Rev 12 §3.3.1.2(7.1 기반 배치에서 이 쌍을 Lss/Rss로 부름) |
   | Auro Lb/Rb (150°) | BL/BR | BL2/BR2 | Trinnov 표(Dolby Lrs ↔ Auro Lb ↔ DTS Lsr) |
   | DTS Lsr/Rsr (150°) | BL/BR | BL2/BR2 | Trinnov 표(같은 행) |

   기존 15개 슬롯의 뜻은 바꾸지 않는다. FL은 Dolby가 Lsc를 둘 때 L을 45° 가까이로 옮기더라도 30°다.

2. **새 슬롯은 25개다.** 녹음 파일 이름에 그대로 쓰는 코드다.

   | 층 | 코드 | 기준 방향 | 대응하는 포맷 채널 |
   | --- | --- | --- | --- |
   | 귀 | FCL FCR | ±15° | Dolby Lc/Rc, DTS Lc/Rc |
   | 귀 | SCL SCR | ±10° | Dolby Lsc/Rsc(스크린) |
   | 귀 | SL1 SR1 | ±75° | Dolby Ls1/Rs1 |
   | 귀 | SL2 SR2 | ±105° | Dolby Ls2/Rs2, DTS Ls/Rs |
   | 귀 | BL1 BR1 | ±120° | Dolby Lrs1/Rrs1 |
   | 귀 | BL2 BR2 | ±150° | Dolby Lrs2/Rrs2 |
   | 귀 | BCL BCR | ±165° | Dolby Lcs/Rcs |
   | 귀 | BC | 180° | 22.2 BC, DTS Cs, Dolby Cs |
   | 높이층 | HFL HFR | ±30°, 30° | 22.2 TpFL/TpFR, Auro HL/HR, DTS Lh/Rh, Dolby Lfh/Rfh |
   | 높이층 | HFC | 0°, 30° | 22.2 TpFC, Auro HC, DTS Ch |
   | 높이층 | HBL HBR | ±135°, 30° | 22.2 TpBL/TpBR, Auro HLs/HRs, DTS Lhr/Rhr, Dolby Lrh/Rrh |
   | 높이층 | HBC | 180°, 30° | 22.2 TpBC, DTS Chr |
   | 천장 | TC | 천정 | 22.2 TpC, Auro T, DTS Oh |
   | 바닥 | DFL DFR | ±45°, −30° | 22.2 BtFL/BtFR, DTS Lb/Rb(low front) |
   | 바닥 | DFC | 0°, −30° | 22.2 BtFC, DTS Cb |

   새 코드는 다른 슬롯으로 가는 포맷 라벨과 같은 글자일 수 없다(대소문자 무시). 그래서 Dolby Lc는 FLC(22.2의 FLc는 FL로 간다)가 아니라 FCL이고, 바닥층은 뒤(back)의 B와 헷갈리지 않게 D(down)로 쓴다. 숫자가 든 SL1·SL2·BL1·BL2는 Dolby의 Ls1/Ls2/Lrs1/Lrs2를 그대로 옮긴 것이며, 녹음 파일 이름은 이 네 코드에 한해 숫자를 받는다. BL1 < BL < BL2는 120°/135°/150° 순서다. LFE와 LFE2는 측정하지 않는 무음 자리다.

3. **포맷마다 공식 순서 파일을 따로 쓴다.** 채널마다 왼쪽 귀, 오른쪽 귀 두 트랙을 공식 순서대로 적고, LFE 채널은 제자리에 무음 두 트랙으로 둔다. 중간 무음 제거 옵션과 상관없이 압축하지 않으며, `ICHL` 청크에 포맷 라벨로 된 트랙 이름(`TpFL-left` 등)을 언제나 넣는다.

   | 파일 | 트랙 | 순서의 출처 |
   | --- | ---: | --- |
   | `nhk_22.2.wav` | 48 | SMPTE ST 2036-2 표 1(= ARIB STD-B59 표 2-1 = ITU-R BS.2051-3 System H = BS.2094-2 AP_00010009) |
   | `auro_13.1.wav` | 28 | Auro "AURO-3D in multi-channel WAV-files" Rev 2(2024) 채널 맵(WAVEX 마스크 0x2FE3F) |
   | `dtsx_30.2.wav` | 64 | ETSI TS 103 491 V1.2.1 표 B-5(= 표 C-4 = TS 103 584 표 3)의 채널 마스크 순서 |
   | `atmos_24.1.10.wav` | 70 | Dolby가 정한 순서가 없다. 아래 참고 |

   DTS는 30.2 디코더의 PCM 출력 순서를 공개하지 않았다. 표 B-5는 "30.2 = 32비트 전부"를 정의하는 스피커별 마스크 순서이고 세 문서에 같은 표가 있어서 이것을 쓴다. DTS-UHD 비트스트림 안의 쌍 단위 활성 마스크(표 7-28)를 펼친 순서는 다르다.

   Dolby는 9.1.6보다 큰 배치의 채널 순서를 공개하지 않았다. 그래서 `atmos_24.1.10.wav`는 Impulcifer가 정한 순서다. 1~16번은 9.1.6 교환 순서(Apple `kAudioChannelLayoutTag_Atmos_9_1_6`, AWS MediaConvert의 Dolby Digital Plus JOC 입력 L R C LFE Ls Rs Lrs Rrs Lw Rw Ltf Rtf Ltm Rtm Ltr Rtr)를 따른다. 17~35번째 채널은 Dolby Atmos Home Theater Installation Guidelines §4가 추가 스피커를 나열한 순서(Lc Rc Lsc Rsc Ls1 Rs1 Ls2 Rs2 Lrs1 Rrs1 Lrs2 Rrs2 Lcs Rcs Cs, 이어서 Lfh Rfh Lrh Rrh)를 따른다. 이 순서로 출력하는 디코더는 없으므로 문서의 표로 대응시켜야 한다. 이 사실은 이 ADR뿐 아니라 사용자 문서와 실행마다 쓰는 README.md에도 적는다.

4. **포맷은 모든 채널을 측정했을 때 감지한다.** 포맷의 LFE가 아닌 채널이 전부 측정됐으면 그 포맷 파일을 자동으로 쓴다. 3.x 확장 필드 `layout_files`(기본 `auto`)로 바꿀 수 있다. `none`은 포맷 파일을 쓰지 않는다. 포맷 id(`22.2`, `13.1`, `24.1.10`, `30.2`)를 쉼표로 나열하면 그 포맷은 일부만 측정했어도 쓰고, 빠진 채널은 무음으로 둔 채 로그에 적는다. 나열하지 않은 포맷은 `auto`처럼 다룬다. 측정이 포맷의 절반 이상을 덮고 7.1(FL FR FC SL SR BL BR) 밖의 슬롯을 하나라도 공유하는데 빠진 채널이 있으면, 빠진 채널을 포맷 라벨과 슬롯 코드로 알려 준다.
   7.1.4나 9.1.6 같은 하위 배치를 따로 감지하지는 않는다. 그 파일은 hrir.wav와 사이드·백 순서만 다른 같은 내용이고, 큰 측정에서는 여러 파일이 겹쳐 생긴다. 그런 측정에서 Atmos 순서 파일이 필요하면 `layout_files=24.1.10`을 쓴다.

5. **hrir.wav와 hesuvi.wav는 새 슬롯을 TBR 뒤에 붙인다.** 순서는 2항의 표와 같고 슬롯마다 왼쪽 귀, 오른쪽 귀다. 끝의 측정하지 않은 슬롯을 잘라 내는 규칙은 그대로라서, 기존 측정의 출력은 바이트 단위로 같다. LFE2는 hrir.wav에 넣지 않는다. 2.x와 비교하는 상수는 `*_2X`로 남기고 3.x 목록이 그것으로 시작하는지 테스트한다.

6. **녹음과 복구도 같은 표를 쓴다.** 녹음 레이아웃 `22.2`, `13.1`, `24.1.10`, `30.2`는 공식 순서를 장치 채널 순서로 쓴다. 22.2에서 FL을 고르면 스윕은 7번 채널(FLc)에, WL은 1번 채널(FL)에 나간다. 복구는 포맷 파일도 원본으로 읽고, 트랙은 인덱스가 아니라 슬롯으로 맞춘다. WL은 nhk_22.2.wav에서 1·2번, hrir.wav에서 17·18번이다. 원본이 여럿이면 표본 하나까지 같아야 하며, 복구한 측정이 포맷을 모두 덮으면 그 포맷 파일도 만든다.

## 결과

- 21방향 커뮤니티 측정은 Auro 13.1을 빠짐없이 덮으므로 `auro_13.1.wav`가 생긴다. 22.2·DTS·Atmos는 빠진 채널을 알려 준다.
- 기존 측정(데모 7개 스피커 포함)은 출력 파일과 바이트가 그대로다.
- 22.2 리그 사용자가 ±60° 녹음을 스펙 라벨대로 `FL.wav`라고 부르면 FLc 자리에 들어간다. 녹음 레이아웃이 슬롯 코드로 파일 이름을 정하고, README.md가 포맷 파일마다 채널·슬롯 대응표를 적고, 22.2에서 FL/FR만 있고 WL/WR이 없으면 그 가능성을 알려 주는 것으로 이 위험을 줄인다.
- 같은 스피커로 두 포맷의 다른 자리를 겸하는 방(예: Auro HL을 Atmos Ltf 스피커로 겸함)은 그 녹음을 두 슬롯 이름으로 복사해야 한다. 대응을 자동으로 넘겨짚지 않는다.
- 이 결정을 되돌리거나 슬롯의 뜻을 바꾸려면 새 ADR이 필요하다.
