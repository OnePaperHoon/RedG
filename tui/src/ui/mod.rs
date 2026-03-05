pub mod dashboard;
pub mod image_config;
pub mod log_panel;
pub mod new_job_form;
pub mod onboarding;
pub mod script_review;

use ratatui::Frame;
use crate::app::{AppMode, AppState};

pub fn render(f: &mut Frame, state: &AppState) {
    match &state.mode {
        AppMode::Dashboard => dashboard::render(f, state),
        AppMode::NewJob    => {
            dashboard::render(f, state);
            // NewJob form은 dashboard 위에 팝업으로 표시 — AppState에서 form 상태를 읽음
            render_new_job_overlay(f, state);
        }
        AppMode::ScriptReview => script_review::render(f, state),
        AppMode::ImageConfig | AppMode::ImageConfigGlobalPopup => image_config::render(f, state),
        AppMode::Confirm(_) => {
            dashboard::render(f, state);
            render_confirm_overlay(f, state);
        }
    }
}

fn render_new_job_overlay(f: &mut Frame, state: &AppState) {
    // new_job_form은 state.new_job_input을 참조해 렌더링
    let form = new_job_form::NewJobForm {
        topic:     state.new_job_input.clone(),
        submitted: false,
        cancelled: false,
    };
    new_job_form::render(f, &form);
}

fn render_confirm_overlay(f: &mut Frame, state: &AppState) {
    use ratatui::{
        layout::{Constraint, Flex, Layout},
        style::{Color, Style},
        text::{Line, Span},
        widgets::{Block, BorderType, Borders, Clear, Paragraph},
    };

    let AppMode::Confirm(action) = &state.mode else { return };
    let msg = match action {
        crate::app::ConfirmAction::DeleteJob(id) => format!("Job '{id}'을 삭제할까요?"),
        crate::app::ConfirmAction::DeleteScene(i) => format!("Scene {}을 삭제할까요?", i + 1),
    };

    let area = f.area();
    let vert = Layout::vertical([Constraint::Fill(1), Constraint::Length(7), Constraint::Fill(1)])
        .flex(Flex::Center).split(area);
    let horiz = Layout::horizontal([Constraint::Fill(1), Constraint::Max(50), Constraint::Fill(1)])
        .flex(Flex::Center).split(vert[1]);
    let popup = horiz[1];

    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" 확인 ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(format!("  {msg}"), Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(Span::styled("  y: 삭제   n/Esc: 취소", Style::default().fg(Color::DarkGray))),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}
