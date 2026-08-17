use orpheus_dsp::{PlaybackSample, ScheduledTrigger, SampleTrigger, TrackId};
use std::sync::Arc;

fn test() {
    // PlaybackSample
    let frames: Arc<[f32]> = Arc::new([0.0; 100]);
    let sample = PlaybackSample::from_mono_frames(frames, 48000);
}
