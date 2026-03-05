# AYG — AI Youtube Generator
## Claude Code 작업 명세서 v5.0

> **이 파일은 Claude Code가 AYG 프로젝트를 구현할 때 참조하는 단일 소스입니다.**
> 구현 전 반드시 전체를 읽고 시작하세요.

---

## 📌 프로젝트 한 줄 요약

주제(텍스트) 입력 → AI 스크립트 생성 → 사용자 검토/편집 → 이미지 설정 → 이미지 생성 → TTS → FFmpeg 영상 합성 → YouTube 자동 업로드

---

## 🗂 목차

1. [기술 스택](#1-기술-스택)
2. [디렉토리 구조](#2-디렉토리-구조)
3. [CLI 진입점 — ayg 바이너리](#3-cli-진입점--ayg-바이너리)
4. [ayg init — TUI 온보딩](#4-ayg-init--tui-온보딩)
5. [config.yaml 구조](#5-configyaml-구조)
6. [전체 파이프라인 플로우](#6-전체-파이프라인-플로우)
7. [TUI 화면 목록 (AppMode)](#7-tui-화면-목록-appmode)
8. [TUI 화면 상세 설계](#8-tui-화면-상세-설계)
9. [IPC 프로토콜 (NDJSON)](#9-ipc-프로토콜-ndjson)
10. [TypeScript 타입 정의](#10-typescript-타입-정의)
11. [Rust 핵심 구조](#11-rust-핵심-구조)
12. [TypeScript 파이프라인 핵심 모듈](#12-typescript-파이프라인-핵심-모듈)
13. [체크포인트 설계](#13-체크포인트-설계)
14. [개발 단계 (Phase)](#14-개발-단계-phase)
15. [빠른 시작](#15-빠른-시작)

---

## 1. 기술 스택

| 레이어 | 기술 | 역할 |
|--------|------|------|
| **CLI 진입점** | Rust (`ayg` 바이너리) | `init` / `run` 서브커맨드 분기 |
| **TUI** | Rust + Ratatui 0.29 + Tokio | 온보딩, 대시보드, 편집 화면 |
| **IPC** | NDJSON over stdin/stdout pipe | Rust ↔ Node.js 프로세스 통신 |
| **파이프라인** | TypeScript + Node.js LTS | 파이프라인 전체 실행 |
| **스크립트 생성** | `@anthropic-ai/sdk` | Claude API — 씬 분할 + 메타데이터 |
| **이미지 (Phase 1)** | nanobanana API (fetch) | 씬별 9:16 이미지 생성 |
| **이미지 (Phase 2)** | ComfyUI REST (로컬) | 로컬 Stable Diffusion |
| **TTS** | `openai` npm SDK (tts-1) | 스크립트 → MP3 |
| **영상 합성** | `fluent-ffmpeg` | 이미지 + 음성 + 자막 → MP4 |
| **업로드** | `googleapis` npm SDK | YouTube Data API v3 |
| **설정** | `./config.yaml` (로컬) | API 키 + 옵션 (`ayg init`으로 생성) |

### Cargo 의존성 (tui/Cargo.toml)

```toml
[dependencies]
ratatui       = "0.29"
crossterm     = "0.28"
tokio         = { version = "1", features = ["full"] }
serde         = { version = "1", features = ["derive"] }
serde_json    = "1"
serde_yaml    = "0.9"
tokio-util    = "0.7"
clap          = { version = "4", features = ["derive"] }
```

### npm 의존성 (pipeline/package.json)

```json
{
  "dependencies": {
    "@anthropic-ai/sdk": "latest",
    "openai": "latest",
    "googleapis": "latest",
    "fluent-ffmpeg": "latest",
    "js-yaml": "latest"
  },
  "devDependencies": {
    "typescript": "^5",
    "tsx": "latest",
    "@types/node": "latest",
    "@types/fluent-ffmpeg": "latest",
    "@types/js-yaml": "latest"
  }
}
```

---

## 2. 디렉토리 구조

```
ayg/
├── tui/                          # Rust 바이너리
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs               # CLI 진입점 (init / run 분기)
│       ├── app.rs                # AppState 중앙 상태
│       ├── ipc.rs                # Node.js spawn + NDJSON pipe
│       ├── config.rs             # config.yaml 읽기/쓰기
│       └── ui/
│           ├── mod.rs            # Layout::split + render 분기
│           ├── onboarding.rs     # ayg init TUI (5단계 입력)
│           ├── dashboard.rs      # 메인 대시보드 (좌/우 패널)
│           ├── script_review.rs  # 스크립트 검토/편집
│           ├── image_config.rs   # 이미지 설정 편집
│           ├── new_job_form.rs   # New Job 팝업 폼
│           └── log_panel.rs      # 로그 스트리밍
│
├── pipeline/                     # TypeScript 파이프라인
│   ├── package.json
│   ├── tsconfig.json
│   └── src/
│       ├── index.ts              # stdin 루프 → Command dispatch
│       ├── ipc.ts                # NDJSON send/recv
│       ├── types.ts              # 전체 IPC 타입 정의
│       ├── config.ts             # config.yaml 파싱
│       ├── runner.ts             # 체크포인트 기반 파이프라인
│       └── steps/
│           ├── scriptGenerator.ts
│           ├── imageGenerator.ts
│           ├── ttsGenerator.ts
│           ├── videoComposer.ts
│           └── uploader.ts
│
├── config.yaml                   # ayg init 생성 (gitignore 필수!)
├── client_secrets.json           # YouTube OAuth (gitignore 필수!)
├── .gitignore
├── workspace/                    # 중간 작업 파일
│   └── {jobId}/
│       ├── checkpoint.json
│       ├── script.json
│       ├── images/               # scene_01.png ...
│       ├── audio/                # scene_01.mp3 ...
│       └── output.mp4
└── output/                       # 최종 완성 영상
```

---

## 3. CLI 진입점 — ayg 바이너리

### 서브커맨드

```
ayg init    # 설정 파일(config.yaml) 생성 — TUI 온보딩 실행
ayg run     # TUI 실행 — Job 대시보드 진입
```

### main.rs 분기 로직

```rust
// tui/src/main.rs
#[derive(Subcommand)]
enum Commands {
    Init,
    Run,
}

fn main() {
    match cli.command {
        Commands::Init => {
            if config_exists() {
                // "Already initialized. [r] Reconfigure [q] Quit"
                run_reconfigure_tui();
            } else {
                run_onboarding_tui();   // → config.yaml 생성
            }
        }
        Commands::Run => {
            if !config_exists() {
                eprintln!("config.yaml not found. Run './ayg init' first.");
                process::exit(1);
            }
            run_dashboard_tui();        // Node.js spawn + 메인 TUI
        }
    }
}
```

### ayg run 시 실행 흐름

```
./ayg run
    │
    ├─ config.yaml 없음 ──▶  에러 메시지 + exit(1)
    │
    └─ config.yaml 있음 ──▶  Node.js 프로세스 spawn
                              └▶  TUI 대시보드 실행
```

---

## 4. ayg init — TUI 온보딩

### 온보딩 단계 (순서 고정)

| Step | 항목 | 필수 여부 | 입력 타입 |
|------|------|-----------|-----------|
| 1/5 | Anthropic API Key | 필수 | 텍스트 (마스킹) |
| 2/5 | OpenAI API Key | 필수 | 텍스트 (마스킹) |
| 3/5 | nanobanana API Key | 필수 | 텍스트 (마스킹) |
| 4/5 | YouTube OAuth (client_secrets.json 경로) | 선택 | 파일 경로 |
| 5/5 | ComfyUI Host URL | 선택 | URL |

### 화면 레이아웃 (각 스텝 공통 구조)

```
── AYG Setup  [N / 5] ─────────────────────────────────────────

  ✓ Anthropic API Key     sk-ant-••••••••••••    ← 완료 항목
  ▶ OpenAI API Key                               ← 현재 입력 중 (주황)
  ○ nanobanana API Key                           ← 미입력
  ○ YouTube OAuth
  ○ ComfyUI Host

  ┌─ {항목명} ────────────────────────────────────────────────┐
  │                                                           │
  │  {입력값}█                                                │
  │                                                           │
  └───────────────────────────────────────────────────────────┘

  {항목 설명 한 줄}
  {관련 URL 안내}

  Enter: next   Esc: back   Tab: skip (선택 항목만)
```

### 완료 화면

```
── AYG Setup — Complete! ──────────────────────────────────────

  ✓ Anthropic API Key     saved
  ✓ OpenAI API Key        saved
  ✓ nanobanana API Key    saved
  ✓ YouTube OAuth         saved
  ✓ ComfyUI Host          saved

  config.yaml created at ./config.yaml

  ┌──────────────────────────────────────────────────────────┐
  │   You're all set! Run AYG with:                         │
  │                                                          │
  │       ./ayg run                                          │
  └──────────────────────────────────────────────────────────┘

  Enter: launch AYG now   q: quit
```

### 온보딩 키 바인딩

| 키 | 동작 |
|----|------|
| `Enter` | 현재 필드 저장 → 다음 항목 |
| `Tab` | 스킵 (선택 항목만) |
| `Esc` | 이전 항목으로 |
| `Backspace` | 문자 삭제 |
| `q` | 온보딩 취소 (config.yaml 미생성) |

> **구현 주의사항**
> - API 키 입력 필드는 입력 중 `•`로 마스킹
> - 유효성 검사 없음 — 입력값 그대로 저장
> - YouTube는 API 키가 아닌 파일 경로 — 파일 존재 여부만 확인 (없어도 저장)
> - 완료 후 `Enter` 입력 시 바로 `ayg run` 모드로 전환

---

## 5. config.yaml 구조

```yaml
# ./config.yaml — ayg init 생성
# ⚠️  .gitignore에 반드시 추가할 것

anthropic:
  apiKey: "sk-ant-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  model: "claude-sonnet-4-20250514"

openai:
  apiKey: "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  ttsModel: "tts-1"
  ttsVoice: "nova"

nanobanana:
  apiKey: "nb-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
  width: 1080
  height: 1920

youtube:
  clientSecrets: "./client_secrets.json"
  defaultPrivacy: "private"

comfyui:
  host: "http://127.0.0.1:8188"

ayg:
  imageBackend: "nanobanana"   # nanobanana | comfyui
  workspace: "./workspace"
  output: "./output"
```

```
# .gitignore (필수)
config.yaml
client_secrets.json
workspace/
output/
```

---

## 6. 전체 파이프라인 플로우

```
[설치]
  git clone → cargo build --release
  cd pipeline && npm install && npx tsc

[초기 설정]
  ./ayg init
      └▶ TUI 온보딩 (5단계)
      └▶ ./config.yaml 생성

[실행]
  ./ayg run
      └▶ Dashboard 진입

  [n] New Job → 주제 입력
      │
      ▼
  [Step 1] 스크립트 생성       ← Claude API
      │  script_ready 이벤트
      ▼
  ★ [REVIEW 1] ScriptReview   ← 사용자 개입
      씬 추가/제거, 자막/낭독 편집
      [Enter] 승인
      │  script_approved 커맨드
      ▼
  [Step 2] 이미지 설정 준비    ← 스크립트 기반 프롬프트 자동 생성
      │  image_config_ready 이벤트
      ▼
  ★ [REVIEW 2] ImageConfig    ← 사용자 개입
      씬별 프롬프트/스타일/네거티브 편집
      [g] 글로벌 스타일 일괄 적용
      [Enter] 승인
      │  image_config_approved 커맨드
      ▼
  [Step 3] 이미지 생성         ← nanobanana / ComfyUI
      ▼
  [Step 4] TTS 생성            ← OpenAI TTS
      ▼
  [Step 5] 영상 합성           ← FFmpeg
      ▼
  [Step 6] YouTube 업로드      ← YouTube Data API v3
      ▼
  Done ✓
```

---

## 7. TUI 화면 목록 (AppMode)

```rust
enum AppMode {
    Onboarding,      // ayg init — API 키 5단계 입력
    Dashboard,       // 메인 화면 — Job 목록 + Preview + Log
    NewJob,          // [n] — 주제 입력 팝업
    BatchInput,      // [b] — 여러 주제 입력
    ScriptReview,    // ★ 스크립트 생성 완료 시 자동 전환
    ImageConfig,     // ★ 스크립트 승인 후 자동 전환
    JobDetail,       // [Enter] — 단일 Job 상세 로그
    Confirm,         // [d] — 삭제 확인 다이얼로그
    Reconfigure,     // ayg init (기존 config 있을 때)
}
```

---

## 8. TUI 화면 상세 설계

> **공통 디자인 원칙 (AOE 스타일)**
> - 배경색: `#0a0a0a` (순수 블랙)
> - 기본 텍스트: `Color::Rgb(0, 204, 102)` (초록)
> - 강조/진행 중: `Color::Rgb(255, 140, 0)` (주황)
> - 완료: `Color::Green`
> - 오류: `Color::Red`
> - 비활성: `Color::DarkGray`
> - 테두리: `BorderType::Plain` (얇은 single line)
> - 하단 고정 상태바: 현재 Job명 | 단계 | 시각

---

### 8.1 Dashboard

```
── AYG [demo] ───────────────────────────── v5.0 ──
┌─ Jobs ──────────────────┐  ┌─ Preview ───────────────────────┐
│ ● 양자컴퓨터란          │  │ Topic:   양자컴퓨터란           │
│ ○ AI의 미래             │  │ Backend: nanobanana             │
│ ✓ 블록체인 기초         │  │ Status:  ● Running              │
│                         │  │                                 │
│                         │  │ script ✓  images ●  tts ○      │
│                         │  │ compose ○  upload ○             │
│                         │  │                                 │
│                         │  │ [████████░░░░] 60%  (3/5)      │
│                         │  ├─ Log ───────────────────────────┤
│                         │  │ > 이미지 3/5 생성 완료          │
│                         │  │ > TTS 변환 시작...              │
└─────────────────────────┘  └─────────────────────────────────┘
● 양자컴퓨터란  │  images  │  15:29
[Job] j/k Nav  Enter Detail  n New  d Del  b Batch  q Quit
```

**레이아웃 분할:**
```rust
// 좌: 30%, 우: 70%
let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
    .split(area);

// 우측: Preview 60% + Log 40%
let right_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
    .split(chunks[1]);
```

**Job 상태 아이콘:**
```
● Running    (주황)
○ Queued     (회색)
✓ Done       (초록)
✕ Failed     (빨강)
⏸ Waiting    (주황, 사용자 개입 대기)
```

---

### 8.2 ScriptReview ★

```
── Script Review ──────────── [양자컴퓨터란] ── 5 scenes ──
┌─ Scenes ─────────────────┐  ┌─ Edit Scene ──────────────────┐
│ > [1] 도입부             │  │ Scene 1 / 5                   │
│   [2] 원리 설명          │  │                               │
│   [3] 활용 사례          │  │ Subtitle (자막)               │
│   [4] 미래 전망          │  │ ┌─────────────────────────┐  │
│   [5] 마무리             │  │ │ 양자컴퓨터는 기존과     │  │
│                          │  │ │ 완전히 다릅니다.█       │  │
│                          │  │ └─────────────────────────┘  │
│                          │  │                               │
│                          │  │ Narration (낭독 텍스트)       │
│                          │  │ ┌─────────────────────────┐  │
│                          │  │ │ 우리가 알고 있는 일반   │  │
│                          │  │ │ 컴퓨터와는 근본적으로   │  │
│                          │  │ │ 다른 방식으로 작동합니다│  │
│                          │  │ └─────────────────────────┘  │
│                          │  │                               │
│                          │  │ Duration: 4s                  │
└──────────────────────────┘  └───────────────────────────────┘
● 편집 중  │  Scene 1/5  │  Tab: 다음 필드  │  Ctrl+S: 저장
[e] Edit  [a] Add  [d] Del  [r] Regenerate  [Enter] Approve
```

**씬 상태 표시:**
```
> [1] 도입부           ← 선택됨 (일반)
> [1] 도입부  ✎        ← 편집 모드
> [1] 도입부  *        ← 수정됨 (미저장)
  [+] 새 씬            ← 추가된 씬
  [3] 활용 사례  ✕     ← 삭제 예정
```

**키 바인딩:**
| 키 | 동작 |
|----|------|
| `j` / `k` | 씬 목록 이동 |
| `e` | 편집 모드 (Subtitle 포커스) |
| `Tab` | Subtitle → Narration 전환 |
| `Ctrl+S` | 현재 씬 저장 |
| `Esc` | 편집 취소 |
| `a` | 현재 씬 아래 빈 씬 추가 |
| `d` | 선택 씬 삭제 (Confirm 팝업) |
| `r` | 스크립트 전체 재생성 |
| `Enter` | **★ 승인 → ImageConfig 전환** |

---

### 8.3 ImageConfig ★

```
── Image Config ───────────── [양자컴퓨터란] ── 5 scenes ──
┌─ Scenes ─────────────────┐  ┌─ Image Settings ─────────────┐
│ > [1] 도입부          ✓  │  │ Scene 1 / 5                  │
│   [2] 원리 설명          │  │                              │
│   [3] 활용 사례          │  │ Prompt                       │
│   [4] 미래 전망          │  │ ┌──────────────────────────┐ │
│   [5] 마무리             │  │ │ futuristic quantum        │ │
│                          │  │ │ computer, glowing blue,   │ │
│                          │  │ │ sci-fi style, 9:16█       │ │
│                          │  │ └──────────────────────────┘ │
│                          │  │                              │
│                          │  │ Style                        │
│                          │  │ ┌──────────────────────────┐ │
│                          │  │ │ cinematic, 4k             │ │
│                          │  │ └──────────────────────────┘ │
│                          │  │                              │
│                          │  │ Negative Prompt              │
│                          │  │ ┌──────────────────────────┐ │
│                          │  │ │ blurry, watermark, text   │ │
│                          │  │ └──────────────────────────┘ │
└──────────────────────────┘  └──────────────────────────────┘
○ 설정 중  │  Scene 1/5  │  Tab: 다음 필드
[e] Edit  [g] Global style  [p] Preview  [Enter] Approve  [Esc] Back
```

**Global Style 팝업 (`[g]` 키):**
```
┌─ Global Style ─────────────────────────────────────────┐
│                                                         │
│  Preset: ● cinematic  ○ anime  ○ flat  ○ custom        │
│                                                         │
│  Append to all prompts:                                 │
│  ┌───────────────────────────────────────────────────┐  │
│  │ cinematic, 4k, photorealistic, sharp focus        │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  Global Negative:                                       │
│  ┌───────────────────────────────────────────────────┐  │
│  │ blurry, low quality, text, watermark              │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  Enter: 전체 씬 적용   Esc: 취소                        │
└─────────────────────────────────────────────────────────┘
```

**씬 완료 상태:**
```
[1] 도입부        ← 미편집 (자동생성 상태)
[1] 도입부  ✓     ← 사용자 확인/저장 완료
[1] 도입부  *     ← 수정됨 (미저장)
[1] 도입부  ✎     ← 편집 중
```

**키 바인딩:**
| 키 | 동작 |
|----|------|
| `j` / `k` | 씬 목록 이동 |
| `e` | 편집 모드 (Prompt 포커스) |
| `Tab` | Prompt → Style → Negative 순환 |
| `Ctrl+S` | 현재 씬 저장 + ✓ 표시 |
| `g` | Global Style 팝업 |
| `p` | 최종 프롬프트 미리보기 |
| `Enter` | **★ 승인 → 이미지 생성 시작** |
| `Esc` | ScriptReview로 돌아가기 |

---

## 9. IPC 프로토콜 (NDJSON)

> 한 줄 = 한 메시지. `\n`으로 구분. 양방향 모두 UTF-8 JSON.

### Rust → Node.js (Commands)

```jsonc
// Job 시작
{ "type": "start_job", "jobId": "abc123", "topic": "양자컴퓨터란", "backend": "nanobanana" }

// 스크립트 승인 (편집된 씬 전달)
{ "type": "script_approved", "jobId": "abc123",
  "scenes": [{ "id": 1, "subtitle": "수정된 자막", "narration": "수정된 낭독", "duration": 4 }] }

// 이미지 설정 승인
{ "type": "image_config_approved", "jobId": "abc123",
  "globalStyle": "cinematic, 4k",
  "globalNegative": "blurry, watermark",
  "scenes": [{ "id": 1, "prompt": "futuristic...", "style": "cinematic...", "negative": "blurry..." }] }

// 스크립트 재생성 요청
{ "type": "regenerate_script", "jobId": "abc123" }

// Job 취소 / 재시작
{ "type": "cancel_job", "jobId": "abc123" }
{ "type": "resume_job", "jobId": "abc123" }

// 배치
{ "type": "batch_start", "topics": ["주제A", "주제B"] }
```

### Node.js → Rust (Events)

```jsonc
// Job 생성됨
{ "type": "job_created", "jobId": "abc123", "topic": "양자컴퓨터란" }

// 스크립트 준비 완료 → TUI ScriptReview 전환 트리거
{ "type": "script_ready", "jobId": "abc123",
  "script": {
    "title": "양자컴퓨터, 5분 만에 이해하기",
    "description": "...",
    "tags": ["양자컴퓨터", "과학"],
    "scenes": [{ "id": 1, "subtitle": "...", "narration": "...", "imagePrompt": "...", "duration": 4 }]
  }
}

// 이미지 설정 준비 완료 → TUI ImageConfig 전환 트리거
{ "type": "image_config_ready", "jobId": "abc123",
  "scenes": [{ "id": 1, "prompt": "futuristic quantum computer...", "style": "", "negative": "" }] }

// 단계 진행
{ "type": "step_start",  "jobId": "abc123", "step": "images" }
{ "type": "step_update", "jobId": "abc123", "step": "images", "progress": 60, "detail": "3/5" }
{ "type": "step_done",   "jobId": "abc123", "step": "images" }
{ "type": "step_error",  "jobId": "abc123", "step": "images", "error": "API timeout" }

// 로그
{ "type": "log", "jobId": "abc123", "level": "info", "message": "이미지 3/5 생성 완료" }

// Job 완료
{ "type": "job_done", "jobId": "abc123", "url": "https://youtu.be/xxxxx" }
```

---

## 10. TypeScript 타입 정의

> **파일: `pipeline/src/types.ts`** — 이 파일을 가장 먼저 작성할 것

```typescript
// ── IPC Commands (Rust → Node.js) ─────────────────────────────────────────
export type Command =
  | { type: 'start_job';             jobId: string; topic: string; backend: Backend }
  | { type: 'script_approved';        jobId: string; scenes: EditedScene[] }
  | { type: 'image_config_approved';  jobId: string; globalStyle: string;
                                       globalNegative: string; scenes: ImageScene[] }
  | { type: 'regenerate_script';      jobId: string }
  | { type: 'cancel_job';             jobId: string }
  | { type: 'resume_job';             jobId: string }
  | { type: 'batch_start';            topics: string[] };

// ── IPC Events (Node.js → Rust) ───────────────────────────────────────────
export type Event =
  | { type: 'job_created';        jobId: string; topic: string }
  | { type: 'script_ready';       jobId: string; script: Script }
  | { type: 'image_config_ready'; jobId: string; scenes: ImageConfigScene[] }
  | { type: 'step_start';         jobId: string; step: Step }
  | { type: 'step_update';        jobId: string; step: Step; progress: number; detail?: string }
  | { type: 'step_done';          jobId: string; step: Step }
  | { type: 'step_error';         jobId: string; step: Step; error: string }
  | { type: 'log';                jobId: string; level: LogLevel; message: string }
  | { type: 'job_done';           jobId: string; url: string };

// ── Domain Types ───────────────────────────────────────────────────────────
export type Step     = 'script' | 'images' | 'tts' | 'compose' | 'upload';
export type Backend  = 'nanobanana' | 'comfyui';
export type LogLevel = 'info' | 'warn' | 'error';

export interface Script {
  title:       string;
  description: string;
  tags:        string[];
  scenes:      ScriptScene[];
}

export interface ScriptScene {
  id:          number;
  subtitle:    string;    // 자막 텍스트
  narration:   string;    // TTS 입력
  imagePrompt: string;    // 이미지 프롬프트 (EN)
  duration:    number;    // 초
}

export interface EditedScene {
  id:        number;
  subtitle:  string;
  narration: string;
  duration:  number;
}

export interface ImageScene {
  id:       number;
  prompt:   string;
  style:    string;
  negative: string;
}

export interface ImageConfigScene {
  id:       number;
  prompt:   string;   // Claude가 생성한 초기 프롬프트
  style:    string;   // 빈 문자열 (사용자가 채울 것)
  negative: string;   // 빈 문자열
}
```

---

## 11. Rust 핵심 구조

### AppState (app.rs)

```rust
pub struct AppState {
    pub mode:         AppMode,
    pub jobs:         Vec<Job>,
    pub selected:     usize,
    pub logs:         VecDeque<LogEntry>,   // 최대 500줄
    pub input_buf:    String,               // 폼 입력 버퍼
    pub backend:      Backend,
    pub should_quit:  bool,

    // ScriptReview 전용
    pub script_scenes:    Vec<ScriptScene>,
    pub selected_scene:   usize,
    pub scene_edit_mode:  bool,
    pub scene_edit_field: SceneField,       // Subtitle | Narration

    // ImageConfig 전용
    pub image_scenes:     Vec<ImageScene>,
    pub image_edit_field: ImageField,       // Prompt | Style | Negative
}

pub struct Job {
    pub id:       String,
    pub topic:    String,
    pub status:   JobStatus,
    pub steps:    HashMap<Step, StepState>,
    pub progress: u8,   // 0–100
}

pub enum JobStatus {
    Queued,
    GeneratingScript,
    AwaitingScriptReview,   // ★ 사용자 개입 대기
    AwaitingImageConfig,    // ★ 사용자 개입 대기
    GeneratingImages,
    GeneratingTTS,
    Composing,
    Uploading,
    Done,
    Failed,
}
```

### IPC — Node.js spawn (ipc.rs)

```rust
// Node.js 파이프라인 spawn
let mut child = Command::new("node")
    .arg("pipeline/dist/index.js")
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()?;

// Node.js → Rust: 비동기 메시지 수신
tokio::spawn(async move {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        let event: IpcEvent = serde_json::from_str(&line)?;
        tx.send(event).await?;
    }
});

// Rust → Node.js: 커맨드 전송
async fn send_command(stdin: &mut ChildStdin, cmd: &Command) -> Result<()> {
    let json = serde_json::to_string(cmd)? + "\n";
    stdin.write_all(json.as_bytes()).await?;
    Ok(())
}
```

---

## 12. TypeScript 파이프라인 핵심 모듈

### ipc.ts

```typescript
import { createInterface } from 'readline';
import type { Command, Event } from './types';

// Node.js → Rust
export function send(event: Event): void {
  process.stdout.write(JSON.stringify(event) + '\n');
}

// Rust → Node.js (AsyncGenerator)
export async function* recv(): AsyncGenerator<Command> {
  const rl = createInterface({ input: process.stdin });
  for await (const line of rl) {
    yield JSON.parse(line) as Command;
  }
}
```

### runner.ts

```typescript
const STEPS: Step[] = ['script', 'images', 'tts', 'compose', 'upload'];

export async function runJob(jobId: string, topic: string, backend: Backend) {
  const cp = loadCheckpoint(jobId) ?? initCheckpoint(jobId, topic);

  for (const step of STEPS) {
    if (cp.steps[step].done) continue;  // 체크포인트 스킵

    // ★ 사용자 개입 대기 포인트
    if (step === 'images' && !cp.scriptApproved) {
      // script_ready 이벤트 발송 후 script_approved 커맨드 대기
      await waitForScriptApproval(jobId, cp);
    }
    if (step === 'images' && !cp.imageConfigApproved) {
      // image_config_ready 이벤트 발송 후 image_config_approved 대기
      await waitForImageConfig(jobId, cp);
    }

    send({ type: 'step_start', jobId, step });
    try {
      await STEP_HANDLERS[step](jobId, cp, backend);
      cp.steps[step].done = true;
      saveCheckpoint(cp);
      send({ type: 'step_done', jobId, step });
    } catch (e) {
      send({ type: 'step_error', jobId, step, error: String(e) });
      return;
    }
  }
}
```

---

## 13. 체크포인트 설계

### workspace/{jobId}/checkpoint.json

```json
{
  "jobId": "abc123",
  "topic": "양자컴퓨터란 무엇인가",
  "created": "2026-03-03T10:00:00Z",
  "status": "AwaitingImageConfig",
  "scriptApproved": true,
  "imageConfigApproved": false,
  "steps": {
    "script":  { "done": true,  "ts": "10:00:05" },
    "images":  { "done": false, "ts": null },
    "tts":     { "done": false, "ts": null },
    "compose": { "done": false, "ts": null },
    "upload":  { "done": false, "ts": null }
  },
  "editedScript": {
    "scenes": [
      { "id": 1, "subtitle": "수정된 자막", "narration": "...", "duration": 4 }
    ]
  },
  "imageConfig": null
}
```

### 재시작 시 동작

```
resume_job 수신
    │
    ├─ scriptApproved: false  →  script 재생성 → ScriptReview 전환
    ├─ scriptApproved: true,
    │  imageConfigApproved: false  →  ImageConfig 전환 (기존 편집 복원)
    └─ imageConfigApproved: true   →  images 단계부터 이어서 실행
```

---

## 14. 개발 단계 (Phase)

### Phase 1 — MVP (구현 순서)

```
[ ] 1.  TS: types.ts 전체 타입 정의
[ ] 2.  TS: ipc.ts + config.ts
[ ] 3.  TS: index.ts stdin 루프 + Command dispatch
[ ] 4.  TS: scriptGenerator.ts (Claude API → script_ready)
[ ] 5.  TS: runner.ts 체크포인트 + 사용자 개입 대기
[ ] 6.  TS: imageGenerator.ts (nanobanana)
[ ] 7.  TS: ttsGenerator.ts (OpenAI TTS)
[ ] 8.  TS: videoComposer.ts (FFmpeg)
[ ] 9.  Rust: main.rs CLI (init/run 분기) + clap
[ ] 10. Rust: config.rs (config.yaml 읽기/쓰기)
[ ] 11. Rust: onboarding.rs (5단계 TUI)
[ ] 12. Rust: ipc.rs (Node.js spawn + NDJSON)
[ ] 13. Rust: dashboard.rs (AOE 스타일 레이아웃)
[ ] 14. Rust: new_job_form.rs (팝업 폼)
[ ] 15. Rust: script_review.rs (좌/우 패널 + 인라인 에디터)
[ ] 16. Rust: image_config.rs (3필드 + GlobalStyle 팝업)
[ ] 17. 통합: E2E 테스트 (주제 입력 → 영상 완성)
```

### Phase 2

```
[ ] TS:   ComfyUI 백엔드 imageGenerator 추가
[ ] TS:   uploader.ts (YouTube Data API v3)
[ ] TS:   배치 처리 (batch_start 커맨드)
[ ] Rust: batch_input.rs UI
[ ] Rust: 로그 스트리밍 패널 개선
[ ] Rust: Reconfigure 화면
```

### Phase 3

```
[ ] TS:   배경음악 삽입 (FFmpeg 믹싱)
[ ] TS:   영상 전환 효과
[ ] Rust: 자막 스타일 커스터마이징 UI
[ ] Rust: config 편집 화면
```

---

## 15. 빠른 시작

```bash
# 1. 클론 & 빌드
git clone https://github.com/OnePaperHoon/ayg
cd ayg

# Rust TUI 빌드
cd tui && cargo build --release && cd ..

# TypeScript 파이프라인 빌드
cd pipeline && npm install && npx tsc && cd ..

# 2. 초기 설정 (TUI 온보딩)
./tui/target/release/ayg init

# 3. 실행
./tui/target/release/ayg run

# (선택) PATH에 추가
cp ./tui/target/release/ayg /usr/local/bin/ayg
ayg init
ayg run
```

### FFmpeg 설치

```bash
brew install ffmpeg        # macOS
sudo apt install ffmpeg    # Ubuntu/Debian
```

---

## ⚠️ 구현 시 주의사항

1. **config.yaml은 반드시 .gitignore에 추가** — API 키 노출 방지
2. **IPC는 NDJSON** — 한 줄에 반드시 하나의 JSON만, `\n`으로 종료
3. **사용자 개입 대기** — `runner.ts`에서 `script_approved` / `image_config_approved` 커맨드를 받을 때까지 async 대기
4. **체크포인트 우선** — 모든 Step 실행 전 `checkpoint.json` 확인, `done: true`면 스킵
5. **Node.js는 `pipeline/dist/index.js` 빌드 결과물 실행** — `tsx` 사용 시 `node --import tsx src/index.ts`로 변경 가능
6. **AOE 컬러 테마** — Ratatui에서 `Color::Rgb(0, 204, 102)` 초록, `Color::Rgb(255, 140, 0)` 주황 사용

---

*AYG — AI Youtube Generator | 설계 명세서 v5.0 | Author: OnePaperHoon | 2026.03.03*
