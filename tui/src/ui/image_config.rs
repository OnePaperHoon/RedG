use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use crate::app::{AppState, ImageField};

const GREEN:  Color = Color::Rgb(0, 204, 102);
const ORANGE: Color = Color::Rgb(255, 140, 0);

#[derive(Debug)]
pub enum ImageConfigAction {
    Approve,
}

// ── 키 처리 ───────────────────────────────────────────────────────────────────
pub fn handle_key(state: &mut AppState, key: KeyEvent) -> Option<ImageConfigAction> {
    // Global Style 팝업
    if matches!(state.mode, crate::app::AppMode::ImageConfigGlobalPopup) {
        return handle_global_popup_key(state, key);
    }

    if state.image_edit_mode {
        return handle_edit_key(state, key);
    }

    let code = match key.code {
        KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
        other            => other,
    };
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            if state.selected_scene + 1 < state.image_scenes.len() {
                state.selected_scene += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if state.selected_scene > 0 {
                state.selected_scene -= 1;
            }
        }
        KeyCode::Char('e') => {
            if !state.image_scenes.is_empty() {
                let s = &state.image_scenes[state.selected_scene];
                state.image_edit_buf  = s.prompt.clone();
                state.image_field     = ImageField::Prompt;
                state.image_edit_mode = true;
            }
        }
        KeyCode::Char('g') => {
            state.global_style_buf = state.global_style.clone();
            state.global_neg_buf   = state.global_negative.clone();
            state.mode = crate::app::AppMode::ImageConfigGlobalPopup;
        }
        KeyCode::Enter => return Some(ImageConfigAction::Approve),
        KeyCode::Esc   => {
            state.mode = crate::app::AppMode::ScriptReview;
        }
        _ => {}
    }
    None
}

fn handle_edit_key(state: &mut AppState, key: KeyEvent) -> Option<ImageConfigAction> {
    match key.code {
        KeyCode::Esc => {
            state.image_edit_mode = false;
            state.image_edit_buf.clear();
        }
        KeyCode::Tab => {
            save_image_buf(state);
            state.image_field = match state.image_field {
                ImageField::Prompt => {
                    let s = &state.image_scenes[state.selected_scene];
                    state.image_edit_buf = s.style.clone();
                    ImageField::Style
                }
                ImageField::Style => {
                    let s = &state.image_scenes[state.selected_scene];
                    state.image_edit_buf = s.negative.clone();
                    ImageField::Negative
                }
                ImageField::Negative => {
                    state.image_edit_mode = false;
                    state.image_edit_buf.clear();
                    ImageField::Prompt
                }
            };
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            save_image_buf(state);
            state.image_edit_mode = false;
            state.image_edit_buf.clear();
        }
        KeyCode::Backspace => { state.image_edit_buf.pop(); }
        KeyCode::Char(c)   => state.image_edit_buf.push(c),
        _ => {}
    }
    None
}

fn handle_global_popup_key(state: &mut AppState, key: KeyEvent) -> Option<ImageConfigAction> {
    // 팝업 내 포커스: 0=style_text, 1=neg_text (Tab 전환)
    match key.code {
        KeyCode::Esc => {
            state.mode = crate::app::AppMode::ImageConfig;
        }
        KeyCode::Enter => {
            state.global_style   = state.global_style_buf.clone();
            state.global_negative = state.global_neg_buf.clone();
            state.mode = crate::app::AppMode::ImageConfig;
        }
        KeyCode::Backspace => { state.global_style_buf.pop(); }
        KeyCode::Char(c)   => state.global_style_buf.push(c),
        _ => {}
    }
    None
}

fn save_image_buf(state: &mut AppState) {
    if state.selected_scene < state.image_scenes.len() {
        match state.image_field {
            ImageField::Prompt   => state.image_scenes[state.selected_scene].prompt   = state.image_edit_buf.clone(),
            ImageField::Style    => state.image_scenes[state.selected_scene].style    = state.image_edit_buf.clone(),
            ImageField::Negative => state.image_scenes[state.selected_scene].negative = state.image_edit_buf.clone(),
        }
    }
}

// ── 렌더링 ────────────────────────────────────────────────────────────────────
pub fn render(f: &mut Frame, state: &AppState) {
    let area = f.area();
    let total = state.image_scenes.len();
    let title = state.current_job_id.as_deref().unwrap_or("unknown");

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(area);

    let header = Line::from(vec![
        Span::styled(" Image Config ", Style::default().fg(GREEN).add_modifier(Modifier::BOLD)),
        Span::styled(format!("── [{title}] ── {total} scenes"), Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(header), chunks[0]);

    let body = Layout::horizontal([
        Constraint::Percentage(30),
        Constraint::Percentage(70),
    ]).split(chunks[1]);

    render_scene_list(f, body[0], state);
    render_settings_panel(f, body[1], state);

    let hint = if state.image_edit_mode {
        "편집 중  │  Tab: 필드전환  │  Ctrl+S: 저장  │  Esc: 취소"
    } else {
        "e:편집  g:Global스타일  Enter:승인  Esc:이전"
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {hint}"),
            Style::default().fg(Color::DarkGray),
        ))),
        chunks[2],
    );

    // Global Style 팝업 오버레이
    if matches!(state.mode, crate::app::AppMode::ImageConfigGlobalPopup) {
        render_global_popup(f, state);
    }
}

fn render_scene_list(f: &mut Frame, area: Rect, state: &AppState) {
    let items: Vec<ListItem> = state.image_scenes.iter().enumerate().map(|(i, s)| {
        let selected = i == state.selected_scene;
        let prefix   = if selected { "▶ " } else { "  " };
        let check    = if !s.prompt.is_empty() { " ✓" } else { "" };
        let label: String = s.prompt.chars().take(18).collect();
        let text  = format!("{prefix}[{}] {}{}", s.id, label, check);
        let style = if selected {
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(GREEN)
        };
        ListItem::new(text).style(style)
    }).collect();

    let list = List::new(items)
        .block(Block::default().title(" Scenes ").borders(Borders::ALL)
            .border_type(BorderType::Plain).border_style(Style::default().fg(GREEN)));

    let mut ls = ListState::default();
    ls.select(Some(state.selected_scene));
    f.render_stateful_widget(list, area, &mut ls);
}

fn render_settings_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(" Image Settings ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(GREEN));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(scene) = state.image_scenes.get(state.selected_scene) else { return };

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(4),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(3),
    ]).split(inner);

    f.render_widget(
        Paragraph::new(Span::styled(
            format!("  Scene {} / {}", scene.id, state.image_scenes.len()),
            Style::default().fg(Color::DarkGray),
        )),
        chunks[0],
    );

    render_img_field(f, chunks[1], chunks[2], "Prompt",
        if state.image_edit_mode && state.image_field == ImageField::Prompt { Some(&state.image_edit_buf) } else { None },
        &scene.prompt,
        state.image_edit_mode && state.image_field == ImageField::Prompt,
    );

    render_img_field(f, chunks[3], chunks[4], "Style",
        if state.image_edit_mode && state.image_field == ImageField::Style { Some(&state.image_edit_buf) } else { None },
        &scene.style,
        state.image_edit_mode && state.image_field == ImageField::Style,
    );

    render_img_field(f, chunks[5], chunks[6], "Negative Prompt",
        if state.image_edit_mode && state.image_field == ImageField::Negative { Some(&state.image_edit_buf) } else { None },
        &scene.negative,
        state.image_edit_mode && state.image_field == ImageField::Negative,
    );
}

fn render_img_field(
    f: &mut Frame,
    label_area: Rect,
    box_area: Rect,
    label: &str,
    edit_buf: Option<&str>,
    display_val: &str,
    active: bool,
) {
    let border_style = if active { Style::default().fg(ORANGE) } else { Style::default().fg(GREEN) };

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

fn render_global_popup(f: &mut Frame, state: &AppState) {
    let area = f.area();
    let popup = centered_rect(70, 14, area);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Global Style ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(ORANGE));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ]).split(inner);

    f.render_widget(
        Paragraph::new(Span::styled("  모든 씬에 공통으로 적용될 스타일을 입력하세요", Style::default().fg(Color::DarkGray))),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(Span::styled("  Style (모든 프롬프트에 추가됨)", Style::default().fg(Color::DarkGray))),
        chunks[1],
    );

    let style_block = Block::default().borders(Borders::ALL)
        .border_type(BorderType::Plain).border_style(Style::default().fg(GREEN));
    let style_inner = style_block.inner(chunks[2]);
    f.render_widget(style_block, chunks[2]);
    f.render_widget(
        Paragraph::new(format!("{}\u{2588}", state.global_style_buf)).style(Style::default().fg(Color::White)),
        style_inner,
    );

    f.render_widget(
        Paragraph::new(Span::styled("  Global Negative", Style::default().fg(Color::DarkGray))),
        chunks[3],
    );

    let neg_block = Block::default().borders(Borders::ALL)
        .border_type(BorderType::Plain).border_style(Style::default().fg(Color::DarkGray));
    let neg_inner = neg_block.inner(chunks[4]);
    f.render_widget(neg_block, chunks[4]);
    f.render_widget(
        Paragraph::new(state.global_neg_buf.clone()).style(Style::default().fg(Color::White)),
        neg_inner,
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            "  Enter: 전체 씬 적용   Esc: 취소",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[6],
    );
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let vert = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ]).flex(Flex::Center).split(area);
    let horiz = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Percentage(percent_x),
        Constraint::Fill(1),
    ]).flex(Flex::Center).split(vert[1]);
    horiz[1]
}
