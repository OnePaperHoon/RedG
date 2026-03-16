use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use crate::app::{AppState, AppMode, Job, JobStatus, Step, StepStatus};

const GREEN:  Color = Color::Rgb(0, 204, 102);
const ORANGE: Color = Color::Rgb(255, 140, 0);

// ── 액션 ─────────────────────────────────────────────────────────────────────
pub enum DashboardAction {
    Resume { job_id: String, topic: String },
}

// ── 키 처리 ───────────────────────────────────────────────────────────────────
pub fn handle_key(state: &mut AppState, key: KeyEvent) -> Option<DashboardAction> {
    // 대소문자 무관 처리 (Caps Lock / Shift 키 대응)
    let code = match key.code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        other            => other,
    };
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if state.selected_job + 1 < state.jobs.len() {
                state.selected_job += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if state.selected_job > 0 {
                state.selected_job -= 1;
            }
        }
        KeyCode::Char('n') => {
            state.new_job_input.clear();
            state.mode = AppMode::NewJob;
        }
        KeyCode::Char('d') => {
            if let Some(job) = state.selected_job() {
                let id = job.id.clone();
                state.mode = AppMode::Confirm(crate::app::ConfirmAction::DeleteJob(id));
            }
        }
        KeyCode::Char('r') => {
            if let Some(job) = state.selected_job() {
                if matches!(job.status, JobStatus::Failed(_)) {
                    return Some(DashboardAction::Resume {
                        job_id: job.id.clone(),
                        topic:  job.topic.clone(),
                    });
                }
            }
        }
        KeyCode::Char('q') => state.should_quit = true,
        _ => {}
    }
    None
}

// ── 렌더링 ────────────────────────────────────────────────────────────────────
pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(1),  // 헤더
        Constraint::Fill(1),    // 메인
        Constraint::Length(1),  // 상태바
        Constraint::Length(1),  // 키힌트
    ]).split(area);

    // 헤더
    render_header(f, chunks[0], state);

    // 좌(30%) / 우(70%)
    let body = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ]).split(chunks[1]);

    render_job_list(f, body[0], state);
    render_right_panel(f, body[1], state);

    // 상태바
    render_statusbar(f, chunks[2], state);

    // 키힌트
    render_keyhint(f, chunks[3]);
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let job_name = state.selected_job().map(|j| j.topic.as_str()).unwrap_or("");
    let line = Line::from(vec![
        Span::styled(" AYG ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled(format!("[{}]", job_name), Style::default().fg(ORANGE)),
        Span::styled(" ─────────────── v5.0", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_job_list(f: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state.jobs.iter().enumerate().map(|(i, job)| {
        let selected = i == state.selected_job;
        let icon  = job.status.icon();
        let icon_style = match &job.status {
            JobStatus::Done          => Style::default().fg(Color::Green),
            JobStatus::Failed(_)     => Style::default().fg(Color::Red),
            JobStatus::AwaitingScriptReview | JobStatus::AwaitingImageConfig => Style::default().fg(ORANGE),
            JobStatus::Queued        => Style::default().fg(Color::DarkGray),
            _                        => Style::default().fg(ORANGE),
        };
        let label: String = job.topic.chars().take(22).collect();
        let line = Line::from(vec![
            Span::styled(format!("{icon} "), icon_style),
            Span::styled(label, if selected {
                Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(GREEN)
            }),
        ]);
        ListItem::new(line)
    }).collect();

    let list = List::new(items)
        .block(Block::default()
            .title(" Jobs ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(GREEN)))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut ls = ListState::default();
    ls.select(Some(state.selected_job));
    f.render_stateful_widget(list, area, &mut ls);
}

fn render_right_panel(f: &mut Frame, area: Rect, state: &AppState) {
    // Preview 60% + Log 40%
    let right = Layout::vertical([
        Constraint::Percentage(60),
        Constraint::Percentage(40),
    ]).split(area);

    render_preview(f, right[0], state);

    let logs = state.selected_job()
        .map(|j| &j.logs)
        .unwrap_or(&state.global_logs);
    crate::ui::log_panel::render(f, right[1], logs, "Log");
}

fn render_preview(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(" Preview ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(GREEN));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(job) = state.selected_job() else {
        f.render_widget(
            Paragraph::new(Span::styled("  No job selected", Style::default().fg(Color::DarkGray))),
            inner,
        );
        return;
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled("  Topic:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(job.topic.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Backend: ", Style::default().fg(Color::DarkGray)),
            Span::styled(state.backend.clone(), Style::default().fg(GREEN)),
        ]),
        Line::from(vec![
            Span::styled("  Status:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} {}", job.status.icon(), job.status.label()),
                step_color(&job.status),
            ),
        ]),
        Line::from(""),
    ];

    // Steps
    let step_line = render_step_line(job);
    lines.push(step_line);
    lines.push(Line::from(""));

    // Progress gauge (텍스트로)
    let pct = job.progress;
    let filled = (pct as usize * 20 / 100).min(20);
    let bar: String = "█".repeat(filled) + &"░".repeat(20 - filled);
    lines.push(Line::from(vec![
        Span::styled(format!("  [{bar}] {pct}%"), Style::default().fg(ORANGE)),
    ]));

    f.render_widget(Paragraph::new(lines), inner);
}

fn render_step_line(job: &Job) -> Line<'static> {
    let steps = [
        (Step::Script,  "script"),
        (Step::Images,  "images"),
        (Step::Tts,     "tts"),
        (Step::Compose, "compose"),
        (Step::Upload,  "upload"),
    ];
    let mut spans = vec![Span::styled("  ", Style::default())];
    for (step, label) in &steps {
        let status = job.steps.get(step).unwrap_or(&StepStatus::Pending);
        let (icon, style) = match status {
            StepStatus::Done    => ("✓", Style::default().fg(Color::Green)),
            StepStatus::Running => ("●", Style::default().fg(ORANGE)),
            StepStatus::Failed  => ("✕", Style::default().fg(Color::Red)),
            StepStatus::Pending => ("○", Style::default().fg(Color::DarkGray)),
        };
        spans.push(Span::styled(format!("{label} {icon}  "), style));
    }
    Line::from(spans)
}

fn step_color(status: &JobStatus) -> Style {
    match status {
        JobStatus::Done        => Style::default().fg(Color::Green),
        JobStatus::Failed(_)   => Style::default().fg(Color::Red),
        JobStatus::AwaitingScriptReview | JobStatus::AwaitingImageConfig => Style::default().fg(ORANGE),
        JobStatus::Queued      => Style::default().fg(Color::DarkGray),
        _                      => Style::default().fg(ORANGE),
    }
}

fn render_statusbar(f: &mut Frame, area: Rect, state: &AppState) {
    let job_name = state.selected_job().map(|j| j.topic.as_str()).unwrap_or("—");
    let step_label = state.selected_job()
        .map(|j| j.status.label().to_string())
        .unwrap_or_default();
    let now = chrono_time();
    let line = Line::from(vec![
        Span::styled(format!(" {job_name}"), Style::default().fg(GREEN)),
        Span::styled(format!("  │  {step_label}"), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("  │  {now}"), Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn render_keyhint(f: &mut Frame, area: Rect) {
    let line = Line::from(Span::styled(
        " j/k: Nav   n: New   r: Resume(Failed)   d: Del   q: Quit",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(Paragraph::new(line), area);
}

fn chrono_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60)   % 60;
    let s =  secs          % 60;
    format!("{h:02}:{m:02}:{s:02}")
}
