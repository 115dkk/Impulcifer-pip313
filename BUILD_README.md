# Impulcifer Nuitka 크로스 플랫폼 빌드 가이드

## 개요
이 가이드는 Impulcifer를 Nuitka를 사용하여 독립 실행 가능한 프로그램으로 빌드하는 방법을 설명합니다.

**지원 플랫폼:**
- ✅ **Windows** (독립 실행 파일 + Inno Setup 인스톨러)
- ✅ **macOS** (앱 번들 + DMG 인스톨러)
- ✅ **Linux** (독립 실행 파일 + AppImage)

## 사전 요구사항

### 1. Python 설치
- Python 3.9 ~ 3.13 버전 필요
- [Python 공식 사이트](https://www.python.org/downloads/)에서 다운로드

### 2. Visual Studio 또는 Build Tools
- Nuitka는 C++ 컴파일러가 필요합니다
- 다음 중 하나를 설치:
  - [Visual Studio Community](https://visualstudio.microsoft.com/downloads/)
  - [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022)
- 설치 시 "Desktop development with C++" 워크로드 선택

### 3. Git (선택사항)
- 소스 코드 다운로드용
- [Git 다운로드](https://git-scm.com/download/win)

## 빌드 방법

### 자동 빌드 (권장)
```batch
# 1. 명령 프롬프트를 관리자 권한으로 실행

# 2. 프로젝트 폴더로 이동
cd E:\Impulcifer

# 3. 빌드 실행
build.bat
```

### 수동 빌드
```batch
# 1. 필수 패키지 설치
python install_requirements.py

# 2. Nuitka 빌드 실행
python build_nuitka.py

# 3. 빌드 옵션 선택
# - 옵션 1: 단일 실행 파일 (배포 편리, 실행 느림)
# - 옵션 2: 폴더 모드 (실행 빠름, 여러 파일)
```

## 빌드 후 확인사항

### 빌드 결과물
- `Impulcifer_Distribution/` 폴더 생성
- 실행 파일: `ImpulciferGUI.exe`
- 설명서: `README.txt`

### 실행 테스트
1. `ImpulciferGUI.exe` 더블클릭
2. Windows Defender 경고가 나오면 "추가 정보" → "실행" 클릭
3. GUI가 정상적으로 표시되는지 확인

## 문제 해결

### 빌드 실패 시
1. **Visual C++ 컴파일러 오류**
   - Visual Studio Build Tools 재설치
   - 시스템 재시작

2. **패키지 설치 오류**
   ```batch
   # pip 업그레이드
   python -m pip install --upgrade pip
   
   # 개별 패키지 수동 설치
   pip install nuitka
   ```

3. **메모리 부족**
   - 다른 프로그램 종료
   - 폴더 모드로 빌드 시도

### 실행 오류 시
1. **DLL 오류**
   - Visual C++ Redistributable 설치
   - https://aka.ms/vs/17/release/vc_redist.x64.exe

2. **리소스 파일 오류**
   - data, font, img 폴더가 포함되었는지 확인
   - `build_nuitka.py`의 include 옵션 확인

3. **권한 오류**
   - 관리자 권한으로 실행
   - 바이러스 백신 예외 설정

## 빌드 최적화

### 빌드 시간 단축
```python
# build_nuitka.py 수정
"--jobs=4",  # CPU 코어 수에 맞게 조정
"--lto=no",  # Link Time Optimization 비활성화
```

### 파일 크기 축소
```python
# 불필요한 모듈 제외
"--nofollow-import-to=test*",
"--nofollow-import-to=*test",
```

### 실행 속도 향상
- 폴더 모드 사용 (옵션 2)
- Windows Defender 예외 추가

## 배포 준비

### 서명 (선택사항)
- 코드 서명 인증서로 서명하면 보안 경고 감소
- signtool.exe 사용

### 인스톨러 생성 (선택사항)
- [NSIS](https://nsis.sourceforge.io/)
- [Inno Setup](https://jrsoftware.org/isinfo.php)

### 배포 체크리스트
- [ ] 모든 기능 테스트
- [ ] 다른 PC에서 실행 테스트
- [ ] 바이러스 검사
- [ ] README 업데이트
- [ ] 버전 정보 확인

## 크로스 플랫폼 빌드 (macOS/Linux)

### macOS 빌드

#### 사전 요구사항
- macOS 11+ (Big Sur 이상)
- Python 3.9 이상
- Xcode Command Line Tools

```bash
# Xcode Command Line Tools 설치
xcode-select --install
```

#### 빌드 방법
```bash
# 1. 프로젝트 폴더로 이동
cd /path/to/Impulcifer-pip313

# 2. 필수 패키지 설치
pip install nuitka ordered-set
pip install -r requirements.txt

# 3. Nuitka 빌드 실행 (자동으로 macOS 감지)
python build_nuitka.py

# 4. 빌드 결과 확인
# dist/macos/Impulcifer.app 생성됨
```

#### DMG 인스톨러 생성 (선택사항)
```bash
# create-dmg 도구 설치
brew install create-dmg

# DMG 생성
mkdir -p dist/dmg
cp -R dist/macos/Impulcifer.app dist/dmg/
create-dmg \
  --volname "Impulcifer" \
  --window-pos 200 120 \
  --window-size 800 400 \
  --icon-size 100 \
  --app-drop-link 600 185 \
  "dist/Impulcifer-macOS.dmg" \
  "dist/dmg/"
```

### Linux 빌드

#### 사전 요구사항
- Ubuntu 20.04+ / Debian 11+ / Fedora 35+ 또는 호환 배포판
- Python 3.9 이상
- 시스템 라이브러리

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y \
  python3-dev \
  patchelf \
  portaudio19-dev \
  python3-tk \
  libasound2-dev \
  libportaudio2

# Fedora/RHEL
sudo dnf install -y \
  python3-devel \
  patchelf \
  portaudio-devel \
  python3-tkinter \
  alsa-lib-devel
```

#### 빌드 방법
```bash
# 1. 프로젝트 폴더로 이동
cd /path/to/Impulcifer-pip313

# 2. 필수 패키지 설치
pip install nuitka ordered-set patchelf
pip install -r requirements.txt

# 3. Nuitka 빌드 실행 (자동으로 Linux 감지)
python build_nuitka.py

# 4. 빌드 결과 확인
# dist/linux/Impulcifer 생성됨
```

#### AppImage 생성 (선택사항)
```bash
# linuxdeploy 다운로드
wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage

# AppDir 구조 생성
mkdir -p AppDir/usr/bin
cp dist/linux/Impulcifer AppDir/usr/bin/impulcifer

# .desktop 파일 생성
mkdir -p AppDir/usr/share/applications
cat > AppDir/usr/share/applications/impulcifer.desktop <<EOF
[Desktop Entry]
Type=Application
Name=Impulcifer
Exec=impulcifer
Icon=impulcifer
Categories=AudioVideo;Audio;
Terminal=false
EOF

# AppImage 생성
./linuxdeploy-x86_64.AppImage \
  --appdir AppDir \
  --output appimage \
  --desktop-file=AppDir/usr/share/applications/impulcifer.desktop
```

## CI/CD 자동 빌드

프로젝트는 GitHub Actions를 통한 자동 크로스 플랫폼 빌드를 지원합니다.

### 워크플로우 파일
- `.github/workflows/build-macos.yml` - macOS 빌드
- `.github/workflows/build-linux.yml` - Linux 빌드
- `.github/workflows/nuitka-distribution.yml` - Windows 빌드
- `.github/workflows/release-cross-platform.yml` - 통합 릴리스

### 자동 빌드 트리거
- `master` 브랜치에 푸시
- `claude/*` 브랜치에 푸시
- 수동 실행 (workflow_dispatch)

### 릴리스 아티팩트
각 플랫폼별 빌드가 완료되면 다음 파일이 생성됩니다:

**Windows:**
- `Impulcifer_Setup.exe` - Windows 인스톨러

**macOS:**
- `Impulcifer-{version}-macOS.dmg` - macOS DMG 이미지

**Linux:**
- `Impulcifer-{version}-x86_64.AppImage` - AppImage
- `Impulcifer-{version}-linux-x86_64.tar.gz` - Tarball 아카이브

## 추가 정보

### 관련 링크
- [Nuitka 공식 문서](https://nuitka.net/doc/user-manual.html)
- [Impulcifer 원본](https://github.com/jaakkopasanen/impulcifer)
- [Python 3.13 호환 버전](https://github.com/115dkk/Impulcifer-pip313)

### 플랫폼별 주의사항

#### Windows
- Visual Studio Build Tools 필요
- Inno Setup 설치 시 자동으로 인스톤러 생성

#### macOS
- Apple Silicon (M1/M2)에서 빌드 시 Intel 버전도 지원
- 코드 서명 없이 배포 시 보안 경고 발생 가능

#### Linux
- 다양한 배포판 지원을 위해 AppImage 권장
- PortAudio 라이브러리 필수 설치

### 지원
문제가 발생하면 GitHub Issues에 문의하세요. 