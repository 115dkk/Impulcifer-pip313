# AGENTS.md

이 저장소의 에이전트 지침 **정본은 [`CLAUDE.md`](./CLAUDE.md)** 다. 모든 코딩
에이전트(Claude Code, Codex, Copilot 등)는 작업 전에 `CLAUDE.md`를 읽고 그 규칙을
그대로 따른다.

The single source of truth for agent instructions in this repository is
[`CLAUDE.md`](./CLAUDE.md). Any coding agent should read and follow it before
making changes. This file is a pointer only — do not duplicate rules here.

특히 자주 놓치는 항목:

- **버전/릴리스**: 3.x(정식: `crates/`, `apps/`, `Cargo.toml`·`Cargo.lock` 등)는
  master 머지 때 `.github/workflows/release-3x.yml`의 `gate`가 누락된 PATCH를 자동
  bump하고 릴리스한다. 2.x(유지보수: `core/`, `autoeq/`, `impulcifer.py`, `gui/`,
  `i18n/`, `infra/`, `updater/`, 의존성, 번들 자산)는 머지만으로 배포되지 않고
  `.github/workflows/publish.yml`을 수동 실행할 때만 릴리스된다. docs/CI/tests만 바꾼
  변경은 어느 쪽도 릴리스를 유발하지 않는다. 자세한 내용은 `CLAUDE.md`의
  "2. 런타임 변경 시 버전 bump", "빌드 / 릴리스 파이프라인" 섹션 참조.
- **i18n / CHANGELOG / PR 검증(Tier 1~3) / BRIR 해시(SHA-256) 무결성**: 각각 `CLAUDE.md`의
  해당 섹션을 따른다.
