use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

const GREEN:  Color = Color::Rgb(0, 204, 102);
const ORANGE: Color = Color::Rgb(255, 140, 0);

pub struct NewJobForm {
    pub topic:     String,
    pub submitted: bool,
    pub cancelled: bool,
}

impl NewJobForm {
    pub fn new() -> Self {
        Self { topic: String::new(), submitted: false, cancelled: false }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if !self.topic.trim().is_empty() {
                    self.submitted = true;
                }
            }
            KeyCode::Esc => self.cancelled = true,
            KeyCode::Backspace => { self.topic.pop(); }
            KeyCode::Char(c)   => self.topic.push(c),
            _ => {}
        }
    }
}

pub fn render(f: &mut Frame, form: &NewJobForm) {
    let area = f.area();
    let popup = centered_rect(60, 9, area);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" New Job — 주제 입력 ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(ORANGE));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Fill(1),
    ]).split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  영상으로 만들 주제를 입력하세요",
            Style::default().fg(Color::DarkGray),
        ))),
        chunks[0],
    );

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(GREEN));
    let input_inner = input_block.inner(chunks[1]);
    f.render_widget(input_block, chunks[1]);

    let display = format!("{}\u{2588}", form.topic);
    f.render_widget(
        Paragraph::new(display).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        input_inner,
    );

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Enter: 시작   Esc: 취소",
            Style::default().fg(Color::DarkGray),
        ))),
        chunks[2],
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
