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

/// Central color palette for the TUI — the single source of truth for the
/// interface's visual language.
///
/// Every pane, overlay, and footer element sources its semantic colors here so
/// the look stays consistent and a future re-theme is a one-file change. The
/// constants are intentionally named by *role* (accent, focus, error, …) rather
/// than by hue, even where two roles currently share a color.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Theme;

impl Theme {
    /// Primary accent for interactive/highlighted elements (keys, digits, bars).
    pub const ACCENT: Color = Color::Cyan;
    /// Border color of the currently focused pane.
    pub const FOCUS: Color = Color::Yellow;
    /// Error / failure feedback.
    pub const ERROR: Color = Color::Red;
    /// Success / affirmative feedback.
    pub const SUCCESS: Color = Color::Green;
    /// Warning / cautionary feedback.
    pub const WARNING: Color = Color::Yellow;
    /// Muted / dim text (hints, legends, inactive cells).
    pub const MUTED: Color = Color::DarkGray;
    /// Playhead / downbeat emphasis.
    pub const PLAYHEAD: Color = Color::Yellow;
}

/// Returns the [`Style`] used for the border of the focused pane.
///
/// This is the single source of truth for the focus highlight, replacing the
/// `fg(Yellow).bold()` literal that was previously duplicated across every pane
/// plugin.
///
/// # Examples
/// ```
/// use ratatui::style::{Color, Modifier};
/// use orpheus_lang::focus_border_style;
///
/// let style = focus_border_style();
/// assert_eq!(style.fg, Some(Color::Yellow));
/// assert!(style.add_modifier.contains(Modifier::BOLD));
/// ```
#[must_use]
pub fn focus_border_style() -> Style {
    Style::default()
        .fg(Theme::FOCUS)
        .add_modifier(Modifier::BOLD)
}

/// Computes fractional progress through the current cycle as a value in `[0.0, 1.0)`.
///
/// This is the same ratio underlying [`format_cycle_position`], exposed as a raw
/// float so downbeat visuals (the pulse glyph and progress bar) can be driven
/// purely from a [`TransportSnapshot`].
///
/// # Examples
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::cycle_progress;
///
/// let session = ReplSession::with_engine(EngineHandle::stub());
/// let snapshot = session.transport_snapshot();
/// assert_eq!(cycle_progress(&snapshot), 0.0);
/// ```
#[must_use]
// Display-only ratio: frame counts far exceed f32's mantissa, but sub-frame
// precision is irrelevant for a one-character pulse and a handful of bar cells.
#[allow(clippy::cast_precision_loss)]
pub fn cycle_progress(snapshot: &orpheus_dsp::TransportSnapshot) -> f32 {
    let frames_per_cycle = snapshot.frames_per_cycle();
    if frames_per_cycle == 0 {
        return 0.0;
    }
    let cycle_offset = snapshot
        .current_frame()
        .saturating_sub(snapshot.current_cycle_start_frame())
        .min(frames_per_cycle);
    cycle_offset as f32 / frames_per_cycle as f32
}

/// Returns the downbeat pulse glyph and its [`Style`] for a given cycle `progress`.
///
/// The glyph is a filled, bright circle (`●`) on the downbeat, brightest at the
/// very start of the cycle and fading as it advances, then an empty, dim circle
/// (`○`) through the back half. Callers should only render it while the
/// transport is playing.
///
/// # Examples
/// ```
/// use ratatui::style::{Color, Modifier};
/// use orpheus_lang::cycle_pulse_glyph;
///
/// let (glyph, style) = cycle_pulse_glyph(0.0);
/// assert_eq!(glyph, '\u{25cf}'); // ●
/// assert!(style.add_modifier.contains(Modifier::BOLD));
///
/// let (glyph, _) = cycle_pulse_glyph(0.75);
/// assert_eq!(glyph, '\u{25cb}'); // ○
/// ```
#[must_use]
pub fn cycle_pulse_glyph(progress: f32) -> (char, Style) {
    let phase = progress.rem_euclid(1.0);
    if phase < 0.5 {
        let mut style = Style::default().fg(Theme::PLAYHEAD);
        if phase < 0.125 {
            style = style.add_modifier(Modifier::BOLD);
        }
        ('\u{25cf}', style)
    } else {
        (
            '\u{25cb}',
            Style::default()
                .fg(Theme::MUTED)
                .add_modifier(Modifier::DIM),
        )
    }
}

/// Renders a fixed-`width` block-glyph bar showing `progress` through the cycle.
///
/// Filled cells use `█` and remaining cells use `░`. The returned string is
/// exactly `width` glyphs wide (empty when `width == 0`).
///
/// # Examples
/// ```
/// use orpheus_lang::cycle_progress_bar;
///
/// assert_eq!(cycle_progress_bar(0.0, 4), "\u{2591}\u{2591}\u{2591}\u{2591}");
/// assert_eq!(cycle_progress_bar(0.5, 4), "\u{2588}\u{2588}\u{2591}\u{2591}");
/// assert_eq!(cycle_progress_bar(1.0, 4), "\u{2588}\u{2588}\u{2588}\u{2588}");
/// ```
#[must_use]
// The arithmetic is bounded: `clamped` is in [0, 1] and the product is rounded
// into [0, width], so the cast back to `usize` cannot truncate meaningfully or
// lose a sign. Bar widths are tiny, well within f32's exact-integer range.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn cycle_progress_bar(progress: f32, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let clamped = progress.clamp(0.0, 1.0);
    let filled = ((clamped * width as f32).round() as usize).min(width);
    let mut bar = String::with_capacity(width * 3);
    for _ in 0..filled {
        bar.push('\u{2588}');
    }
    for _ in filled..width {
        bar.push('\u{2591}');
    }
    bar
}

/// Level at or above which a meter is treated as clipping (full-scale).
const METER_CLIP_LEVEL: f32 = 1.0;
/// Level at or above which a meter enters the cautionary "hot" zone.
const METER_WARN_LEVEL: f32 = 0.8;

/// Returns the [`Theme`] color for a per-track meter at the given peak `level`
/// (ADR 0013).
///
/// The meter reads accent while nominal, shifts to the warning color as it
/// approaches full-scale, and turns to the error color at clip, so a hot track
/// is legible by hue alone.
///
/// # Examples
/// ```
/// use ratatui::style::Color;
/// use orpheus_lang::{meter_color, Theme};
///
/// assert_eq!(meter_color(0.2), Theme::ACCENT);
/// assert_eq!(meter_color(0.85), Theme::WARNING);
/// assert_eq!(meter_color(1.0), Theme::ERROR);
/// ```
#[must_use]
pub fn meter_color(level: f32) -> Color {
    if level >= METER_CLIP_LEVEL {
        Theme::ERROR
    } else if level >= METER_WARN_LEVEL {
        Theme::WARNING
    } else {
        Theme::ACCENT
    }
}

/// Renders a fixed-`width` block-glyph meter bar for a peak `level` in `[0, 1]`
/// (ADR 0013).
///
/// Filled cells use `█` and remaining cells use `░`, so the bar is exactly
/// `width` glyphs wide (empty when `width == 0`). Out-of-range levels are
/// clamped, keeping the bar well-behaved at any pane width.
///
/// # Examples
/// ```
/// use orpheus_lang::meter_bar;
///
/// assert_eq!(meter_bar(0.0, 4), "\u{2591}\u{2591}\u{2591}\u{2591}");
/// assert_eq!(meter_bar(0.5, 4), "\u{2588}\u{2588}\u{2591}\u{2591}");
/// assert_eq!(meter_bar(1.0, 4), "\u{2588}\u{2588}\u{2588}\u{2588}");
/// assert_eq!(meter_bar(0.5, 0), "");
/// ```
#[must_use]
// The arithmetic is bounded exactly as in `cycle_progress_bar`: `clamped` is in
// [0, 1], the product rounds into [0, width], and bar widths are tiny.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn meter_bar(level: f32, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let clamped = level.clamp(0.0, 1.0);
    let filled = ((clamped * width as f32).round() as usize).min(width);
    let mut bar = String::with_capacity(width * 3);
    for _ in 0..filled {
        bar.push('\u{2588}');
    }
    for _ in filled..width {
        bar.push('\u{2591}');
    }
    bar
}

/// Builds the transient status-toast [`Line`] shown just above the footer.
///
/// Success toasts use the theme success color, failures use the error color, so
/// feedback stays legible regardless of which panes are open.
///
/// # Examples
/// ```
/// use orpheus_lang::status_toast_line;
///
/// let line = status_toast_line("saved", false);
/// assert!(line.spans.iter().any(|span| span.content.contains("saved")));
/// ```
#[must_use]
pub fn status_toast_line(message: &str, is_error: bool) -> Line<'static> {
    let (prefix, background, foreground) = if is_error {
        ("\u{2717} Failed", Theme::ERROR, Color::White)
    } else {
        ("\u{2713} Success", Theme::SUCCESS, Color::Black)
    };
    Line::from(vec![
        Span::styled(
            format!(" {prefix} "),
            Style::default()
                .bg(background)
                .fg(foreground)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {message} "),
            Style::default().bg(Theme::MUTED).fg(Color::White),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::ReplSession;
    use orpheus_dsp::EngineHandle;

    #[test]
    fn focus_border_style_is_bold_focus_color() {
        let style = focus_border_style();
        assert_eq!(style.fg, Some(Theme::FOCUS));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn cycle_pulse_glyph_pulses_on_the_downbeat() {
        // Start of the cycle: filled and bright (bold).
        let (glyph, style) = cycle_pulse_glyph(0.0);
        assert_eq!(glyph, '\u{25cf}');
        assert_eq!(style.fg, Some(Theme::PLAYHEAD));
        assert!(style.add_modifier.contains(Modifier::BOLD));

        // Still on the downbeat half but past the peak: filled, no longer bold.
        let (glyph, style) = cycle_pulse_glyph(0.3);
        assert_eq!(glyph, '\u{25cf}');
        assert!(!style.add_modifier.contains(Modifier::BOLD));

        // Back half of the cycle: empty and dim.
        let (glyph, style) = cycle_pulse_glyph(0.75);
        assert_eq!(glyph, '\u{25cb}');
        assert_eq!(style.fg, Some(Theme::MUTED));
        assert!(style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn cycle_progress_bar_fills_proportionally() {
        assert_eq!(
            cycle_progress_bar(0.0, 4),
            "\u{2591}\u{2591}\u{2591}\u{2591}"
        );
        assert_eq!(
            cycle_progress_bar(0.5, 4),
            "\u{2588}\u{2588}\u{2591}\u{2591}"
        );
        assert_eq!(
            cycle_progress_bar(1.0, 4),
            "\u{2588}\u{2588}\u{2588}\u{2588}"
        );
        // Out-of-range and zero-width inputs stay well-behaved.
        assert_eq!(cycle_progress_bar(2.0, 3), "\u{2588}\u{2588}\u{2588}");
        assert_eq!(cycle_progress_bar(0.5, 0), "");
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn cycle_progress_is_zero_at_cycle_start() {
        let session = ReplSession::with_engine(EngineHandle::stub());
        let snapshot = session.transport_snapshot();
        assert_eq!(cycle_progress(&snapshot), 0.0);
    }

    #[test]
    fn meter_bar_fills_proportionally() {
        assert_eq!(meter_bar(0.0, 4), "\u{2591}\u{2591}\u{2591}\u{2591}");
        assert_eq!(meter_bar(0.5, 4), "\u{2588}\u{2588}\u{2591}\u{2591}");
        assert_eq!(meter_bar(1.0, 4), "\u{2588}\u{2588}\u{2588}\u{2588}");
        // Out-of-range and zero-width inputs stay well-behaved.
        assert_eq!(meter_bar(2.0, 3), "\u{2588}\u{2588}\u{2588}");
        assert_eq!(meter_bar(-1.0, 3), "\u{2591}\u{2591}\u{2591}");
        assert_eq!(meter_bar(0.5, 0), "");
    }

    #[test]
    fn meter_color_shifts_from_accent_to_error_with_level() {
        assert_eq!(meter_color(0.0), Theme::ACCENT);
        assert_eq!(meter_color(0.79), Theme::ACCENT);
        assert_eq!(meter_color(0.8), Theme::WARNING);
        assert_eq!(meter_color(0.95), Theme::WARNING);
        assert_eq!(meter_color(1.0), Theme::ERROR);
        assert_eq!(meter_color(1.5), Theme::ERROR);
    }

    #[test]
    fn status_toast_line_uses_theme_feedback_colors() {
        let ok = status_toast_line("done", false);
        assert_eq!(ok.spans[0].style.bg, Some(Theme::SUCCESS));
        assert!(ok.spans.iter().any(|span| span.content.contains("done")));

        let err = status_toast_line("boom", true);
        assert_eq!(err.spans[0].style.bg, Some(Theme::ERROR));
        assert!(err.spans.iter().any(|span| span.content.contains("boom")));
    }

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
