use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

// ── 온보딩 상태 ───────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardingStep { Anthropic, OpenAI, Nanobanana, YouTube, ComfyUI, Done }

pub struct OnboardingState {
    pub step:      OnboardingStep,
    pub inputs:    [String; 5],  // [anthropic, openai, nanobanana, youtube, comfyui]
    pub input_buf: String,
    pub cancelled: bool,
}

impl OnboardingState {
    pub fn new() -> Self {
        Self {
            step:      OnboardingStep::Anthropic,
            inputs:    Default::default(),
            input_buf: String::new(),
            cancelled: false,
        }
    }

    fn current_idx(&self) -> usize {
        match self.step {
            OnboardingStep::Anthropic  => 0,
            OnboardingStep::OpenAI     => 1,
            OnboardingStep::Nanobanana => 2,
            OnboardingStep::YouTube    => 3,
            OnboardingStep::ComfyUI    => 4,
            OnboardingStep::Done       => 5,
        }
    }

    fn is_required(idx: usize) -> bool { idx < 3 }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.advance(false),
            KeyCode::Tab   => self.advance(true),  // 선택항목 스킵
            KeyCode::Esc   => self.go_back(),
            KeyCode::Backspace => { self.input_buf.pop(); }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                    self.cancelled = true;
                } else {
                    self.input_buf.push(c);
                }
            }
            _ => {}
        }
    }

    fn advance(&mut self, skip: bool) {
        let idx = self.current_idx();
        if idx >= 5 { return; }

        // 필수 항목은 스킵 불가
        if skip && Self::is_required(idx) { return; } // is_required is an associated fn

        // 값 저장 (스킵이 아닐 때만)
        if !skip {
            self.inputs[idx] = self.input_buf.clone();
        }
        self.input_buf.clear();

        self.step = match idx {
            0 => OnboardingStep::OpenAI,
            1 => OnboardingStep::Nanobanana,
            2 => OnboardingStep::YouTube,
            3 => OnboardingStep::ComfyUI,
            4 => OnboardingStep::Done,
            _ => OnboardingStep::Done,
        };
    }

    fn go_back(&mut self) {
        let idx = self.current_idx();
        if idx == 0 { return; }
        self.input_buf = self.inputs[idx.saturating_sub(1)].clone();
        self.step = match idx {
            1 => OnboardingStep::Anthropic,
            2 => OnboardingStep::OpenAI,
            3 => OnboardingStep::Nanobanana,
            4 => OnboardingStep::YouTube,
            _ => return,
        };
    }
}

// ── 렌더링 ────────────────────────────────────────────────────────────────────

const GREEN:  Color = Color::Rgb(0, 204, 102);
const ORANGE: Color = Color::Rgb(255, 140, 0);

static ITEMS: &[(&str, &str, bool, &str)] = &[
    ("Anthropic API Key",                  "Claude 스크립트 생성용 — https://console.anthropic.com/",             true,  "sk-ant-..."),
    ("OpenAI API Key",                     "TTS(음성 합성)용 — https://platform.openai.com/api-keys",            true,  "sk-..."),
    ("nanobanana API Key",                 "이미지 생성용 — https://nanobanana.io/",                             true,  "nb-..."),
    ("YouTube OAuth (client_secrets.json 경로)", "선택 사항 — 파일 경로 입력 또는 Tab으로 건너뜀",             false, "./client_secrets.json"),
    ("ComfyUI Host URL",                   "선택 사항 — 로컬 ComfyUI 주소 또는 Tab으로 건너뜀",                false, "http://127.0.0.1:8188"),
];

pub fn render(f: &mut Frame, state: &OnboardingState) {
    let area = f.area();
    // 중앙 정렬된 박스
    let vert = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(28.min(area.height)),
        Constraint::Fill(1),
    ]).split(area);
    let horiz = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Max(72),
        Constraint::Fill(1),
    ]).split(vert[1]);
    let box_area = horiz[1];

    if matches!(state.step, OnboardingStep::Done) {
        render_done(f, box_area, state);
    } else {
        render_step(f, box_area, state);
    }
}

fn render_step(f: &mut Frame, area: Rect, state: &OnboardingState) {
    let idx = state.current_idx();
    let total = 5usize;

    let outer = Block::default()
        .title(format!(" AYG Setup  [{} / {}] ", idx + 1, total))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(GREEN));
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let chunks = Layout::vertical([
        Constraint::Length(total as u16 + 2), // 항목 목록
        Constraint::Length(1),                // 여백
        Constraint::Length(5),                // 입력 박스
        Constraint::Length(1),                // 설명
        Constraint::Length(1),                // URL
        Constraint::Fill(1),
        Constraint::Length(1),                // 키 힌트
    ]).split(inner);

    // 항목 목록
    render_item_list(f, chunks[0], state, idx);

    // 입력 박스
    let (label, _desc, _required, placeholder) = ITEMS[idx];
    render_input_box(f, chunks[2], label, &state.input_buf, placeholder, idx);

    // 설명
    let (_l, desc, required, _ph) = ITEMS[idx];
    let hint = if required { "" } else { " (선택)" };
    let desc_line = Line::from(vec![
        Span::styled(format!("  {desc}{hint}"), Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(desc_line), chunks[3]);

    // 키 힌트
    let skip_hint = if !is_required_idx(idx) { "  Tab: skip" } else { "" };
    let hint_line = Line::from(Span::styled(
        format!("  Enter: next   Esc: back{skip_hint}   Ctrl+C: quit"),
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(Paragraph::new(hint_line), chunks[6]);
}

fn is_required_idx(idx: usize) -> bool { idx < 3 }

fn render_item_list(f: &mut Frame, area: Rect, state: &OnboardingState, current: usize) {
    let mut lines = vec![];
    for (i, (label, _, _, _)) in ITEMS.iter().enumerate() {
        let done   = i < current;
        let active = i == current;

        let (icon, icon_style) = if done {
            ("✓", Style::default().fg(Color::Green))
        } else if active {
            ("▶", Style::default().fg(ORANGE))
        } else {
            ("○", Style::default().fg(Color::DarkGray))
        };

        let value = if done {
            let v = &state.inputs[i];
            if v.is_empty() { "skipped".to_string() } else { mask_key(v) }
        } else {
            String::new()
        };

        let label_style = if active {
            Style::default().fg(ORANGE).add_modifier(Modifier::BOLD)
        } else if done {
            Style::default().fg(GREEN)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        lines.push(Line::from(vec![
            Span::styled(format!("  {icon} "), icon_style),
            Span::styled(format!("{:<35}", label), label_style),
            Span::styled(value, Style::default().fg(Color::DarkGray)),
        ]));
    }
    f.render_widget(Paragraph::new(lines), area);
}

fn render_input_box(f: &mut Frame, area: Rect, label: &str, value: &str, _placeholder: &str, idx: usize) {
    let masked = if idx < 3 { mask_input(value) } else { value.to_string() };
    let display = format!("{masked}\u{2588}"); // █ cursor

    let block = Block::default()
        .title(format!(" {label} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(ORANGE));
    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(
        Paragraph::new(display).style(Style::default().fg(Color::White)),
        inner,
    );
}

fn render_done(f: &mut Frame, area: Rect, state: &OnboardingState) {
    let block = Block::default()
        .title(" AYG Setup — Complete! ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(GREEN));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![Line::from("")];
    for (i, (label, _, _, _)) in ITEMS.iter().enumerate() {
        let v = &state.inputs[i];
        let status = if v.is_empty() { "skipped".to_string() } else { "saved".to_string() };
        lines.push(Line::from(vec![
            Span::styled("  ✓ ", Style::default().fg(Color::Green)),
            Span::styled(format!("{:<35}", label), Style::default().fg(GREEN)),
            Span::styled(status, Style::default().fg(Color::DarkGray)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  config.yaml created at ./config.yaml", Style::default().fg(Color::DarkGray))));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  You're all set! Run AYG with:", Style::default().fg(GREEN))));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("       ./ayg run", Style::default().fg(ORANGE).add_modifier(Modifier::BOLD))));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  Enter: launch AYG now   q: quit", Style::default().fg(Color::DarkGray))));

    f.render_widget(Paragraph::new(lines), inner);
}

fn mask_key(s: &str) -> String {
    if s.len() <= 8 { return "••••••••".into(); }
    let prefix: String = s.chars().take(6).collect();
    format!("{}••••••••", prefix)
}

fn mask_input(s: &str) -> String {
    "•".repeat(s.len())
}

// ── 온보딩 실행 루프 ──────────────────────────────────────────────────────────
pub async fn run(terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>) -> anyhow::Result<()> {
    let mut state = OnboardingState::new();

    loop {
        terminal.draw(|f| render(f, &state))?;

        if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
            if key.kind == KeyEventKind::Press {
                state.handle_key(key);
            }
        }

        if state.cancelled {
            return Ok(());
        }

        if matches!(state.step, OnboardingStep::Done) {
            // config.yaml 저장
            crate::config::write_initial_config(
                &state.inputs[0],
                &state.inputs[1],
                &state.inputs[2],
                &state.inputs[3],
                &state.inputs[4],
            )?;

            // 마지막 화면 렌더링 후 Enter 대기
            terminal.draw(|f| render(f, &state))?;
            loop {
                if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Enter => {
                                // ayg run 모드로 전환 — main에서 처리
                                return Ok(());
                            }
                            KeyCode::Char('q') => return Ok(()),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
