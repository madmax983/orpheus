//! Styling and formatting logic for the Terminal User Interface (TUI).
//!
//! This module provides the visual vocabulary for the Orpheus REPL. It maps raw
//! engine data (like the current transport position, active pattern queues, and
//! DSP routing) into `ratatui` UI primitives (`Style`, `Span`, `Line`). By centralizing
//! color schemes and formatting logic here, we ensure a consistent visual language
//! across all panels of the interface and keep the orchestrator clean.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::session::{MixerView, TransportView};

const MIN_BINDING_LEGEND_ROWS: usize = 6;

/// Represents the high-level semantic status of the audio transport from the user's perspective.
///
/// While the underlying DSP engine only knows if it is "playing" or "stopped", the UI
/// tracks additional transition states (like waiting for the next cycle boundary to sync
/// a newly scheduled pattern) to provide better visual feedback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiTransportState {
    /// The transport is running normally, playing the currently active pattern.
    Playing,
    /// The transport is paused; no audio is being generated.
    Stopped,
    /// The transport is currently playing but a new pattern is scheduled to take over
    /// at the next cycle boundary.
    Syncing,
    /// The transport is stopped, but a pattern has been cued up. It will begin playing
    /// as soon as the transport is started.
    Queued,
}

/// Derives the current [`UiTransportState`] by cross-referencing the REPL's pending pattern queue
/// with the actual engine transport status.
///
/// This provides the logic to transition between states like [`UiTransportState::Playing`]
/// and [`UiTransportState::Syncing`].
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::{transport_state, UiTransportState};
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line(":stop").unwrap();
/// session.render_test_block_for_tui(1);
/// let view = session.transport_view();
/// assert_eq!(transport_state(&view), UiTransportState::Stopped);
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

/// Returns a human-readable string representation of the current transport state.
///
/// Useful for displaying the status in the top bar of the TUI.
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::format_transport_status;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line(":stop").unwrap();
/// session.render_test_block_for_tui(1);
/// let view = session.transport_view();
/// assert_eq!(format_transport_status(&view), "stopped");
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

/// Returns the visual [`Style`] (color and modifier) associated with the current transport state.
///
/// For example, active playback is green, while syncing transitions are cyan.
///
/// # Examples
/// ```
/// use ratatui::style::Color;
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::transport_status_style;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line(":stop").unwrap();
/// session.render_test_block_for_tui(1);
/// let view = session.transport_view();
/// assert_eq!(transport_status_style(&view).fg, Some(Color::Yellow));
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

/// Formats the main transport status indicator into a styled `ratatui` [`Line`].
///
/// This constructs the composite text seen in the UI (e.g., "Transport: playing -> `pattern_b`"),
/// applying the correct semantic color to the state keyword.
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::transport_status_line;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line(":stop").unwrap();
/// session.render_test_block_for_tui(1);
/// let view = session.transport_view();
/// let line = transport_status_line("Transport: ", &view, true);
/// assert!(line.spans.iter().any(|span| span.content == "stopped"));
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

/// Formats the status of the DSP mixer routing into a styled [`Line`].
///
/// Warns the user visually if they have redefined pedal routing but have not yet
/// applied it to the active graph.
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::routing_status_line;
///
/// let session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.mixer_view();
/// let line = routing_status_line(&view);
/// assert!(line.spans.iter().any(|span| span.content == "live"));
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

/// Returns the [`Style`] used to highlight the currently active (live) pattern binding
/// in the environment list.
///
/// # Examples
/// ```
/// use ratatui::style::Color;
/// use orpheus_lang::live_binding_style;
///
/// let style = live_binding_style();
/// assert_eq!(style.fg, Some(Color::Green));
/// ```
#[must_use]
pub fn live_binding_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

/// Returns the [`Style`] used to highlight a pattern binding that is queued to play next.
///
/// The specific color changes depending on whether the transport is currently running (syncing)
/// or stopped (queued).
///
/// # Examples
/// ```
/// use ratatui::style::Color;
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::pending_binding_style;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line(":stop").unwrap();
/// session.render_test_block_for_tui(1);
/// let view = session.transport_view();
/// assert_eq!(pending_binding_style(&view).fg, Some(Color::Cyan));
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

/// Constructs a formatted list item for a single variable binding in the environment panel.
///
/// Automatically prefixes the line with `[live]` or `[next]` with the appropriate colors
/// if the binding name matches the active or pending transport state.
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::binding_list_item;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line("drums = bd sn").unwrap();
/// session.render_test_block_for_tui(1);
/// let view = session.transport_view();
/// let item = binding_list_item("drums: Pattern".to_string(), &view);
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

/// Constructs the explanatory legend item for the environment binding list.
///
/// This acts as a visual guide at the bottom of the list, explaining what the colored
/// `[live]` and `[next]` tags signify.
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::binding_legend_item;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// let legend = binding_legend_item(&view);
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

/// Determines whether the environment panel has enough vertical space and context
/// to warrant displaying the binding legend.
///
/// Hides the legend if there are no active/pending patterns, or if the screen is too small,
/// prioritizing the display of the actual variables.
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::should_show_binding_legend;
///
/// let session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
/// assert!(!should_show_binding_legend(10, 2, &view));
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

/// Returns the muted [`Style`] used for keyboard shortcut hints (e.g., `(ESC to close)`).
///
/// # Examples
/// ```
/// use ratatui::style::Color;
/// use orpheus_lang::key_legend_style;
///
/// let style = key_legend_style();
/// assert_eq!(style.fg, Some(Color::DarkGray));
/// ```
#[must_use]
pub fn key_legend_style() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM)
}

/// Returns the highlighted [`Style`] used for the outer border of the Help overlay popup.
///
/// # Examples
/// ```
/// use ratatui::style::Color;
/// use orpheus_lang::help_overlay_border_style;
///
/// let style = help_overlay_border_style();
/// assert_eq!(style.fg, Some(Color::Cyan));
/// ```
#[must_use]
pub fn help_overlay_border_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Returns the muted [`Style`] used for the footer text in the Help overlay.
///
/// # Examples
/// ```
/// use ratatui::style::Color;
/// use orpheus_lang::help_overlay_footer_style;
///
/// let style = help_overlay_footer_style();
/// assert_eq!(style.fg, Some(Color::Gray));
/// ```
#[must_use]
pub fn help_overlay_footer_style() -> Style {
    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)
}

/// Formats the current exact audio frame position into a human-readable cycle coordinate.
///
/// The output is expressed as `<Cycle Index>.<Fractional Progress>`, where the fractional
/// progress is a value from `000` to `999` (e.g., `12.500` indicates we are exactly halfway
/// through the 12th cycle).
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::format_cycle_position;
///
/// let session = ReplSession::with_engine(EngineHandle::stub());
/// let snapshot = session.transport_snapshot();
/// assert_eq!(format_cycle_position(&snapshot), "0.000");
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

/// Formats the current engine tempo (Beats Per Minute) for display.
///
/// Truncates unnecessary decimal places for whole-number tempos (e.g., `120.0` becomes `120`),
/// while preserving precision for fractional tempos.
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::format_tempo_bpm;
///
/// let session = ReplSession::with_engine(EngineHandle::stub());
/// let snapshot = session.transport_snapshot();
/// assert_eq!(format_tempo_bpm(&snapshot), "120");
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
    fn test_ui_styles() {
        assert_eq!(live_binding_style().fg, Some(ratatui::style::Color::Green));
        assert_eq!(key_legend_style().fg, Some(ratatui::style::Color::DarkGray));
        assert_eq!(
            help_overlay_border_style().fg,
            Some(ratatui::style::Color::Cyan)
        );
        assert_eq!(
            help_overlay_footer_style().fg,
            Some(ratatui::style::Color::Gray)
        );
    }

    #[test]
    fn test_binding_legend_item() {
        let session = ReplSession::with_engine(EngineHandle::stub());
        let view = session.transport_view();
        let _legend = binding_legend_item(&view);
    }

    #[test]
    fn test_should_show_binding_legend() {
        let session = ReplSession::with_engine(EngineHandle::stub());
        let view = session.transport_view();
        assert!(!should_show_binding_legend(10, 2, &view));
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
