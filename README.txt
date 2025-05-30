# Impulcifer GUI 설치 완료!

HRIR 측정 및 헤드폰 바이노럴 헤드트래킹 HRTF 시스템 Impulcifer GUI가 성공적으로 설치되었습니다.

## 프로그램 실행 방법
- 시작 메뉴 또는 바탕화면 바로 가기를 통해 ImpulciferGUI를 실행하세요.
- (또는) 설치된 폴더 (기본값: C:\Program Files (x86)\Impulcifer) 안의 ImpulciferGUI.exe를 실행하세요.

## 사용 전 주의사항
- Windows Defender나 다른 백신 프로그램에서 처음 실행 시 경고가 나올 수 있습니다.
  이는 Nuitka로 컴파일된 프로그램에서 나타날 수 있는 현상이며, 안전한 프로그램입니다.
  필요한 경우 예외 처리를 해주세요.
- 프로그램 첫 실행 시 초기화 과정으로 인해 약간의 시간이 소요될 수 있습니다.
- 녹음 기능을 사용한다면, 원활한 사용을 위해 오디오 인터페이스와 마이크가 올바르게 연결 및 설정되어 있는지 확인해주세요.

## 문제 해결
프로그램이 실행되지 않는 경우:
1. 운영체제: Windows 10 또는 Windows 11 (64비트) 환경인지 확인해주세요.
2. Visual C++ Redistributable: 최신 Visual C++ Redistributable 패키지가 설치되어 있는지 확인하고, 없다면 다음 링크에서 설치해주세요:
   https://aka.ms/vs/17/release/vc_redist.x64.exe
3. 바이러스 백신 예외 처리: 사용하시는 바이러스 백신 프로그램에서 ImpulciferGUI.exe 또는 설치 폴더를 예외로 설정해주세요.
4. 관리자 권한 실행: 드물지만, 일부 시스템에서는 관리자 권한으로 프로그램을 실행해야 정상 작동하는 경우가 있습니다. (마우스 오른쪽 버튼 클릭 > 관리자 권한으로 실행)

## 추가 정보
- 원본 프로젝트: https://github.com/jaakkopasanen/impulcifer
- Python 3.13 호환 버전 (본 설치 프로그램): https://github.com/115dkk/Impulcifer-pip313

Impulcifer GUI를 사용해주셔서 감사합니다!