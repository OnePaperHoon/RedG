use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use serde::{Deserialize, Serialize};

// ── IPC 메시지 타입 (Rust 측) ─────────────────────────────────────────────────

/// Rust → Node.js 커맨드
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcCommand {
    StartJob       { #[serde(rename = "jobId")] job_id: String, topic: String, backend: String },
    ScriptApproved { #[serde(rename = "jobId")] job_id: String, scenes: Vec<EditedScene> },
    ImageConfigApproved {
        #[serde(rename = "jobId")] job_id:         String,
        #[serde(rename = "globalStyle")]  global_style:   String,
        #[serde(rename = "globalNegative")] global_negative: String,
        scenes: Vec<ImageScenePayload>,
    },
    RegenerateScript { #[serde(rename = "jobId")] job_id: String },
    CancelJob        { #[serde(rename = "jobId")] job_id: String },
    ResumeJob        { #[serde(rename = "jobId")] job_id: String },
    BatchStart       { topics: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditedScene {
    pub id:        u32,
    pub subtitle:  String,
    pub narration: String,
    pub duration:  u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageScenePayload {
    pub id:       u32,
    pub prompt:   String,
    pub style:    String,
    pub negative: String,
}

/// Node.js → Rust 이벤트
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcEvent {
    JobCreated       { #[serde(rename = "jobId")] job_id: String, topic: String },
    ScriptReady      { #[serde(rename = "jobId")] job_id: String, script: serde_json::Value },
    ImageConfigReady { #[serde(rename = "jobId")] job_id: String, scenes: serde_json::Value },
    StepStart        { #[serde(rename = "jobId")] job_id: String, step: String },
    StepUpdate       { #[serde(rename = "jobId")] job_id: String, step: String, progress: u8, detail: Option<String> },
    StepDone         { #[serde(rename = "jobId")] job_id: String, step: String },
    StepError        { #[serde(rename = "jobId")] job_id: String, step: String, error: String },
    Log              { #[serde(rename = "jobId")] job_id: String, level: String, message: String },
    JobDone          { #[serde(rename = "jobId")] job_id: String, url: String },
}

// ── Node.js 스폰 및 IPC 통신 ──────────────────────────────────────────────────

pub struct IpcHandle {
    pub child: Child,
}

/// Node.js 파이프라인 프로세스를 스폰하고 IPC 채널을 반환한다.
pub async fn spawn_pipeline(
    event_tx: UnboundedSender<IpcEvent>,
    mut cmd_rx: UnboundedReceiver<IpcCommand>,
) -> anyhow::Result<IpcHandle> {
    // 실행 파일 위치 기준으로 프로젝트 루트를 계산한다.
    // 바이너리 경로: tui/target/{profile}/ayg.exe → parent×3 = tui/ → parent = 프로젝트 루트
    let project_root = std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.parent()  // tui/target/debug 또는 release
             .and_then(|p| p.parent())  // tui/target
             .and_then(|p| p.parent())  // tui
             .and_then(|p| p.parent())  // 프로젝트 루트
             .map(|p| p.to_path_buf())
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let pipeline_js = project_root.join("pipeline").join("dist").join("index.js");

    // config.yaml은 ayg 실행 위치(CWD) 기준으로 저장되므로
    // Node.js도 같은 CWD에서 실행해야 config.yaml을 찾을 수 있다
    let run_dir = std::env::current_dir()
        .unwrap_or(project_root);

    let mut child = Command::new("node")
        .arg(&pipeline_js)
        .current_dir(&run_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())   // inherit 대신 piped — TUI 화면 오염 방지
        .spawn()
        .map_err(|e| anyhow::anyhow!(
            "Node.js 실행 실패 (경로: {}): {}", pipeline_js.display(), e
        ))?;

    let stdout = child.stdout.take().expect("stdout not captured");
    let stderr = child.stderr.take().expect("stderr not captured");
    let mut stdin = child.stdin.take().expect("stdin not captured");

    // Node.js → Rust: stdout 읽기 태스크 (NDJSON 이벤트)
    let stdout_tx = event_tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim().to_string();
            if trimmed.is_empty() { continue; }
            match serde_json::from_str::<IpcEvent>(&trimmed) {
                Ok(event) => {
                    if stdout_tx.send(event).is_err() { break; }
                }
                Err(_) => {
                    // JSON 파싱 실패 시 log 이벤트로 전달
                    let ev = IpcEvent::Log {
                        job_id:  String::new(),
                        level:   "warn".into(),
                        message: format!("[pipeline] {trimmed}"),
                    };
                    let _ = stdout_tx.send(ev);
                }
            }
        }
    });

    // Node.js → Rust: stderr 읽기 태스크 (TUI Log 패널로 전달)
    let stderr_tx = event_tx;
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim().to_string();
            if trimmed.is_empty() { continue; }
            let ev = IpcEvent::Log {
                job_id:  String::new(),
                level:   "warn".into(),
                message: format!("[pipeline] {trimmed}"),
            };
            if stderr_tx.send(ev).is_err() { break; }
        }
    });

    // Rust → Node.js: stdin 쓰기 태스크
    tokio::spawn(async move {
        while let Some(cmd) = cmd_rx.recv().await {
            match serde_json::to_string(&cmd) {
                Ok(json) => {
                    let line = format!("{}\n", json);
                    if stdin.write_all(line.as_bytes()).await.is_err() { break; }
                }
                Err(e) => eprintln!("[ipc] serialize error: {e}"),
            }
        }
    });

    Ok(IpcHandle { child })
}
