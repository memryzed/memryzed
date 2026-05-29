// Copyright 2026 Memryzed contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Review TUI for pending memories.
//!
//! `memryzed review` opens this full-screen interface. The user
//! walks the pending queue and approves, pins, or rejects each
//! candidate. Keys: up/down to move, a approve, p approve+pin,
//! r reject (archive), q quit.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::io;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use memryzed_core::clock::{format_epoch_iso, now_epoch_seconds};
use memryzed_core::embedder::{make_default, Embedder};
use memryzed_core::memory::{self, Memory};
use memryzed_core::{DataDir, Database};

/// Run the review TUI against the given data directory.
///
/// Returns the number of pending memories acted on. When the queue
/// is empty, prints a message and returns without entering the
/// alternate screen.
pub fn run(data_dir: &DataDir) -> Result<usize> {
    let mut db = Database::open(&data_dir.db_file())?;
    let pending = memory::list_pending(&db, None)?;
    if pending.is_empty() {
        println!("No pending memories to review.");
        return Ok(0);
    }
    let embedder = make_default(&data_dir.models_dir())?;

    let mut terminal = setup_terminal()?;
    let result = run_loop(&mut terminal, &mut db, embedder.as_ref(), pending);
    restore_terminal(&mut terminal)?;
    result
}

struct App {
    pending: Vec<Memory>,
    state: ListState,
    acted: usize,
    status_line: String,
}

impl App {
    fn new(pending: Vec<Memory>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            pending,
            state,
            acted: 0,
            status_line: String::from("a approve   p approve+pin   r reject   q quit"),
        }
    }

    fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    fn remove_selected(&mut self) {
        if let Some(i) = self.state.selected() {
            if i < self.pending.len() {
                self.pending.remove(i);
                let next = if self.pending.is_empty() {
                    None
                } else {
                    Some(i.min(self.pending.len() - 1))
                };
                self.state.select(next);
            }
        }
    }

    fn move_down(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        self.state.select(Some((i + 1) % self.pending.len()));
    }

    fn move_up(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let i = self.state.selected().unwrap_or(0);
        let n = self.pending.len();
        self.state.select(Some((i + n - 1) % n));
    }
}

fn run_loop(
    terminal: &mut Terminal<impl Backend>,
    db: &mut Database,
    embedder: &dyn Embedder,
    pending: Vec<Memory>,
) -> Result<usize> {
    let mut app = App::new(pending);
    loop {
        terminal.draw(|f| draw(f, &mut app))?;

        if app.pending.is_empty() {
            // Nothing left; redraw once then exit on any key.
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    break;
                }
            }
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Down | KeyCode::Char('j') => app.move_down(),
            KeyCode::Up | KeyCode::Char('k') => app.move_up(),
            KeyCode::Char('a') => act_approve(db, embedder, &mut app, false),
            KeyCode::Char('p') => act_approve(db, embedder, &mut app, true),
            KeyCode::Char('r') => act_reject(db, &mut app),
            _ => {}
        }
    }
    Ok(app.acted)
}

fn act_approve(db: &mut Database, embedder: &dyn Embedder, app: &mut App, pin: bool) {
    let Some(i) = app.selected() else { return };
    let id = app.pending[i].id.clone();
    match memory::approve(db, &id, pin, embedder, now_epoch_seconds()) {
        Ok(_) => {
            app.acted += 1;
            app.status_line = format!("{} {}", if pin { "Pinned" } else { "Approved" }, short(&id));
            app.remove_selected();
        }
        Err(e) => app.status_line = format!("error: {e}"),
    }
}

fn act_reject(db: &mut Database, app: &mut App) {
    let Some(i) = app.selected() else { return };
    let id = app.pending[i].id.clone();
    match memory::archive(db, &id, now_epoch_seconds()) {
        Ok(_) => {
            app.acted += 1;
            app.status_line = format!("Rejected {}", short(&id));
            app.remove_selected();
        }
        Err(e) => app.status_line = format!("error: {e}"),
    }
}

fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(6),
            Constraint::Length(1),
        ])
        .split(f.area());

    let header = Paragraph::new(format!("Memryzed review - {} pending", app.pending.len()))
        .style(Style::default().add_modifier(Modifier::BOLD));
    f.render_widget(header, chunks[0]);

    let items: Vec<ListItem> = app
        .pending
        .iter()
        .map(|m| {
            let conf = m
                .confidence
                .map(|c| format!("{c:.2}"))
                .unwrap_or_else(|| "-".into());
            ListItem::new(format!(
                "[{}] {:<11} {}",
                conf,
                m.scope.as_db_str(),
                truncate(&m.content, 60)
            ))
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Pending"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, chunks[1], &mut app.state);

    let detail_text = match app.selected().and_then(|i| app.pending.get(i)) {
        Some(m) => format!(
            "scope: {}\nkind: {}\nconfidence: {}\ncreated: {}\n\n{}",
            m.scope.as_db_str(),
            m.kind.as_db_str(),
            m.confidence
                .map(|c| format!("{c:.2}"))
                .unwrap_or_else(|| "-".into()),
            format_epoch_iso(m.created_at),
            m.content,
        ),
        None => String::from("queue empty - press any key to exit"),
    };
    let detail = Paragraph::new(detail_text)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Detail"));
    f.render_widget(detail, chunks[2]);

    let footer =
        Paragraph::new(app.status_line.clone()).style(Style::default().add_modifier(Modifier::DIM));
    f.render_widget(footer, chunks[3]);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use memryzed_core::memory::{insert_pending, NewMemory, Scope};

    #[test]
    fn app_navigation_wraps() {
        let db = Database::open_in_memory().unwrap();
        insert_pending(&db, NewMemory::new(Scope::Global, "a"), 1).unwrap();
        insert_pending(&db, NewMemory::new(Scope::Global, "b"), 2).unwrap();
        let pending = memory::list_pending(&db, None).unwrap();
        let mut app = App::new(pending);
        assert_eq!(app.selected(), Some(0));
        app.move_up();
        assert_eq!(app.selected(), Some(1), "up wraps to last");
        app.move_down();
        assert_eq!(app.selected(), Some(0), "down wraps to first");
    }

    #[test]
    fn remove_selected_shrinks_and_reselects() {
        let db = Database::open_in_memory().unwrap();
        for c in ["a", "b", "c"] {
            insert_pending(&db, NewMemory::new(Scope::Global, c), 1).unwrap();
        }
        let pending = memory::list_pending(&db, None).unwrap();
        let mut app = App::new(pending);
        app.remove_selected();
        assert_eq!(app.pending.len(), 2);
        assert_eq!(app.selected(), Some(0));
    }

    #[test]
    fn truncate_adds_ellipsis() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world this is long", 10), "hello w...");
    }
}
