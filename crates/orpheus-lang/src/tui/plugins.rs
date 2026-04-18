use std::cell::{Cell, RefCell};
use std::rc::Rc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Widget;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui_hypertile::{EventOutcome, HypertileEvent, KeyCode, Modifiers};
use ratatui_hypertile_extras::HypertilePlugin;

use super::state::SharedState;
use super::style::{
    binding_legend_item, binding_list_item, routing_status_line, should_show_binding_legend,
    transport_status_line,
};

// ---------------------------------------------------------------------------
// REPL Plugin
// ---------------------------------------------------------------------------

pub struct ReplPlugin {
    pub state: Rc<RefCell<SharedState>>,
}

impl HypertilePlugin for ReplPlugin {
    fn render(&self, area: Rect, buf: &mut Buffer, is_focused: bool) {
        let state = self.state.borrow();

        let mut lines = state
            .transcript
            .iter()
            .flat_map(|entry| {
                let style = if entry.starts_with("> ") {
                    Style::default().fg(Color::DarkGray)
                } else if entry.starts_with("\u{2717} ") {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else if entry.starts_with("\u{26a0}\u{fe0f} ") {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else if entry.starts_with("\u{2713} ") {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };
                entry
                    .split('\n')
                    .map(move |line| Line::styled(line.to_owned(), style))
            })
            .collect::<Vec<_>>();

        let transport = state.transport_view();
        lines.push(transport_status_line("Transport: ", &transport, true));
        lines.push(Line::raw(format!(
            "> {}",
            state.display_input_with_cursor()
        )));
        lines.push(Line::styled(
            state.input_hint(),
            Style::default().fg(Color::DarkGray),
        ));

        let mut block = Block::default().title("REPL").borders(Borders::ALL);
        if is_focused {
            block = block.border_style(Style::default().fg(Color::Yellow).bold());
        }

        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }

    fn on_event(&mut self, event: &HypertileEvent) -> EventOutcome {
        let HypertileEvent::Key(chord) = event else {
            return EventOutcome::Ignored;
        };

        let mut state = self.state.borrow_mut();

        // Ctrl combos
        if chord.modifiers == Modifiers::CTRL {
            match chord.code {
                KeyCode::Char('a') => state.move_cursor_home(),
                KeyCode::Char('d') => state.delete(),
                KeyCode::Char('e') => state.move_cursor_end(),
                KeyCode::Char('k') => state.kill_to_end(),
                KeyCode::Char('l') => state.clear_transcript(),
                KeyCode::Char('u') => state.kill_to_start(),
                KeyCode::Char('w') => state.delete_previous_word(),
                _ => return EventOutcome::Ignored,
            }
            return EventOutcome::Consumed;
        }

        // Alt combos
        if chord.modifiers == Modifiers::ALT {
            match chord.code {
                KeyCode::Char('b') => state.move_cursor_previous_word(),
                KeyCode::Char('f') => state.move_cursor_next_word(),
                _ => return EventOutcome::Ignored,
            }
            return EventOutcome::Consumed;
        }

        // No modifiers
        if !chord.modifiers.is_empty() {
            return EventOutcome::Ignored;
        }

        match chord.code {
            KeyCode::Enter => state.submit_line(),
            KeyCode::Backspace => state.backspace(),
            KeyCode::Delete => state.delete(),
            KeyCode::Left => state.move_cursor_left(),
            KeyCode::Right => state.move_cursor_right(),
            KeyCode::Home => state.move_cursor_home(),
            KeyCode::End => state.move_cursor_end(),
            KeyCode::Up => state.recall_previous_history(),
            KeyCode::Down => state.recall_next_history(),
            KeyCode::Tab => state.complete_input(),
            KeyCode::Char(' ') if state.input.is_empty() => state.toggle_transport_hotkey(),
            KeyCode::Char(c) => state.insert_character(c),
            _ => return EventOutcome::Ignored,
        }
        EventOutcome::Consumed
    }
}

// ---------------------------------------------------------------------------
// Bindings Plugin
// ---------------------------------------------------------------------------

pub struct BindingsPlugin {
    pub state: Rc<RefCell<SharedState>>,
    scroll: Cell<usize>,
    last_height: Cell<u16>,
}

impl BindingsPlugin {
    pub const fn new(state: Rc<RefCell<SharedState>>) -> Self {
        Self {
            state,
            scroll: Cell::new(0),
            last_height: Cell::new(0),
        }
    }

    fn binding_lines(&self, height: u16) -> Vec<ListItem<'static>> {
        let state = self.state.borrow();
        let transport = state.session.transport_view();
        let bindings = state.session.binding_summaries();
        if bindings.is_empty() {
            vec![ListItem::new("No bindings yet")]
        } else {
            let mut items = bindings
                .into_iter()
                .map(|summary| binding_list_item(summary, &transport))
                .collect::<Vec<_>>();
            if should_show_binding_legend(height, items.len(), &transport) {
                items.push(ListItem::new(""));
                items.push(binding_legend_item(&transport));
            }
            items
        }
    }

    fn viewport_rows(height: u16) -> usize {
        usize::from(height.saturating_sub(2)).max(1)
    }

    fn scroll_step(&self) -> usize {
        Self::viewport_rows(self.last_height.get())
            .saturating_sub(1)
            .max(1)
    }
}

impl HypertilePlugin for BindingsPlugin {
    fn render(&self, area: Rect, buf: &mut Buffer, is_focused: bool) {
        self.last_height.set(area.height);

        let items = self.binding_lines(area.height);
        let viewport_rows = Self::viewport_rows(area.height);
        let scroll = self.scroll.get();

        let (title, display_items) = if items.len() <= viewport_rows {
            ("Bindings".to_owned(), items)
        } else {
            let max_scroll = items.len().saturating_sub(viewport_rows);
            let offset = scroll.min(max_scroll);
            let start = offset + 1;
            let end = (offset + viewport_rows).min(items.len());
            (
                format!("Bindings {start}-{end}/{} PgUp/PgDn", items.len()),
                items.into_iter().skip(offset).take(viewport_rows).collect(),
            )
        };

        let mut block = Block::default().title(title).borders(Borders::ALL);
        if is_focused {
            block = block.border_style(Style::default().fg(Color::Yellow).bold());
        }

        List::new(display_items).block(block).render(area, buf);
    }

    fn on_event(&mut self, event: &HypertileEvent) -> EventOutcome {
        let HypertileEvent::Key(chord) = event else {
            return EventOutcome::Ignored;
        };
        if !chord.modifiers.is_empty() {
            return EventOutcome::Ignored;
        }
        match chord.code {
            KeyCode::PageUp => {
                let scroll = self.scroll.get();
                self.scroll.set(scroll.saturating_sub(self.scroll_step()));
                EventOutcome::Consumed
            }
            KeyCode::PageDown => {
                let height = self.last_height.get();
                let items = self.binding_lines(height);
                let max_scroll = items.len().saturating_sub(Self::viewport_rows(height));
                let scroll = self.scroll.get();
                self.scroll
                    .set(scroll.saturating_add(self.scroll_step()).min(max_scroll));
                EventOutcome::Consumed
            }
            _ => EventOutcome::Ignored,
        }
    }
}

// ---------------------------------------------------------------------------
// Transport Plugin
// ---------------------------------------------------------------------------

pub struct TransportPlugin {
    pub state: Rc<RefCell<SharedState>>,
}

impl HypertilePlugin for TransportPlugin {
    fn render(&self, area: Rect, buf: &mut Buffer, is_focused: bool) {
        let state = self.state.borrow();
        let transport = state.transport_view();
        let mixer = state.mixer_view();

        let mut lines = vec![Line::from(vec![
            Span::raw("Pattern: "),
            Span::styled(
                transport.active_pattern_name().unwrap_or("none"),
                crate::tui::style::live_binding_style(),
            ),
        ])];
        if let Some(pending_pattern_name) = transport.pending_pattern_name() {
            lines.push(Line::from(vec![
                Span::raw("Next: "),
                Span::styled(
                    pending_pattern_name.to_owned(),
                    crate::tui::style::pending_binding_style(&transport),
                ),
            ]));
        }
        lines.push(routing_status_line(&mixer));
        if !mixer.tui_summary().is_empty() {
            lines.push(Line::raw(""));
            for line in mixer.tui_summary() {
                lines.push(line.clone());
            }
        }
        let key_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(Color::DarkGray);

        lines.push(Line::from(vec![
            Span::styled("Space", key_style),
            Span::styled(": toggle (empty input)", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Open", key_style),
            Span::styled(": :open <path>", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Transport", key_style),
            Span::styled(": :play / :stop", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Mixer", key_style),
            Span::styled(": :track / :bus new|fx / :send / :mixer", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Set", key_style),
            Span::styled(": :tempo <bpm>", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Render", key_style),
            Span::styled(": :render <binding> <path> [cyc]", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Export", key_style),
            Span::styled(": :export <bind> <path> [cyc] | stems", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Analyze", key_style),
            Span::styled(": :roll / :stats / :explain", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Help", key_style),
            Span::styled(": ?", desc_style),
        ]));
        if let Some(message) = &state.status_message {
            if message.contains("error")
                || message.contains("failed")
                || message.contains("unknown")
                || message.contains("usage:")
            {
                lines.push(Line::styled(
                    format!("Note: \u{2717} {message}"),
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                lines.push(Line::styled(
                    format!("Note: \u{2713} {message}"),
                    Style::default().fg(Color::LightGreen),
                ));
            }
        }
        lines.push(Line::raw("Quit: Esc or :quit"));

        let mut block = Block::default().title("Transport").borders(Borders::ALL);
        if is_focused {
            block = block.border_style(Style::default().fg(Color::Yellow).bold());
        }

        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}
