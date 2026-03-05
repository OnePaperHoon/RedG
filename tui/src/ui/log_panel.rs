use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};
use crate::app::LogEntry;

const GREEN:  Color = Color::Rgb(0, 204, 102);

pub fn render(f: &mut Frame, area: Rect, logs: &std::collections::VecDeque<LogEntry>, title: &str) {
    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(GREEN));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let height = inner.height as usize;
    let lines: Vec<Line> = logs
        .iter()
        .rev()
        .take(height)
        .rev()
        .map(|e| {
            let (prefix, color) = match e.level.as_str() {
                "error" => ("✕", Color::Red),
                "warn"  => ("!", Color::Yellow),
                _       => (">", GREEN),
            };
            Line::from(vec![
                Span::styled(format!("{prefix} "), Style::default().fg(color)),
                Span::styled(e.message.clone(), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    f.render_widget(Paragraph::new(lines), inner);
}
