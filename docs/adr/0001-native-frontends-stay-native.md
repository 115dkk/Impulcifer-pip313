# ADR 0001: 네이티브 프론트엔드는 네이티브답게 — CTk의 서비스 계층 단일화 기각

- **상태:** 결정됨 (2026-08-02, 이슈 #138)
- **결정자:** 유지보수자 (115dkk)

## 맥락

2026-08-01 격주 아키텍처 감사(이슈 #138)의 C2 후보는 "BRIR 요청 조립이 4곳에
중복되어 있으니 CTk GUI도 WebView처럼 `application/impulcifer_service.py`를
경유시켜 어댑터를 단일화하자"는 제안이었다. 근거는 중복 제거와 seam 증명(어댑터
2개)이었다.

## 결정

**기각한다.** CTk(네이티브 Tk) 프론트엔드는 지금처럼 `impulcifer.main(**kwargs)`을
직접 호출하고, Tk 관용구(Tk 변수, 다이얼로그 스레딩, 파일 복사)를 유지한다.
JSON 서비스 계층(`ImpulciferApplicationService`)은 WebView 프론트엔드 전용
어댑터로 남는다. 네이티브 GUI를 웹뷰용 JSON 요청/폴링 문법에 끼워맞추는 것은
중복 몇 줄을 줄이는 대가로 네이티브의 관용구와 즉시성을 죽이는 교환이므로
받아들이지 않는다.

## 의도된 프론트엔드별 계약 (드리프트가 아님)

- **헤드폰 보상 파일 (감사 F029/Q1):** CTk는 선택 파일을 측정 폴더의
  `headphones.wav`로 **복사**하고(파일 기반 네이티브 관용구 — 측정 폴더가
  self-contained해진다), WebView 서비스는 `headphone_compensation_file` 경로를
  **그대로 전달**한다(JSON API 관용구). 둘 다 의도된 계약이며 어느 한쪽으로
  통일하지 않는다.

## 프론트엔드 간 드리프트를 고치는 방법

계약을 한 어댑터로 합치는 방식이 아니라, **각 프론트엔드 안에서** 정본을
기준으로 고친다.

- 파라미터 **기본값**의 정본은 `core.pipeline.ProcessingConfig`다. CTk는
  `gui/brir_args.processing_default()`, WebView는 `bootstrap()`의
  `brir_defaults`로 같은 정본을 읽는다 (감사 F019/F020에서 정리).
- **DSP 의미**(예: decay 단일값의 채널 fan-out 범위)의 정본은 CLI 동작이다
  (감사 F018/Q2: GUI의 7채널 fan-out은 드리프트로 판정, 15채널
  `SPEAKER_NAMES` 기준으로 수정).
- CTk **UX 의미**의 정본은 Stable 탭이다 (감사 F021/Q5: Studio 녹음 채널
  의미는 Stable에 맞춘다).

## 향후 감사/리팩토링 지침

"CTk를 서비스 경유로 단일화", "프론트엔드 요청 조립기 통합" 류의 제안을
다시 올리지 말 것. 이 문서가 그 제안의 기각 기록이다.
