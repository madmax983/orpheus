import re

with open('crates/orpheus-dsp/src/voice.rs', 'r') as f:
    content = f.read()

# Replace the next_mono_sample method
search = """    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn next_mono_sample(&mut self) -> Option<f32> {
        match &mut self.state {
            ActiveVoiceState::DrumSynth {
                kind,
                frame_index,
                duration_frames,
                sample_rate_hz,
                noise_state,
            } => {
                if *frame_index >= *duration_frames {
                    return None;
                }

                let progress = f64::from(*frame_index) / f64::from(*duration_frames);
                let time = f64::from(*frame_index) / *sample_rate_hz;
                let envelope = 1.0 - progress;
                let sample = match kind {
                    VoiceKind::KickLike => {
                        let frequency = (-120.0_f64).mul_add(progress, 160.0);
                        (time * frequency * f64::from(TAU)).sin() * envelope.powi(3) * 0.85
                    }
                    VoiceKind::SnareLike => next_noise(noise_state) * envelope.powi(2) * 0.65,
                    VoiceKind::ClapLike => {
                        let burst = if progress < 0.12 || (0.2..0.32).contains(&progress) {
                            1.0
                        } else {
                            0.5
                        };
                        next_noise(noise_state) * envelope.powi(2) * burst * 0.55
                    }
                    VoiceKind::HiHatLike => {
                        next_noise(noise_state).signum() * envelope.powi(2) * 0.35
                    }
                    VoiceKind::AnalogSaw
                    | VoiceKind::AnalogPulse
                    | VoiceKind::AnalogTri
                    | VoiceKind::AnalogNoise => {
                        unreachable!("analog voices should not enter the drum synth path")
                    }
                };

                *frame_index = frame_index.saturating_add(1);
                Some(sample as f32)
            }
            ActiveVoiceState::AnalogSynth {
                voice,
                params,
                frame_index,
                duration_frames,
            } => {
                if *frame_index >= *duration_frames {
                    return None;
                }

                let envelope = sample_edge_envelope(
                    *frame_index,
                    *duration_frames,
                    synth_edge_ramp_frames(*duration_frames),
                ) as f32;
                let sample = voice.next_sample(params) * envelope;
                *frame_index = frame_index.saturating_add(1);
                Some(sample)
            }
            ActiveVoiceState::Sample {
                frames,
                frame_position,
                frame_step,
                frame_limit,
                gain,
                high_pass,
                low_pass,
                rendered_frames,
                total_output_frames,
                edge_ramp_frames,
            } => {
                if *rendered_frames >= *total_output_frames {
                    return None;
                }
                let index = if frame_step.is_sign_negative() {
                    if *frame_position <= *frame_limit {
                        return None;
                    }
                    (frame_position.ceil() as usize).checked_sub(1)?
                } else {
                    if *frame_position >= *frame_limit {
                        return None;
                    }
                    frame_position.floor() as usize
                };
                let mut sample = f64::from(*frames.get(index)?) * *gain;
                if let Some(filter) = low_pass {
                    sample = filter.process(sample);
                }
                if let Some(filter) = high_pass {
                    sample = filter.process(sample);
                }
                let envelope =
                    sample_edge_envelope(*rendered_frames, *total_output_frames, *edge_ramp_frames);
                let sample = (sample * envelope) as f32;
                *frame_position += *frame_step;
                *rendered_frames = rendered_frames.saturating_add(1);
                Some(sample)
            }
        }
    }"""

replace = """    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn next_mono_sample(&mut self) -> Option<f32> {
        match &mut self.state {
            ActiveVoiceState::DrumSynth {
                kind,
                frame_index,
                duration_frames,
                sample_rate_hz,
                noise_state,
            } => next_drum_synth_sample(
                kind,
                frame_index,
                *duration_frames,
                *sample_rate_hz,
                noise_state,
            ),
            ActiveVoiceState::AnalogSynth {
                voice,
                params,
                frame_index,
                duration_frames,
            } => next_analog_synth_sample(voice, params, frame_index, *duration_frames),
            ActiveVoiceState::Sample {
                frames,
                frame_position,
                frame_step,
                frame_limit,
                gain,
                high_pass,
                low_pass,
                rendered_frames,
                total_output_frames,
                edge_ramp_frames,
            } => next_playback_sample(
                frames,
                frame_position,
                *frame_step,
                *frame_limit,
                *gain,
                high_pass,
                low_pass,
                rendered_frames,
                *total_output_frames,
                *edge_ramp_frames,
            ),
        }
    }"""

if search not in content:
    print("Could not find the target string!")
else:
    content = content.replace(search, replace)

    # Add the helper functions to the end of the file
    helpers = """

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn next_drum_synth_sample(
    kind: &VoiceKind,
    frame_index: &mut u32,
    duration_frames: u32,
    sample_rate_hz: f64,
    noise_state: &mut u32,
) -> Option<f32> {
    if *frame_index >= duration_frames {
        return None;
    }

    let progress = f64::from(*frame_index) / f64::from(duration_frames);
    let time = f64::from(*frame_index) / sample_rate_hz;
    let envelope = 1.0 - progress;
    let sample = match kind {
        VoiceKind::KickLike => {
            let frequency = (-120.0_f64).mul_add(progress, 160.0);
            (time * frequency * f64::from(TAU)).sin() * envelope.powi(3) * 0.85
        }
        VoiceKind::SnareLike => next_noise(noise_state) * envelope.powi(2) * 0.65,
        VoiceKind::ClapLike => {
            let burst = if progress < 0.12 || (0.2..0.32).contains(&progress) {
                1.0
            } else {
                0.5
            };
            next_noise(noise_state) * envelope.powi(2) * burst * 0.55
        }
        VoiceKind::HiHatLike => next_noise(noise_state).signum() * envelope.powi(2) * 0.35,
        VoiceKind::AnalogSaw
        | VoiceKind::AnalogPulse
        | VoiceKind::AnalogTri
        | VoiceKind::AnalogNoise => {
            unreachable!("analog voices should not enter the drum synth path")
        }
    };

    *frame_index = frame_index.saturating_add(1);
    Some(sample as f32)
}

fn next_analog_synth_sample(
    voice: &mut AnalogVoice,
    params: &AnalogVoiceParams,
    frame_index: &mut u32,
    duration_frames: u32,
) -> Option<f32> {
    if *frame_index >= duration_frames {
        return None;
    }

    let envelope = sample_edge_envelope(
        *frame_index,
        duration_frames,
        synth_edge_ramp_frames(duration_frames),
    ) as f32;
    let sample = voice.next_sample(params) * envelope;
    *frame_index = frame_index.saturating_add(1);
    Some(sample)
}

#[allow(clippy::cast_precision_loss, clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn next_playback_sample(
    frames: &[f32],
    frame_position: &mut f64,
    frame_step: f64,
    frame_limit: f64,
    gain: f64,
    high_pass: &mut Option<OnePoleHighPass>,
    low_pass: &mut Option<OnePoleLowPass>,
    rendered_frames: &mut u32,
    total_output_frames: u32,
    edge_ramp_frames: u32,
) -> Option<f32> {
    if *rendered_frames >= total_output_frames {
        return None;
    }
    let index = if frame_step.is_sign_negative() {
        if *frame_position <= frame_limit {
            return None;
        }
        (frame_position.ceil() as usize).checked_sub(1)?
    } else {
        if *frame_position >= frame_limit {
            return None;
        }
        frame_position.floor() as usize
    };
    let mut sample = f64::from(*frames.get(index)?) * gain;
    if let Some(filter) = low_pass {
        sample = filter.process(sample);
    }
    if let Some(filter) = high_pass {
        sample = filter.process(sample);
    }
    let envelope = sample_edge_envelope(*rendered_frames, total_output_frames, edge_ramp_frames);
    let sample = (sample * envelope) as f32;
    *frame_position += frame_step;
    *rendered_frames = rendered_frames.saturating_add(1);
    Some(sample)
}
"""
    with open('crates/orpheus-dsp/src/voice.rs', 'w') as f:
        f.write(content + helpers)
    print("Done")
