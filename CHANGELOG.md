# Changelog
Here you'll find the history of changes. The version numbering complies with [SemVer]() system which means that when the
first number changes, something has broken and you need to check your commands and/or files, when the second number
changes there are only new features available and nothing old has broken and when the last number changes, old bugs have
been fixed and old features improved.

## 1.4.0 - 2024-12-20
### GUI에 추가된 기능들
- **임펄스 응답 사전 응답(Pre-response) 길이 조절 옵션**: 임펄스 응답의 시작 부분을 자르는 길이를 ms 단위로 조절할 수 있습니다. (기본값: 1.0ms)
- **JamesDSP용 트루 스테레오 IR(.wav) 생성 기능**: FL/FR 채널만 포함하는 jamesdsp.wav 파일을 생성합니다.
- **Hangloose Convolver용 개별 채널 스테레오 IR(.wav) 생성 기능**: 각 스피커 채널별로 별도의 스테레오 IR 파일을 생성합니다.
- **인터랙티브 플롯 HTML 파일 생성 기능**: Bokeh 기반의 대화형 플롯을 HTML 파일로 생성합니다.
- **마이크 착용 편차 보정(Microphone Deviation Correction) 기능**: 좌우 마이크 위치 차이로 인한 편차를 보정합니다. (강도: 0.0-1.0)

### 개선사항
- GUI의 고급 옵션(Advanced options) 섹션에 모든 새로운 기능들이 추가되었습니다.
- 각 기능에 대한 툴팁이 추가되어 사용자가 쉽게 이해할 수 있도록 했습니다.

## 1.0.0 - 2020-07-20
Performance improvements. Main features are supported and Impulcifer is relatively stable.
