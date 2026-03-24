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
        eprintln!("{} {:?}", "✗ error:".red().bold(), error);
        std::process::exit(1);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Run(Option<PathBuf>),
    Help,
    Version,
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
        Err(error) => (
            EngineHandle::stub(),
            None,
            Some(format!("audio output disabled: {error}")),
        ),
    };

    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        orpheus_lang::tui::run_with_engine_and_path(engine, startup_path.as_deref(), warning)?;
    } else {
        orpheus_lang::repl::run_stdio_with_engine_and_path(
            engine,
            startup_path.as_deref(),
            warning,
        )?;
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
                    "unexpected argument '{path_str}' found\n\nUsage: orpheus [PATH]\n\nFor more information, try '--help'."
                ));
            }
            Ok(CliAction::Run(Some(PathBuf::from(path))))
        }
        _ => Err(anyhow!("usage: orpheus [path/to/song.ode]")),
    }
}

fn print_help() {
    println!("\x1b[1;36mOrpheus\x1b[0m - A cycle-based live-coding audio environment");
    println!();
    println!("\x1b[1;33mUsage:\x1b[0m orpheus [OPTIONS] [PATH]");
    println!();
    println!("\x1b[1;33mArguments:\x1b[0m");
    println!("  \x1b[1;32m[PATH]\x1b[0m  Optional startup .ode file to load");
    println!();
    println!("\x1b[1;33mOptions:\x1b[0m");
    println!("  \x1b[1;32m-h, --help\x1b[0m     Print help");
    println!("  \x1b[1;32m-V, --version\x1b[0m  Print version");
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
            |error| eprintln!("audio stream error: {error}"),
            None,
        )
        .context("failed to build the audio output stream")?;
    stream
        .play()
        .context("failed to start the audio output stream")?;

    Ok((engine, stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_return_run_none_when_args_empty() {
        let args: Vec<OsString> = vec![];
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(action, CliAction::Run(None));
    }

    #[test]
    fn should_return_run_path_when_single_arg_provided() {
        let args: Vec<OsString> = vec![OsString::from("song.ode")];
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(action, CliAction::Run(Some(PathBuf::from("song.ode"))));
    }

    #[test]
    fn should_return_help_when_dash_h_provided() {
        let args: Vec<OsString> = vec![OsString::from("-h")];
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(action, CliAction::Help);
    }

    #[test]
    fn should_return_help_when_dash_dash_help_provided() {
        let args: Vec<OsString> = vec![OsString::from("--help")];
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(action, CliAction::Help);
    }

    #[test]
    fn should_return_version_when_dash_v_provided() {
        let args: Vec<OsString> = vec![OsString::from("-V")];
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(action, CliAction::Version);
    }

    #[test]
    fn should_return_version_when_dash_dash_version_provided() {
        let args: Vec<OsString> = vec![OsString::from("--version")];
        let action = startup_path_from_args(args).unwrap();
        assert_eq!(action, CliAction::Version);
    }

    #[test]
    fn should_return_error_when_invalid_flag_provided() {
        let args: Vec<OsString> = vec![OsString::from("--invalid")];
        let err = startup_path_from_args(args).unwrap_err();
        assert!(
            err.to_string()
                .contains("unexpected argument '--invalid' found")
        );
    }

    #[test]
    fn should_return_error_when_multiple_args_provided() {
        let args: Vec<OsString> = vec![OsString::from("file1.ode"), OsString::from("file2.ode")];
        let err = startup_path_from_args(args).unwrap_err();
        assert_eq!(err.to_string(), "usage: orpheus [path/to/song.ode]");
    }
}
