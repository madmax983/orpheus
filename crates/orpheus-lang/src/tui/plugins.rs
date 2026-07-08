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
use crate::orca::{BANG, EMPTY, is_valid_glyph, playhead_frame};

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
        lines.push(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(state.display_input_with_cursor()),
        ]));
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

        let legend = [
            ("Undo", "Ctrl-Z / Ctrl-Y"),
            ("Space", "toggle (empty input)"),
            ("Open", ":open <path>"),
            ("Transport", ":play / :stop"),
            ("Mixer", ":track / :bus new|fx / :send / :mixer"),
            ("Set", ":tempo <bpm>"),
            ("Render", ":render <binding> <path> [cyc]"),
            ("Export", ":export <bind> <path> [cyc] | stems"),
            ("Import", ":import stems <dir>"),
            ("Analyze", ":roll / :stats / :explain"),
            ("Help", "?"),
        ];

        for (key, desc) in legend {
            lines.push(Line::from(vec![
                Span::styled(format!("{key:<9} "), key_style),
                Span::styled(format!(": {desc}"), desc_style),
            ]));
        }
        if let Some((message, is_error)) = &state.status_message {
            lines.push(Line::raw(""));
            let (prefix, bg, fg) = if *is_error {
                ("\u{2717} Failed", Color::Red, Color::White)
            } else {
                ("\u{2713} Success", Color::Green, Color::Black)
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {prefix} "),
                    Style::default().bg(bg).fg(fg).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {message} "),
                    Style::default().bg(Color::DarkGray).fg(Color::White),
                ),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled(format!("{:<9} ", "Quit"), key_style),
            Span::styled(": Esc or :quit", desc_style),
        ]));

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

// ---------------------------------------------------------------------------
// Orca Grid Plugin
// ---------------------------------------------------------------------------

/// TUI pane hosting the Orca grid surface.
///
/// The pane owns only view state (the edit cursor); the grid engine and its
/// publisher live in [`SharedState`] so the grid keeps sounding while the pane
/// is closed and re-publication happens on the shared TUI tick (ADR 0008).
pub struct OrcaPlugin {
    pub state: Rc<RefCell<SharedState>>,
    cursor_x: usize,
    cursor_y: usize,
}

impl OrcaPlugin {
    pub const fn new(state: Rc<RefCell<SharedState>>) -> Self {
        Self {
            state,
            cursor_x: 0,
            cursor_y: 0,
        }
    }

    #[cfg(test)]
    const fn cursor(&self) -> (usize, usize) {
        (self.cursor_x, self.cursor_y)
    }
}

/// Style for one grid glyph, mirroring Orca's visual grammar: empty cells
/// recede, uppercase (live) operators pop, lowercase (dormant) operators and
/// data stay muted, bangs flash.
fn orca_glyph_style(glyph: char) -> Style {
    match glyph {
        EMPTY => Style::default().fg(Color::DarkGray),
        BANG => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        ':' => Style::default().fg(Color::Magenta),
        '0'..='9' => Style::default().fg(Color::Cyan),
        glyph if glyph.is_ascii_uppercase() => Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().fg(Color::Gray),
    }
}

impl HypertilePlugin for OrcaPlugin {
    fn render(&self, area: Rect, buf: &mut Buffer, is_focused: bool) {
        let state = self.state.borrow();
        let snapshot = state.session.transport_snapshot();
        let grid = state.orca.engine().grid();

        let playhead = if state.orca.is_running() {
            playhead_frame(
                snapshot.current_frame(),
                snapshot.current_cycle_start_frame(),
                snapshot.frames_per_cycle(),
                state.orca.frames_per_cycle(),
            )
        } else {
            None
        };
        let playhead_column = playhead
            .and_then(|frame| usize::try_from(frame).ok())
            .filter(|frame| *frame < grid.width());

        let mut lines = Vec::with_capacity(grid.height() + 1);
        for y in 0..grid.height() {
            let mut spans = Vec::with_capacity(grid.width());
            for x in 0..grid.width() {
                let glyph = grid.glyph_at(x, y).unwrap_or(EMPTY);
                let mut style = orca_glyph_style(glyph);
                if playhead_column == Some(x) {
                    style = style.bg(Color::DarkGray);
                }
                if is_focused && (x, y) == (self.cursor_x, self.cursor_y) {
                    style = Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD);
                }
                spans.push(Span::styled(glyph.to_string(), style));
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::styled(
            "Space run/stop   arrows move   glyph write   Bksp erase",
            Style::default().fg(Color::DarkGray),
        ));

        let status = if state.orca.is_running() {
            "running"
        } else {
            "stopped"
        };
        let title = format!(
            "Orca {}x{} f {}/{} [{status}]",
            grid.width(),
            grid.height(),
            playhead.unwrap_or(0),
            state.orca.frames_per_cycle(),
        );

        let mut block = Block::default().title(title).borders(Borders::ALL);
        if is_focused {
            block = block.border_style(Style::default().fg(Color::Yellow).bold());
        }

        Paragraph::new(Text::from(lines))
            .block(block)
            .render(area, buf);
    }

    fn on_event(&mut self, event: &HypertileEvent) -> EventOutcome {
        let HypertileEvent::Key(chord) = event else {
            return EventOutcome::Ignored;
        };
        if !chord.modifiers.is_empty() {
            return EventOutcome::Ignored;
        }

        let (width, height) = {
            let state = self.state.borrow();
            let grid = state.orca.engine().grid();
            (grid.width(), grid.height())
        };

        match chord.code {
            KeyCode::Left => self.cursor_x = self.cursor_x.saturating_sub(1),
            KeyCode::Right => self.cursor_x = (self.cursor_x + 1).min(width - 1),
            KeyCode::Up => self.cursor_y = self.cursor_y.saturating_sub(1),
            KeyCode::Down => self.cursor_y = (self.cursor_y + 1).min(height - 1),
            KeyCode::Backspace | KeyCode::Delete => {
                self.state.borrow_mut().orca.engine_mut().grid_mut().set(
                    self.cursor_x,
                    self.cursor_y,
                    EMPTY,
                );
            }
            KeyCode::Char(' ') => self.state.borrow_mut().toggle_orca_running(),
            KeyCode::Char(glyph) if is_valid_glyph(glyph) => {
                self.state.borrow_mut().orca.engine_mut().grid_mut().set(
                    self.cursor_x,
                    self.cursor_y,
                    glyph,
                );
            }
            _ => return EventOutcome::Ignored,
        }
        EventOutcome::Consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orpheus_dsp::EngineHandle;
    use ratatui_hypertile::KeyChord;

    fn orca_plugin() -> OrcaPlugin {
        OrcaPlugin::new(Rc::new(RefCell::new(
            SharedState::new(EngineHandle::stub()),
        )))
    }

    fn key(code: KeyCode) -> HypertileEvent {
        HypertileEvent::Key(KeyChord::new(code))
    }

    fn glyph_at(plugin: &OrcaPlugin, x: usize, y: usize) -> Option<char> {
        plugin.state.borrow().orca.engine().grid().glyph_at(x, y)
    }

    #[test]
    fn orca_cursor_moves_with_arrows_and_clamps_at_edges() {
        let mut plugin = orca_plugin();
        assert_eq!(plugin.cursor(), (0, 0));

        assert_eq!(plugin.on_event(&key(KeyCode::Left)), EventOutcome::Consumed);
        assert_eq!(plugin.on_event(&key(KeyCode::Up)), EventOutcome::Consumed);
        assert_eq!(plugin.cursor(), (0, 0), "clamped at the top-left corner");

        plugin.on_event(&key(KeyCode::Right));
        plugin.on_event(&key(KeyCode::Down));
        assert_eq!(plugin.cursor(), (1, 1));

        for _ in 0..64 {
            plugin.on_event(&key(KeyCode::Right));
            plugin.on_event(&key(KeyCode::Down));
        }
        assert_eq!(
            plugin.cursor(),
            (15, 7),
            "clamped at the bottom-right corner"
        );
    }

    #[test]
    fn orca_typing_a_valid_glyph_writes_at_the_cursor() {
        let mut plugin = orca_plugin();
        plugin.on_event(&key(KeyCode::Right));

        let outcome = plugin.on_event(&key(KeyCode::Char('E')));
        assert_eq!(outcome, EventOutcome::Consumed);
        assert_eq!(glyph_at(&plugin, 1, 0), Some('E'));
        assert_eq!(plugin.cursor(), (1, 0), "writing does not move the cursor");
    }

    #[test]
    fn orca_backspace_and_delete_erase_the_cell() {
        let mut plugin = orca_plugin();
        plugin.on_event(&key(KeyCode::Char('D')));
        assert_eq!(glyph_at(&plugin, 0, 0), Some('D'));

        plugin.on_event(&key(KeyCode::Backspace));
        assert_eq!(glyph_at(&plugin, 0, 0), Some('.'));

        plugin.on_event(&key(KeyCode::Char('C')));
        plugin.on_event(&key(KeyCode::Delete));
        assert_eq!(glyph_at(&plugin, 0, 0), Some('.'));
    }

    #[test]
    fn orca_invalid_glyphs_are_ignored() {
        let mut plugin = orca_plugin();
        assert_eq!(
            plugin.on_event(&key(KeyCode::Char('@'))),
            EventOutcome::Ignored
        );
        assert_eq!(glyph_at(&plugin, 0, 0), Some('.'));
        assert_eq!(plugin.on_event(&key(KeyCode::Enter)), EventOutcome::Ignored);
    }

    #[test]
    fn orca_comment_glyph_is_insertable() {
        let mut plugin = orca_plugin();
        assert_eq!(
            plugin.on_event(&key(KeyCode::Char('#'))),
            EventOutcome::Consumed
        );
        assert_eq!(glyph_at(&plugin, 0, 0), Some('#'));
    }

    #[test]
    fn orca_modified_keys_pass_through() {
        let mut plugin = orca_plugin();
        let chord = KeyChord::with_modifiers(KeyCode::Char('e'), Modifiers::CTRL);
        assert_eq!(
            plugin.on_event(&HypertileEvent::Key(chord)),
            EventOutcome::Ignored
        );
        assert_eq!(glyph_at(&plugin, 0, 0), Some('.'));
    }

    #[test]
    fn orca_space_toggles_the_grid_clock_and_publishes() {
        let mut plugin = orca_plugin();
        assert!(!plugin.state.borrow().orca.is_running());

        plugin.on_event(&key(KeyCode::Char(' ')));
        {
            let state = plugin.state.borrow();
            assert!(state.orca.is_running());
            assert!(
                state
                    .session
                    .binding_summaries()
                    .iter()
                    .any(|summary| summary == "orca: Pattern<Sample>"),
                "starting the grid publishes its first cycle immediately"
            );
        }

        plugin.on_event(&key(KeyCode::Char(' ')));
        assert!(!plugin.state.borrow().orca.is_running());
    }

    #[test]
    fn orca_pane_renders_grid_playhead_and_status() {
        let plugin = orca_plugin();
        let area = Rect::new(0, 0, 30, 12);
        let mut buf = Buffer::empty(area);
        plugin.render(area, &mut buf, false);
        let rendered = crate::tui::buffer_to_string(&buf);

        assert!(rendered.contains("Orca 16x8 f 0/16 [stopped]"));
        assert!(
            rendered.contains("................"),
            "an empty grid row of 16 cells is visible"
        );
        assert!(rendered.contains("Space run/stop"));

        plugin
            .state
            .borrow_mut()
            .orca
            .engine_mut()
            .grid_mut()
            .set(2, 1, 'E');
        let mut buf = Buffer::empty(area);
        plugin.render(area, &mut buf, false);
        let rendered = crate::tui::buffer_to_string(&buf);
        assert!(rendered.contains("..E............."));
    }
}
