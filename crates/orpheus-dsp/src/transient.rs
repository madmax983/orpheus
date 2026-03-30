use std::sync::Arc;

const SILENCE_FLOOR: f32 = 1.0e-4;
const ATTACK_COEFFICIENT: f32 = 0.65;
const RELEASE_COEFFICIENT: f32 = 0.02;
const THRESHOLD_MULTIPLIER: f32 = 3.0;
const MIN_THRESHOLD: f32 = 0.005;

#[allow(clippy::redundant_pub_crate)]
#[allow(clippy::cast_precision_loss)]
pub(crate) fn detect_transient_markers(frames: &[f32], sample_rate_hz: u32) -> Arc<[f64]> {
    if frames.is_empty() {
        return Arc::from([]);
    }

    let envelope = amplitude_envelope(frames);
    let mut flux = vec![0.0_f32; frames.len()];
    for index in 1..frames.len() {
        flux[index] = (envelope[index] - envelope[index - 1]).max(0.0);
    }

    let mean_flux = flux.iter().sum::<f32>() / (flux.len() as f32);
    let threshold = (mean_flux * THRESHOLD_MULTIPLIER).max(MIN_THRESHOLD);
    let min_gap = ((sample_rate_hz as usize) / 1_000).clamp(8, 1_024);
    let local_radius = (min_gap / 2).clamp(4, 128);
    let mut markers = Vec::new();
    let mut last_peak: Option<usize> = None;

    for index in 1..flux.len().saturating_sub(1) {
        let current = flux[index];
        if current < threshold || current <= flux[index - 1] || current < flux[index + 1] {
            continue;
        }

        let window_start = index.saturating_sub(local_radius);
        let window_end = (index + local_radius + 1).min(flux.len());
        let local_mean = flux[window_start..window_end].iter().sum::<f32>()
            / ((window_end - window_start) as f32);
        if current < local_mean.max(threshold) {
            continue;
        }

        if let Some(previous_peak) = last_peak
            && index.saturating_sub(previous_peak) < min_gap
        {
            if current > flux[previous_peak] {
                *markers
                    .last_mut()
                    .expect("last peak should exist when replacing a close transient") =
                    find_transient_start(frames, index);
                last_peak = Some(index);
            }
            continue;
        }

        markers.push(find_transient_start(frames, index));
        last_peak = Some(index);
    }

    if markers.is_empty()
        && let Some(first_non_silent) = frames
            .iter()
            .position(|sample| sample.abs() > SILENCE_FLOOR)
    {
        markers.push(first_non_silent);
    }

    let frame_count = frames.len() as f64;
    Arc::from(
        markers
            .into_iter()
            .map(|index| (index as f64) / frame_count)
            .collect::<Vec<_>>(),
    )
}

#[allow(clippy::redundant_pub_crate)]
#[allow(clippy::cast_precision_loss)]
pub(crate) fn rebase_transient_markers(markers: &[f64], start: f64, end: f64) -> Arc<[f64]> {
    let range = end - start;
    if !range.is_finite() || range <= f64::EPSILON {
        return Arc::from([]);
    }

    Arc::from(
        markers
            .iter()
            .copied()
            .filter(|marker| *marker >= start && *marker < end)
            .map(|marker| (marker - start) / range)
            .collect::<Vec<_>>(),
    )
}

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn resolve_onset_slice(markers: &[f64], onset_index: u32) -> Option<(f64, f64)> {
    if markers.is_empty() {
        return (onset_index == 0).then_some((0.0, 1.0));
    }

    let onset_index = usize::try_from(onset_index).ok()?;
    let start = *markers.get(onset_index)?;
    let end = markers.get(onset_index + 1).copied().unwrap_or(1.0);
    (start < end).then_some((start, end))
}

fn amplitude_envelope(frames: &[f32]) -> Vec<f32> {
    let mut envelope = Vec::with_capacity(frames.len());
    let mut previous = 0.0_f32;
    for sample in frames {
        let magnitude = sample.abs();
        let coefficient = if magnitude > previous {
            ATTACK_COEFFICIENT
        } else {
            RELEASE_COEFFICIENT
        };
        previous += coefficient * (magnitude - previous);
        envelope.push(previous);
    }
    envelope
}

fn find_transient_start(frames: &[f32], peak_index: usize) -> usize {
    let mut onset_index = peak_index.min(frames.len().saturating_sub(1));
    while onset_index > 0 && frames[onset_index - 1].abs() > SILENCE_FLOOR {
        onset_index -= 1;
    }
    onset_index
}

#[cfg(test)]
mod tests {
    use super::{detect_transient_markers, rebase_transient_markers, resolve_onset_slice};

    #[test]
    fn detect_transient_markers_finds_separated_impulses() {
        let mut frames = vec![0.0_f32; 300];
        for index in 10..14 {
            frames[index] = 1.0;
        }
        for index in 110..114 {
            frames[index] = 0.6;
        }
        for index in 210..214 {
            frames[index] = 0.3;
        }

        let markers = detect_transient_markers(&frames, 48_000);

        assert_eq!(markers.len(), 3);
        assert!((markers[0] - (10.0 / 300.0)).abs() < 1.0e-6);
        assert!((markers[1] - (110.0 / 300.0)).abs() < 1.0e-6);
        assert!((markers[2] - (210.0 / 300.0)).abs() < 1.0e-6);
    }

    #[test]
    fn rebase_transient_markers_discards_markers_outside_region() {
        let rebased = rebase_transient_markers(&[0.1, 0.4, 0.8], 0.25, 0.75);

        assert_eq!(rebased.len(), 1);
        assert!((rebased[0] - 0.3).abs() < 1.0e-9);
    }

    #[test]
    fn resolve_onset_slice_uses_following_marker_or_sample_end() {
        assert_eq!(resolve_onset_slice(&[0.1, 0.4, 0.8], 1), Some((0.4, 0.8)));
        assert_eq!(resolve_onset_slice(&[0.1, 0.4, 0.8], 2), Some((0.8, 1.0)));
        assert_eq!(resolve_onset_slice(&[], 0), Some((0.0, 1.0)));
        assert_eq!(resolve_onset_slice(&[0.1, 0.4], 3), None);
    }
}
