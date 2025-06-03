@echo off
chcp 65001 > nul
echo === Impulcifer GUI 빌드 도구 ===
echo.

REM Python 설치 확인
python --version >nul 2>&1
if errorlevel 1 (
    echo [오류] Python이 설치되어 있지 않습니다.
    echo Python 3.9 이상을 설치해주세요.
    pause
    exit /b 1
)

echo [1/3] 필수 패키지 설치 중...
python install_requirements.py
if errorlevel 1 (
    echo [오류] 패키지 설치에 실패했습니다.
    pause
    exit /b 1
)

echo.
echo [2/3] Nuitka 빌드 시작...
python build_nuitka.py
if errorlevel 1 (
    echo [오류] 빌드에 실패했습니다.
    pause
    exit /b 1
)

echo.
echo [3/3] 빌드 완료!
echo.
echo 실행 파일 위치: Impulcifer_Distribution 폴더
echo.
pause