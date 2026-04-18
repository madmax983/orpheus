use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::session::{MixerView, TransportView};

const MIN_BINDING_LEGEND_ROWS: usize = 6;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiTransportState {
    Playing,
    Stopped,
    Syncing,
    Queued,
}

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

pub fn format_transport_status(view: &TransportView) -> &'static str {
    match transport_state(view) {
        UiTransportState::Playing => "playing",
        UiTransportState::Stopped => "stopped",
        UiTransportState::Syncing => "syncing",
        UiTransportState::Queued => "queued",
    }
}

pub fn transport_status_style(view: &TransportView) -> Style {
    let color = match transport_state(view) {
        UiTransportState::Playing => Color::Green,
        UiTransportState::Stopped => Color::Yellow,
        UiTransportState::Syncing => Color::Cyan,
        UiTransportState::Queued => Color::Blue,
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

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

pub fn live_binding_style() -> Style {
    Style::default()
        .fg(Color::Green)
        .add_modifier(Modifier::BOLD)
}

pub fn pending_binding_style(transport: &TransportView) -> Style {
    let color = match transport_state(transport) {
        UiTransportState::Queued => Color::Blue,
        UiTransportState::Playing | UiTransportState::Stopped | UiTransportState::Syncing => {
            Color::Cyan
        }
    };
    Style::default().fg(color).add_modifier(Modifier::BOLD)
}

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

pub fn key_legend_style() -> Style {
    Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM)
}

pub fn help_overlay_border_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn help_overlay_footer_style() -> Style {
    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)
}

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
    fn test_format_cycle_position_and_tempo() {
        let session = ReplSession::with_engine(EngineHandle::stub());
        let view = session.transport_view();
        let snapshot = view.snapshot();

        let cycle_pos = format_cycle_position(snapshot);
        assert_eq!(cycle_pos, "0.000");

        let tempo = format_tempo_bpm(snapshot);
        assert_eq!(tempo, "120");
    }

    #[test]
    fn test_transport_status_formatting() {
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        // Stop engine
        session.eval_line(":stop").unwrap();
        session.render_test_block_for_tui(1);
        let view = session.transport_view();

        assert_eq!(transport_state(&view), UiTransportState::Stopped);
        assert_eq!(format_transport_status(&view), "stopped");

        let style = transport_status_style(&view);
        assert_eq!(style.fg, Some(Color::Yellow));

        // Start engine
        session.eval_line(":play").unwrap();
        session.render_test_block_for_tui(1);
        let view2 = session.transport_view();
        assert_eq!(transport_state(&view2), UiTransportState::Playing);
        assert_eq!(format_transport_status(&view2), "playing");

        let style2 = transport_status_style(&view2);
        assert_eq!(style2.fg, Some(Color::Green));
    }

    #[test]
    fn test_routing_status_line() {
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        // By default, no pending routing.
        let mixer_view = session.mixer_view();
        let line = routing_status_line(&mixer_view);
        assert_eq!(line.spans[1].content, "live");

        // Apply routing change.
        session.eval_line(":track new test").unwrap();
        session.render_test_block_for_tui(1); // Advance time for engine to pick up routing change.
        let mixer_view_pending = session.mixer_view();
        let pending_line = routing_status_line(&mixer_view_pending);
        assert_eq!(pending_line.spans[1].content, "pending");
    }

    #[test]
    fn test_binding_legend_visibility() {
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        let view = session.transport_view();
        // Should be false when no active or pending patterns
        assert!(!should_show_binding_legend(10, 2, &view));

        session.eval_line("pat = bd sn").unwrap();
        let view_with_binding = session.transport_view();

        // Enough space (10 rows for 1 binding + legend = > 6 rows total and enough room)
        assert!(should_show_binding_legend(10, 1, &view_with_binding));

        // Not enough space (only 3 visible rows)
        assert!(!should_show_binding_legend(5, 1, &view_with_binding));
    }

    #[test]
    fn test_binding_list_item_styling() {
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        // Initially, no patterns
        let view = session.transport_view();
        let item = binding_list_item("other: bd".to_owned(), &view);
        assert!(format!("{item:?}").contains("other: bd"));

        // Add pending pattern
        session.eval_line("pat = bd sn").unwrap();
        let view_pending = session.transport_view();

        let item_pending = binding_list_item("pat: bd sn".to_owned(), &view_pending);
        // It should have [next] prefix
        assert!(format!("{item_pending:?}").contains("[next]"));
        assert!(format!("{item_pending:?}").contains("pat: bd sn"));

        // Advance time so it becomes active
        session.render_test_block_for_tui(session.frames_until_boundary_for_tui() + 1);
        let view_active = session.transport_view();

        let item_active = binding_list_item("pat: bd sn".to_owned(), &view_active);
        // It should have [live] prefix
        assert!(format!("{item_active:?}").contains("[live]"));
        assert!(format!("{item_active:?}").contains("pat: bd sn"));
    }
}
