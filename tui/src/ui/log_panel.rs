use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
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

    let height    = inner.height as usize;
    let prefix_w  = 2usize; // "✕ "
    let msg_width = (inner.width as usize).saturating_sub(prefix_w).max(1);

    // 최신 로그부터 역순으로 시각적 줄 수 누적, height 만큼 채울 항목 선택
    let mut selected: Vec<&LogEntry> = Vec::new();
    let mut used = 0usize;
    for entry in logs.iter().rev() {
        let visual = ((entry.message.chars().count() + msg_width - 1) / msg_width).max(1);
        if used + visual > height { break; }
        used += visual;
        selected.push(entry);
    }
    selected.reverse();

    let lines: Vec<Line> = selected.iter().map(|e| {
        let (prefix, color) = match e.level.as_str() {
            "error" => ("✕", Color::Red),
            "warn"  => ("!", Color::Yellow),
            _       => (">", GREEN),
        };
        Line::from(vec![
            Span::styled(format!("{prefix} "), Style::default().fg(color)),
            Span::styled(e.message.clone(), Style::default().fg(Color::White)),
        ])
    }).collect();

    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
