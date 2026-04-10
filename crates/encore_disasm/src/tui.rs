use std::io;
use std::io::stdout;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Terminal;

use crate::Disasm;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Left,
    Right,
}

struct App {
    focus: Panel,
    left_lines: Vec<String>,
    right_lines: Vec<String>,
    left_state: ListState,
    right_state: ListState,
}

impl App {
    fn new(disasm: &Disasm) -> Self {
        let mut left_lines = Vec::new();

        if !disasm.arity_table.is_empty() {
            left_lines.push(format!(
                "── Arity table ({} entries) ──",
                disasm.arity_table.len()
            ));
            for &(tag, arity) in &disasm.arity_table {
                left_lines.push(format!("  tag {tag}: arity {arity}"));
            }
            left_lines.push(String::new());
        }

        left_lines.push(format!("── Globals ({} entries) ──", disasm.globals.len()));
        for (idx, desc) in &disasm.globals {
            left_lines.push(format!("  g{idx} = {desc}"));
        }

        let mut right_lines = Vec::new();
        for instr in &disasm.instructions {
            if let Some(label) = &instr.label {
                right_lines.push(String::new());
                right_lines.push(format!("<{label}>:"));
            }
            let op_str = instr.op.to_string();
            if let Some(comment) = &instr.comment {
                right_lines.push(format!("{:04x}:  {:<30} ; {comment}", instr.addr, op_str));
            } else {
                right_lines.push(format!("{:04x}:  {}", instr.addr, op_str));
            }
        }

        let mut left_state = ListState::default();
        left_state.select(Some(0));
        let mut right_state = ListState::default();
        right_state.select(Some(0));

        Self {
            focus: Panel::Right,
            left_lines,
            right_lines,
            left_state,
            right_state,
        }
    }

    fn focused_state(&mut self) -> (&mut ListState, usize) {
        match self.focus {
            Panel::Left => (&mut self.left_state, self.left_lines.len()),
            Panel::Right => (&mut self.right_state, self.right_lines.len()),
        }
    }

    fn scroll_up(&mut self) {
        let (state, _) = self.focused_state();
        let i = state.selected().unwrap_or(0);
        state.select(Some(i.saturating_sub(1)));
    }

    fn scroll_down(&mut self) {
        let (state, len) = self.focused_state();
        let i = state.selected().unwrap_or(0);
        if i + 1 < len {
            state.select(Some(i + 1));
        }
    }

    fn page_up(&mut self, page_size: usize) {
        let (state, _) = self.focused_state();
        let i = state.selected().unwrap_or(0);
        state.select(Some(i.saturating_sub(page_size)));
    }

    fn page_down(&mut self, page_size: usize) {
        let (state, len) = self.focused_state();
        let i = state.selected().unwrap_or(0);
        state.select(Some((i + page_size).min(len.saturating_sub(1))));
    }
}

pub fn run(disasm: Disasm) -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&disasm);

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::horizontal([
                Constraint::Percentage(30),
                Constraint::Percentage(70),
            ])
            .split(frame.area());

            let left_border_style = if app.focus == Panel::Left {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let right_border_style = if app.focus == Panel::Right {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let left_block = Block::default()
                .title(" Globals ")
                .borders(Borders::ALL)
                .border_style(left_border_style);

            let right_block = Block::default()
                .title(format!(" Code ({} bytes) ", disasm.code_len))
                .borders(Borders::ALL)
                .border_style(right_border_style);

            let highlight_style = Style::default().add_modifier(Modifier::REVERSED);

            let left_items: Vec<ListItem> = app
                .left_lines
                .iter()
                .map(|s| ListItem::new(Line::raw(s.as_str())))
                .collect();

            let left_list = List::new(left_items)
                .block(left_block)
                .highlight_style(if app.focus == Panel::Left {
                    highlight_style
                } else {
                    Style::default()
                });

            let right_items: Vec<ListItem> = app
                .right_lines
                .iter()
                .map(|s| ListItem::new(Line::raw(s.as_str())))
                .collect();

            let right_list = List::new(right_items)
                .block(right_block)
                .highlight_style(if app.focus == Panel::Right {
                    highlight_style
                } else {
                    Style::default()
                });

            frame.render_stateful_widget(left_list, chunks[0], &mut app.left_state);
            frame.render_stateful_widget(right_list, chunks[1], &mut app.right_state);
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let page_size = (terminal.size()?.height as usize).saturating_sub(4);
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
                KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
                KeyCode::PageUp => app.page_up(page_size),
                KeyCode::PageDown => app.page_down(page_size),
                KeyCode::Left | KeyCode::Char('h') => app.focus = Panel::Left,
                KeyCode::Right | KeyCode::Char('l') => app.focus = Panel::Right,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
