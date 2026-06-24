import sys

with open("crates/orpheus-lang/src/value.rs", "r") as f:
    text = f.read()

# 1. try_query_transform_method
search1 = """    #[allow(clippy::too_many_lines)]
    fn try_query_transform_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Roll { steps, inner } => T::roll_events(inner.try_query(span)?, *steps),
            Self::Strum { inner } => T::strum_events(inner.try_query(span)?),
            Self::Arp {
                steps,
                direction,
                inner,
            } => T::arp_events(inner.try_query(span)?, *steps, *direction),
            Self::Invert { count, inner } => T::invert_events(inner.try_query(span)?, *count),
            Self::Drop { count, inner } => T::drop_events(inner.try_query(span)?, *count),
            Self::Degrees { collection, inner } => {
                apply_value_transform(inner, span, |value| value.map_degrees(collection))
            }
            Self::Transpose { semitones, inner } => {
                apply_value_transform(inner, span, |value| value.transpose_semitones(*semitones))
            }
            Self::TransposePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Transpose)
            }
            Self::Fast { factor, inner } => query_fast(inner, *factor, span),
            Self::Slow { factor, inner } => query_slow(inner, *factor, span),
            Self::Shift { offset, inner } => query_shift(inner, offset, span),
            Self::Rev { inner } => query_rev(inner, span),
            Self::Gain { factor, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_gain(*factor))
            }
            Self::GainPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Gain)
            }
            Self::Pitch { semitones, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_rate(semitones_to_rate_multiplier(*semitones));
            }),
            Self::PitchPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pitch)
            }
            Self::TunedPitch {
                semitones,
                tuning,
                inner,
            } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_rate(semitones_to_tuned_rate(*semitones, tuning));
            }),
            Self::TunedPitchPattern {
                control,
                tuning,
                inner,
            } => apply_tuned_pitch_pattern(inner, control, span, tuning),
            Self::Rate { factor, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_rate(*factor))
            }
            Self::RatePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Rate)
            }
            Self::Onset { onset_index, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_onset(*onset_index);
            }),
            Self::OnsetPattern { control, inner } => apply_onset_pattern(inner, control, span),
            Self::Slice { start, end, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_slice(*start, *end);
            }),
            Self::SlicePattern {
                start_control,
                end_control,
                inner,
            } => apply_slice_pattern(inner, start_control, end_control, span),
            Self::SliceIdxPattern {
                control,
                segments,
                inner,
            } => apply_slice_idx_pattern(inner, control, *segments, span),
            Self::Pedal {
                pedal_program,
                inner,
            } => apply_value_mutation(inner, span, |value| {
                *value = value.attach_pedal_program(pedal_program);
            }),
            Self::Rand { site_salt } => query_rand(*site_salt, span),
            _ => self.try_query_audio_effect(span),
        }
    }"""
replace1 = """    fn try_query_transform_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Roll { steps, inner } => T::roll_events(inner.try_query(span)?, *steps),
            Self::Strum { inner } => T::strum_events(inner.try_query(span)?),
            Self::Arp {
                steps,
                direction,
                inner,
            } => T::arp_events(inner.try_query(span)?, *steps, *direction),
            Self::Invert { count, inner } => T::invert_events(inner.try_query(span)?, *count),
            Self::Drop { count, inner } => T::drop_events(inner.try_query(span)?, *count),
            Self::Degrees { collection, inner } => {
                apply_value_transform(inner, span, |value| value.map_degrees(collection))
            }
            Self::Transpose { semitones, inner } => {
                apply_value_transform(inner, span, |value| value.transpose_semitones(*semitones))
            }
            Self::TransposePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Transpose)
            }
            Self::Fast { factor, inner } => query_fast(inner, *factor, span),
            Self::Slow { factor, inner } => query_slow(inner, *factor, span),
            Self::Shift { offset, inner } => query_shift(inner, offset, span),
            Self::Rev { inner } => query_rev(inner, span),
            Self::Gain { factor, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_gain(*factor))
            }
            Self::GainPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Gain)
            }
            _ => self.try_query_transform_pitch_method(span),
        }
    }

    fn try_query_transform_pitch_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Pitch { semitones, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_rate(semitones_to_rate_multiplier(*semitones));
            }),
            Self::PitchPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pitch)
            }
            Self::TunedPitch {
                semitones,
                tuning,
                inner,
            } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_rate(semitones_to_tuned_rate(*semitones, tuning));
            }),
            Self::TunedPitchPattern {
                control,
                tuning,
                inner,
            } => apply_tuned_pitch_pattern(inner, control, span, tuning),
            Self::Rate { factor, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_rate(*factor))
            }
            Self::RatePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Rate)
            }
            _ => self.try_query_transform_misc_method(span),
        }
    }

    fn try_query_transform_misc_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Onset { onset_index, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_onset(*onset_index);
            }),
            Self::OnsetPattern { control, inner } => apply_onset_pattern(inner, control, span),
            Self::Slice { start, end, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_slice(*start, *end);
            }),
            Self::SlicePattern {
                start_control,
                end_control,
                inner,
            } => apply_slice_pattern(inner, start_control, end_control, span),
            Self::SliceIdxPattern {
                control,
                segments,
                inner,
            } => apply_slice_idx_pattern(inner, control, *segments, span),
            Self::Pedal {
                pedal_program,
                inner,
            } => apply_value_mutation(inner, span, |value| {
                *value = value.attach_pedal_program(pedal_program);
            }),
            Self::Rand { site_salt } => query_rand(*site_salt, span),
            _ => self.try_query_audio_effect(span),
        }
    }"""
text = text.replace(search1, replace1)

# 2. try_query_audio_effect_method
search2 = """    #[allow(clippy::too_many_lines)]
    fn try_query_audio_effect_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Delay { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_delay_mix(*mix))
            }
            Self::DelayPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayMix)
            }
            Self::DelayTime { time, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_delay_time(*time))
            }
            Self::DelayTimePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayTime)
            }
            Self::DelayFeedback { feedback, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_delay_feedback(*feedback);
            }),
            Self::DelayFeedbackPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayFeedback)
            }
            Self::Hpf { cutoff_hz, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_hpf(*cutoff_hz);
            }),
            Self::HpfPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Hpf)
            }
            Self::Lpf { cutoff_hz, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_lpf(*cutoff_hz);
            }),
            Self::LpfPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Lpf)
            }
            Self::Reverb { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_reverb_mix(*mix))
            }
            Self::ReverbPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbMix)
            }
            Self::ReverbRoom { room, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_reverb_room(*room);
            }),
            Self::ReverbRoomPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbRoom)
            }
            Self::ReverbDamp { damp, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_reverb_damp(*damp);
            }),
            Self::ReverbDampPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbDamp)
            }
            Self::Res { resonance, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_resonance(*resonance);
            }),
            Self::ResPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Res)
            }
            Self::Drive { drive, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_drive(*drive);
            }),
            Self::DrivePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Drive)
            }
            _ => self.try_query_modulation_effect(span),
        }
    }"""
replace2 = """    fn try_query_audio_effect_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Delay { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_delay_mix(*mix))
            }
            Self::DelayPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayMix)
            }
            Self::DelayTime { time, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_delay_time(*time))
            }
            Self::DelayTimePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayTime)
            }
            Self::DelayFeedback { feedback, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_delay_feedback(*feedback);
            }),
            Self::DelayFeedbackPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::DelayFeedback)
            }
            Self::Hpf { cutoff_hz, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_hpf(*cutoff_hz);
            }),
            Self::HpfPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Hpf)
            }
            Self::Lpf { cutoff_hz, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_lpf(*cutoff_hz);
            }),
            Self::LpfPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Lpf)
            }
            Self::Res { resonance, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_resonance(*resonance);
            }),
            Self::ResPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Res)
            }
            Self::Drive { drive, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_drive(*drive);
            }),
            Self::DrivePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Drive)
            }
            _ => self.try_query_reverb_method(span),
        }
    }

    fn try_query_reverb_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Reverb { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_reverb_mix(*mix))
            }
            Self::ReverbPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbMix)
            }
            Self::ReverbRoom { room, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_reverb_room(*room);
            }),
            Self::ReverbRoomPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbRoom)
            }
            Self::ReverbDamp { damp, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_reverb_damp(*damp);
            }),
            Self::ReverbDampPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ReverbDamp)
            }
            _ => self.try_query_modulation_effect(span),
        }
    }"""
text = text.replace(search2, replace2)

# 3. try_query_modulation_effect_method
search3 = """    #[allow(clippy::too_many_lines)]
    fn try_query_modulation_effect_method(
        &self,
        span: &TimeSpan,
    ) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Chorus { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_chorus_mix(*mix))
            }
            Self::ChorusPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusMix)
            }
            Self::ChorusDepth { depth, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_chorus_depth(*depth);
            }),
            Self::ChorusDepthPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusDepth)
            }
            Self::ChorusRate { rate, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_chorus_rate(*rate);
            }),
            Self::ChorusRatePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusRate)
            }
            Self::PulseWidth { pulse_width, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_pulse_width(*pulse_width);
            }),
            Self::PulseWidthPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::PulseWidth)
            }
            Self::Pan { amount, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_pan(*amount))
            }
            Self::PanPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pan)
            }
            Self::Compressor { mix, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_compressor_mix(*mix);
            }),
            Self::CompressorPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::CompressorMix)
            }
            Self::CompressorThreshold { threshold, inner } => {
                apply_value_mutation(inner, span, |value| {
                    *value = value.adjust_compressor_threshold(*threshold);
                })
            }
            Self::CompressorThresholdPattern { control, inner } => apply_control_pattern(
                inner,
                control,
                span,
                ControlPatternKind::CompressorThreshold,
            ),
            Self::CompressorRatio { ratio, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_compressor_ratio(*ratio);
            }),
            Self::CompressorRatioPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::CompressorRatio)
            }

            _ => unreachable!("handled in previous try_query stages"),
        }
    }"""
replace3 = """    fn try_query_modulation_effect_method(
        &self,
        span: &TimeSpan,
    ) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::PulseWidth { pulse_width, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_pulse_width(*pulse_width);
            }),
            Self::PulseWidthPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::PulseWidth)
            }
            Self::Pan { amount, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_pan(*amount))
            }
            Self::PanPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::Pan)
            }
            _ => self.try_query_chorus_method(span),
        }
    }

    fn try_query_chorus_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Chorus { mix, inner } => {
                apply_value_mutation(inner, span, |value| *value = value.adjust_chorus_mix(*mix))
            }
            Self::ChorusPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusMix)
            }
            Self::ChorusDepth { depth, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_chorus_depth(*depth);
            }),
            Self::ChorusDepthPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusDepth)
            }
            Self::ChorusRate { rate, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_chorus_rate(*rate);
            }),
            Self::ChorusRatePattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::ChorusRate)
            }
            _ => self.try_query_compressor_method(span),
        }
    }

    fn try_query_compressor_method(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, EvalError> {
        match self {
            Self::Compressor { mix, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_compressor_mix(*mix);
            }),
            Self::CompressorPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::CompressorMix)
            }
            Self::CompressorThreshold { threshold, inner } => {
                apply_value_mutation(inner, span, |value| {
                    *value = value.adjust_compressor_threshold(*threshold);
                })
            }
            Self::CompressorThresholdPattern { control, inner } => apply_control_pattern(
                inner,
                control,
                span,
                ControlPatternKind::CompressorThreshold,
            ),
            Self::CompressorRatio { ratio, inner } => apply_value_mutation(inner, span, |value| {
                *value = value.adjust_compressor_ratio(*ratio);
            }),
            Self::CompressorRatioPattern { control, inner } => {
                apply_control_pattern(inner, control, span, ControlPatternKind::CompressorRatio)
            }

            _ => unreachable!("handled in previous try_query stages"),
        }
    }"""
text = text.replace(search3, replace3)

with open("crates/orpheus-lang/src/value.rs", "w") as f:
    f.write(text)
