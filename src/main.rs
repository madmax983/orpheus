use std::io::IsTerminal;

use anyhow::{Context, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use orpheus_dsp::EngineHandle;

fn main() -> anyhow::Result<()> {
    let (engine, _stream) = match start_live_audio() {
        Ok((engine, stream)) => (engine, Some(stream)),
        Err(error) => {
            eprintln!("audio output disabled: {error}");
            (EngineHandle::stub(), None)
        }
    };

    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        orpheus_lang::tui::run_with_engine(engine)?;
    } else {
        orpheus_lang::repl::run_stdio_with_engine(engine)?;
    }
    Ok(())
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
