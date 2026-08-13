//! Integration tests verifying the initialization and execution of the virtual analog pedal graph runtime.

#![allow(
    clippy::suboptimal_flops,
    clippy::cast_precision_loss,
    clippy::needless_range_loop
)]
use std::sync::Arc;

use orpheus_dsp::{
    ClipModel, FilterMode, NodeRef, PEDAL_CONTROL_INTERVAL_SAMPLES, PedalInstance, PedalNode,
    PedalProgram, PedalStage, PreampModel, SignalKind, ToneModel,
};

const SAMPLE_RATE_HZ: f32 = 48_000.0;
const FRAMES: usize = 256;

fn sine_buffer(freq_hz: f32, amplitude: f32) -> Vec<f32> {
    (0..FRAMES)
        .map(|index| {
            let phase = (index as f32) * core::f32::consts::TAU * freq_hz / SAMPLE_RATE_HZ;
            phase.sin() * amplitude
        })
        .collect()
}

fn render(program: Arc<PedalProgram>, input: &[f32]) -> Vec<f32> {
    let mut runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);
    let mut output = vec![0.0_f32; input.len()];
    runtime.process_buffer(input, &mut output);
    output
}

#[test]
fn pedal_runtime_renders_serial_chain() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { input |> preamp(model=jfet_clean) |> clip(model=silicon_hard) |> tone(model=mid_hump) |> output }",
            "serial chain",
        )
        .with_graph(
            vec![
                PedalNode::constant(4.0),
                PedalNode::stage(PedalStage::Preamp {
                    input: NodeRef::Input,
                    gain: NodeRef::node(0),
                    model: PreampModel::JfetClean,
                }),
                PedalNode::constant(1.8),
                PedalNode::stage(PedalStage::Clip {
                    input: NodeRef::node(1),
                    drive: NodeRef::node(2),
                    model: ClipModel::SiliconHard,
                }),
                PedalNode::constant(1_800.0),
                PedalNode::constant(0.35),
                PedalNode::stage(PedalStage::Tone {
                    input: NodeRef::node(3),
                    cutoff_hz: NodeRef::node(4),
                    resonance: NodeRef::node(5),
                    model: ToneModel::MidHump,
                }),
            ],
            NodeRef::node(6),
        ),
    );

    let input = sine_buffer(220.0, 0.45);
    let output = render(program, &input);

    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(output.iter().any(|sample| sample.abs() > 1.0e-4));
}

#[test]
fn pedal_runtime_mixes_named_branches() {
    let input = sine_buffer(330.0, 0.35);

    let dry_program = Arc::new(
        PedalProgram::new("graph { input |> output }", "dry only")
            .with_graph(Vec::new(), NodeRef::Input),
    );
    let wet_program = Arc::new(
        PedalProgram::new(
            "graph { input |> preamp(model=opamp_tight) |> clip(model=red_led) |> output }",
            "wet only",
        )
        .with_graph(
            vec![
                PedalNode::constant(5.0),
                PedalNode::stage(PedalStage::Preamp {
                    input: NodeRef::Input,
                    gain: NodeRef::node(0),
                    model: PreampModel::OpampTight,
                }),
                PedalNode::constant(1.4),
                PedalNode::stage(PedalStage::Clip {
                    input: NodeRef::node(1),
                    drive: NodeRef::node(2),
                    model: ClipModel::RedLed,
                }),
            ],
            NodeRef::node(3),
        ),
    );
    let mixed_program = Arc::new(
        PedalProgram::new(
            "graph { dry = input; wet = input |> preamp(model=opamp_tight) |> clip(model=red_led); mix(dry * 0.25, wet * 0.75) |> output }",
            "dry wet mix",
        )
        .with_graph(
            vec![
                PedalNode::constant(5.0),
                PedalNode::stage(PedalStage::Preamp {
                    input: NodeRef::Input,
                    gain: NodeRef::node(0),
                    model: PreampModel::OpampTight,
                }),
                PedalNode::constant(1.4),
                PedalNode::stage(PedalStage::Clip {
                    input: NodeRef::node(1),
                    drive: NodeRef::node(2),
                    model: ClipModel::RedLed,
                }),
                PedalNode::constant(0.25),
                PedalNode::mul(NodeRef::Input, NodeRef::node(4)),
                PedalNode::constant(0.75),
                PedalNode::mul(NodeRef::node(3), NodeRef::node(6)),
                PedalNode::mix(vec![NodeRef::node(5), NodeRef::node(7)]),
            ],
            NodeRef::node(8),
        ),
    );

    let dry = render(dry_program, &input);
    let wet = render(wet_program, &input);
    let mixed = render(mixed_program, &input);

    for ((mixed_sample, dry_sample), wet_sample) in mixed.iter().zip(&dry).zip(&wet) {
        let expected = (dry_sample * 0.25) + (wet_sample * 0.75);
        assert!((mixed_sample - expected).abs() < 1.0e-4);
    }
}

#[test]
fn pedal_models_produce_distinct_transfer_shapes() {
    let input = sine_buffer(440.0, 0.55);

    let silicon = Arc::new(
        PedalProgram::new(
            "graph { input |> clip(model=silicon_hard) |> output }",
            "silicon",
        )
        .with_graph(
            vec![
                PedalNode::constant(2.0),
                PedalNode::stage(PedalStage::Clip {
                    input: NodeRef::Input,
                    drive: NodeRef::node(0),
                    model: ClipModel::SiliconHard,
                }),
            ],
            NodeRef::node(1),
        ),
    );
    let germanium = Arc::new(
        PedalProgram::new(
            "graph { input |> clip(model=germanium_soft) |> output }",
            "germanium",
        )
        .with_graph(
            vec![
                PedalNode::constant(2.0),
                PedalNode::stage(PedalStage::Clip {
                    input: NodeRef::Input,
                    drive: NodeRef::node(0),
                    model: ClipModel::GermaniumSoft,
                }),
            ],
            NodeRef::node(1),
        ),
    );

    let silicon_out = render(silicon, &input);
    let germanium_out = render(germanium, &input);
    let delta: f32 = silicon_out
        .iter()
        .zip(&germanium_out)
        .map(|(left, right)| (left - right).abs())
        .sum();

    assert!(silicon_out.iter().all(|sample| sample.is_finite()));
    assert!(germanium_out.iter().all(|sample| sample.is_finite()));
    assert!(
        delta > 0.25,
        "expected distinct model fingerprints, got delta={delta}"
    );
}

#[test]
fn pedal_filter_resonance_changes_response() {
    let input = sine_buffer(1_100.0, 0.55);

    let low_resonance = Arc::new(
        PedalProgram::new(
            "graph { input |> filter(kind=lowpass, cutoff=1400, resonance=0.0) |> output }",
            "low resonance",
        )
        .with_graph(
            vec![
                PedalNode::constant(1_400.0),
                PedalNode::constant(0.0),
                PedalNode::stage(PedalStage::Filter {
                    input: NodeRef::Input,
                    kind: FilterMode::LowPass,
                    cutoff_hz: NodeRef::node(0),
                    resonance: NodeRef::node(1),
                }),
            ],
            NodeRef::node(2),
        ),
    );
    let high_resonance = Arc::new(
        PedalProgram::new(
            "graph { input |> filter(kind=lowpass, cutoff=1400, resonance=0.9) |> output }",
            "high resonance",
        )
        .with_graph(
            vec![
                PedalNode::constant(1_400.0),
                PedalNode::constant(0.9),
                PedalNode::stage(PedalStage::Filter {
                    input: NodeRef::Input,
                    kind: FilterMode::LowPass,
                    cutoff_hz: NodeRef::node(0),
                    resonance: NodeRef::node(1),
                }),
            ],
            NodeRef::node(2),
        ),
    );

    let low = render(low_resonance, &input);
    let high = render(high_resonance, &input);
    let delta: f32 = low
        .iter()
        .zip(&high)
        .map(|(left, right)| (left - right).abs())
        .sum();

    assert!(
        delta > 0.05,
        "expected resonance to shape filter output, got delta={delta}"
    );
}

#[test]
fn pedal_tone_neutral_preserves_signal_shape() {
    let input = sine_buffer(440.0, 0.42);

    let neutral = Arc::new(
        PedalProgram::new(
            "graph { input |> tone(model=neutral) |> output }",
            "neutral tone",
        )
        .with_graph(
            vec![
                PedalNode::constant(1_800.0),
                PedalNode::constant(0.65),
                PedalNode::stage(PedalStage::Tone {
                    input: NodeRef::Input,
                    cutoff_hz: NodeRef::node(0),
                    resonance: NodeRef::node(1),
                    model: ToneModel::Neutral,
                }),
            ],
            NodeRef::node(2),
        ),
    );

    let output = render(neutral, &input);
    let max_delta = input
        .iter()
        .zip(&output)
        .map(|(left, right)| (left - right).abs())
        .fold(0.0_f32, f32::max);

    assert!(
        max_delta < 1.0e-4,
        "expected neutral tone to preserve the dry signal, got max_delta={max_delta}"
    );
}

#[test]
fn pedal_neutral_tone_reserves_no_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { input |> tone(model=neutral) |> output }",
            "neutral tone",
        )
        .with_graph(
            vec![
                PedalNode::constant(1_800.0),
                PedalNode::constant(0.65),
                PedalNode::stage(PedalStage::Tone {
                    input: NodeRef::Input,
                    cutoff_hz: NodeRef::node(0),
                    resonance: NodeRef::node(1),
                    model: ToneModel::Neutral,
                }),
            ],
            NodeRef::node(2),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert_eq!(runtime.tail_frames(), 0);
}

#[test]
fn pedal_lfo_modulates_cutoff_at_control_rate() {
    let trace_frames = PEDAL_CONTROL_INTERVAL_SAMPLES * 4;
    let rate_hz = 12.0_f32;
    let depth = 600.0_f32;
    let offset = 1_600.0_f32;
    let phase_increment =
        core::f32::consts::TAU * rate_hz * (PEDAL_CONTROL_INTERVAL_SAMPLES as f32) / SAMPLE_RATE_HZ;
    let control_trace_program = Arc::new(
        PedalProgram::new(
            "graph { wobble = lfo(rate=12, depth=600, offset=1600); wobble }",
            "cutoff trace",
        )
        .with_graph(
            vec![PedalNode::lfo(rate_hz, depth, offset)],
            NodeRef::node(0),
        ),
    );
    let trace = render(control_trace_program, &vec![0.0_f32; trace_frames]);

    for (block_index, chunk) in trace
        .chunks_exact(PEDAL_CONTROL_INTERVAL_SAMPLES)
        .enumerate()
    {
        let held = chunk[0];
        let expected = offset + ((block_index as f32) * phase_increment).sin() * depth;
        assert!(
            chunk.iter().all(|sample| (sample - held).abs() < 1.0e-6),
            "expected held control values within a control block, got {chunk:?}"
        );
        assert!(
            (held - expected).abs() < 1.0e-4,
            "expected control-rate LFO step {expected}, got {held}"
        );
    }
    assert!(
        (trace[0] - trace[PEDAL_CONTROL_INTERVAL_SAMPLES]).abs() > 1.0e-3,
        "expected cutoff control to advance between control blocks"
    );

    let modulated_program = Arc::new(
        PedalProgram::new(
            "graph { wobble = lfo(rate=12, depth=600, offset=1600); input |> filter(kind=lowpass, cutoff=wobble, resonance=0.1) |> output }",
            "modulated filter",
        )
        .with_graph(
            vec![
                PedalNode::lfo(12.0, 600.0, 1_600.0),
                PedalNode::constant(0.1),
                PedalNode::stage(PedalStage::Filter {
                    input: NodeRef::Input,
                    kind: FilterMode::LowPass,
                    cutoff_hz: NodeRef::node(0),
                    resonance: NodeRef::node(1),
                }),
            ],
            NodeRef::node(2),
        ),
    );
    let static_program = Arc::new(
        PedalProgram::new(
            "graph { input |> filter(kind=lowpass, cutoff=1600, resonance=0.1) |> output }",
            "static filter",
        )
        .with_graph(
            vec![
                PedalNode::constant(1_600.0),
                PedalNode::constant(0.1),
                PedalNode::stage(PedalStage::Filter {
                    input: NodeRef::Input,
                    kind: FilterMode::LowPass,
                    cutoff_hz: NodeRef::node(0),
                    resonance: NodeRef::node(1),
                }),
            ],
            NodeRef::node(2),
        ),
    );

    let input = sine_buffer(4_000.0, 0.7);
    let modulated = render(modulated_program, &input);
    let static_out = render(static_program, &input);
    let delta: f32 = modulated
        .iter()
        .zip(&static_out)
        .map(|(left, right)| (left - right).abs())
        .sum();

    assert!(
        delta > 0.05,
        "expected LFO-modulated cutoff to change the rendered signal"
    );
}

#[test]
fn pedal_control_chains_update_without_block_lag() {
    let trace_frames = PEDAL_CONTROL_INTERVAL_SAMPLES * 4;
    let rate_hz = 10.0_f32;
    let depth = 0.25_f32;
    let offset = 0.5_f32;
    let phase_increment =
        core::f32::consts::TAU * rate_hz * (PEDAL_CONTROL_INTERVAL_SAMPLES as f32) / SAMPLE_RATE_HZ;
    let program = Arc::new(
        PedalProgram::new("graph { wobble = lfo(...); wobble + 0.1 }", "control chain").with_graph(
            vec![
                PedalNode::lfo(rate_hz, depth, offset),
                PedalNode::constant(0.1),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Add {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
            ],
            NodeRef::node(2),
        ),
    );
    let trace = render(program, &vec![0.0_f32; trace_frames]);

    for (block_index, chunk) in trace
        .chunks_exact(PEDAL_CONTROL_INTERVAL_SAMPLES)
        .enumerate()
    {
        let expected = offset + ((block_index as f32) * phase_increment).sin() * depth + 0.1;
        assert!(
            (chunk[0] - expected).abs() < 1.0e-4,
            "expected control chain step {expected}, got {}",
            chunk[0]
        );
    }
}

#[test]
fn pedal_feedback_requires_explicit_delay() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { input |> feedback(amount=0.7, delay=1.sample) |> output }",
            "feedback",
        )
        .with_graph(
            vec![
                PedalNode::constant(0.7),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(0), 0, Some(2_400.0)),
            ],
            NodeRef::node(1),
        ),
    );
    let mut input = vec![0.0_f32; 32];
    input[0] = 1.0;

    let output = render(program, &input);

    assert!((output[0] - 1.0).abs() < 1.0e-6);
    assert!(
        output[1].abs() > 1.0e-4,
        "expected delayed feedback on the next sample"
    );
    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(
        output.iter().all(|sample| sample.abs() < 4.0),
        "expected bounded feedback output, got {output:?}"
    );
}

#[test]
fn pedal_feedback_zero_amount_has_no_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { input |> feedback(amount=0.0, delay=32.sample) |> output }",
            "zero feedback",
        )
        .with_graph(
            vec![
                PedalNode::constant(0.0),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(0), 32, None),
            ],
            NodeRef::node(1),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert_eq!(runtime.tail_frames(), 0);
}

#[test]
fn pedal_env_follow_captures_peaks_between_control_ticks() {
    let program = Arc::new(
        PedalProgram::new("graph { env = env_follow(input); env }", "env follower").with_graph(
            vec![PedalNode::env_follow(NodeRef::Input, 1.0, 40.0)],
            NodeRef::node(0),
        ),
    );
    let mut input = vec![0.0_f32; PEDAL_CONTROL_INTERVAL_SAMPLES * 2];
    input[PEDAL_CONTROL_INTERVAL_SAMPLES / 2] = 1.0;

    let output = render(program, &input);

    assert!(
        output[PEDAL_CONTROL_INTERVAL_SAMPLES / 2] > 0.01,
        "expected env follower to catch an in-block peak, got {output:?}"
    );
}

#[test]
fn pedal_env_follow_control_chains_track_in_block_peaks() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { env = env_follow(input); env * 0.5 }",
            "env follower control chain",
        )
        .with_graph(
            vec![
                PedalNode::env_follow(NodeRef::Input, 1.0, 40.0),
                PedalNode::constant(0.5),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
            ],
            NodeRef::node(2),
        ),
    );
    let mut input = vec![0.0_f32; PEDAL_CONTROL_INTERVAL_SAMPLES * 2];
    input[PEDAL_CONTROL_INTERVAL_SAMPLES / 2] = 1.0;

    let output = render(program, &input);

    assert!(
        output[PEDAL_CONTROL_INTERVAL_SAMPLES / 2] > 0.005,
        "expected derived control chain to react inside the control block, got {output:?}"
    );
}

#[test]
fn pedal_direct_input_control_chains_update_each_sample() {
    let program = Arc::new(
        PedalProgram::new("graph { input * 0.5 }", "direct input control").with_graph(
            vec![
                PedalNode::constant(0.5),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::Input,
                        right: NodeRef::node(0),
                    },
                ),
            ],
            NodeRef::node(1),
        ),
    );
    let input = vec![0.0_f32, 1.0, 0.0, -1.0];
    let output = render(program, &input);

    assert_eq!(output, vec![0.0, 0.5, 0.0, -0.5]);
}

#[test]
fn pedal_input_driven_feedback_amount_reserves_max_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { amt = input * 0.5; input |> feedback(amount=amt, delay=32.sample) |> output }",
            "input-driven feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::constant(0.5),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::Input,
                        right: NodeRef::node(0),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(1), 32, None),
            ],
            NodeRef::node(2),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert!(
        runtime.tail_frames() > 10_000,
        "expected direct input-driven feedback amount to reserve the clamped max tail, got {}",
        runtime.tail_frames()
    );
}

#[test]
fn pedal_env_follow_feedback_amount_reserves_max_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { env = env_follow(input); amt = env * 0.8; input |> feedback(amount=amt, delay=32.sample) |> output }",
            "env-follow feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::env_follow(NodeRef::Input, 1.0, 40.0),
                PedalNode::constant(0.8),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(2), 32, None),
            ],
            NodeRef::node(3),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert!(
        runtime.tail_frames() > 10_000,
        "expected env-follow feedback amount to reserve the clamped max tail, got {}",
        runtime.tail_frames()
    );
}

#[test]
fn pedal_stage_env_follow_feedback_amount_reserves_max_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { driven = input |> preamp(model=jfet_clean); env = env_follow(driven); amt = env * 0.8; driven |> feedback(amount=amt, delay=32.sample) |> output }",
            "stage env-follow feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::constant(8.0),
                PedalNode::stage(PedalStage::Preamp {
                    input: NodeRef::Input,
                    gain: NodeRef::node(0),
                    model: PreampModel::JfetClean,
                }),
                PedalNode::env_follow(NodeRef::node(1), 1.0, 40.0),
                PedalNode::constant(0.8),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::node(2),
                        right: NodeRef::node(3),
                    },
                ),
                PedalNode::feedback(NodeRef::node(1), NodeRef::node(4), 32, None),
            ],
            NodeRef::node(5),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert!(
        runtime.tail_frames() > 10_000,
        "expected stage env-follow feedback amount to reserve the clamped max tail, got {}",
        runtime.tail_frames()
    );
}

#[test]
fn pedal_zeroed_env_follow_feedback_amount_reserves_no_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { env = env_follow(input); amt = env * 0.0; input |> feedback(amount=amt, delay=32.sample) |> output }",
            "zeroed env-follow feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::env_follow(NodeRef::Input, 1.0, 40.0),
                PedalNode::constant(0.0),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(2), 32, None),
            ],
            NodeRef::node(3),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert_eq!(runtime.tail_frames(), 0);
}

#[test]
fn pedal_zeroed_input_feedback_amount_reserves_no_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { amt = (input * 0.0) + (-1.0); input |> feedback(amount=amt, delay=32.sample) |> output }",
            "zeroed input feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::constant(0.0),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::Input,
                        right: NodeRef::node(0),
                    },
                ),
                PedalNode::constant(-1.0),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Add {
                        left: NodeRef::node(1),
                        right: NodeRef::node(2),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(3), 32, None),
            ],
            NodeRef::node(4),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert_eq!(runtime.tail_frames(), 0);
}

#[test]
fn pedal_cancelled_env_follow_feedback_amount_reserves_no_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { env = env_follow(input); neg = env * -1.0; amt = env + neg; input |> feedback(amount=amt, delay=32.sample) |> output }",
            "cancelled env-follow feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::env_follow(NodeRef::Input, 1.0, 40.0),
                PedalNode::constant(-1.0),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Add {
                        left: NodeRef::node(0),
                        right: NodeRef::node(2),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(3), 32, None),
            ],
            NodeRef::node(4),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert_eq!(runtime.tail_frames(), 0);
}

#[test]
fn pedal_cancelled_input_feedback_amount_reserves_no_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { neg = input * -1.0; amt = input + neg; input |> feedback(amount=amt, delay=32.sample) |> output }",
            "cancelled input feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::constant(-1.0),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::Input,
                        right: NodeRef::node(0),
                    },
                ),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Add {
                        left: NodeRef::Input,
                        right: NodeRef::node(1),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(2), 32, None),
            ],
            NodeRef::node(3),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert_eq!(runtime.tail_frames(), 0);
}

#[test]
fn pedal_hybrid_env_follow_uses_current_block_modulators() {
    let rate_hz = 8.0_f32;
    let depth = 0.2_f32;
    let offset = 0.4_f32;
    let phase_increment =
        core::f32::consts::TAU * rate_hz * (PEDAL_CONTROL_INTERVAL_SAMPLES as f32) / SAMPLE_RATE_HZ;
    let expected_block_two_lfo = offset + phase_increment.sin() * depth;
    let program = Arc::new(
        PedalProgram::new(
            "graph { env = env_follow(input); mod = lfo(...); env * mod }",
            "hybrid control chain",
        )
        .with_graph(
            vec![
                PedalNode::env_follow(NodeRef::Input, 0.0, 40.0),
                PedalNode::lfo(rate_hz, depth, offset),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
            ],
            NodeRef::node(2),
        ),
    );
    let mut input = vec![0.0_f32; PEDAL_CONTROL_INTERVAL_SAMPLES * 2];
    input[PEDAL_CONTROL_INTERVAL_SAMPLES] = 1.0;

    let output = render(program, &input);

    assert!(
        (output[PEDAL_CONTROL_INTERVAL_SAMPLES] - expected_block_two_lfo).abs() < 1.0e-4,
        "expected first sample of the new block to use the refreshed LFO value, got {output:?}"
    );
}

#[test]
fn pedal_stateful_filters_reserve_tail_frames() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { input |> filter(kind=lowpass, cutoff=1200, resonance=0.2) |> output }",
            "stateful filter",
        )
        .with_graph(
            vec![
                PedalNode::constant(1_200.0),
                PedalNode::constant(0.2),
                PedalNode::stage(PedalStage::Filter {
                    input: NodeRef::Input,
                    kind: FilterMode::LowPass,
                    cutoff_hz: NodeRef::node(0),
                    resonance: NodeRef::node(1),
                }),
            ],
            NodeRef::node(2),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert!(runtime.tail_frames() > 0);
}

#[test]
fn pedal_sag_does_not_reserve_tail_frames() {
    let program = Arc::new(
        PedalProgram::new("graph { input |> sag(amount=0.5) |> output }", "sag").with_graph(
            vec![
                PedalNode::constant(0.5),
                PedalNode::stage(PedalStage::Sag {
                    input: NodeRef::Input,
                    amount: NodeRef::node(0),
                }),
            ],
            NodeRef::node(1),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert_eq!(runtime.tail_frames(), 0);
}

#[test]
fn pedal_dark_feedback_reserves_filter_ringout_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { input |> feedback(amount=0.5, delay=1.sample, tone=20hz) |> output }",
            "dark feedback",
        )
        .with_graph(
            vec![
                PedalNode::constant(0.5),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(0), 1, Some(20.0)),
            ],
            NodeRef::node(1),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert!(
        runtime.tail_frames() > 1_000,
        "expected dark feedback tone filter to reserve substantial ringout, got {}",
        runtime.tail_frames()
    );
}

#[test]
fn pedal_feedback_constant_control_expressions_reserve_bounded_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { amt = 0.1 * 0.5; input |> feedback(amount=amt, delay=32.sample) |> output }",
            "derived feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::constant(0.1),
                PedalNode::constant(0.5),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(2), 32, None),
            ],
            NodeRef::node(3),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert!(
        runtime.tail_frames() < 200,
        "expected low derived feedback amount to reserve a short tail, got {}",
        runtime.tail_frames()
    );
}

#[test]
fn pedal_feedback_signed_control_expressions_still_reserve_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { amt = (-0.5) * (-0.5); input |> feedback(amount=amt, delay=32.sample) |> output }",
            "signed feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::constant(-0.5),
                PedalNode::constant(-0.5),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Mul {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(2), 32, None),
            ],
            NodeRef::node(3),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert!(
        runtime.tail_frames() >= 64,
        "expected signed intermediates to preserve a positive feedback tail, got {}",
        runtime.tail_frames()
    );
}

#[test]
fn pedal_negative_feedback_expressions_reserve_no_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { amt = -1.0 + 0.25; input |> feedback(amount=amt, delay=32.sample) |> output }",
            "negative feedback amount",
        )
        .with_graph(
            vec![
                PedalNode::constant(-1.0),
                PedalNode::constant(0.25),
                PedalNode::new(
                    SignalKind::Control,
                    orpheus_dsp::PedalNodeKind::Add {
                        left: NodeRef::node(0),
                        right: NodeRef::node(1),
                    },
                ),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(2), 32, None),
            ],
            NodeRef::node(3),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert_eq!(runtime.tail_frames(), 0);
}

#[test]
fn pedal_serial_stateful_stages_accumulate_tail() {
    let program = Arc::new(
        PedalProgram::new(
            "graph { input |> feedback(amount=0.5, delay=1.sample, tone=20hz) |> filter(kind=lowpass, cutoff=20, resonance=0.2) |> output }",
            "serial stateful chain",
        )
        .with_graph(
            vec![
                PedalNode::constant(0.5),
                PedalNode::feedback(NodeRef::Input, NodeRef::node(0), 1, Some(20.0)),
                PedalNode::constant(20.0),
                PedalNode::constant(0.2),
                PedalNode::stage(PedalStage::Filter {
                    input: NodeRef::node(1),
                    kind: FilterMode::LowPass,
                    cutoff_hz: NodeRef::node(2),
                    resonance: NodeRef::node(3),
                }),
            ],
            NodeRef::node(4),
        ),
    );
    let runtime = PedalInstance::new(program, SAMPLE_RATE_HZ);

    assert!(
        runtime.tail_frames() > 5_000,
        "expected serial stateful stages to accumulate tail time, got {}",
        runtime.tail_frames()
    );
}
