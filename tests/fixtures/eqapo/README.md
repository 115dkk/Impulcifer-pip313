# EqualizerAPO-XT 오디오 회귀 픽스처

[115dkk/EqualizerAPO-XT](https://github.com/115dkk/EqualizerAPO-XT)의
`Tests/AudioRegressionTests`에서 가져온 설정 파일과 골든 레퍼런스 출력이다
(가져온 시점 커밋: `170ed8b`). 레퍼런스 `.raw`는 실제 EqualizerAPO-XT
`FilterEngine`(C++)이 결정적 입력 신호를 처리해 출력한 인터리브드
little-endian float32 스테레오 버퍼로, XT 저장소에서는 세대 간 DSP 회귀를
잡는 데 사용된다.

`tests/test_eqapo_reference.py`가 이 코퍼스로 `core/eqapo.py`의 합성 크기
응답이 실제 EqualizerAPO 엔진의 응답과 일치하는지 교차 검증한다. Impulcifer가
바이패스하는 명령(`Copy`, `Delay`, `LoudnessCorrection`)의 케이스는 가져오지
않았다.

| 케이스 | 입력 (48000 Hz, 2ch) | 프레임 수 |
| --- | --- | --- |
| `preamp_minus6` | DC 1.0 | 4800 |
| `biquad_peaking_1khz` | 임펄스 (양 채널 t=0) | 8192 |
| `graphiceq_15band` | 임펄스 | 8192 |
| `iir_order2_lowpass` | 임펄스 | 256 |
| `convolution_short` | 임펄스 (`ir_short.wav` 컨볼브) | 4096 |
| `channel_left_only` | DC 1.0 | 256 |

`ir_short.wav`는 mono/48 kHz/float32, 64탭이며 `h[0]=1.0, h[20]=0.5,
h[40]=0.25`만 non-zero다.

재생성하려면 XT 저장소에서:

```powershell
AudioRegressionTests.exe --generate-references `
  --config-dir Tests\AudioRegressionTests\configs `
  --ref-dir Tests\AudioRegressionTests\references
```
