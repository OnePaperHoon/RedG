use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use crate::app::{AppState, SceneField, ScriptScene};

const GREEN:  Color = Color::Rgb(0, 204, 102);
const ORANGE: Color = Color::Rgb(255, 140, 0);

// ── 키 처리 ───────────────────────────────────────────────────────────────────
pub fn handle_key(state: &mut AppState, key: KeyEvent) -> Option<ScriptReviewAction> {
    if state.scene_edit_mode {
        return handle_edit_key(state, key);
    }

    let code = match key.code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        other            => other,
    };
    match code {
        KeyCode::Char('j') | KeyCode::Down  => {
            if state.selected_scene + 1 < state.script_scenes.len() {
                state.selected_scene += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if state.selected_scene > 0 {
                state.selected_scene -= 1;
            }
        }
        KeyCode::Char('e') => {
            if !state.script_scenes.is_empty() {
                let s = &state.script_scenes[state.selected_scene];
                state.scene_edit_buf  = s.subtitle.clone();
                state.scene_field     = SceneField::Subtitle;
                state.scene_edit_mode = true;
            }
        }
        KeyCode::Char('a') => {
            let new_id = state.script_scenes.len() as u32 + 1;
            let insert_pos = state.selected_scene + 1;
            state.script_scenes.insert(insert_pos, ScriptScene {
                id: new_id, subtitle: String::new(), narration: String::new(),
                image_prompt: String::new(), duration: 5,
            });
            state.selected_scene = insert_pos;
        }
        KeyCode::Char('d') => {
            if !state.script_scenes.is_empty() {
                return Some(ScriptReviewAction::ConfirmDelete(state.selected_scene));
            }
        }
        KeyCode::Char('r') => {
            return Some(ScriptReviewAction::Regenerate);
        }
        KeyCode::Enter => {
            return Some(ScriptReviewAction::Approve);
        }
        _ => {}
    }
    None
}

fn handle_edit_key(state: &mut AppState, key: KeyEvent) -> Option<ScriptReviewAction> {
    match key.code {
        KeyCode::Esc => {
            state.scene_edit_mode = false;
            state.scene_edit_buf.clear();
        }
        KeyCode::Tab => {
            // 필드 전환 및 저장
            save_edit_buf(state);
            state.scene_field = match state.scene_field {
                SceneField::Subtitle  => {
                    let s = &state.script_scenes[state.selected_scene];
                    state.scene_edit_buf = s.narration.clone();
                    SceneField::Narration
                }
                SceneField::Narration => {
                    state.scene_edit_mode = false;
                    state.scene_edit_buf.clear();
                    SceneField::Subtitle
                }
            };
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            save_edit_buf(state);
            state.scene_edit_mode = false;
            state.scene_edit_buf.clear();
        }
        KeyCode::Backspace => { state.scene_edit_buf.pop(); }
        KeyCode::Char(c)   => state.scene_edit_buf.push(c),
        KeyCode::Enter     => {
            // 줄바꿈 삽입
            state.scene_edit_buf.push('\n');
        }
        _ => {}
    }
    None
}

fn save_edit_buf(state: &mut AppState) {
    if state.selected_scene < state.script_scenes.len() {
        match state.scene_field {
            SceneField::Subtitle  => state.script_scenes[state.selected_scene].subtitle  = state.scene_edit_buf.clone(),
            SceneField::Narration => state.script_scenes[state.selected_scene].narration = state.scene_edit_buf.clone(),
        }
    }
}

#[derive(Debug)]
pub enum ScriptReviewAction {
    Approve,
    Regenerate,
    ConfirmDelete(usize),
}

pub fn delete_scene(state: &mut AppState, idx: usize) {
    if idx < state.script_scenes.len() {
        state.script_scenes.remove(idx);
        if state.selected_scene >= state.script_scenes.len() && state.selected_scene > 0 {
            state.selected_scene -= 1;
        }
    }
}

// ── 렌더링 ────────────────────────────────────────────────────────────────────
pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();
    let total = state.script_scenes.len();
    let title = state.current_job_id.as_deref().unwrap_or("unknown");

    // 타이틀바
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(area);

    // 헤더
    let header = Line::from(vec![
        Span::styled(" Script Review ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled(format!("── [{title}] ── {total} scenes"), Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    // 좌/우 분할
    let body = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ]).split(chunks[1]);

    render_scene_list(f, body[0], state);
    render_edit_panel(f, body[1], state);

    // 상태바
    let field_hint = if state.scene_edit_mode {
        let f_name = match state.scene_field { SceneField::Subtitle => "Subtitle", SceneField::Narration => "Narration" };
        format!("{f_name} 편집 중  │  Tab: 필드전환  │  Ctrl+S: 저장  │  Esc: 취소")
    } else {
        format!("Scene {}/{total}  │  e:편집  a:추가  d:삭제  r:재생성  Enter:승인", state.selected_scene + 1)
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {field_hint}"),
            Style::default().fg(Color::DarkGray),
        ))),
        chunks[2],
    );
}

fn render_scene_list(f: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state.script_scenes.iter().enumerate().map(|(i, s)| {
        let selected = i == state.selected_scene;
        let prefix = if selected { "▶ " } else { "  " };
        let label  = if s.subtitle.is_empty() { "(empty)".to_string() } else { s.subtitle.chars().take(20).collect() };
        let text   = format!("{prefix}[{}] {}", s.id, label);
        let style  = if selected {
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(GREEN)
        };
        ListItem::new(text).style(style)
    }).collect();

    let list = List::new(items)
        .block(Block::default().title(" Scenes ").borders(Borders::ALL)
            .border_type(BorderType::Plain).border_style(Style::default().fg(GREEN)));

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected_scene));
    f.render_stateful_widget(list, area, &mut list_state);
}

fn render_edit_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(" Edit Scene ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(GREEN));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(scene) = state.script_scenes.get(state.selected_scene) else { return };

    let chunks = Layout::vertical([
        Constraint::Length(1),  // scene id
        Constraint::Length(1),  // subtitle label
        Constraint::Length(4),  // subtitle box
        Constraint::Length(1),  // narration label
        Constraint::Length(5),  // narration box
        Constraint::Length(1),  // duration
    ]).split(inner);

    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  Scene {} / {}", scene.id, state.script_scenes.len()),
            Style::default().fg(Color::DarkGray),
        )),
        chunks[0],
    );

    // Subtitle
    render_field_box(f, chunks[1], chunks[2], "Subtitle (자막)",
        if state.scene_edit_mode && state.scene_field == SceneField::Subtitle {
            Some(&state.scene_edit_buf)
        } else {
            None
        },
        &scene.subtitle,
        state.scene_edit_mode && state.scene_field == SceneField::Subtitle,
    );

    // Narration
    render_field_box(f, chunks[3], chunks[4], "Narration (낭독)",
        if state.scene_edit_mode && state.scene_field == SceneField::Narration {
            Some(&state.scene_edit_buf)
        } else {
            None
        },
        &scene.narration,
        state.scene_edit_mode && state.scene_field == SceneField::Narration,
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  Duration: {}s", scene.duration),
            Style::default().fg(Color::DarkGray),
        )),
        chunks[5],
    );
}

fn render_field_box(
    f: &mut Frame,
    label_area: Rect,
    box_area: Rect,
    label: &str,
    edit_buf: Option<&str>,
    display_val: &str,
    active: bool,
) {
    let border_style = if active {
        Style::default().fg(ORANGE)
    } else {
        Style::default().fg(GREEN)
    };

    f.render_widget(
        Paragraph::new(Span::styled(format!("  {label}"), Style::default().fg(Color::DarkGray))),
        label_area,
    );

    let content = if let Some(buf) = edit_buf {
        format!("{buf}\u{2588}")
    } else {
        display_val.to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(border_style);
    let inner = block.inner(box_area);
    f.render_widget(block, box_area);
    f.render_widget(
        Paragraph::new(content).wrap(Wrap { trim: false }).style(Style::default().fg(Color::White)),
        inner,
    );
}
