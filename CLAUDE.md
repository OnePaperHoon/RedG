# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 프로젝트 개요

**AYG (AI Youtube Generator)** — 텍스트 주제 입력부터 YouTube 업로드까지 자동화하는 CLI 도구.

```
주제 입력 → 스크립트 생성(Claude) → [검토] → 이미지 설정 → [검토]
→ 이미지 생성 → TTS → FFmpeg 영상 합성 → YouTube 업로드
```

전체 명세: `AYG_SPEC.md` (단일 소스 — 구현 전 반드시 읽을 것)

---

## 기술 스택

| 레이어 | 기술 |
|--------|------|
| CLI/TUI | Rust + Ratatui 0.29 + Tokio (`tui/`) |
| 파이프라인 | TypeScript + Node.js LTS (`pipeline/`) |
| IPC | NDJSON over stdin/stdout (Rust ↔ Node.js) |
| 스크립트 생성 | `@anthropic-ai/sdk` (Claude API) |
| 이미지 생성 | nanobanana API (Phase 1) / ComfyUI (Phase 2) |
| TTS | OpenAI `tts-1` |
| 영상 합성 | `fluent-ffmpeg` |
| 업로드 | `googleapis` (YouTube Data API v3) |

---

## 디렉토리 구조

```
ayg/
├── tui/                    # Rust 바이너리
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs         # CLI 진입점 (init/run 분기, clap)
│       ├── app.rs          # AppState 중앙 상태 + AppMode enum
│       ├── ipc.rs          # Node.js spawn + NDJSON 비동기 통신
│       ├── config.rs       # config.yaml 읽기/쓰기
│       └── ui/
│           ├── onboarding.rs     # 5단계 API 키 입력 TUI
│           ├── dashboard.rs      # 좌(30%) Job 목록 + 우(70%) Preview/Log
│           ├── script_review.rs  # 좌 씬 목록 + 우 인라인 에디터
│           ├── image_config.rs   # 씬별 Prompt/Style/Negative + GlobalStyle 팝업
│           ├── new_job_form.rs   # 주제 입력 팝업
│           └── log_panel.rs      # 로그 스트리밍
│
├── pipeline/               # TypeScript 파이프라인
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── index.ts        # stdin 루프 → Command dispatch
│       ├── ipc.ts          # NDJSON send/recv
│       ├── types.ts        # IPC 타입 정의 (가장 먼저 작성)
│       ├── config.ts       # config.yaml 파싱
│       ├── runner.ts       # 체크포인트 기반 파이프라인 + 사용자 개입 대기
│       └── steps/
│           ├── scriptGenerator.ts
│           ├── imageGenerator.ts
│           ├── ttsGenerator.ts
│           ├── videoComposer.ts
│           └── uploader.ts
│
├── config.yaml             # gitignore 필수! (ayg init으로 생성)
├── client_secrets.json     # YouTube OAuth (gitignore 필수!)
├── workspace/{jobId}/      # 중간 작업 파일
│   ├── checkpoint.json
│   ├── script.json
│   ├── images/
│   ├── audio/
│   └── output.mp4
└── output/                 # 최종 영상
```

---

## 빌드 & 실행 명령

```bash
# Rust TUI 빌드
cd tui && cargo build --release

# TypeScript 파이프라인 빌드
cd pipeline && npm install && npx tsc

# 초기 설정 (TUI 온보딩)
./tui/target/release/ayg init

# 실행
./tui/target/release/ayg run

# tsx로 직접 실행 (개발 시)
cd pipeline && npx tsx src/index.ts
```

---

## 아키텍처 핵심

### IPC 프로토콜 (NDJSON)
- **Rust → Node.js**: Commands (`start_job`, `script_approved`, `image_config_approved` 등)
- **Node.js → Rust**: Events (`script_ready`, `image_config_ready`, `step_start/update/done/error`, `job_done`)
- 한 줄 = 한 JSON 메시지, `\n` 구분, UTF-8

### 사용자 개입 포인트 (★)
파이프라인은 두 지점에서 TUI 승인을 기다린다:
1. **ScriptReview**: `script_ready` 이벤트 발송 → `script_approved` 커맨드 수신 대기
2. **ImageConfig**: `image_config_ready` 이벤트 발송 → `image_config_approved` 커맨드 수신 대기

### 체크포인트 (`workspace/{jobId}/checkpoint.json`)
모든 Step 실행 전 `done: true` 확인 → 이미 완료된 단계 스킵. 프로세스 재시작 시 중단 지점부터 재개.

### TUI AppMode 흐름
```
Onboarding → (init 완료) → Dashboard
Dashboard → [n] NewJob → (start_job) → ScriptReview ★ → ImageConfig ★ → Dashboard
```

### AOE 컬러 테마 (Ratatui)
- 배경: `#0a0a0a`
- 기본 텍스트: `Color::Rgb(0, 204, 102)` (초록)
- 강조/진행 중: `Color::Rgb(255, 140, 0)` (주황)
- 완료: `Color::Green` | 오류: `Color::Red` | 비활성: `Color::DarkGray`
- 테두리: `BorderType::Plain`

---

## 구현 주의사항

- `config.yaml`, `client_secrets.json`, `workspace/`, `output/` → `.gitignore` 필수
- Node.js 파이프라인은 `pipeline/dist/index.js` (빌드 결과물)를 실행
- `pipeline/src/types.ts`를 **가장 먼저** 작성한 뒤 다른 모듈 구현
- Rust IPC: stdout 수신은 `tokio::spawn`으로 비동기, stdin 송신은 `write_all` + `\n`
