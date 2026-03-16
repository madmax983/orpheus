use crossterm::style::Stylize;
use orpheus_dsp::EngineHandle;
use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::session::Session;

/// Runs the phase-one Orpheus REPL over standard input and output.
///
/// Blank lines are ignored. `:quit` exits the session.
///
/// # Errors
///
/// Returns any terminal I/O failure encountered while reading input or
/// writing REPL output.
pub fn run_stdio() -> io::Result<()> {
    run_stdio_with_engine(EngineHandle::stub())
}

/// Runs the phase-one Orpheus REPL with the provided audio engine handle.
///
/// Blank lines are ignored. `:quit` exits the session.
///
/// # Errors
///
/// Returns any terminal I/O failure encountered while reading input or
/// writing REPL output.
pub fn run_stdio_with_engine(engine: EngineHandle) -> io::Result<()> {
    run_stdio_with_engine_and_path(engine, None, None)
}

/// Runs the phase-one Orpheus REPL with an optional startup `.ode` preload.
///
/// # Errors
///
/// Returns startup file load failures or terminal I/O failures encountered
/// while the REPL is active.
pub fn run_stdio_with_engine_and_path(
    engine: EngineHandle,
    startup_path: Option<&Path>,
    warning: Option<String>,
) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut session = Session::with_engine(engine);

    let mut stdout = stdout.lock();
    let mut stderr = stderr.lock();

    if let Some(msg) = warning {
        writeln!(stderr, "{}", format!("! {msg}").yellow().bold())?;
    }

    if let Some(path) = startup_path {
        match session.open_file(path) {
            Ok(msg) => writeln!(stdout, "{}", msg.green())?,
            Err(msg) => writeln!(stderr, "{}", format!("! {msg}").red().bold())?,
        }
    }

    run_with_handles(stdin.lock(), stdout, stderr, &mut session)
}

fn run_with_handles<R, W, E>(
    mut reader: R,
    mut stdout: W,
    mut stderr: E,
    session: &mut Session,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
    E: Write,
{
    let mut line = String::new();
    loop {
        write!(stdout, "{}", "> ".dark_grey())?;
        stdout.flush()?;
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ":quit" {
            break;
        }

        match session.eval_line(trimmed) {
            Ok(message) => writeln!(stdout, "{}", message.green())?,
            Err(message) => writeln!(stderr, "{}", format!("✗ {message}").red().bold())?,
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::Session;

    static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn temp_wav_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("orpheus-render-{}.wav", unique_temp_suffix()))
    }

    fn temp_csv_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("orpheus-export-{}.csv", unique_temp_suffix()))
    }

    #[test]
    fn eval_line_reuses_prior_bindings() {
        let mut session = Session::new();

        assert_eq!(
            session.eval_line("drums = bd sn cp sn"),
            Ok("✓ bound drums: Pattern<Sample>".to_owned())
        );
        assert_eq!(
            session.eval_line("copy = drums"),
            Ok("✓ bound copy: Pattern<Sample>".to_owned())
        );
    }

    #[test]
    fn sample_patterns_drive_the_embedded_audio_engine() {
        let mut session = Session::new();

        session.eval_line("drums = bd sn cp sn").unwrap();
        let frames = session.engine().frames_until_boundary_for_test() + 256;
        let rendered = session.engine().render_test_block(frames);

        assert!(
            rendered
                .iter()
                .any(|sample: &f32| sample.abs() > f32::EPSILON)
        );
    }

    #[test]
    fn render_command_exports_a_bound_pattern() {
        let mut session = Session::new();
        let path = temp_wav_path();

        session.eval_line("song = bd sn cp sn").unwrap();
        let message = session
            .eval_line(&format!(":render song {} 2", path.display()))
            .unwrap();

        assert!(message.contains("rendered `song`"));
        assert!(path.exists());
        assert!(fs::metadata(&path).unwrap().len() > 44);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_a_bound_pattern_to_csv() {
        let mut session = Session::new();
        let path = temp_csv_path();

        session.eval_line("song = bd sn cp sn").unwrap();
        let message = session
            .eval_line(&format!(":export song {} 2", path.display()))
            .unwrap();

        assert!(message.contains("exported `song`"));
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains(
            "start_num,start_den,start_float,end_num,end_den,end_float,sample,gain,pan,rate"
        ));
        assert!(contents.contains("bd"));
        assert!(contents.contains("sn"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_number_pattern_to_csv() {
        let mut session = Session::new();
        let path = temp_csv_path();

        session.eval_line("notes = 1 2 3").unwrap();
        let message = session
            .eval_line(&format!(":export notes {} 1", path.display()))
            .unwrap();

        assert!(message.contains("exported `notes`"));
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(
            contents.contains("start_num,start_den,start_float,end_num,end_den,end_float,value")
        );
        assert!(contents.contains('1'));
        assert!(contents.contains('2'));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn samples_command_loads_directory_overrides_for_live_playback() {
        let mut session = Session::new();
        let directory = temp_directory("repl-samples");
        write_wav(directory.join("bd.wav"), &[0.25, 0.0, 0.0, 0.0]);

        let message = session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session.eval_line("drums = bd").unwrap();
        let rendered = session.render_test_block_for_tui(4);
        let expected = 0.25 * edge_envelope(0, 4);

        assert!(message.contains("loaded"));
        assert!((rendered[0] - expected).abs() < f32::EPSILON);
        assert!((rendered[1] - expected).abs() < f32::EPSILON);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reload_samples_command_swaps_sample_bank_at_cycle_boundary() {
        let mut session = Session::new();
        let directory = temp_directory("repl-reload");
        write_wav(directory.join("bd.wav"), &[0.1, 0.0, 0.0, 0.0]);

        session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session.eval_line("drums = bd bd").unwrap();

        let first_trigger = session.render_test_block_for_tui(4);
        let first_expected = 0.1 * edge_envelope(0, 4);
        assert!((first_trigger[0] - first_expected).abs() < f32::EPSILON);

        write_wav(directory.join("bd.wav"), &[0.9, 0.0, 0.0, 0.0]);
        let message = session.eval_line(":reload-samples").unwrap();
        assert!(message.contains("reloaded"));

        let frames_per_cycle = session.transport_snapshot().frames_per_cycle();
        let frames_until_second_trigger = (frames_per_cycle / 2).saturating_sub(4);
        let _ = session.render_test_block_for_tui(frames_until_second_trigger);
        let second_trigger_same_cycle = session.render_test_block_for_tui(4);
        assert!((second_trigger_same_cycle[0] - first_expected).abs() < f32::EPSILON);

        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        let first_trigger_next_cycle = session.render_test_block_for_tui(4);
        let reloaded_expected = 0.9 * edge_envelope(0, 4);
        assert!((first_trigger_next_cycle[0] - reloaded_expected).abs() < f32::EPSILON);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sample_playback_params_flow_into_live_engine() {
        let mut session = Session::new();
        let directory = temp_directory("repl-sample-params");
        fs::write(
            directory.join("samples.ron"),
            "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
        )
        .unwrap();
        write_wav(directory.join("vox.wav"), &[0.2, 0.4, 0.6, 0.8]);

        session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session
            .eval_line(
                r#"lead = sample("vox_ah") |> slice(0.25, 1) |> rate(2) |> gain(0.5) |> pan(-1)"#,
            )
            .unwrap();

        let rendered = session.render_test_block_for_tui(4);
        let first_expected = 0.2 * edge_envelope(0, 2);
        let second_expected = 0.4 * edge_envelope(1, 2);
        assert!((rendered[0] - first_expected).abs() < f32::EPSILON);
        assert!(rendered[1].abs() < f32::EPSILON);
        assert!((rendered[2] - second_expected).abs() < f32::EPSILON);
        assert!(rendered[3].abs() < f32::EPSILON);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn open_command_replaces_session_bindings_from_ode_file() {
        let mut session = Session::new();
        let song = fixture("song.ode");

        session.eval_line("scratch = 1").unwrap();

        let message = session
            .eval_line(&format!(":open {}", song.display()))
            .unwrap();

        assert!(message.contains("opened"));
        assert_eq!(
            session.binding_summaries(),
            vec![
                "drums: Pattern<Sample>".to_owned(),
                "song: Pattern<Sample>".to_owned()
            ]
        );
        assert_eq!(session.last_loaded_pattern_name(), Some("song".to_owned()));
        assert_eq!(
            session.eval_line("copy = song"),
            Ok("✓ bound copy: Pattern<Sample>".to_owned())
        );
        assert_eq!(
            session.eval_line(":render scratch out.wav 1"),
            Err("no binding named `scratch`".to_owned())
        );
    }

    #[test]
    fn render_command_rejects_unknown_bindings() {
        let mut session = Session::new();

        let error = session.eval_line(":render nope out.wav 1").unwrap_err();

        assert!(error.contains("no binding named `nope`"));
    }

    #[test]
    fn tempo_command_updates_engine_transport() {
        let mut session = Session::new();

        let message = session.eval_line(":tempo 90").unwrap();
        let _ = session.render_test_block_for_tui(1);

        assert_eq!(message, "tempo set to 90 BPM");
        assert_eq!(
            session.transport_snapshot().tempo_bpm().to_bits(),
            90.0_f32.to_bits()
        );
    }

    #[test]
    fn tempo_command_rejects_non_positive_values() {
        let mut session = Session::new();

        let error = session.eval_line(":tempo 0").unwrap_err();

        assert_eq!(error, "tempo must be a finite positive BPM");
    }

    #[test]
    fn stop_and_play_commands_update_transport_state() {
        let mut session = Session::new();
        session.eval_line("drums = bd sn cp sn").unwrap();
        let _ = session.render_test_block_for_tui(256);

        let stop_message = session.eval_line(":stop").unwrap();
        let _ = session.render_test_block_for_tui(1);
        assert_eq!(stop_message, "transport stopped");
        assert!(!session.transport_snapshot().is_playing());

        let play_message = session.eval_line(":play").unwrap();
        let _ = session.render_test_block_for_tui(1);
        assert_eq!(play_message, "transport playing");
        assert!(session.transport_snapshot().is_playing());
    }

    #[test]
    fn last_loaded_pattern_name_tracks_sample_bindings() {
        let mut session = Session::new();

        session.eval_line("drums = bd sn cp sn").unwrap();
        assert_eq!(session.last_loaded_pattern_name(), Some("drums".to_owned()));

        session.eval_line("warp = fast(2)").unwrap();
        assert_eq!(session.last_loaded_pattern_name(), Some("drums".to_owned()));
    }

    #[test]
    fn transport_view_tracks_active_and_pending_pattern_names() {
        let mut session = Session::new();

        session.eval_line("drums = bd sn").unwrap();
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), None);
        assert_eq!(view.pending_pattern_name(), Some("drums"));

        let _ = session.render_test_block_for_tui(256);
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), Some("drums"));
        assert_eq!(view.pending_pattern_name(), None);

        session.eval_line("backbeat = sn cp").unwrap();
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), Some("drums"));
        assert_eq!(view.pending_pattern_name(), Some("backbeat"));

        let _ = session.render_test_block_for_tui(1);
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), Some("drums"));
        assert_eq!(view.pending_pattern_name(), Some("backbeat"));

        let frames = session.frames_until_boundary_for_tui();
        let _ = session.render_test_block_for_tui(frames);
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), Some("backbeat"));
        assert_eq!(view.pending_pattern_name(), None);
    }

    fn temp_directory(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("orpheus-samples-{name}-{}", unique_temp_suffix()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn unique_temp_suffix() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        format!("{timestamp}-{counter}")
    }

    fn write_wav(path: impl AsRef<Path>, frames: &[f32]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in frames {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn edge_envelope(frame_index: u32, total_frames: u32) -> f32 {
        let ramp_frames = total_frames.div_ceil(2).clamp(1, 32);
        let attack = normalized_edge_gain(frame_index, ramp_frames);
        let release = normalized_edge_gain(
            total_frames.saturating_sub(frame_index.saturating_add(1)),
            ramp_frames,
        );
        attack.min(release)
    }

    #[allow(clippy::cast_precision_loss)]
    fn normalized_edge_gain(distance_from_edge: u32, ramp_frames: u32) -> f32 {
        (((distance_from_edge as f32) + 0.5) / (ramp_frames as f32)).min(1.0)
    }
}
