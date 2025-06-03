# Impulcifer TrueHD/MLP 지원 및 자동 채널 생성 가이드

## 새로운 기능

1. **TrueHD/MLP 파일 지원**
   - 재생 및 테스트 신호로 사용 가능
   - 자동 채널 레이아웃 감지

2. **선택적 자동 채널 생성**
   - FC (센터): FL + FR의 평균
   - TSL (탑 사이드 좌): TFL (60%) + SL (40%)
   - TSR (탑 사이드 우): TFR (60%) + SR (40%)

3. **TrueHD 레이아웃 출력**
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

3. **자동 채널 생성 옵션** (선택적)
   - ✅ "Generate FC (Center) from FL+FR": 센터 채널이 없을 때 자동 생성
   - ✅ "Generate TSL from TFL+SL": 좌측 상단 측면 채널 자동 생성
   - ✅ "Generate TSR from TFR+SR": 우측 상단 측면 채널 자동 생성

4. GENERATE 버튼 클릭

### 3. 옵션 선택 가이드

#### 언제 자동 생성을 사용할까?

**자동 생성 권장 상황:**
- 센터 스피커가 없거나 접근이 어려운 경우 → FC 생성
- 천장 측면 스피커(TSL/TSR)가 없는 경우 → TSL/TSR 생성
- 빠른 프로토타이핑이 필요한 경우

**자동 생성 비권장 상황:**
- 정확한 측정이 중요한 전문 프로덕션
- 모든 스피커가 설치되어 있는 경우
- 음향 연구나 정밀 측정이 필요한 경우

## 명령줄 사용법

### 기본 사용
```bash
# TrueHD 재생 및 녹음
python recorder.py --play test.mlp --record data/my_hrir/FL,FR,SL,SR.wav

# Impulcifer 처리 (자동 생성 없이)
python impulcifer.py --dir_path data/my_hrir --test_signal test.mlp
```

### TrueHD 레이아웃 생성
```bash
# 자동 생성 없이 레이아웃만 생성
python impulcifer.py --dir_path data/my_hrir --test_signal test.mlp --truehd-layouts

# FC만 자동 생성
python impulcifer.py --dir_path data/my_hrir --test_signal test.mlp \
    --truehd-layouts --auto-generate-fc

# 모든 가능한 채널 자동 생성
python impulcifer.py --dir_path data/my_hrir --test_signal test.mlp \
    --truehd-layouts --auto-generate-all

# 선택적 자동 생성
python impulcifer.py --dir_path data/my_hrir --test_signal test.mlp \
    --truehd-layouts --auto-generate-fc --auto-generate-tsl
```

## 출력 파일

### 기본 출력
- `hrir.wav`: 표준 HRIR 파일
- `hesuvi.wav`: HeSuVi 호환 형식

### TrueHD 레이아웃 출력 (옵션)
- `11cmaster.wav`: 7.0.4 레이아웃
- `13cmaster.wav`: 7.0.6 레이아웃

### 추가 정보 파일
- `README.md`: 측정 정보, 통계, 자동 생성된 채널 목록
- `*_channels.txt`: TrueHD 소스의 채널 정보

## 측정 예시

### 예시 1: 기본 7.1.4 설정
```
실제 스피커: FL, FR, SL, SR, BL, BR, TFL, TFR, TBL, TBR
목표: 13채널 TrueHD 레이아웃

1. 10개 스피커 측정
2. GUI에서:
   - TrueHD layouts ✅
   - Generate FC ✅
   - Generate TSL ✅
   - Generate TSR ✅
3. 결과: 완전한 13채널 레이아웃
```

### 예시 2: 최소 설정
```
실제 스피커: FL, FR, SL, SR, BL, BR
목표: 11채널 TrueHD 레이아웃

1. 6개 스피커 측정
2. GUI에서:
   - TrueHD layouts ✅
   - Generate FC ✅
   - (천장 스피커는 측정 필요)
3. 결과: FC만 자동 생성된 부분적 레이아웃
```

## 주의사항

1. **자동 생성의 한계**
   - 자동 생성된 채널은 실제 측정보다 정확도가 떨어집니다
   - 가능하면 실제 스피커를 측정하는 것이 좋습니다

2. **채널 수 제한**
   - 대부분의 오디오 인터페이스는 8채널로 제한됩니다
   - 11채널 이상은 전문 장비가 필요할 수 있습니다

3. **FFmpeg 요구사항**
   - TrueHD 지원을 위해 FFmpeg가 설치되어 있어야 합니다
   - 시스템 PATH에 ffmpeg와 ffprobe가 있어야 합니다

## 문제 해결

### "Cannot generate channel" 오류
- 필요한 소스 채널이 측정되지 않았습니다
- 예: TSL 생성에는 TFL과 SL이 모두 필요합니다

### FFmpeg 관련 오류
```python
# utils.py에서 FFmpeg 경로 직접 지정
FFMPEG_PATH = r"C:\ffmpeg\bin\ffmpeg.exe"  # Windows
FFPROBE_PATH = r"C:\ffmpeg\bin\ffprobe.exe"
```

### 채널 매핑 확인
생성된 `*_channels.txt` 파일을 확인하여 올바른 채널 순서 확인