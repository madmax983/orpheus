//! Styling and formatting utilities for the Orpheus TUI.
//!
//! This module provides shared visual styles and text formatting functions for UI components.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::session::{MixerView, TransportView};

const MIN_BINDING_LEGEND_ROWS: usize = 6;

/// Represents the visual playback state of the engine.
///
/// This abstract state simplifies the transport logic for UI components, collapsing complex conditions (like syncing) into discrete UI representations.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::UiTransportState;
///
/// let state = UiTransportState::Playing;
/// assert_eq!(state, UiTransportState::Playing);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiTransportState {
    Playing,
    Stopped,
    Syncing,
    Queued,
}

/// Determines the current transport state for the UI.
///
/// Evaluates both the active and pending patterns to infer if the transport is actively playing, stopped, queuing a new pattern, or syncing a pending queue to the downbeat.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::transport_state;
/// use orpheus_lang::session::TransportView;
///
/// // transport_state(&view) -> UiTransportState::Playing
/// ```
#[must_use]
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

/// Returns a human-readable string representation of the transport state.
///
/// Used to populate the transport status text in the UI header.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::format_transport_status;
///
/// // format_transport_status(&view) -> "playing"
/// ```
#[must_use]
pub fn format_transport_status(view: &TransportView) -> &'static str {
    match transport_state(view) {
        UiTransportState::Playing => "playing",
        UiTransportState::Stopped => "stopped",
        UiTransportState::Syncing => "syncing",
        UiTransportState::Queued => "queued",
    }
}

/// Returns the text style for the current transport state.
///
/// Assigns intuitive colors to states (e.g., green for playing, yellow for stopped) to provide immediate visual feedback to the user.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::transport_status_style;
/// use ratatui::style::{Color, Modifier, Style};
///
/// // transport_status_style(&view) -> Style::default().fg(Color::Green)
/// ```
#[must_use]
pub fn transport_status_style(view: &TransportView) -> Style {
    let color = match transport_state(view) {
        UiTransportState::Playing => Color::Green,
        UiTransportState::Stopped => Color::Yellow,
        UiTransportState::Syncing => Color::Cyan,
        UiTransportState::Queued => Color::Blue,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

/// Constructs a stylized line of text displaying the transport status.
///
/// Combines the status text, its corresponding style, and optionally the pending pattern target if one is queued.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::transport_status_line;
/// use ratatui::text::Line;
///
/// // transport_status_line("Transport: ", &view, true) -> Line<'static>
/// ```
#[must_use]
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

/// Constructs a stylized line indicating the mixer's routing state.
///
/// Provides feedback on whether the routing is live or if there are pending changes waiting to be applied.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::routing_status_line;
/// use ratatui::text::Line;
///
/// // routing_status_line(&mixer_view) -> Line<'static>
/// ```
#[must_use]
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

/// Returns the style used to highlight active/live bindings.
///
/// Makes it clear which bindings are currently executing patterns in the session.
///
/// # Examples
///
/// ```ignore
/// use ratatui::style::{Color, Modifier, Style};
/// use orpheus_lang::tui::style::live_binding_style;
///
/// let style = live_binding_style();
/// assert_eq!(style.fg, Some(Color::Green));
/// assert!(style.add_modifier.contains(Modifier::BOLD));
/// ```
#[must_use]
pub fn live_binding_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

/// Returns the style used to highlight pending bindings.
///
/// Colors the bindings that are queued up to play on the next cycle boundary.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::pending_binding_style;
/// use ratatui::style::{Color, Modifier, Style};
///
/// // let style = pending_binding_style(&view);
/// ```
#[must_use]
pub fn pending_binding_style(transport: &TransportView) -> Style {
    let color = match transport_state(transport) {
        UiTransportState::Queued => Color::Blue,
        UiTransportState::Playing | UiTransportState::Stopped | UiTransportState::Syncing => {
            Color::Cyan
        }
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

/// Formats a binding summary string into a stylized list item.
///
/// Applies appropriate styles based on whether the binding is live, pending, or inactive to distinguish them in the bindings pane.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::binding_list_item;
/// use ratatui::widgets::ListItem;
///
/// // let item = binding_list_item("drums: bd sn".to_string(), &view);
/// ```
#[must_use]
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

/// Creates a stylized legend explaining the colors used for bindings.
///
/// Helps new users understand the distinction between active and pending binding styles.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::binding_legend_item;
/// use ratatui::widgets::ListItem;
///
/// // let legend = binding_legend_item(&view);
/// ```
#[must_use]
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

/// Determines if the binding legend should be displayed.
///
/// The legend is hidden if there are no active/pending bindings or if the pane height is too small, maximizing screen real estate for the actual bindings.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::should_show_binding_legend;
///
/// // let show = should_show_binding_legend(10, 2, &view);
/// // assert!(show);
/// ```
#[must_use]
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

/// Returns the style used for keyboard shortcut legends.
///
/// Uses dimmed colors to minimize visual noise while still providing helpful hints.
///
/// # Examples
///
/// ```ignore
/// use ratatui::style::{Color, Modifier, Style};
/// use orpheus_lang::tui::style::key_legend_style;
///
/// let style = key_legend_style();
/// assert_eq!(style.fg, Some(Color::DarkGray));
/// assert!(style.add_modifier.contains(Modifier::DIM));
/// ```
#[must_use]
pub fn key_legend_style() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM)
}

/// Returns the style for the help overlay border.
///
/// Uses a bold, distinct color to make the modal pop out from the background.
///
/// # Examples
///
/// ```ignore
/// use ratatui::style::{Color, Modifier, Style};
/// use orpheus_lang::tui::style::help_overlay_border_style;
///
/// let style = help_overlay_border_style();
/// assert_eq!(style.fg, Some(Color::Cyan));
/// assert!(style.add_modifier.contains(Modifier::BOLD));
/// ```
#[must_use]
pub fn help_overlay_border_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Returns the style for the footer text in the help overlay.
///
/// Dimmed style to subordinate it to the main help content.
///
/// # Examples
///
/// ```ignore
/// use ratatui::style::{Color, Modifier, Style};
/// use orpheus_lang::tui::style::help_overlay_footer_style;
///
/// let style = help_overlay_footer_style();
/// assert_eq!(style.fg, Some(Color::Gray));
/// assert!(style.add_modifier.contains(Modifier::DIM));
/// ```
#[must_use]
pub fn help_overlay_footer_style() -> Style {
    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)
}

/// Formats the current cycle position as a string.
///
/// Provides a clear `<cycle_index>.<progress_millis>` readout to help users synchronize their actions with the music.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::format_cycle_position;
///
/// // let cycle_str = format_cycle_position(&snapshot);
/// // assert_eq!(cycle_str, "0.000");
/// ```
#[must_use]
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

/// Formats the current tempo in Beats Per Minute (BPM).
///
/// Strips fractional parts if they are negligible to keep the display clean, showing precise decimals only when necessary.
///
/// # Examples
///
/// ```ignore
/// use orpheus_lang::tui::style::format_tempo_bpm;
///
/// // let bpm_str = format_tempo_bpm(&snapshot);
/// // assert_eq!(bpm_str, "120");
/// ```
#[must_use]
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
