mod app;
mod config;
mod ipc;
mod ui;

use std::io;
use std::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;
use clap::{Parser, Subcommand};

use app::{
    AppMode, AppState, ConfirmAction, ImageScene, Job, JobStatus, LogEntry, ScriptScene, Step,
    StepStatus,
};
use ipc::{EditedScene, ImageScenePayload, IpcCommand, IpcEvent};

// ── CLI ───────────────────────────────────────────────────────────────────────
#[derive(Parser)]
#[command(name = "ayg", about = "AYG — AI Youtube Generator", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// API 키 설정 (config.yaml 생성)
    Init,
    /// TUI 대시보드 실행
    Run,
}

// ── 메인 ─────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            if config::config_exists() {
                println!("config.yaml이 이미 존재합니다.");
                println!("삭제 후 다시 실행하면 재설정할 수 있습니다: rm config.yaml");
            } else {
                run_onboarding().await?;
            }
        }
        Commands::Run => {
            if !config::config_exists() {
                eprintln!("config.yaml을 찾을 수 없습니다. 먼저 'ayg init'을 실행하세요.");
                std::process::exit(1);
            }
            run_dashboard().await?;
        }
    }
    Ok(())
}

// ── 온보딩 ────────────────────────────────────────────────────────────────────
async fn run_onboarding() -> anyhow::Result<()> {
    setup_panic_hook();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = ui::onboarding::run(&mut terminal).await;

    teardown_terminal(&mut terminal)?;
    result
}

// ── 대시보드 (메인 TUI) ───────────────────────────────────────────────────────
async fn run_dashboard() -> anyhow::Result<()> {
    setup_panic_hook();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = dashboard_loop(&mut terminal).await;

    teardown_terminal(&mut terminal)?;
    result
}

async fn dashboard_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    let mut state = AppState::new();

    // IPC 채널
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<IpcEvent>();
    let (cmd_tx,   cmd_rx)       = mpsc::unbounded_channel::<IpcCommand>();

    // Node.js 파이프라인 스폰
    let mut _ipc = ipc::spawn_pipeline(event_tx, cmd_rx).await?;

    // 키보드 이벤트 채널 (블로킹 스레드)
    let (key_tx, mut key_rx) = mpsc::unbounded_channel::<KeyEvent>();
    std::thread::spawn(move || {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(50)) {
                if let Ok(Event::Key(key)) = event::read() {
                    // Press 이벤트만 처리 — Release/Repeat 이벤트로 인한 이중 입력 방지
                    if key.kind == KeyEventKind::Press {
                        if key_tx.send(key).is_err() { break; }
                    }
                }
            }
        }
    });

    let mut tick = tokio::time::interval(Duration::from_millis(100));

    loop {
        terminal.draw(|f| ui::render(f, &state))?;

        tokio::select! {
            biased;
            Some(key) = key_rx.recv() => {
                handle_key(&mut state, key, &cmd_tx);
            }
            Some(ev) = event_rx.recv() => {
                handle_ipc_event(&mut state, ev);
            }
            _ = tick.tick() => {}
        }

        if state.should_quit { break; }
    }

    Ok(())
}

// ── 키 처리 ───────────────────────────────────────────────────────────────────
fn handle_key(state: &mut AppState, key: KeyEvent, cmd_tx: &mpsc::UnboundedSender<IpcCommand>) {
    match state.mode.clone() {
        AppMode::Dashboard => ui::dashboard::handle_key(state, key),

        AppMode::NewJob => {
            match key.code { // key is Copy
                KeyCode::Enter => {
                    let topic = state.new_job_input.trim().to_string();
                    if !topic.is_empty() {
                        let job_id = format!("job_{}", timestamp_id());
                        let job = Job::new(job_id.clone(), topic.clone());
                        state.jobs.push(job);
                        state.selected_job = state.jobs.len() - 1;
                        let _ = cmd_tx.send(IpcCommand::StartJob {
                            job_id,
                            topic,
                            backend: state.backend.clone(),
                        });
                        state.mode = AppMode::Dashboard;
                    }
                }
                KeyCode::Esc => {
                    state.new_job_input.clear();
                    state.mode = AppMode::Dashboard;
                }
                KeyCode::Backspace => { state.new_job_input.pop(); }
                KeyCode::Char(c)   => state.new_job_input.push(c),
                _ => {}
            }
        }

        AppMode::ScriptReview => {
            use ui::script_review::{handle_key as sr_key, ScriptReviewAction};
            if let Some(action) = sr_key(state, key) {
                match action {
                    ScriptReviewAction::Approve => {
                        let job_id = state.current_job_id.clone().unwrap_or_default();
                        let scenes: Vec<EditedScene> = state.script_scenes.iter().map(|s| EditedScene {
                            id:        s.id,
                            subtitle:  s.subtitle.clone(),
                            narration: s.narration.clone(),
                            duration:  s.duration,
                        }).collect();
                        let _ = cmd_tx.send(IpcCommand::ScriptApproved { job_id: job_id.clone(), scenes });
                        // 상태 업데이트
                        if let Some(job) = state.job_mut(&job_id) {
                            job.status = JobStatus::AwaitingImageConfig;
                        }
                        state.mode = AppMode::ImageConfig;
                    }
                    ScriptReviewAction::Regenerate => {
                        let job_id = state.current_job_id.clone().unwrap_or_default();
                        let _ = cmd_tx.send(IpcCommand::RegenerateScript { job_id: job_id.clone() });
                        if let Some(job) = state.job_mut(&job_id) {
                            job.status = JobStatus::GeneratingScript;
                        }
                    }
                    ScriptReviewAction::ConfirmDelete(idx) => {
                        state.mode = AppMode::Confirm(ConfirmAction::DeleteScene(idx));
                    }
                }
            }
        }

        AppMode::ImageConfig | AppMode::ImageConfigGlobalPopup => {
            use ui::image_config::{handle_key as ic_key, ImageConfigAction};
            if let Some(action) = ic_key(state, key) {
                match action {
                    ImageConfigAction::Approve => {
                        let job_id = state.current_job_id.clone().unwrap_or_default();
                        let scenes: Vec<ImageScenePayload> = state.image_scenes.iter().map(|s| ImageScenePayload {
                            id:       s.id,
                            prompt:   s.prompt.clone(),
                            style:    s.style.clone(),
                            negative: s.negative.clone(),
                        }).collect();
                        let _ = cmd_tx.send(IpcCommand::ImageConfigApproved {
                            job_id:          job_id.clone(),
                            global_style:    state.global_style.clone(),
                            global_negative: state.global_negative.clone(),
                            scenes,
                        });
                        if let Some(job) = state.job_mut(&job_id) {
                            job.status = JobStatus::GeneratingImages;
                        }
                        state.mode = AppMode::Dashboard;
                    }
                }
            }
        }

        AppMode::Confirm(action) => {
            let is_scene_delete = matches!(action, ConfirmAction::DeleteScene(_));
            let code = match key.code {
                KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
                other            => other,
            };
            match code {
                KeyCode::Char('y') => {
                    match action {
                        ConfirmAction::DeleteJob(id) => {
                            state.jobs.retain(|j| j.id != id);
                            if state.selected_job >= state.jobs.len() && state.selected_job > 0 {
                                state.selected_job -= 1;
                            }
                        }
                        ConfirmAction::DeleteScene(idx) => {
                            ui::script_review::delete_scene(state, idx);
                        }
                    }
                    state.mode = AppMode::Dashboard;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    state.mode = if is_scene_delete { AppMode::ScriptReview } else { AppMode::Dashboard };
                }
                _ => {}
            }
        }
    }
}

// ── IPC 이벤트 처리 ───────────────────────────────────────────────────────────
fn handle_ipc_event(state: &mut AppState, event: IpcEvent) {
    match event {
        IpcEvent::JobCreated { job_id, .. } => {
            if let Some(job) = state.job_mut(&job_id) {
                job.status = JobStatus::GeneratingScript;
            }
        }

        IpcEvent::ScriptReady { job_id, script } => {
            // 씬 파싱
            if let Ok(scenes) = serde_json::from_value::<Vec<ScriptScene>>(script["scenes"].clone()) {
                state.script_scenes  = scenes;
                state.selected_scene = 0;
                state.scene_edit_mode = false;
                state.current_job_id = Some(job_id.clone());
            }
            if let Some(job) = state.job_mut(&job_id) {
                job.status = JobStatus::AwaitingScriptReview;
                job.steps.insert(Step::Script, StepStatus::Done);
            }
            state.mode = AppMode::ScriptReview;
        }

        IpcEvent::ImageConfigReady { job_id, scenes } => {
            if let Ok(img_scenes) = serde_json::from_value::<Vec<ImageScene>>(scenes) {
                state.image_scenes   = img_scenes;
                state.selected_scene = 0;
                state.image_edit_mode = false;
                state.current_job_id = Some(job_id.clone());
            }
            if let Some(job) = state.job_mut(&job_id) {
                job.status = JobStatus::AwaitingImageConfig;
            }
            state.mode = AppMode::ImageConfig;
        }

        IpcEvent::StepStart { job_id, step } => {
            if let Some(job) = state.job_mut(&job_id) {
                let s = parse_step(&step);
                job.steps.insert(s.clone(), StepStatus::Running);
                job.status = match s {
                    Step::Script  => JobStatus::GeneratingScript,
                    Step::Images  => JobStatus::GeneratingImages,
                    Step::Tts     => JobStatus::GeneratingTTS,
                    Step::Compose => JobStatus::Composing,
                    Step::Upload  => JobStatus::Uploading,
                };
            }
        }

        IpcEvent::StepUpdate { job_id, progress, .. } => {
            if let Some(job) = state.job_mut(&job_id) {
                job.progress = progress;
            }
        }

        IpcEvent::StepDone { job_id, step } => {
            if let Some(job) = state.job_mut(&job_id) {
                job.steps.insert(parse_step(&step), StepStatus::Done);
                job.progress = calc_progress(&job.steps);
            }
        }

        IpcEvent::StepError { job_id, step, error } => {
            if let Some(job) = state.job_mut(&job_id) {
                job.steps.insert(parse_step(&step), StepStatus::Failed);
                job.status = JobStatus::Failed(error.clone());
            }
            state.add_log(LogEntry { job_id, level: "error".into(), message: error });
        }

        IpcEvent::Log { job_id, level, message } => {
            state.add_log(LogEntry { job_id, level, message });
        }

        IpcEvent::JobDone { job_id, url } => {
            if let Some(job) = state.job_mut(&job_id) {
                job.status   = JobStatus::Done;
                job.progress = 100;
            }
            state.add_log(LogEntry {
                job_id,
                level:   "info".into(),
                message: format!("완료: {url}"),
            });
        }
    }
}

// ── 유틸 ─────────────────────────────────────────────────────────────────────
fn parse_step(s: &str) -> Step {
    match s {
        "script"  => Step::Script,
        "images"  => Step::Images,
        "tts"     => Step::Tts,
        "compose" => Step::Compose,
        "upload"  => Step::Upload,
        _         => Step::Script,
    }
}

fn calc_progress(steps: &std::collections::HashMap<Step, StepStatus>) -> u8 {
    let total = 5u8;
    let done  = steps.values().filter(|s| matches!(s, StepStatus::Done)).count() as u8;
    done * 100 / total
}

fn timestamp_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("{ms:x}")
}

fn setup_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stderr(), LeaveAlternateScreen);
        original(info);
    }));
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
