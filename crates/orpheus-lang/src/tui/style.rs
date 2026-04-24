//! Styling and formatting utilities for the Terminal User Interface (TUI).
//!
//! This module provides functions for formatting text, computing status colors,
//! and building UI components (like `ratatui` Lines and Spans) to ensure a
//! consistent visual language across the interactive session view.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::session::{MixerView, TransportView};

const MIN_BINDING_LEGEND_ROWS: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Represents the high-level playback state of the audio engine as displayed in the UI.
///
/// This state combines the low-level engine transport snapshot with the REPL's
/// pattern queue to distinguish between patterns that are actively playing,
/// patterns queued to start at the next cycle, and a fully stopped engine.
pub enum UiTransportState {
    /// The engine is running and actively rendering the current pattern.
    Playing,
    /// The engine is stopped and producing no audio.
    Stopped,
    /// The engine is playing, but a new pattern is queued and waiting for the next cycle boundary.
    Syncing,
    /// The engine is stopped, but a pattern has been queued and will start when playback resumes.
    Queued,
}

/// Computes the overall UI transport state from a transport snapshot view.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::{transport_state, UiTransportState};
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// assert_eq!(transport_state(&view), UiTransportState::Stopped);
/// ```
pub fn transport_state(view: &TransportView) -> UiTransportState {
    if view.pending_pattern_name().is_some() {
        if view.snapshot().has_pending_pattern() && view.snapshot().is_playing() {
            UiTransportState::Syncing
        } else {
            UiTransportState::Queued
        }
    } else if view.snapshot().is_playing() {
        UiTransportState::Playing
    } else {
        UiTransportState::Stopped
    }
}

/// Returns a human-readable string representation of the current transport state.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::format_transport_status;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// assert_eq!(format_transport_status(&view), "stopped");
/// ```
pub fn format_transport_status(view: &TransportView) -> &'static str {
    match transport_state(view) {
        UiTransportState::Playing => "playing",
        UiTransportState::Stopped => "stopped",
        UiTransportState::Syncing => "syncing",
        UiTransportState::Queued => "queued",
    }
}

/// Returns the appropriate `ratatui` text style based on the current transport state.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::transport_status_style;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// let style = transport_status_style(&view);
/// ```
pub fn transport_status_style(view: &TransportView) -> Style {
    let color = match transport_state(view) {
        UiTransportState::Playing => Color::Green,
        UiTransportState::Stopped => Color::Yellow,
        UiTransportState::Syncing => Color::Cyan,
        UiTransportState::Queued => Color::Blue,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

/// Builds a formatted `Line` displaying the transport status and optionally the pending pattern target.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::transport_status_line;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// let line = transport_status_line("Status: ", &view, false);
/// ```
pub fn transport_status_line(
    prefix: &'static str,
    view: &TransportView,
    include_target: bool,
) -> Line<'static> {
    let mut spans = vec![
        Span::raw(prefix),
        Span::styled(format_transport_status(view), transport_status_style(view)),
    ];
    if include_target && let Some(pending_pattern_name) = view.pending_pattern_name() {
        spans.push(Span::raw(" -> "));
        spans.push(Span::raw(pending_pattern_name.to_owned()));
    }
    Line::from(spans)
}

/// Builds a formatted `Line` indicating whether there are pending mixer routing updates.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::routing_status_line;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.mixer_view();
/// let line = routing_status_line(&view);
/// ```
pub fn routing_status_line(mixer: &MixerView) -> Line<'static> {
    let status = if mixer.has_pending_routing() {
        Span::styled(
            "pending",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("live", Style::default().fg(Color::Green))
    };
    Line::from(vec![Span::raw("Routing: "), status])
}

/// Returns the text style used to highlight the currently active (live) binding in the UI.
///
/// ## Examples
/// ```
/// use orpheus_lang::tui::style::live_binding_style;
/// let style = live_binding_style();
/// ```
pub fn live_binding_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

/// Returns the text style used to highlight a pending (queued) binding in the UI.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::pending_binding_style;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// let style = pending_binding_style(&view);
/// ```
pub fn pending_binding_style(transport: &TransportView) -> Style {
    let color = match transport_state(transport) {
        UiTransportState::Queued => Color::Blue,
        UiTransportState::Playing | UiTransportState::Stopped | UiTransportState::Syncing => {
            Color::Cyan
        }
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

/// Builds a `ListItem` for a binding summary, applying appropriate highlighting if it is live or pending.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::binding_list_item;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// let item = binding_list_item("pattern: bd".to_string(), &view);
/// ```
pub fn binding_list_item(
    summary: String,
    transport: &TransportView,
) -> ratatui::widgets::ListItem<'static> {
    use ratatui::widgets::ListItem;

    let name = summary
        .split_once(": ")
        .map_or(summary.as_str(), |(name, _)| name);
    if transport.active_pattern_name() == Some(name) {
        return ListItem::new(Line::from(vec![
            Span::styled("[live] ", live_binding_style()),
            Span::raw(summary),
        ]));
    }
    if transport.pending_pattern_name() == Some(name) {
        return ListItem::new(Line::from(vec![
            Span::styled("[next] ", pending_binding_style(transport)),
            Span::raw(summary),
        ]));
    }
    ListItem::new(summary)
}

/// Builds a `ListItem` displaying the legend for binding highlights (live vs pending).
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::binding_legend_item;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// let item = binding_legend_item(&view);
/// ```
pub fn binding_legend_item(transport: &TransportView) -> ratatui::widgets::ListItem<'static> {
    use ratatui::widgets::ListItem;

    ListItem::new(Line::from(vec![
        Span::raw("Legend: "),
        Span::styled("[live]", live_binding_style()),
        Span::raw(" active  "),
        Span::styled("[next]", pending_binding_style(transport)),
        Span::raw(" pending"),
    ]))
}

/// Determines whether the binding legend should be displayed based on available vertical space.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::should_show_binding_legend;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// let show = should_show_binding_legend(20, 2, &view);
/// assert!(!show);
/// ```
pub fn should_show_binding_legend(
    bindings_height: u16,
    binding_count: usize,
    transport: &TransportView,
) -> bool {
    if transport.active_pattern_name().is_none() && transport.pending_pattern_name().is_none() {
        return false;
    }
    let visible_rows = usize::from(bindings_height.saturating_sub(2));
    visible_rows >= MIN_BINDING_LEGEND_ROWS && visible_rows >= binding_count.saturating_add(2)
}

/// Returns the text style used for keyboard shortcut legends.
///
/// ## Examples
/// ```
/// use orpheus_lang::tui::style::key_legend_style;
/// let style = key_legend_style();
/// ```
pub fn key_legend_style() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM)
}

/// Returns the text style used for the border of the help overlay modal.
///
/// ## Examples
/// ```
/// use orpheus_lang::tui::style::help_overlay_border_style;
/// let style = help_overlay_border_style();
/// ```
pub fn help_overlay_border_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Returns the text style used for the footer of the help overlay modal.
///
/// ## Examples
/// ```
/// use orpheus_lang::tui::style::help_overlay_footer_style;
/// let style = help_overlay_footer_style();
/// ```
pub fn help_overlay_footer_style() -> Style {
    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)
}

/// Formats the current cycle and sub-cycle progress into a string (e.g., "1.500").
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::format_cycle_position;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let snapshot = session.transport_snapshot();
/// let fmt = format_cycle_position(&snapshot);
/// assert_eq!(fmt, "0.000");
/// ```
pub fn format_cycle_position(snapshot: &orpheus_dsp::TransportSnapshot) -> String {
    let frames_per_cycle = snapshot.frames_per_cycle();
    if frames_per_cycle == 0 {
        return "0.000".to_owned();
    }
    let cycle_index = snapshot.current_cycle_start_frame() / frames_per_cycle;
    let cycle_offset = snapshot
        .current_frame()
        .saturating_sub(snapshot.current_cycle_start_frame());
    let progress_millis = cycle_offset.saturating_mul(1000) / frames_per_cycle;
    format!("{cycle_index}.{progress_millis:03}")
}

/// Formats the current tempo into a string, preserving decimals only when necessary.
///
/// ## Examples
/// ```
/// use orpheus_lang::session::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::tui::style::format_tempo_bpm;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let snapshot = session.transport_snapshot();
/// let fmt = format_tempo_bpm(&snapshot);
/// assert_eq!(fmt, "120");
/// ```
pub fn format_tempo_bpm(snapshot: &orpheus_dsp::TransportSnapshot) -> String {
    let tempo_bpm = snapshot.tempo_bpm();
    if tempo_bpm.fract().abs() < f32::EPSILON {
        format!("{tempo_bpm:.0}")
    } else {
        format!("{tempo_bpm:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::ReplSession;
    use orpheus_dsp::EngineHandle;

    #[test]
    fn should_format_cycle_position_and_tempo() {
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        let snapshot = session.transport_snapshot();
        assert_eq!(format_cycle_position(&snapshot), "0.000");
        assert_eq!(format_tempo_bpm(&snapshot), "120");

        // Advance 1.5 cycles at 120 BPM (96000 frames per cycle at 48kHz)
        session.render_test_block_for_tui(96000 + 48000);
        let snapshot = session.transport_snapshot();
        assert_eq!(format_cycle_position(&snapshot), "1.500");
    }

    #[test]
    fn should_return_correct_transport_state() {
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        // Stop the engine and render to ensure the state applies
        session.eval_line(":stop").unwrap();
        session.render_test_block_for_tui(1);

        // Initially stopped
        let view = session.transport_view();
        assert_eq!(transport_state(&view), UiTransportState::Stopped);
        assert_eq!(format_transport_status(&view), "stopped");
        assert_eq!(
            transport_status_style(&view),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        );

        // Start playing
        session.eval_line(":play").unwrap();
        session.render_test_block_for_tui(1);
        let view = session.transport_view();
        assert_eq!(transport_state(&view), UiTransportState::Playing);
        assert_eq!(format_transport_status(&view), "playing");
        assert_eq!(
            transport_status_style(&view),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        );

        // Enqueue a pattern, which puts us in Queued/Syncing state.
        // `pending_pattern_name()` will only be set if `session.eval_line` sets it on `pattern_display`.
        session.eval_line("p = bd").unwrap();
        let view = session.transport_view();

        // `has_pending_pattern` is false because we haven't reached a boundary / engine hasn't seen the queue yet.
        // But `pending_pattern_name` is Some("p"). So it resolves to `UiTransportState::Queued`
        assert_eq!(transport_state(&view), UiTransportState::Queued);
        assert_eq!(format_transport_status(&view), "queued");
        assert_eq!(
            transport_status_style(&view),
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD)
        );
    }
}
