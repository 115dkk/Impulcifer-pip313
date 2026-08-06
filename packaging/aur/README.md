# AUR 패키지 (`impulcifer-py313-bin`)

릴리스 파이프라인이 GitHub Release의 Linux tarball을 소스로 하는 AUR
바이너리 패키지를 자동 발행/갱신한다.

## 동작 방식

- `.github/workflows/publish.yml`의 `publish-aur` job이 `create-release`
  성공 후 실행된다.
- job은 방금 발행된 `Impulcifer-<버전>-linux-x86_64.tar.gz`를 내려받아
  SHA-256을 계산하고, `PKGBUILD.in`의 `@PKGVER@` / `@SHA256@`를 치환한
  PKGBUILD를 [KSXGitHub/github-actions-deploy-aur] 액션으로
  `aur.archlinux.org/impulcifer-py313-bin`에 커밋한다 (.SRCINFO는 액션이
  Arch 컨테이너에서 자동 생성).
- 저장소 시크릿 `AUR_SSH_PRIVATE_KEY`가 없으면 job은 **조용히 스킵**되고
  파이프라인은 그린으로 유지된다. 즉 시크릿을 넣기 전까지는 아무 일도
  일어나지 않는다.

## 최초 1회 설정 (메인테이너)

1. [AUR 계정](https://aur.archlinux.org/register) 생성 (이미 있으면 생략).
2. AUR 전용 SSH 키 생성: `ssh-keygen -t ed25519 -f aur -C aur@impulcifer -N ""`
3. 공개키(`aur.pub`)를 AUR 계정 설정(My Account → SSH Public Key)에 등록.
4. 개인키(`aur`) 내용을 이 GitHub 저장소의 Actions 시크릿
   `AUR_SSH_PRIVATE_KEY`로 등록.
5. 다음 릴리스부터 자동 발행된다. 패키지 베이스는 첫 push 때 AUR이
   자동 생성하므로 웹에서 미리 만들 필요 없다.

## 수동 발행이 필요할 때

`PKGBUILD.in`의 자리표시자를 치환한 뒤 통상적인 AUR 절차를 따른다:

```bash
sed -e 's/@PKGVER@/2.10.6/' \
    -e "s/@SHA256@/$(sha256sum Impulcifer-2.10.6-linux-x86_64.tar.gz | cut -d' ' -f1)/" \
    PKGBUILD.in > PKGBUILD
makepkg --printsrcinfo > .SRCINFO
git -C <aur-clone> add PKGBUILD .SRCINFO && git commit && git push
```

[KSXGitHub/github-actions-deploy-aur]: https://github.com/KSXGitHub/github-actions-deploy-aur
