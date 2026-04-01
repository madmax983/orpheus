use std::sync::Arc;

use orpheus_dsp::{
    ClipModel, FilterMode, NodeRef, PedalInstance, PedalNode, PedalProgram, PedalStage,
    PreampModel, ToneModel,
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
