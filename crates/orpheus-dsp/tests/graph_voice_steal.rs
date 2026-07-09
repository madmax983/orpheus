//! Offline-render tests for graph voice pool stealing (ADR 0009 addendum).
//!
//! A poly-limited program used to drop notes silently once its pool was
//! full. With stealing, an exhausted pool hands its most-released (else
//! oldest) voice to the new note, click-free: the graph state is kept so the
//! envelope retriggers from its current level, and the note's gain/pan ramp
//! linearly from the stolen values over `VOICE_STEAL_RAMP_SECONDS`.

use orpheus_dsp::{
    GraphVoiceBank, GraphVoiceSpec, StealPolicy, SvfMode, TrackId, VoiceNodeSpec, VoiceSignalRef,
};

const SR: f32 = 48_000.0;

/// A sustained lead voice (sine x ADSR, sustain 0.8) with the given pool
/// size — small enough that three overlapping notes exhaust a poly-2 pool.
fn lead_spec(polyphony: usize) -> GraphVoiceSpec {
    GraphVoiceSpec::new(
        "lead",
        0.02,
        vec![
            VoiceNodeSpec::Sine {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Adsr {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                decay_s: 0.005,
                sustain: 0.8,
                release_s: 0.02,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(0),
                right: VoiceSignalRef::Node(1),
            },
        ],
        VoiceSignalRef::Node(2),
    )
    .expect("lead spec should validate")
    .with_polyphony(polyphony)
    .expect("test polyphony is within bounds")
    .with_steal_policy(StealPolicy::Oldest)
}

/// Renders the three-overlapping-note schedule and returns the mono-per-side
/// track output plus each trigger's accept/drop result.
///
/// Schedule: two 220 Hz notes (gate 400 frames) start at frame 0; a long
/// 440 Hz note starts at frame 100, while both pooled voices still sound.
fn render_overlap_schedule(polyphony: usize, frames: usize) -> (Vec<(f32, f32)>, [bool; 3]) {
    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![lead_spec(polyphony)]);
    let track = TrackId::new(0);

    let first = bank.trigger("lead", track, 400, 220.0, 0.5, 0.0);
    let second = bank.trigger("lead", track, 400, 220.0, 0.5, 0.0);
    let mut third = false;

    let mut output = Vec::with_capacity(frames);
    let mut mix = vec![(0.0_f32, 0.0_f32); 1];
    for frame in 0..frames {
        if frame == 100 {
            third = bank.trigger("lead", track, 10_000, 440.0, 0.5, 0.0);
        }
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        output.push(mix[0]);
    }
    (output, [first, second, third])
}

fn zero_crossings(samples: &[f32]) -> usize {
    samples
        .windows(2)
        .filter(|window| window[0].signum() != window[1].signum())
        .count()
}

fn max_frame_delta(frames: &[(f32, f32)]) -> f32 {
    frames
        .windows(2)
        .map(|window| {
            let (left_a, right_a) = window[0];
            let (left_b, right_b) = window[1];
            (left_b - left_a).abs().max((right_b - right_a).abs())
        })
        .fold(0.0_f32, f32::max)
}

fn rms(frames: &[(f32, f32)]) -> f32 {
    let energy: f32 = frames
        .iter()
        .map(|&(left, right)| left.mul_add(left, right * right))
        .sum();
    #[allow(clippy::cast_precision_loss)]
    (energy / frames.len() as f32).sqrt()
}

#[test]
fn third_overlapping_note_steals_and_sounds_on_a_poly_two_pool() {
    let (output, accepted) = render_overlap_schedule(2, 3_000);

    assert_eq!(
        accepted,
        [true, true, true],
        "all three overlapping notes must be accepted (the third steals)"
    );

    // The first two notes sound before the steal...
    assert!(
        rms(&output[10..100]) > 0.01,
        "the first two notes must be audible before the steal"
    );

    // ...and the third note is still sounding long after the surviving
    // 220 Hz note's gate (400) + release (960) have ended. Its zero-crossing
    // count identifies it as the 440 Hz note, so the steal genuinely played
    // the new note rather than re-sounding an old one.
    let tail: Vec<f32> = output[2_200..3_000].iter().map(|&(left, _)| left).collect();
    assert!(
        rms(&output[2_200..3_000]) > 0.01,
        "the stolen-in third note must still be sounding"
    );
    let crossings = zero_crossings(&tail);
    // 440 Hz over 800 frames at 48 kHz -> ~14.7 crossings.
    assert!(
        (10..=20).contains(&crossings),
        "the sounding tail must be the 440 Hz note, got {crossings} crossings"
    );
}

#[test]
fn steal_handover_is_click_free_within_the_documented_ramp() {
    // The same schedule with poly 3 never steals; its largest frame-to-frame
    // step (note onsets, envelope attacks, the 440 Hz slope) bounds what
    // "smooth" means for this material. The stealing render must not step
    // harder: no hard envelope reset, no gain/pan jump beyond the ramp.
    let (stealing, _) = render_overlap_schedule(2, 3_000);
    let (control, _) = render_overlap_schedule(3, 3_000);

    let steal_delta = max_frame_delta(&stealing);
    let control_delta = max_frame_delta(&control);
    assert!(
        stealing
            .iter()
            .all(|&(left, right)| left.is_finite() && right.is_finite())
    );
    assert!(
        steal_delta <= control_delta.mul_add(1.5, 1e-3),
        "stealing must not introduce a discontinuity: steal max delta \
         {steal_delta}, no-steal max delta {control_delta}"
    );
}

/// A voice whose audio output is `p1` shaped by a sustained ADSR — the
/// per-note parameter is directly audible, so a param jump at a steal would
/// be a hard output discontinuity.
fn param_level_spec(polyphony: usize) -> GraphVoiceSpec {
    GraphVoiceSpec::new(
        "plevel",
        0.02,
        vec![
            VoiceNodeSpec::Adsr {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                decay_s: 0.005,
                sustain: 0.8,
                release_s: 0.02,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Param(0),
                right: VoiceSignalRef::Node(0),
            },
        ],
        VoiceSignalRef::Node(1),
    )
    .expect("param level spec should validate")
    .with_polyphony(polyphony)
    .expect("test polyphony is within bounds")
    .with_steal_policy(StealPolicy::Oldest)
}

/// Renders two overlapping notes carrying different `p1` values on the
/// param-level voice: the second starts at frame 200 while the first still
/// sounds, stealing on a poly-1 pool (and not stealing on poly-2).
fn render_param_overlap(polyphony: usize, frames: usize) -> Vec<(f32, f32)> {
    let mut bank = GraphVoiceBank::with_user_programs(SR, vec![param_level_spec(polyphony)]);
    let track = TrackId::new(0);

    assert!(bank.trigger_with_params(
        "plevel",
        track,
        10_000,
        220.0,
        0.5,
        0.0,
        [0.2, 0.0, 0.0, 0.0]
    ));
    let mut output = Vec::with_capacity(frames);
    let mut mix = vec![(0.0_f32, 0.0_f32); 1];
    for frame in 0..frames {
        if frame == 200 {
            assert!(bank.trigger_with_params(
                "plevel",
                track,
                10_000,
                220.0,
                0.5,
                0.0,
                [0.9, 0.0, 0.0, 0.0]
            ));
        }
        mix[0] = (0.0, 0.0);
        bank.render_frame(&mut mix);
        output.push(mix[0]);
    }
    output
}

#[test]
fn param_steal_handover_is_zipper_free_within_the_documented_ramp() {
    // Same schedule, poly 2: nothing is stolen, so its largest frame step
    // (note onsets, 1 ms envelope attacks) bounds what "smooth" means for
    // this material. The stealing render's `p1` moves 0.2 -> 0.9 on a
    // sounding voice; without the param ramp that is a 0.56 output step in
    // one frame, far beyond the attack slope.
    let stealing = render_param_overlap(1, 3_000);
    let control = render_param_overlap(2, 3_000);

    assert!(
        stealing
            .iter()
            .all(|&(left, right)| left.is_finite() && right.is_finite())
    );
    let steal_delta = max_frame_delta(&stealing);
    let control_delta = max_frame_delta(&control);
    assert!(
        steal_delta <= control_delta.mul_add(1.5, 1e-3),
        "a param-carrying steal must not introduce a discontinuity: steal \
         max delta {steal_delta}, no-steal max delta {control_delta}"
    );
}

#[test]
fn param_steal_lands_on_the_new_notes_level_after_the_ramp() {
    // Sample-and-hold semantics resume once the ramp ends: well after the
    // 2 ms window the output level is the NEW note's p1 x sustain, exactly
    // as if the note had been triggered fresh.
    let stealing = render_param_overlap(1, 3_000);
    let steady = rms(&stealing[1_500..3_000]);
    // p1 = 0.9, sustain 0.8, trigger gain 0.5: mono level 0.36. The rms
    // helper sums both sides' energy, so the equal-power center pan's
    // 1/sqrt(2) per side recombines to the mono level.
    let expected = 0.9 * 0.8 * 0.5;
    assert!(
        (steady - expected).abs() < 0.02,
        "the post-ramp steady level must be the new note's ({expected}), got {steady}"
    );
}

/// The task-motivating patch: a saw through an SVF lowpass whose cutoff is
/// the per-note `p1`, shaped by a sustained ADSR.
fn cutoff_lead_spec(polyphony: usize) -> GraphVoiceSpec {
    GraphVoiceSpec::new(
        "acid",
        0.02,
        vec![
            VoiceNodeSpec::Saw {
                freq: VoiceSignalRef::Freq,
            },
            VoiceNodeSpec::Constant { value: 0.7 },
            VoiceNodeSpec::Svf {
                input: VoiceSignalRef::Node(0),
                cutoff_hz: VoiceSignalRef::Param(0),
                q: VoiceSignalRef::Node(1),
                mode: SvfMode::Lowpass,
            },
            VoiceNodeSpec::Adsr {
                gate: VoiceSignalRef::Gate,
                attack_s: 0.001,
                decay_s: 0.005,
                sustain: 0.8,
                release_s: 0.02,
            },
            VoiceNodeSpec::Mul {
                left: VoiceSignalRef::Node(2),
                right: VoiceSignalRef::Node(3),
            },
        ],
        VoiceSignalRef::Node(4),
    )
    .expect("cutoff lead spec should validate")
    .with_polyphony(polyphony)
    .expect("test polyphony is within bounds")
    .with_steal_policy(StealPolicy::Oldest)
}

#[test]
fn cutoff_driving_param_steal_stays_smooth() {
    // Two overlapping notes whose p1 drives svf_lp cutoff (400 -> 4000 Hz)
    // on a stolen voice: the rendered output must stay bounded by the
    // no-steal control's frame deltas — the cutoff glides instead of
    // zipping.
    let render = |polyphony: usize| {
        let mut bank = GraphVoiceBank::with_user_programs(SR, vec![cutoff_lead_spec(polyphony)]);
        let track = TrackId::new(0);
        assert!(bank.trigger_with_params(
            "acid",
            track,
            10_000,
            110.0,
            0.5,
            0.0,
            [400.0, 0.0, 0.0, 0.0]
        ));
        let mut output = Vec::with_capacity(3_000);
        let mut mix = vec![(0.0_f32, 0.0_f32); 1];
        for frame in 0..3_000 {
            if frame == 200 {
                assert!(bank.trigger_with_params(
                    "acid",
                    track,
                    10_000,
                    110.0,
                    0.5,
                    0.0,
                    [4_000.0, 0.0, 0.0, 0.0]
                ));
            }
            mix[0] = (0.0, 0.0);
            bank.render_frame(&mut mix);
            output.push(mix[0]);
        }
        output
    };

    let stealing = render(1);
    let control = render(2);
    assert!(
        stealing
            .iter()
            .all(|&(left, right)| left.is_finite() && right.is_finite())
    );
    let steal_delta = max_frame_delta(&stealing);
    let control_delta = max_frame_delta(&control);
    assert!(
        steal_delta <= control_delta.mul_add(1.5, 1e-3),
        "a cutoff-swapping steal must stay smooth: steal max delta \
         {steal_delta}, no-steal max delta {control_delta}"
    );
}

#[test]
fn poly_three_pool_fits_the_schedule_without_stealing() {
    // Regression: with room for all three notes nothing is stolen, so the
    // overlap region carries all three voices — audibly more energy than the
    // poly-2 render, where the steal silenced one of the 220 Hz notes.
    let (stealing, _) = render_overlap_schedule(2, 3_000);
    let (roomy, accepted) = render_overlap_schedule(3, 3_000);

    assert_eq!(accepted, [true, true, true]);
    let overlap_stealing = rms(&stealing[150..400]);
    let overlap_roomy = rms(&roomy[150..400]);
    assert!(
        overlap_roomy > overlap_stealing * 1.2,
        "a poly-3 pool must keep all three notes sounding through the \
         overlap (rms {overlap_roomy} vs stolen {overlap_stealing})"
    );
}
