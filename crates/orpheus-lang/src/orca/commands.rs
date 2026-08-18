//! Host-side interpreter for `$` self commands (v7).
//!
//! In the reference client, `$` (and inbound UDP datagrams) inject raw
//! command strings into `commander.js`, whose `trigger()` parses
//! `name:value` and dispatches into its `actives` table. The grid engine
//! deliberately emits [`super::OrcaIoEvent::Command`] uninterpreted
//! (section 9.5 of the design doc); this module is the host half: a pure,
//! transport-free parser that maps each command string onto a typed
//! [`OrcaCommand`] where an Orpheus equivalent exists.
//!
//! Grammar, transcribed from `commander.js` `trigger()`:
//!
//! - the name is everything before the first `:`, trimmed, with non-word
//!   characters (`\W`) removed, lowercased;
//! - the value is the remainder of the message after `name.len() + 1`
//!   characters (the reference quirk: the *cleaned* name's length indexes
//!   the raw string);
//! - every command also answers to its first-two-letter shorthand
//!   (`commander.js` builds them in insertion order, so `co` resolves to
//!   `color`, which overwrites `copy`'s shorthand);
//! - numeric values follow `parseInt`: optional sign, leading digits,
//!   trailing junk ignored, no digits means no number.
//!
//! What maps where (see design doc section 13.1 for the full table):
//!
//! - `bpm`/`apm` set the *global* Orpheus transport tempo — in the
//!   reference a grid `$bpm` retunes the one global clock, and the grid
//!   surface keeps that reach (ADR 0011). Values are clamped to the
//!   reference clock's 60-300 range (`clock.js` `setSpeed`), which doubles
//!   as the safety clamp for glyph-typo tempos; `0` is a reference no-op
//!   (`setSpeed` ignores falsy values).
//! - `frame`/`rewind`/`skip` move the grid frame counter, clamped to the
//!   reference range `0..=9_999_999` (`clock.js` `setFrame`); a missing or
//!   non-numeric value is a no-op exactly like `setFrame(NaN)`.
//! - `play`/`stop` start and stop the grid clock (the `OrcaPublisher`),
//!   not the whole Orpheus transport — the grid is one source among many
//!   here, unlike the reference where the clock *is* the program.
//!
//! Everything else in the `actives` table is recognized but divergent
//! ([`CommandOutcome::Divergent`]): it either configures transports that
//! Orpheus configures through `:orca` (`midi` `udp` `osc` `ip` `cc` `pg`),
//! drives the reference's cursor/theme (`copy` `paste` `erase` `find`
//! `select` `color` `time`), writes into the grid from files or strings
//! (`inject` `write`), or single-steps the frame timer (`run`, meaningless
//! under cycle-ahead materialization). Unrecognized names are
//! [`CommandOutcome::Unknown`]. Both surface as status-line notes and
//! change nothing.

/// The reference frame-counter ceiling (`clock.js` `setFrame` clamps to
/// `0..=9999999`).
pub const MAX_GRID_FRAME: u64 = 9_999_999;

/// [`MAX_GRID_FRAME`] as an `i64`, for signed clamping.
const MAX_GRID_FRAME_I64: i64 = 9_999_999;

/// Reference tempo bounds (`clock.js` `setSpeed` clamps to 60-300 BPM).
const MIN_BPM: i64 = 60;
/// See [`MIN_BPM`].
const MAX_BPM: i64 = 300;

/// A `$`/UDP command with a direct Orpheus mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrcaCommand {
    /// `bpm:N` / `apm:N` — set the global transport tempo, clamped to the
    /// reference's 60-300 BPM range.
    Bpm(u32),
    /// `frame:N` — set the grid frame counter, clamped to zero through
    /// [`MAX_GRID_FRAME`].
    Frame(u64),
    /// `rewind:N` — move the grid frame counter back `N` frames
    /// (saturating at 0; a negative `N` skips forward, as in the
    /// reference's unchecked `f - value`).
    Rewind(i64),
    /// `skip:N` — move the grid frame counter forward `N` frames.
    Skip(i64),
    /// `play` — start the grid clock (no-op while running).
    Play,
    /// `stop` — stop the grid clock (no-op while stopped).
    Stop,
}

/// The result of interpreting one raw command string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandOutcome {
    /// A supported command, ready to apply.
    Apply(OrcaCommand),
    /// A recognized reference command with no Orpheus mapping; the name is
    /// the full (long-form) reference command name.
    Divergent(&'static str),
    /// A supported command whose value makes it a no-op in the reference
    /// too (missing/non-numeric where a number is required, or `bpm:0`).
    NoOp(&'static str),
    /// Not a reference command at all; carries the cleaned name.
    Unknown(String),
}

/// One entry in the reference `actives` table.
#[derive(Clone, Copy)]
enum Kind {
    Bpm,
    Frame,
    Rewind,
    Skip,
    Play,
    Stop,
    Divergent(&'static str),
}

/// Resolves a cleaned command name, long form or two-letter shorthand.
/// Shorthand collisions follow the reference's insertion order (`color`
/// overwrites `copy`'s `co`).
fn lookup(name: &str) -> Option<Kind> {
    Some(match name {
        // Time (bpm and apm both set the tempo; Orpheus has no eased
        // tempo, so apm applies immediately — a documented divergence).
        "bpm" | "bp" | "apm" | "ap" => Kind::Bpm,
        "frame" | "fr" => Kind::Frame,
        "rewind" | "re" => Kind::Rewind,
        "skip" | "sk" => Kind::Skip,
        // Controls.
        "play" | "pl" => Kind::Play,
        "stop" | "st" => Kind::Stop,
        "run" | "ru" => Kind::Divergent("run"),
        // Ports: configured through `:orca` in Orpheus.
        "osc" | "os" => Kind::Divergent("osc"),
        "udp" | "ud" => Kind::Divergent("udp"),
        "midi" | "mi" => Kind::Divergent("midi"),
        "ip" => Kind::Divergent("ip"),
        "cc" => Kind::Divergent("cc"),
        "pg" => Kind::Divergent("pg"),
        // Cursor / edit / theme: reference UI concerns.
        "copy" => Kind::Divergent("copy"),
        "paste" | "pa" => Kind::Divergent("paste"),
        "erase" | "er" => Kind::Divergent("erase"),
        "time" | "ti" => Kind::Divergent("time"),
        // `co` resolves to color, not copy (reference shorthand
        // collision).
        "color" | "co" => Kind::Divergent("color"),
        "find" | "fi" => Kind::Divergent("find"),
        "select" | "se" => Kind::Divergent("select"),
        "inject" | "in" => Kind::Divergent("inject"),
        "write" | "wr" => Kind::Divergent("write"),
        _ => return None,
    })
}

/// The cleaned command name: trimmed, non-word characters (`\W`, i.e.
/// anything but ASCII alphanumerics and `_`) removed, lowercased.
fn clean_name(head: &str) -> String {
    head.trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// `parseInt` semantics: optional leading whitespace and sign, then leading
/// digits (trailing junk ignored); `None` when no digits follow.
fn parse_leading_int(value: &str) -> Option<i64> {
    let trimmed = value.trim_start();
    let (negative, rest) = trimmed.strip_prefix('-').map_or_else(
        || (false, trimmed.strip_prefix('+').unwrap_or(trimmed)),
        |rest| (true, rest),
    );
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    // Absurdly long digit runs saturate rather than error.
    let magnitude: i64 = digits.parse().unwrap_or(i64::MAX);
    Some(if negative { -magnitude } else { magnitude })
}

/// Interprets one raw command string (from a `$` operator or an inbound
/// UDP datagram) per the reference `commander.js` grammar.
#[must_use]
pub fn parse_command(raw: &str) -> CommandOutcome {
    let head = raw.split(':').next().unwrap_or("");
    let name = clean_name(head);
    // The reference indexes the *raw* message by the cleaned name's length
    // plus one (`msg.substr(cmd.length + 1)`); out-of-range or mid-glyph
    // indexes read as an empty value.
    let value = raw.get(name.len() + 1..).unwrap_or("");
    let Some(kind) = lookup(&name) else {
        return CommandOutcome::Unknown(name);
    };
    match kind {
        Kind::Bpm => match parse_leading_int(value) {
            // `setSpeed` ignores falsy values, so 0 is a reference no-op.
            None | Some(0) => CommandOutcome::NoOp("bpm"),
            Some(bpm) => CommandOutcome::Apply(OrcaCommand::Bpm(
                // Clamped to 60-300, so the conversion cannot fail.
                u32::try_from(bpm.clamp(MIN_BPM, MAX_BPM)).unwrap_or(300),
            )),
        },
        Kind::Frame => parse_leading_int(value).map_or(CommandOutcome::NoOp("frame"), |frame| {
            // Clamped to 0..=MAX_GRID_FRAME, so the conversion cannot fail.
            CommandOutcome::Apply(OrcaCommand::Frame(
                u64::try_from(frame.clamp(0, MAX_GRID_FRAME_I64)).unwrap_or(MAX_GRID_FRAME),
            ))
        }),
        Kind::Rewind => parse_leading_int(value).map_or(CommandOutcome::NoOp("rewind"), |by| {
            CommandOutcome::Apply(OrcaCommand::Rewind(by))
        }),
        Kind::Skip => parse_leading_int(value).map_or(CommandOutcome::NoOp("skip"), |by| {
            CommandOutcome::Apply(OrcaCommand::Skip(by))
        }),
        Kind::Play => CommandOutcome::Apply(OrcaCommand::Play),
        Kind::Stop => CommandOutcome::Apply(OrcaCommand::Stop),
        Kind::Divergent(name) => CommandOutcome::Divergent(name),
    }
}

/// Applies a frame-moving command to a current frame value, clamping to
/// the reference range. Pure, so grid hosts and tests share the exact
/// arithmetic.
#[must_use]
pub fn adjusted_frame(current: u64, delta: i64) -> u64 {
    let ceiling = i128::from(MAX_GRID_FRAME);
    let next = (i128::from(current) + i128::from(delta)).clamp(0, ceiling);
    // Clamped to 0..=MAX_GRID_FRAME, so the conversion cannot fail.
    u64::try_from(next).unwrap_or(MAX_GRID_FRAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bpm_parses_and_clamps_to_the_reference_range() {
        assert_eq!(
            parse_command("bpm:140"),
            CommandOutcome::Apply(OrcaCommand::Bpm(140))
        );
        assert_eq!(
            parse_command("bpm:20"),
            CommandOutcome::Apply(OrcaCommand::Bpm(60)),
            "clamps below clock.js's minimum",
        );
        assert_eq!(
            parse_command("bpm:999"),
            CommandOutcome::Apply(OrcaCommand::Bpm(300)),
            "clamps above clock.js's maximum",
        );
    }

    use proptest::prelude::*;

    proptest! {
        /// 👺 Havoc: Tests that injecting arbitrary generated strings into the
        /// command parser correctly ignores them without panicking via out-of-bounds indexing or slicing errors.
        #[test]
        fn test_havoc_proptest_orca_commands(s in "\\PC*") {
            // Havoc: Inject arbitrary property-generated strings to simulate invalid commands.
            // It shouldn't panic on string slicing.
            let _ = parse_command(&s);
        }
    }

    #[test]
    fn bpm_zero_and_non_numeric_values_are_reference_no_ops() {
        // `setSpeed` skips falsy values; parseInt of junk is NaN.
        assert_eq!(parse_command("bpm:0"), CommandOutcome::NoOp("bpm"));
        assert_eq!(parse_command("bpm:fast"), CommandOutcome::NoOp("bpm"));
        assert_eq!(parse_command("bpm"), CommandOutcome::NoOp("bpm"));
        assert_eq!(parse_command("bpm:"), CommandOutcome::NoOp("bpm"));
    }

    #[test]
    fn apm_maps_to_the_same_tempo_command() {
        assert_eq!(
            parse_command("apm:90"),
            CommandOutcome::Apply(OrcaCommand::Bpm(90))
        );
    }

    #[test]
    fn shorthands_resolve_like_the_reference_table() {
        assert_eq!(
            parse_command("bp:120"),
            CommandOutcome::Apply(OrcaCommand::Bpm(120))
        );
        assert_eq!(
            parse_command("fr:3"),
            CommandOutcome::Apply(OrcaCommand::Frame(3))
        );
        assert_eq!(
            parse_command("pl"),
            CommandOutcome::Apply(OrcaCommand::Play)
        );
        assert_eq!(
            parse_command("st"),
            CommandOutcome::Apply(OrcaCommand::Stop)
        );
        // Insertion-order collision: `co` is color (overwrote copy).
        assert_eq!(parse_command("co"), CommandOutcome::Divergent("color"));
    }

    #[test]
    fn names_are_trimmed_stripped_of_non_word_characters_and_lowercased() {
        assert_eq!(
            parse_command("BPM:140"),
            CommandOutcome::Apply(OrcaCommand::Bpm(140))
        );
        // `\W` removal inside the name resolves the command, but the
        // reference then indexes the value by the *cleaned* name's length
        // (`msg.substr(cmd.length + 1)`), so the value misaligns to
        // `m:140` — NaN — and the command no-ops. Quirk replicated.
        assert_eq!(parse_command("b*pm:140"), CommandOutcome::NoOp("bpm"));
        // Without a value the misalignment cannot bite.
        assert_eq!(
            parse_command("PLAY"),
            CommandOutcome::Apply(OrcaCommand::Play)
        );
    }

    #[test]
    fn values_follow_parse_int_semantics() {
        // Trailing junk is ignored, signs are honored.
        assert_eq!(
            parse_command("bpm:140abc"),
            CommandOutcome::Apply(OrcaCommand::Bpm(140))
        );
        assert_eq!(
            parse_command("rewind:-4"),
            CommandOutcome::Apply(OrcaCommand::Rewind(-4))
        );
        assert_eq!(
            parse_command("skip:+2"),
            CommandOutcome::Apply(OrcaCommand::Skip(2))
        );
    }

    #[test]
    fn frame_clamps_to_the_reference_counter_range() {
        assert_eq!(
            parse_command("frame:12"),
            CommandOutcome::Apply(OrcaCommand::Frame(12))
        );
        assert_eq!(
            parse_command("frame:-5"),
            CommandOutcome::Apply(OrcaCommand::Frame(0))
        );
        assert_eq!(
            parse_command("frame:99999999999"),
            CommandOutcome::Apply(OrcaCommand::Frame(MAX_GRID_FRAME))
        );
        assert_eq!(parse_command("frame:x"), CommandOutcome::NoOp("frame"));
    }

    #[test]
    fn rewind_and_skip_without_a_number_are_no_ops() {
        // The reference computes `f - NaN` and `setFrame(NaN)` returns.
        assert_eq!(parse_command("rewind"), CommandOutcome::NoOp("rewind"));
        assert_eq!(parse_command("skip:go"), CommandOutcome::NoOp("skip"));
    }

    #[test]
    fn play_and_stop_ignore_their_value() {
        assert_eq!(
            parse_command("play:whatever"),
            CommandOutcome::Apply(OrcaCommand::Play)
        );
        assert_eq!(
            parse_command("stop"),
            CommandOutcome::Apply(OrcaCommand::Stop)
        );
    }

    #[test]
    fn recognized_reference_commands_without_a_mapping_are_divergent() {
        for (raw, name) in [
            ("osc:49162", "osc"),
            ("udp:49161;49160", "udp"),
            ("midi:0;1", "midi"),
            ("ip:127.0.0.1", "ip"),
            ("cc:64", "cc"),
            ("pg:0;0;0;1", "pg"),
            ("copy", "copy"),
            ("paste", "paste"),
            ("erase", "erase"),
            ("run", "run"),
            ("time", "time"),
            ("color:aaa;bbb;ccc", "color"),
            ("find:abc", "find"),
            ("select:0;0", "select"),
            ("inject:block", "inject"),
            ("write:x;1;2", "write"),
        ] {
            assert_eq!(
                parse_command(raw),
                CommandOutcome::Divergent(name),
                "for {raw}"
            );
        }
    }

    #[test]
    fn unknown_and_empty_commands_report_their_cleaned_name() {
        assert_eq!(
            parse_command("frobnicate:9"),
            CommandOutcome::Unknown("frobnicate".to_owned())
        );
        assert_eq!(parse_command(""), CommandOutcome::Unknown(String::new()));
    }

    #[test]
    fn adjusted_frame_saturates_at_the_reference_bounds() {
        assert_eq!(adjusted_frame(10, -4), 6);
        assert_eq!(adjusted_frame(2, -9), 0);
        assert_eq!(adjusted_frame(MAX_GRID_FRAME, 5), MAX_GRID_FRAME);
        assert_eq!(adjusted_frame(0, 7), 7);
    }
}
