//! Integration tests for `analog_voice`.
use orpheus_dsp::{AnalogVoice, AnalogVoiceParams, OscShape};

const fn params(shape: OscShape) -> AnalogVoiceParams {
    AnalogVoiceParams {
        osc_shape: shape,
        freq_hz: 220.0,
        pulse_width: 0.5,
        cutoff_hz: 1_200.0,
        resonance: 0.2,
        drive: 1.0,
        gain: 0.5,
    }
}

#[test]
fn analog_voice_renders_finite_output_for_each_shape() {
    let mut voice = AnalogVoice::new(48_000.0);

    for shape in [
        OscShape::Saw,
        OscShape::Pulse,
        OscShape::Tri,
        OscShape::Noise,
    ] {
        let params = params(shape);
        for _ in 0..128 {
            assert!(voice.next_sample(&params).is_finite());
        }
        voice.reset();
    }
}

#[test]
fn analog_voice_reset_restores_the_same_output_sequence() {
    let mut voice = AnalogVoice::new(48_000.0);
    let params = params(OscShape::Saw);

    let first = (0..128)
        .map(|_| voice.next_sample(&params))
        .collect::<Vec<_>>();

    voice.reset();

    let second = (0..128)
        .map(|_| voice.next_sample(&params))
        .collect::<Vec<_>>();

    assert_eq!(first, second);
}

#[test]
fn analog_voice_pulse_width_changes_pulse_output() {
    let mut voice = AnalogVoice::new(48_000.0);

    let narrow_params = AnalogVoiceParams {
        pulse_width: 0.2,
        ..params(OscShape::Pulse)
    };
    let wide_params = AnalogVoiceParams {
        pulse_width: 0.8,
        ..params(OscShape::Pulse)
    };

    let narrow = (0..64)
        .map(|_| voice.next_sample(&narrow_params))
        .collect::<Vec<_>>();

    voice.reset();

    let wide = (0..64)
        .map(|_| voice.next_sample(&wide_params))
        .collect::<Vec<_>>();

    assert_ne!(narrow, wide);
}

#[test]
fn analog_voice_lower_gain_reduces_output_energy_for_each_shape() {
    let mut voice = AnalogVoice::new(48_000.0);

    for shape in [
        OscShape::Saw,
        OscShape::Pulse,
        OscShape::Tri,
        OscShape::Noise,
    ] {
        let quiet_params = AnalogVoiceParams {
            gain: 0.2,
            ..params(shape)
        };
        let loud_params = AnalogVoiceParams {
            gain: 0.8,
            ..params(shape)
        };

        let quiet_energy = (0..128)
            .map(|_| voice.next_sample(&quiet_params).abs())
            .sum::<f32>();

        voice.reset();

        let loud_energy = (0..128)
            .map(|_| voice.next_sample(&loud_params).abs())
            .sum::<f32>();

        assert!(quiet_energy < loud_energy);
        voice.reset();
    }
}

#[test]
fn analog_voice_pulse_width_sequences_are_deterministic_after_reset() {
    let mut voice = AnalogVoice::new(48_000.0);
    let params = AnalogVoiceParams {
        pulse_width: 0.3,
        ..params(OscShape::Pulse)
    };

    let first = (0..128)
        .map(|_| voice.next_sample(&params))
        .collect::<Vec<_>>();

    voice.reset();

    let second = (0..128)
        .map(|_| voice.next_sample(&params))
        .collect::<Vec<_>>();

    assert_eq!(first, second);
}
