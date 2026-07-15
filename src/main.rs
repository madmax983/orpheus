//! The root binary crate for the Orpheus live-coding audio environment.
//!
//! This crate parses command-line arguments, sets up the real-time audio backend using `cpal`,
//! launches the Digital Signal Processing engine, and hands over control to the `orpheus-lang`
//! Read-Eval-Print Loop (REPL) or Terminal User Interface (TUI).

use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use crossterm::style::Stylize;
use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;

fn main() {
    if let Err(error) = run() {
        eprintln!("{} {}", "\u{2717} Failed:".red().bold(), error);
        for cause in error.chain().skip(1) {
            eprintln!("  {} {}", "->".dark_grey(), cause);
        }
        std::process::exit(1);
    }
}

/// The default number of cycles rendered by `orpheus render` when `--cycles` is
/// omitted (the reference song's full 36-cycle timeline).
const DEFAULT_RENDER_CYCLES: u64 = 36;

#[derive(Debug)]
enum CliAction {
    Help,
    Version,
    Run(Option<PathBuf>),
    /// Headless `render <file> --master <out.wav> [--cycles N]` subcommand:
    /// render a `.ode` file to a master WAV without starting the REPL/audio.
    RenderMaster {
        path: PathBuf,
        out: PathBuf,
        cycles: u64,
    },
}

fn run() -> anyhow::Result<()> {
    let action = startup_path_from_args(env::args_os().skip(1))?;
    let startup_path = match action {
        CliAction::Help => {
            print_help();
            return Ok(());
        }
        CliAction::Version => {
            println!("orpheus {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        CliAction::RenderMaster { path, out, cycles } => {
            return render_master(&path, &out, cycles);
        }
        CliAction::Run(path) => path,
    };

    let (engine, _stream, warning) = match start_live_audio() {
        Ok((engine, stream)) => (engine, Some(stream), None),
        Err(error) => {
            let mut message = format!(
                "{}\n  {}",
                "Audio Output Disabled:".yellow().bold(),
                error.to_string().red()
            );
            for cause in error.chain().skip(1) {
                use std::fmt::Write;
                let _ = write!(
                    &mut message,
                    "\n  {} {}",
                    "->".dark_grey(),
                    cause.to_string().dark_grey()
                );
            }
            (EngineHandle::stub(), None, Some(message))
        }
    };

    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        orpheus_lang::run_with_engine_and_path(engine, startup_path.as_deref(), warning)?;
    } else {
        orpheus_lang::run_stdio_with_engine_and_path(engine, startup_path.as_deref(), warning)?;
    }
    Ok(())
}

fn startup_path_from_args(args: impl IntoIterator<Item = OsString>) -> anyhow::Result<CliAction> {
    let args = args.into_iter().collect::<Vec<_>>();
    if args
        .first()
        .is_some_and(|first| first.to_string_lossy() == "render")
    {
        return parse_render_args(&args[1..]);
    }
    match args.as_slice() {
        [] => Ok(CliAction::Run(None)),
        [path] => {
            let path_str = path.to_string_lossy();
            if path_str == "--help" || path_str == "-h" {
                return Ok(CliAction::Help);
            }
            if path_str == "--version" || path_str == "-V" {
                return Ok(CliAction::Version);
            }
            if path_str.starts_with('-') {
                return Err(anyhow!(
                    "unexpected argument {} found\n\n{} orpheus {}\n\nFor more information, try {}.",
                    format!("'{path_str}'").yellow().bold(),
                    "Usage:".green().bold(),
                    "[PATH]".cyan(),
                    "'--help'".green()
                ));
            }
            Ok(CliAction::Run(Some(PathBuf::from(path))))
        }
        _ => Err(anyhow!(
            "{} orpheus {}",
            "Usage:".green().bold(),
            "[path/to/song.ode]".cyan()
        )),
    }
}

/// Parse the arguments that follow the `render` subcommand:
/// `render <file> --master <out.wav> [--cycles N]`.
fn parse_render_args(args: &[OsString]) -> anyhow::Result<CliAction> {
    const USAGE: &str = "orpheus render <file> --master <out.wav> [--cycles N]";

    let mut path: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut cycles: u64 = DEFAULT_RENDER_CYCLES;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let text = arg.to_string_lossy();
        match text.as_ref() {
            "--master" => {
                let value = iter.next().ok_or_else(|| {
                    anyhow!(
                        "{} `--master` requires an output WAV path\n\n{} {}",
                        "error:".red().bold(),
                        "Usage:".green().bold(),
                        USAGE.cyan()
                    )
                })?;
                out = Some(PathBuf::from(value));
            }
            "--cycles" => {
                let value = iter.next().ok_or_else(|| {
                    anyhow!(
                        "{} `--cycles` requires a positive integer\n\n{} {}",
                        "error:".red().bold(),
                        "Usage:".green().bold(),
                        USAGE.cyan()
                    )
                })?;
                let parsed = value
                    .to_string_lossy()
                    .parse::<u64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| {
                        anyhow!(
                            "{} `--cycles` must be a positive integer, found {}",
                            "error:".red().bold(),
                            format!("'{}'", value.to_string_lossy()).yellow().bold()
                        )
                    })?;
                cycles = parsed;
            }
            other if other.starts_with('-') => {
                return Err(anyhow!(
                    "{} unexpected argument {} for `render`\n\n{} {}",
                    "error:".red().bold(),
                    format!("'{other}'").yellow().bold(),
                    "Usage:".green().bold(),
                    USAGE.cyan()
                ));
            }
            _ => {
                if path.is_none() {
                    path = Some(PathBuf::from(arg));
                } else {
                    return Err(anyhow!(
                        "{} unexpected argument {} for `render`\n\n{} {}",
                        "error:".red().bold(),
                        format!("'{text}'").yellow().bold(),
                        "Usage:".green().bold(),
                        USAGE.cyan()
                    ));
                }
            }
        }
    }

    let path = path.ok_or_else(|| {
        anyhow!(
            "{} `render` requires an input {} file\n\n{} {}",
            "error:".red().bold(),
            ".ode".cyan(),
            "Usage:".green().bold(),
            USAGE.cyan()
        )
    })?;
    let out = out.ok_or_else(|| {
        anyhow!(
            "{} `render` requires {}\n\n{} {}",
            "error:".red().bold(),
            "--master <out.wav>".cyan(),
            "Usage:".green().bold(),
            USAGE.cyan()
        )
    })?;

    Ok(CliAction::RenderMaster { path, out, cycles })
}

/// The stack size, in bytes, of the dedicated worker thread that runs an
/// offline master render.
///
/// Offline rendering drives deeply-recursive work: the pattern evaluator walks
/// nested combinator trees, and the DSP graph processor (Faust-style
/// `Seq`/`Par`/`Spl`/`Mrg`/`Rec` combinators) recurses through the whole node
/// tree once per frame. On Linux and macOS the ~8 MiB default *main-thread*
/// stack absorbs this, but Windows gives the main thread only ~1 MiB by
/// default — a deep-enough user graph there can blow past the guard page and
/// crash with `STATUS_ACCESS_VIOLATION` (0xc0000005). Running the render on a
/// worker thread with a large, explicit stack makes the render's headroom
/// independent of the platform's main-thread stack size.
const RENDER_THREAD_STACK_SIZE: usize = 64 * 1024 * 1024;

/// Render a `.ode` file down to a master WAV without starting cpal/audio.
///
/// The actual work runs on a dedicated worker thread with an explicit
/// [`RENDER_THREAD_STACK_SIZE`] stack so the render never depends on the
/// platform's (Windows-small) main-thread stack; see that constant for why.
fn render_master(path: &Path, out: &Path, cycles: u64) -> anyhow::Result<()> {
    let path = path.to_path_buf();
    let out = out.to_path_buf();
    std::thread::Builder::new()
        .name("orpheus-render".to_owned())
        .stack_size(RENDER_THREAD_STACK_SIZE)
        .spawn(move || render_master_inner(&path, &out, cycles))
        .context("failed to spawn offline render worker thread")?
        .join()
        .map_err(|_| anyhow!("offline render worker thread panicked"))?
}

/// The body of an offline master render, executed on the render worker thread.
///
/// Drives a headless [`ReplSession`] backed by a stub engine: load the file,
/// then reuse the existing `:export master` offline render path.
fn render_master_inner(path: &Path, out: &Path, cycles: u64) -> anyhow::Result<()> {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    session
        .open_file(path)
        .map_err(|error| anyhow!("failed to open `{}`: {error}", path.display()))?;
    let message = session
        .eval_line(&format!(":export master {} {}", out.display(), cycles))
        .map_err(|error| anyhow!("failed to render master: {error}"))?;
    println!("{} {}", "\u{2713}".green(), message.green());
    Ok(())
}

fn print_help() {
    println!(
        "{} - A cycle-based live-coding audio environment",
        "Orpheus".cyan().bold()
    );
    println!();
    println!("{} orpheus [OPTIONS] [PATH]", "Usage:".yellow().bold());
    println!();
    println!("{}", "Arguments:".yellow().bold());
    println!(
        "  {}  Optional startup .ode file to load",
        "[PATH]".green().bold(),
    );
    println!();
    println!("{}", "Options:".yellow().bold());
    println!("  {}       Print help", "-h, --help".green().bold());
    println!("  {}    Print version", "-V, --version".green().bold());
    println!();
    println!("{}", "Dashboard Mode:".yellow().bold());
    println!("  Run without a file to open the interactive live-coding TUI/REPL.");
    println!();
    println!("{}", "Render Mode (no REPL):".yellow().bold());
    println!(
        "  orpheus render {} --master {} [--cycles {}]",
        "<file>".green().bold(),
        "<out.wav>".green().bold(),
        "N".green().bold(),
    );
    println!("  Render a .ode file to a master WAV headlessly (default cycles = 36).");
}

fn start_live_audio() -> anyhow::Result<(EngineHandle, Stream)> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("no default output device is available")?;
    let supported_config = device
        .supported_output_configs()
        .context("failed to query supported audio configs")?
        .find(|config| config.sample_format() == SampleFormat::F32)
        .map(cpal::SupportedStreamConfigRange::with_max_sample_rate)
        .or_else(|| {
            device
                .default_output_config()
                .ok()
                .filter(|config| config.sample_format() == SampleFormat::F32)
        })
        .ok_or_else(|| anyhow!("default output device does not expose an f32 stream config"))?;
    let config = supported_config.config();
    let (engine, mut renderer) = EngineHandle::split_for_stream_config(&config)
        .map_err(|error| anyhow!("failed to initialize render engine: {error}"))?;
    let stream = device
        .build_output_stream(
            &config,
            move |output: &mut [f32], _info| {
                if renderer.render_into_interleaved(output).is_err() {
                    output.fill(0.0);
                }
            },
            |error| {
                eprintln!("{} {}", "\u{2717} Audio stream error:".red().bold(), error);
            },
            None,
        )
        .context("failed to build the audio output stream")?;
    stream
        .play()
        .context("failed to start the audio output stream")?;

    Ok((engine, stream))
}

#[cfg(test)]
impl PartialEq for CliAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Help, Self::Help) | (Self::Version, Self::Version) => true,
            (Self::Run(l0), Self::Run(r0)) => l0 == r0,
            (
                Self::RenderMaster {
                    path: lp,
                    out: lo,
                    cycles: lc,
                },
                Self::RenderMaster {
                    path: rp,
                    out: ro,
                    cycles: rc,
                },
            ) => lp == rp && lo == ro && lc == rc,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    /// Regression guard for the Windows `STATUS_ACCESS_VIOLATION` (0xc0000005)
    /// crash: an offline render must not depend on the caller's stack size.
    ///
    /// `render_master` re-spawns the deeply-recursive render onto a worker
    /// thread with a large explicit stack ([`RENDER_THREAD_STACK_SIZE`]). We
    /// drive it here from a deliberately tiny (96 KiB) caller-thread stack
    /// using a deep DSP graph: with the re-spawn the render succeeds regardless
    /// of the caller stack; if the re-spawn is removed the deep per-frame graph
    /// recursion runs on the 96 KiB caller stack and overflows — exactly the
    /// class of failure Windows' ~1 MiB main-thread stack exhibits.
    #[test]
    fn render_master_is_independent_of_caller_stack_size() {
        // A pure graph-voice program (no sample files needed) with a deep
        // effect chain, kept within the parser's AST-depth cap.
        let mut chain = String::from("saw(freq)");
        for _ in 0..24 {
            chain.push_str(" |> gain(0.5)");
        }
        let source = format!("v = voice {{ osc = saw(freq) ; {chain} }}\nm = v v v v\n");

        let unique = format!(
            "{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir();
        let ode_path = dir.join(format!("orpheus_render_stack_{unique}.ode"));
        let wav_path = dir.join(format!("orpheus_render_stack_{unique}.wav"));
        std::fs::write(&ode_path, source).expect("write temp .ode");

        // 96 KiB: small enough that running this deep render *inline* on this
        // thread overflows (verified empirically), yet ample for
        // `render_master`'s own spawn/join wrapper. The fix moves the heavy
        // work onto its own large-stack worker, so the render must still
        // succeed here; without the fix this thread would overflow — the same
        // failure mode as Windows' ~1 MiB main-thread stack.
        let ode_for_thread = ode_path.clone();
        let wav_for_thread = wav_path.clone();
        let result = std::thread::Builder::new()
            .stack_size(96 * 1024)
            .spawn(move || render_master(&ode_for_thread, &wav_for_thread, 1))
            .expect("spawn small-stack caller")
            .join()
            .expect("small-stack caller thread must not overflow");

        let _ = std::fs::remove_file(&ode_path);
        let _ = std::fs::remove_file(&wav_path);

        assert!(result.is_ok(), "render_master failed: {result:?}");
    }

    #[test]
    fn startup_path_from_args_handles_empty_args() {
        let args: Vec<OsString> = vec![];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Run(None) => (),
            _ => panic!("Expected Run(None)"),
        }
    }

    #[test]
    fn startup_path_from_args_handles_help_flag() {
        let args = vec![OsString::from("--help")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Help => (),
            _ => panic!("Expected Help"),
        }

        let args = vec![OsString::from("-h")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Help => (),
            _ => panic!("Expected Help"),
        }
    }

    #[test]
    fn startup_path_from_args_handles_version_flag() {
        let args = vec![OsString::from("--version")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Version => (),
            _ => panic!("Expected Version"),
        }

        let args = vec![OsString::from("-V")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Version => (),
            _ => panic!("Expected Version"),
        }
    }

    #[test]
    fn startup_path_from_args_handles_file_path() {
        let args = vec![OsString::from("test.ode")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Run(Some(path)) => assert_eq!(path, PathBuf::from("test.ode")),
            _ => panic!("Expected Run(Some)"),
        }
    }

    #[test]
    fn startup_path_from_args_rejects_unknown_flags() {
        let args = vec![OsString::from("--unknown")];
        let error = startup_path_from_args(args).unwrap_err();
        assert!(error.to_string().contains("unexpected argument"));
    }

    #[test]
    fn startup_path_from_args_rejects_multiple_args() {
        let args = vec![OsString::from("file1.ode"), OsString::from("file2.ode")];
        let error = startup_path_from_args(args).unwrap_err();
        assert!(error.to_string().contains("orpheus"));
        assert!(error.to_string().contains("[path/to/song.ode]"));
    }

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn render_parses_file_master_and_cycles() {
        let args = os(&[
            "render", "song.ode", "--master", "out.wav", "--cycles", "12",
        ]);
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(
            action,
            CliAction::RenderMaster {
                path: PathBuf::from("song.ode"),
                out: PathBuf::from("out.wav"),
                cycles: 12,
            }
        );
    }

    #[test]
    fn render_defaults_cycles_to_36() {
        let args = os(&["render", "song.ode", "--master", "out.wav"]);
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(
            action,
            CliAction::RenderMaster {
                path: PathBuf::from("song.ode"),
                out: PathBuf::from("out.wav"),
                cycles: 36,
            }
        );
    }

    #[test]
    fn render_accepts_flags_before_positional_file() {
        // Flag order should not matter; `--master` may precede the input file.
        let args = os(&["render", "--master", "out.wav", "song.ode"]);
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(
            action,
            CliAction::RenderMaster {
                path: PathBuf::from("song.ode"),
                out: PathBuf::from("out.wav"),
                cycles: 36,
            }
        );
    }

    #[test]
    fn render_requires_input_file() {
        let args = os(&["render", "--master", "out.wav"]);
        let error = startup_path_from_args(args).unwrap_err();
        assert!(
            error.to_string().contains(".ode"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn render_requires_master_output() {
        let args = os(&["render", "song.ode"]);
        let error = startup_path_from_args(args).unwrap_err();
        assert!(
            error.to_string().contains("--master"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn render_rejects_master_without_value() {
        let args = os(&["render", "song.ode", "--master"]);
        let error = startup_path_from_args(args).unwrap_err();
        assert!(
            error.to_string().contains("--master"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn render_rejects_non_numeric_cycles() {
        let args = os(&["render", "song.ode", "--master", "out.wav", "--cycles", "x"]);
        let error = startup_path_from_args(args).unwrap_err();
        assert!(
            error.to_string().contains("--cycles"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn render_rejects_zero_cycles() {
        let args = os(&["render", "song.ode", "--master", "out.wav", "--cycles", "0"]);
        let error = startup_path_from_args(args).unwrap_err();
        assert!(
            error.to_string().contains("--cycles"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn render_rejects_extra_positional_args() {
        let args = os(&["render", "a.ode", "b.ode", "--master", "out.wav"]);
        let error = startup_path_from_args(args).unwrap_err();
        assert!(
            error.to_string().contains("unexpected argument"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn render_rejects_unknown_flag() {
        let args = os(&["render", "song.ode", "--master", "out.wav", "--bogus"]);
        let error = startup_path_from_args(args).unwrap_err();
        assert!(
            error.to_string().contains("unexpected argument"),
            "unexpected error: {error}"
        );
    }
}
