# Impulcifer TrueHD/MLP 지원 가이드

## 새로운 기능

1. **TrueHD/MLP 파일 지원**
   - 재생 및 테스트 신호로 사용 가능
   - 자동 채널 레이아웃 감지

2. **TrueHD 레이아웃 출력**
   - 11채널 (7.0.4): FL-FR-FC-BL-BR-SL-SR-TFL-TFR-TBL-TBR
   - 13채널 (7.0.6): FL-FR-FC-BL-BR-SL-SR-TFL-TFR-TSL-TSR-TBL-TBR

## GUI 사용법

### 1. 기본 워크플로우

1. **Recorder 창**
   - "File to play": TrueHD 파일 선택 (.mlp, .thd, .truehd)
   - "Record to file": 출력 파일 경로 (예: `data/my_hrir/FL,FR,SL,SR,BL,BR,TFL,TFR,TBL,TBR.wav`)
   - RECORD 버튼 클릭

2. **Impulcifer 창**
   - "Your recordings": 녹음 폴더 선택
   - "Test signal used": 동일한 TrueHD 파일 선택
   - 원하는 처리 옵션 선택

### 2. TrueHD 레이아웃 생성 (Advanced Options)

1. "Advanced options" 체크박스 활성화

2. "TrueHD layouts (11ch/13ch)" 체크박스 활성화
   - 이 옵션을 켜면 11채널과 13채널 레이아웃 파일이 생성됩니다

3. GENERATE 버튼 클릭

## 명령줄 사용법

### 기본 사용
```bash
# TrueHD 재생 및 녹음
python recorder.py --play test.mlp --record data/my_hrir/FL,FR,SL,SR.wav

# Impulcifer 처리
python impulcifer.py --dir_path data/my_hrir --test_signal test.mlp
```

### TrueHD 레이아웃 생성
```bash
# 레이아웃 생성
python impulcifer.py --dir_path data/my_hrir --test_signal test.mlp --truehd-layouts
```

## 출력 파일

### 기본 출력
- `hrir.wav`: 표준 HRIR 파일
- `hesuvi.wav`: HeSuVi 호환 형식

### TrueHD 레이아웃 출력 (옵션)
- `truehd_11ch_Xch.wav`: 7.0.4 레이아웃 (X는 사용 가능한 채널 수)
- `truehd_13ch_Xch.wav`: 7.0.6 레이아웃 (X는 사용 가능한 채널 수)

### 추가 정보 파일
- `README.md`: 측정 정보, 통계
- `*_channels.txt`: TrueHD 소스의 채널 정보

## 측정 예시

### 예시 1: 기본 7.1.4 설정
```
실제 스피커: FL, FR, FC, SL, SR, BL, BR, TFL, TFR, TBL, TBR
목표: 13채널 TrueHD 레이아웃

1. 11개 스피커 측정 (FC 포함)
2. GUI에서:
   - TrueHD layouts ✅
3. 결과: 사용 가능한 스피커를 기반으로 한 11채널 또는 13채널 레이아웃
```

### 예시 2: 최소 설정
```
실제 스피커: FL, FR, SL, SR, BL, BR
목표: 11채널 TrueHD 레이아웃

1. 6개 스피커 측정
2. GUI에서:
   - TrueHD layouts ✅
3. 결과: 사용 가능한 스피커를 기반으로 한 부분적 레이아웃 (FC, TFL, TFR, TBL, TBR 등 천장 채널 누락 시 해당 채널 제외)
```

## 주의사항

1. **채널 수 제한**
   - 대부분의 오디오 인터페이스는 8채널로 제한됩니다
   - 11채널 이상은 전문 장비가 필요할 수 있습니다

2. **FFmpeg 요구사항**
   - TrueHD 지원을 위해 FFmpeg가 설치되어 있어야 합니다
   - 시스템 PATH에 ffmpeg와 ffprobe가 있어야 합니다

## 문제 해결

### FFmpeg 관련 오류
```python
# utils.py에서 FFmpeg 경로 직접 지정
FFMPEG_PATH = r"C:\ffmpeg\bin\ffmpeg.exe"  # Windows
FFPROBE_PATH = r"C:\ffmpeg\bin\ffprobe.exe"
```

### 채널 매핑 확인
생성된 `*_channels.txt` 파일을 확인하여 올바른 채널 순서 확인