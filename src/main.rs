//! The root binary crate for the Orpheus live-coding audio environment.
//!
//! This crate parses command-line arguments, sets up the real-time audio backend using `cpal`,
//! launches the Digital Signal Processing engine, and hands over control to the `orpheus-lang`
//! Read-Eval-Print Loop (REPL) or Terminal User Interface (TUI).

use std::env;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use crossterm::style::Stylize;
use orpheus_dsp::EngineHandle;

fn main() {
    if let Err(error) = run() {
        eprintln!("{} {}", "\u{2717} Failed:".red().bold(), error);
        for cause in error.chain().skip(1) {
            eprintln!("  {} {}", "->".dark_grey(), cause);
        }
        std::process::exit(1);
    }
}

#[derive(Debug)]
enum CliAction {
    Help,
    Version,
    Run(Option<PathBuf>),
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
        CliAction::Run(path) => path,
    };

    let (engine, _stream, warning) = match start_live_audio() {
        Ok((engine, stream)) => (engine, Some(stream), None),
        Err(error) => {
            let mut message = format!("{}\n  {}", "Audio Output Disabled:".yellow().bold(), error);
            for cause in error.chain().skip(1) {
                use std::fmt::Write;
                let _ = write!(&mut message, "\n  {} {}", "->".dark_grey(), cause);
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
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

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
}
