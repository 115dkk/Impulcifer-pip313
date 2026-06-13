# 접근성 — WCAG 2.2 명암비 / 타깃 크기

명암비는 모두 `gui/theme/__init__.py`의 디자인 토큰을 sRGB 상대휘도(WCAG 공식)로 환산해 계산했다.
2× HiDPI 캡처이므로 픽셀 측정값은 ÷2하여 CSS px로 환산했다. 본문 텍스트 AA 기준은 4.5:1,
대형 텍스트(≥18.66px bold 또는 ≥24px)·UI 컴포넌트 경계는 3:1이다.

---

## [HIGH] A11Y-1 — 모든 드롭다운의 흰 텍스트가 accent 채움 위에서 AA 미달  ✅ 수정됨

- **스킨/탭**: 양 스킨 · 양 테마 · 녹음/설정 등 `CTkOptionMenu`가 쓰이는 모든 탭
- **기준**: WCAG 2.2 SC 1.4.3 Contrast (Minimum)
- **증거**: `stable_recorder_dark/light`, `studio_recorder_dark`, `studio_settings_dark`, `studio_settings_light`
  — Host API('Windows DirectSound'), 재생/녹음 장치('주 사운드 드라이버' 등), 채널 강제 지정('2 (Stereo)'),
  언어('한국어'), 테마('다크')가 모두 흰색 `#f3f5f7` 텍스트 + 솔리드 accent `#3B82F6` 채움.
- **계산**: 흰색 L≈0.911, accent `#3B82F6` L≈0.236 → **3.37:1** (4.5:1 미달). accent 토큰이 dark/light 동일이라
  라이트·다크 모두 동일하게 실패. 글리프 높이 ~30px/2× = ~15px CSS = 본문 텍스트라 4.5:1 적용.
  (채움 자체는 배경 대비 3:1 이상이라 SC 1.4.11은 통과 — 오직 **라벨 텍스트**가 1.4.3 미달.)
- **영향**: 드롭다운 선택값은 GUI에서 가장 반복적으로 읽히는 문자열이자 장치 라우팅을 확인하는 핵심 정보다.
- **수정**: `CTkOptionMenu` 채움을 accent-strong `#2563EB`(L≈0.155)로 darken → 흰색 대비 **4.69:1**로 AA 통과.
  chevron 버튼 영역은 `#1d4ed8`/`#1e40af`로 한 단계 더 어둡게 해 분리 어포던스 유지. `gui/theme/pulse.json`.
- **검증**: 수정 후 픽셀 샘플 `(59,130,246)#3B82F6 → (37,99,235)#2563EB` 확인.

---

## [HIGH] A11Y-2 — 정보 탭 VERSION/PYTHON 문자열이 accent로 AA 미달  ✅ 수정됨

- **스킨/탭**: 양 스킨 · 정보(정보) 탭 hero
- **기준**: WCAG 2.2 SC 1.4.3
- **증거**: `studio_info_light`, `studio_info_dark`, `stable_info_dark` — 'VERSION 2.7.8 · PYTHON 3.13.3 · 개발 환경'
  모노 11px bold가 accent `#3B82F6`.
- **계산**: 라이트 카드(bg-2 `#dde0e3` L≈0.742) 위 accent → **2.78:1** (명백 실패). 다크 카드(bg-2 `#23272a` L≈0.019)
  위 accent → **4.15:1** (역시 4.5:1 미달). 11px = 본문 텍스트라 4.5:1 적용. 버그 리포트 시 사용자가 읽어야 하는 정보 텍스트.
- **수정**: `accent` → `fg-1`(`#3a3d42` 라이트 / `#b5b8ba` 다크). 라이트 ~9:1, 다크 ~7.5:1로 여유 있게 통과.
  `gui/tabs/info_tab.py`, `gui/skins/studio_info_tab.py`.
- **검증**: 수정 후 `studio_info_light`에서 버전 문자열이 진회색으로 또렷하게 렌더됨을 육안 확인.

---

## [MEDIUM] A11Y-3 — Studio 활성 사이드바 라벨 명암비 미달  ✅ 수정됨

- **스킨/탭**: Studio · 전 탭 사이드바
- **기준**: WCAG 2.2 SC 1.4.3
- **증거**: `studio_settings_light/dark`, `studio_info_light/dark` — 활성 항목('설정'/'정보')이 accent 텍스트+아이콘 +
  accent-soft 알약. 역설적으로 **비활성** 라벨(fg-0/fg-1 잉크)이 더 잘 읽힌다.
- **계산**: accent `#3B82F6` on accent-soft 라이트 `#dbeafe` → **3.01:1**, 다크 `#1e3a5f` → **3.17:1**. 13px bold는
  대형 텍스트 기준(18.66px bold)에 못 미쳐 4.5:1 적용 → 양 테마 실패.
- **수정**: 활성 텍스트 `accent` → `fg-0`(고대비 잉크). accent-soft 알약 채움 + bold가 선택 상태를 계속 표시하므로
  파란 글자를 잃어도 "선택됨" 신호는 유지. 라이트 ~15:1, 다크 ~10:1로 통과. `gui/skins/studio_shell.py`.
- **검증**: 수정 후 `studio_settings_dark`에서 '설정'이 고대비 텍스트로 또렷함을 육안 확인.

---

## [LOW] 섹션 배지(UI/dir/env/link) 텍스트가 accent-soft 위에서 작은 텍스트 기준 미달

- **스킨/탭**: Studio · 설정/정보. **기준**: SC 1.4.3.
- **증거**: `studio_settings_light/dark`, `studio_info_light/dark` — 'UI','dir','env','link' 모노 약어 배지.
- **계산**: accent on accent-soft 라이트 ~3.04:1 / 다크 ~3.10:1. 약 8px CSS의 작은 텍스트라 4.5:1 미달.
- **등급 근거(downgrade→low)**: 배지는 바로 옆 고대비 한글 섹션 제목(화면 설정/기본 경로/환경 등)이 실제 의미를
  전달하는 **장식적 카테고리 마커**이며, 두 비율 모두 비텍스트/대형 컴포넌트 3:1 바닥은 넘는다. 정보 단독 전달자가 아님.
- **권고**: 유지해도 무방. 굳이 올린다면 배지 텍스트를 더 진한 잉크/굵기로, 또는 글리프 아이콘으로 대체.

---

## [LOW] Studio 처리옵션 토글 스위치 높이가 24 CSS px 경계선

- **스킨/탭**: Studio · 처리(처리 옵션 카드). **기준**: SC 2.5.8 Target Size (Minimum).
- **증거**: `studio_impulcifer_dark`, `studio_impulcifer_dark_bottom` — `CTkSwitch` 트랙 높이 ~20–22 CSS px로 24 미만.
- **등급 근거**: 각 스위치가 큰 클릭 가능 카드 행(~80 CSS px) 안에 있고, 행마다 1개씩 넉넉한 간격이라 SC 2.5.8의
  **Spacing 예외**로도 통과. 거짓 실패는 아니나 폴리시 노트.
- **권고**: 카드 전체 클릭으로 토글되는 현재 동작 유지. 선택적으로 `CTkSwitch` 높이를 ≥24 CSS px로 상향.

---

## 참고 — 빨강 녹음 버튼 명암비는 거짓 양성 (소스 대조로 정정)

한 검토자가 Studio '녹음 시작' 빨강 버튼을 `#fb594d`로 가정해 2.9:1(미달)로 보고했으나, GUI 소스
(`gui/skins/studio_recorder_tab.py`의 `cta_color="#dc2626"`)는 더 진한 `#dc2626`을 쓴다. 흰 텍스트 대비
**약 4.8:1로 AA 통과**다. 상세는 [02-consistency-ux.md](02-consistency-ux.md#빨강-녹음-버튼-명암비--거짓-양성) 참조.
