//! The `effect` module defines the audio effect processing trait and built-in effects.
//!
//! Effects process stereo audio buffers in-place and are used by [`EffectBus`](crate::effect_bus::EffectBus)
//! instances to apply send/return processing. All effect implementations must be
//! allocation-free during `process_stereo` to remain safe for the real-time audio thread.

/// A stereo audio effect that processes interleaved sample buffers in-place.
///
/// Implementations must be allocation-free during [`process_stereo`](Effect::process_stereo)
/// to remain safe for real-time audio thread use.
pub trait Effect: Send + std::fmt::Debug {
    /// Processes a stereo interleaved buffer in-place.
    ///
    /// `buffer` length is always a multiple of 2 (interleaved L, R pairs).
    fn process_stereo(&mut self, buffer: &mut [f32]);

    /// Resets all internal state (e.g. delay lines, filter history).
    fn reset(&mut self);
}

/// A stereo ping-pong delay effect with feedback and dry/wet mix control.
///
/// Uses a pre-allocated circular buffer sized at construction time from
/// `delay_time_secs * sample_rate`. The delay alternates between left and
/// right channels for a stereo spread effect.
#[derive(Debug)]
pub struct StereoDelay {
    buffer_left: Vec<f32>,
    buffer_right: Vec<f32>,
    write_pos: usize,
    feedback: f32,
    mix: f32,
}

impl StereoDelay {
    /// Creates a new stereo delay with pre-allocated buffers.
    ///
    /// # Parameters
    /// - `sample_rate`: Output sample rate in Hz.
    /// - `delay_time_secs`: Delay time in seconds (clamped to 0.001..5.0).
    /// - `feedback`: Feedback amount (clamped to 0.0..0.95).
    /// - `mix`: Wet/dry mix (clamped to 0.0..1.0).
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn new(sample_rate: u32, delay_time_secs: f32, feedback: f32, mix: f32) -> Self {
        let delay_time = delay_time_secs.clamp(0.001, 5.0);
        let delay_samples = (f64::from(sample_rate) * f64::from(delay_time)).ceil() as usize;
        let delay_samples = delay_samples.max(1);
        Self {
            buffer_left: vec![0.0; delay_samples],
            buffer_right: vec![0.0; delay_samples],
            write_pos: 0,
            feedback: feedback.clamp(0.0, 0.95),
            mix: mix.clamp(0.0, 1.0),
        }
    }
}

impl Effect for StereoDelay {
    fn process_stereo(&mut self, buffer: &mut [f32]) {
        let delay_len = self.buffer_left.len();
        for frame in buffer.chunks_exact_mut(2) {
            let dry_left = frame[0];
            let dry_right = frame[1];

            let delayed_left = self.buffer_left[self.write_pos];
            let delayed_right = self.buffer_right[self.write_pos];

            // Cross-feed for ping-pong: left input feeds right delay and vice versa
            self.buffer_left[self.write_pos] = delayed_right.mul_add(self.feedback, dry_left);
            self.buffer_right[self.write_pos] = delayed_left.mul_add(self.feedback, dry_right);

            frame[0] = delayed_left.mul_add(self.mix, dry_left);
            frame[1] = delayed_right.mul_add(self.mix, dry_right);

            self.write_pos = (self.write_pos + 1) % delay_len;
        }
    }

    fn reset(&mut self) {
        self.buffer_left.fill(0.0);
        self.buffer_right.fill(0.0);
        self.write_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn delay_produces_echo_at_correct_offset() {
        let sample_rate = 48_000;
        let delay_secs = 0.01; // 480 samples
        let mut delay = StereoDelay::new(sample_rate, delay_secs, 0.0, 1.0);

        let delay_samples = (f64::from(sample_rate) * f64::from(delay_secs)).ceil() as usize;

        // Feed an impulse then silence
        let total_frames = delay_samples * 3;
        let mut buffer = vec![0.0_f32; total_frames * 2];
        // Impulse at frame 0
        buffer[0] = 1.0; // left
        buffer[1] = 1.0; // right

        delay.process_stereo(&mut buffer);

        // Dry impulse at frame 0 should be preserved (mix=1.0 adds wet on top)
        assert!(buffer[0].abs() > 0.9, "dry impulse left preserved");
        assert!(buffer[1].abs() > 0.9, "dry impulse right preserved");

        // Echo should appear at delay_samples offset
        let echo_idx = delay_samples * 2; // interleaved index
        assert!(
            buffer[echo_idx].abs() > 0.9,
            "echo left at offset {delay_samples}: got {}",
            buffer[echo_idx]
        );
        assert!(
            buffer[echo_idx + 1].abs() > 0.9,
            "echo right at offset {delay_samples}: got {}",
            buffer[echo_idx + 1]
        );

        // Before echo offset, only the impulse frame should be non-zero
        for i in 1..delay_samples {
            let left = buffer[i * 2];
            let right = buffer[i * 2 + 1];
            assert!(
                left.abs() < f32::EPSILON && right.abs() < f32::EPSILON,
                "frame {i} should be silent before echo, got ({left}, {right})"
            );
        }
    }

    #[test]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn delay_feedback_produces_multiple_echoes() {
        let sample_rate = 48_000;
        let delay_secs = 0.01;
        let mut delay = StereoDelay::new(sample_rate, delay_secs, 0.5, 1.0);

        let delay_samples = (f64::from(sample_rate) * f64::from(delay_secs)).ceil() as usize;
        let total_frames = delay_samples * 4;
        let mut buffer = vec![0.0_f32; total_frames * 2];
        buffer[0] = 1.0;
        buffer[1] = 1.0;

        delay.process_stereo(&mut buffer);

        // First echo
        let echo1 = buffer[delay_samples * 2].abs();
        // Second echo (feedback of first)
        let echo2 = buffer[delay_samples * 4].abs();

        assert!(echo1 > 0.4, "first echo should be audible: {echo1}");
        assert!(echo2 > 0.1, "second echo should be present: {echo2}");
        assert!(echo2 < echo1, "second echo should be quieter than first");
    }

    #[test]
    fn reset_clears_delay_buffer() {
        let sample_rate = 48_000;
        let mut delay = StereoDelay::new(sample_rate, 0.01, 0.5, 1.0);

        // Feed impulse
        let mut buffer = vec![0.0_f32; 100];
        buffer[0] = 1.0;
        buffer[1] = 1.0;
        delay.process_stereo(&mut buffer);

        // Reset
        delay.reset();

        // Render silence — should get no echoes
        let mut buffer = vec![0.0_f32; 2000];
        delay.process_stereo(&mut buffer);

        let max = buffer.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(
            max < f32::EPSILON,
            "after reset, output should be silent, max={max}"
        );
    }

    #[test]
    fn zero_mix_passes_dry_signal_only() {
        let sample_rate = 48_000;
        let mut delay = StereoDelay::new(sample_rate, 0.01, 0.5, 0.0);

        let mut buffer = vec![0.0_f32; 2000];
        buffer[0] = 0.75;
        buffer[1] = 0.5;

        delay.process_stereo(&mut buffer);

        // Dry signal preserved
        assert!((buffer[0] - 0.75).abs() < f32::EPSILON);
        assert!((buffer[1] - 0.5).abs() < f32::EPSILON);

        // No wet signal anywhere after the impulse
        for i in 1..1000 {
            assert!(
                buffer[i * 2].abs() < f32::EPSILON && buffer[i * 2 + 1].abs() < f32::EPSILON,
                "frame {i} should be silent with mix=0"
            );
        }
    }
}
