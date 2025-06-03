if failed_packages:
    print(f"\n설치 실패한 패키지: {', '.join(failed_packages)}")
    print("문제 해결 방법:")
    print("1. 관리자 권한으로 실행해보세요")
    print("2. 가상 환경을 사용해보세요")
    print("3. 수동으로 설치해보세요: pip install <package_name>")
    sys.exit(1)  # 오류 코드 반환
else:
    print("\n✓ 모든 패키지가 성공적으로 설치되었습니다!")
    print("\n이제 build_nuitka.py를 실행하여 빌드할 수 있습니다.")