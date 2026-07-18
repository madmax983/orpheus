//! The `sample_bank` module manages collections of loaded audio samples.
//!
//! A `SampleBank` acts as an in-memory repository mapping string identifiers (like "bd" or "sn")
//! to fully decoded `DecodedSample` buffers, allowing the engine to quickly look up
//! and trigger audio events.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::SampleTrigger;
use crate::sample::{DecodedSample, SampleError, load_wav_bytes, load_wav_for_test};
use crate::sample_manifest::{
    SampleManifest, SampleManifestLoadError, SampleRegion, load_sample_manifest,
};
use crate::transient::{detect_transient_markers, rebase_transient_markers, resolve_onset_slice};
use crate::voice::VoiceKind;

const KICK_WAV: &[u8] = include_bytes!("../assets/kick.wav");
const SNARE_WAV: &[u8] = include_bytes!("../assets/snare.wav");
const CLAP_WAV: &[u8] = include_bytes!("../assets/clap.wav");
const HIHAT_WAV: &[u8] = include_bytes!("../assets/hihat.wav");
const SAMPLE_MANIFEST_FILE: &str = "samples.ron";
const DEFAULT_WATCH_INTERVAL: Duration = Duration::from_millis(50);

/// A read-only audio buffer fully loaded into heap memory.
///
/// The engine decodes all interactive audio files at boot and converts them into
/// `PlaybackSample` structs. By keeping everything natively in memory as raw `f32` floats,
/// the DSP thread can hot-swap drum hits instantly without blocking on file system I/O
/// or decoding overhead during live performance.
///
/// ## Examples
///
/// ```
/// use orpheus_dsp::PlaybackSample;
/// use std::sync::Arc;
///
/// // Create a dummy synthetic one-second buffer.
/// let synthetic_frames = Arc::<[f32]>::from(vec![0.5; 48000].into_boxed_slice());
/// let buffer = PlaybackSample::from_mono_frames(synthetic_frames, 48000);
///
/// assert_eq!(buffer.sample_rate_hz(), 48000);
/// ```
#[derive(Clone, PartialEq)]
pub struct PlaybackSample {
    frames: Arc<[f32]>,
    sample_rate_hz: u32,
}

impl PlaybackSample {
    /// Builds a playback buffer directly from mono frames.
    ///
    /// This is the construction path for buffers that do not come from a
    /// decoded file (tests, procedural buffers). Non-positive sample rates
    /// are clamped to 1 Hz so duration math stays finite.
    #[must_use]
    pub fn from_mono_frames(frames: impl Into<Arc<[f32]>>, sample_rate_hz: u32) -> Self {
        Self {
            frames: frames.into(),
            sample_rate_hz: sample_rate_hz.max(1),
        }
    }

    /// Exposes the underlying shared audio frame array.
    ///
    /// Returning `&Arc<[f32]>` avoids deep clones when multiple playback voices
    /// simultaneously read from the same underlying decoded file.
    #[must_use]
    pub const fn frames(&self) -> &Arc<[f32]> {
        &self.frames
    }

    /// The original sample rate (e.g., `44100` or `48000`) of the decoded audio file.
    ///
    /// This is used by the synthesis engine to calculate proper playback speeds and pitch shifting.
    #[must_use]
    pub const fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// The buffer's duration in seconds at its native rate.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn duration_seconds(&self) -> f64 {
        self.frames.len() as f64 / f64::from(self.sample_rate_hz)
    }
}

/// Buffers embed in voice specs, which print through `Debug` in REPL
/// `explain` tables and error messages — summarise the frames instead of
/// dumping them.
impl std::fmt::Debug for PlaybackSample {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PlaybackSample")
            .field("frames", &self.frames.len())
            .field("sample_rate_hz", &self.sample_rate_hz)
            .finish()
    }
}

impl From<DecodedSample> for PlaybackSample {
    fn from(sample: DecodedSample) -> Self {
        let frames = match sample.channels {
            1 => sample.frames,
            2 => sample
                .frames
                .chunks_exact(2)
                .map(|channel_pair| (channel_pair[0] + channel_pair[1]) * 0.5)
                .collect(),
            _ => unreachable!("decoded samples are constrained to mono or stereo"),
        };

        Self {
            frames: Arc::<[f32]>::from(frames),
            sample_rate_hz: sample.sample_rate_hz,
        }
    }
}

/// A central, in-memory repository for decoding, storing, and addressing audio samples.
///
/// The `SampleBank` bridges the gap between the interactive language REPL (which
/// schedules sound using abstract string identifiers like `"bd"` or `"sn"`) and the
/// high-performance audio synthesis thread (which requires immediately readable `f32` buffers).
///
/// Rather than reading from disk or decoding WAV files every time an event fires, the
/// `SampleBank` loads the entire working set of samples into heap memory before playback
/// starts. This ensures that the hot DSP loop never blocks on I/O.
///
/// # Examples
///
/// In a standard application boot sequence, you will typically initialize the bank
/// with the built-in drum machine assets, and then resolve tokens to extract
/// `PlaybackSample` instances for rendering.
///
/// ```
/// use orpheus_dsp::SampleBank;
///
/// // Load the core library ("bd", "sn", "cp", "hh").
/// let bank = SampleBank::load_builtin();
///
/// // Safely query for a resolved audio buffer.
/// let kick_buffer = bank.get_by_token("bd").expect("bd is a guaranteed built-in");
///
/// // Unknown identifiers degrade gracefully to `None` so the audio thread doesn't panic.
/// let missing = bank.get_by_token("glitch");
/// assert!(missing.is_none());
/// ```
///
/// ## Panics
/// The `SampleBank` uses robust `BTreeMap` structures internally, but developers should
/// guarantee that sample arrays are not mutated while the DSP loop is actively reading them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SampleBank {
    samples: BTreeMap<Box<str>, SampleEntry>,
}

#[derive(Clone, Debug, PartialEq)]
struct SampleEntry {
    sample: PlaybackSample,
    rate: f64,
    slice_start: f64,
    slice_end: f64,
    onset_markers: Arc<[f64]>,
}

impl SampleEntry {
    fn direct(sample: PlaybackSample) -> Self {
        Self {
            onset_markers: detect_transient_markers(sample.frames(), sample.sample_rate_hz()),
            sample,
            rate: 1.0,
            slice_start: 0.0,
            slice_end: 1.0,
        }
    }

    fn compose_region(&self, region: &SampleRegion) -> Self {
        let current_range = self.slice_end - self.slice_start;
        Self {
            onset_markers: rebase_transient_markers(&self.onset_markers, region.start, region.end),
            sample: self.sample.clone(),
            rate: self.rate * region.rate,
            slice_start: current_range.mul_add(region.start, self.slice_start),
            slice_end: current_range.mul_add(region.end, self.slice_start),
        }
    }

    fn compose_trigger(&self, trigger: &SampleTrigger) -> SampleTrigger {
        let (slice_start, slice_end) = self.compose_slice_bounds(trigger);
        let mut composed = SampleTrigger::named(trigger.token())
            .with_gain(trigger.gain())
            .with_pan(trigger.pan())
            .with_delay_mix(trigger.delay_mix())
            .with_delay_time(trigger.delay_time())
            .with_delay_feedback(trigger.delay_feedback())
            .with_reverb_mix(trigger.reverb_mix())
            .with_reverb_room(trigger.reverb_room())
            .with_reverb_damp(trigger.reverb_damp())
            .with_chorus_mix(trigger.chorus_mix())
            .with_chorus_depth(trigger.chorus_depth())
            .with_chorus_rate(trigger.chorus_rate())
            .with_compressor_mix(trigger.compressor_mix())
            .with_compressor_threshold(trigger.compressor_threshold())
            .with_compressor_ratio(trigger.compressor_ratio())
            .with_resonance(trigger.resonance())
            .with_drive(trigger.drive())
            .with_pulse_width(trigger.pulse_width())
            .with_rate(self.rate * trigger.rate())
            .with_slice(slice_start, slice_end);
        if let Some(onset_index) = trigger.onset_index() {
            composed = composed.with_onset(onset_index);
        }
        if let Some(cutoff_hz) = trigger.hpf_cutoff_hz() {
            composed = composed.with_hpf_cutoff_hz(cutoff_hz);
        }
        if let Some(cutoff_hz) = trigger.lpf_cutoff_hz() {
            composed = composed.with_lpf_cutoff_hz(cutoff_hz);
        }
        if let Some(pedal_program) = trigger.pedal_program() {
            composed = composed.with_pedal_program(pedal_program.clone());
        }
        composed
    }

    fn compose_slice_bounds(&self, trigger: &SampleTrigger) -> (f64, f64) {
        let mut base_start = self.slice_start;
        let mut base_end = self.slice_end;
        if let Some(onset_index) = trigger.onset_index()
            && let Some((onset_start, onset_end)) =
                resolve_onset_slice(&self.onset_markers, onset_index)
        {
            (base_start, base_end) =
                compose_relative_slice(base_start, base_end, onset_start, onset_end);
        }

        compose_relative_slice(
            base_start,
            base_end,
            trigger.slice_start(),
            trigger.slice_end(),
        )
    }
}

impl SampleBank {
    /// Loads the standard built-in Drum Machine samples into the bank.
    #[must_use]
    pub fn load_builtin() -> Self {
        let mut bank = Self::default();
        for token in ["bd", "sn", "cp", "hh"] {
            if let Ok(sample) = load_builtin_sample_for_test(token) {
                bank.samples
                    .insert(token.into(), SampleEntry::direct(sample.into()));
            }
        }
        bank
    }

    /// Resolves a strongly-typed built-in voice kind to its loaded audio buffer in memory.
    ///
    /// This provides the fastest, allocation-free path for the audio thread to locate drum
    /// sample data during playback.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::{SampleBank, VoiceKind};
    ///
    /// let bank = SampleBank::load_builtin();
    /// assert!(bank.get(VoiceKind::KickLike).is_some());
    /// ```
    #[must_use]
    pub fn get(&self, voice: VoiceKind) -> Option<&PlaybackSample> {
        self.get_by_token(voice.token())
    }

    /// Resolves a string identifier to its loaded audio buffer in memory.
    ///
    /// This allows dynamic lookups for custom sample names loaded via a manifest
    /// (e.g., `"my_synth_C4"`).
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::SampleBank;
    ///
    /// let bank = SampleBank::load_builtin();
    /// assert!(bank.get_by_token("sn").is_some());
    /// assert!(bank.get_by_token("nonexistent").is_none());
    /// ```
    #[must_use]
    pub fn get_by_token(&self, token: &str) -> Option<&PlaybackSample> {
        self.samples.get(token).map(|entry| &entry.sample)
    }

    /// Exposes a list of all currently loaded string identifiers in the bank.
    ///
    /// This is primarily used by the language REPL layer to provide auto-completion
    /// suggestions or validation error messages containing available sample names.
    #[must_use]
    pub fn available_tokens(&self) -> Vec<String> {
        self.samples.keys().map(ToString::to_string).collect()
    }

    fn insert_token(&mut self, token: &str, sample: PlaybackSample) {
        self.insert_entry(token, SampleEntry::direct(sample));
    }

    fn insert_entry(&mut self, token: &str, entry: SampleEntry) {
        self.samples.insert(token.into(), entry);
    }

    pub(crate) fn resolve_trigger(
        &self,
        trigger: &SampleTrigger,
    ) -> Option<(&PlaybackSample, SampleTrigger)> {
        let entry = self.samples.get(trigger.token())?;
        Some((&entry.sample, entry.compose_trigger(trigger)))
    }
}

/// Errors raised while scanning a sample directory for override assets.
#[derive(Debug, Error)]
pub enum SampleBankError {
    /// An I/O error occurred while reading the sample directory.
    #[error("failed to read sample directory `{path}`: {message}")]
    DirectoryIo {
        /// The path to the sample directory.
        path: Box<str>,
        /// The OS-level error message.
        message: Box<str>,
    },
    /// An I/O error occurred while reading the `samples.ron` manifest.
    #[error("failed to read sample manifest `{path}`: {message}")]
    ManifestIo {
        /// The path to the manifest file.
        path: Box<str>,
        /// The OS-level error message.
        message: Box<str>,
    },
    /// The `samples.ron` manifest contained invalid syntax or values.
    #[error("failed to parse sample manifest `{path}`: {message}")]
    ManifestParse {
        /// The path to the manifest file.
        path: Box<str>,
        /// A description of the parsing failure.
        message: Box<str>,
    },
    /// A manifest alias points to a target sample that was not found.
    #[error("sample manifest `{path}` aliases `{alias}` to unknown token `{target}`")]
    ManifestAliasTarget {
        /// The path to the manifest file.
        path: Box<str>,
        /// The alias name.
        alias: Box<str>,
        /// The unresolved target.
        target: Box<str>,
    },
    /// A manifest alias refers to itself directly or indirectly.
    #[error("sample manifest `{path}` contains an alias cycle at `{alias}` via `{target}`")]
    ManifestAliasCycle {
        /// The path to the manifest file.
        path: Box<str>,
        /// The alias involved in the cycle.
        alias: Box<str>,
        /// The target leading to the cycle.
        target: Box<str>,
    },
    /// A manifest region points to a target sample that was not found.
    #[error("sample manifest `{path}` region `{region}` targets unknown token `{target}`")]
    ManifestRegionTarget {
        /// The path to the manifest file.
        path: Box<str>,
        /// The region name.
        region: Box<str>,
        /// The unresolved target.
        target: Box<str>,
    },
    /// A manifest region refers to itself directly or indirectly.
    #[error("sample manifest `{path}` contains a region cycle at `{region}` via `{target}`")]
    ManifestRegionCycle {
        /// The path to the manifest file.
        path: Box<str>,
        /// The region involved in the cycle.
        region: Box<str>,
        /// The target leading to the cycle.
        target: Box<str>,
    },
    /// An error occurred while decoding a sample file from disk.
    #[error("failed to decode sample override `{path}`: {source}")]
    Decode {
        /// The path to the audio file.
        path: Box<str>,
        /// The underlying decoding error.
        #[source]
        source: SampleError,
    },
}

/// A non-fatal issue discovered during background sample-library scanning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleLibraryScanError {
    path: Box<str>,
    message: Box<str>,
}

impl SampleLibraryScanError {
    fn new(path: impl Into<Box<str>>, message: impl Into<Box<str>>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    fn from_decode(path: &Path, source: &SampleError) -> Self {
        Self::new(path.display().to_string(), source.to_string())
    }

    fn from_bank_error(source: &SampleBankError) -> Self {
        Self::new("", source.to_string())
    }

    /// The path associated with the scan issue, when one is available.
    #[must_use]
    pub const fn path(&self) -> &str {
        &self.path
    }

    /// Human-readable issue details suitable for logging.
    #[must_use]
    pub const fn message(&self) -> &str {
        &self.message
    }
}

/// A complete sample-bank snapshot published by a background library watcher.
#[derive(Clone, Debug)]
pub struct SampleLibraryReload {
    revision: u64,
    bank: SampleBank,
    tokens: Vec<String>,
    errors: Vec<SampleLibraryScanError>,
}

impl SampleLibraryReload {
    fn new(revision: u64, bank: SampleBank, errors: Vec<SampleLibraryScanError>) -> Self {
        let tokens = bank.available_tokens();
        Self {
            revision,
            bank,
            tokens,
            errors,
        }
    }

    /// Monotonically increasing revision assigned by the watcher thread.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Immutable loaded sample-bank snapshot.
    #[must_use]
    pub const fn bank(&self) -> &SampleBank {
        &self.bank
    }

    /// Tokens available in the published bank.
    #[must_use]
    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    /// Non-fatal scan errors encountered while producing this snapshot.
    #[must_use]
    pub fn errors(&self) -> &[SampleLibraryScanError] {
        &self.errors
    }
}

/// Polling configuration for [`SampleLibraryWatcher`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SampleLibraryWatcherConfig {
    /// How often the background worker checks for local filesystem changes.
    pub poll_interval: Duration,
}

impl Default for SampleLibraryWatcherConfig {
    fn default() -> Self {
        Self {
            poll_interval: DEFAULT_WATCH_INTERVAL,
        }
    }
}

/// Background watcher for a local sample-library directory.
///
/// The watcher performs all file-system scans and audio decoding on its worker
/// thread. Callers receive complete immutable [`SampleBank`] snapshots through
/// [`Self::try_recv`] and can hand them to the audio engine without blocking the
/// render thread.
pub struct SampleLibraryWatcher {
    reload_rx: mpsc::Receiver<SampleLibraryReload>,
    stop_tx: mpsc::Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}

impl SampleLibraryWatcher {
    /// Starts watching `directory` recursively for supported sample changes.
    ///
    /// # Errors
    ///
    /// Returns [`SampleBankError`] if the initial directory inventory cannot be
    /// read.
    pub fn spawn(
        directory: impl AsRef<Path>,
        config: SampleLibraryWatcherConfig,
    ) -> Result<Self, SampleBankError> {
        let directory = directory.as_ref().to_path_buf();
        let initial_inventory = sample_library_inventory(&directory)?;
        let poll_interval = if config.poll_interval.is_zero() {
            DEFAULT_WATCH_INTERVAL
        } else {
            config.poll_interval
        };
        let (reload_tx, reload_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_sample_library_watcher(
                &directory,
                initial_inventory,
                poll_interval,
                &reload_tx,
                &stop_rx,
            );
        });

        Ok(Self {
            reload_rx,
            stop_tx,
            worker: Some(worker),
        })
    }

    /// Receives one published reload snapshot, if one is currently available.
    #[must_use]
    pub fn try_recv(&mut self) -> Option<SampleLibraryReload> {
        self.reload_rx.try_recv().ok()
    }
}

impl Drop for SampleLibraryWatcher {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Loads a sample bank from built-ins plus any supported WAV overrides found in
/// `directory`.
///
/// Supported filenames load directly as lowercased token names. Drum filename
/// aliases also keep overriding the built-in tokens:
/// `bd`/`kick`, `sn`/`snare`, `cp`/`clap`, and `hh`/`hat`/`hihat`.
///
/// If `samples.ron` exists, explicit `tokens`, `regions`, and `aliases` are
/// loaded after directory inference and override it.
///
/// # Errors
///
/// Returns [`SampleBankError`] if the directory cannot be read or if one of the
/// sample files or manifest entries fail to load.
pub fn load_sample_bank_from_directory(
    directory: impl AsRef<Path>,
) -> Result<SampleBank, SampleBankError> {
    load_sample_bank_from_directory_inner(directory.as_ref(), DecodeFailureMode::Strict)
        .map(|scan| scan.bank)
}

fn load_sample_bank_from_directory_lossy(
    directory: &Path,
) -> Result<SampleDirectoryScan, SampleBankError> {
    load_sample_bank_from_directory_inner(directory, DecodeFailureMode::CollectErrors)
}

fn load_sample_bank_from_directory_inner(
    directory: &Path,
    decode_failure_mode: DecodeFailureMode,
) -> Result<SampleDirectoryScan, SampleBankError> {
    let sample_paths = collect_supported_sample_paths(directory)?;
    let manifest_path = directory.join(SAMPLE_MANIFEST_FILE);
    let manifest = load_optional_manifest(&manifest_path)?;

    let mut bank = SampleBank::load_builtin();
    let mut inferred_tokens = BTreeMap::<String, PlaybackSample>::new();
    let mut candidates: BTreeMap<&'static str, (u8, PlaybackSample)> = BTreeMap::new();
    let mut errors = Vec::new();

    for path in sample_paths {
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let sample = match load_wav_for_test(&path) {
            Ok(sample) => sample,
            Err(source) => {
                handle_decode_error(&path, source, decode_failure_mode, &mut errors)?;
                continue;
            }
        };
        let playback_sample: PlaybackSample = sample.into();
        let Some(inferred_token) = inferred_token_from_path(directory, &path) else {
            continue;
        };
        inferred_tokens
            .entry(inferred_token)
            .or_insert_with(|| playback_sample.clone());
        if is_top_level_sample_path(directory, &path)
            && let Some((token, priority)) = token_from_stem(stem)
        {
            match candidates.get(token) {
                Some((existing_priority, _)) if *existing_priority <= priority => {}
                _ => {
                    candidates.insert(token, (priority, playback_sample));
                }
            }
        }
    }

    for (token, sample) in inferred_tokens {
        bank.insert_token(&token, sample);
    }
    for (token, (_priority, sample)) in candidates {
        bank.insert_token(token, sample);
    }
    apply_manifest_tokens(
        &mut bank,
        manifest.as_ref(),
        directory,
        decode_failure_mode,
        &mut errors,
    )?;
    apply_manifest_regions(&mut bank, manifest.as_ref(), &manifest_path)?;
    apply_manifest_aliases(&mut bank, manifest.as_ref(), &manifest_path)?;

    Ok(SampleDirectoryScan { bank, errors })
}

/// Loads one embedded built-in sample for deterministic tests.
///
/// # Errors
///
/// Returns [`SampleError`] when the built-in name is unknown or the embedded WAV
/// bytes fail to decode.
pub fn load_builtin_sample_for_test(name: &str) -> Result<DecodedSample, SampleError> {
    let (bytes, display_path) = builtin_sample_bytes(name)?;
    load_wav_bytes(bytes, display_path)
}

fn builtin_sample_bytes(name: &str) -> Result<(&'static [u8], &'static str), SampleError> {
    match name {
        "bd" => Ok((KICK_WAV, "builtin://kick.wav")),
        "sn" => Ok((SNARE_WAV, "builtin://snare.wav")),
        "cp" => Ok((CLAP_WAV, "builtin://clap.wav")),
        "hh" => Ok((HIHAT_WAV, "builtin://hihat.wav")),
        _ => Err(SampleError::UnknownBuiltinSample(name.to_owned())),
    }
}

fn is_supported_sample_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "wav" | "wave"))
}

fn token_from_stem(stem: &str) -> Option<(&'static str, u8)> {
    match stem.to_ascii_lowercase().as_str() {
        "bd" => Some(("bd", 0)),
        "kick" => Some(("bd", 1)),
        "sn" => Some(("sn", 0)),
        "snare" => Some(("sn", 1)),
        "cp" => Some(("cp", 0)),
        "clap" => Some(("cp", 1)),
        "hh" => Some(("hh", 0)),
        "hat" | "hihat" => Some(("hh", 1)),
        _ => None,
    }
}

fn load_optional_manifest(path: &Path) -> Result<Option<SampleManifest>, SampleBankError> {
    if !path.is_file() {
        return Ok(None);
    }

    load_sample_manifest(path)
        .map(Some)
        .map_err(|error| match error {
            SampleManifestLoadError::Io { path, message } => {
                SampleBankError::ManifestIo { path, message }
            }
            SampleManifestLoadError::Parse { path, message } => {
                SampleBankError::ManifestParse { path, message }
            }
        })
}

fn apply_manifest_tokens(
    bank: &mut SampleBank,
    manifest: Option<&SampleManifest>,
    directory: &Path,
    decode_failure_mode: DecodeFailureMode,
    errors: &mut Vec<SampleLibraryScanError>,
) -> Result<(), SampleBankError> {
    let Some(manifest) = manifest else {
        return Ok(());
    };

    for (token, relative_path) in &manifest.tokens {
        let path = directory.join(relative_path);
        let sample = match load_wav_for_test(&path) {
            Ok(sample) => sample,
            Err(source) => {
                handle_decode_error(&path, source, decode_failure_mode, errors)?;
                continue;
            }
        };
        bank.insert_token(token, sample.into());
    }

    Ok(())
}

fn apply_manifest_aliases(
    bank: &mut SampleBank,
    manifest: Option<&SampleManifest>,
    manifest_path: &Path,
) -> Result<(), SampleBankError> {
    let Some(manifest) = manifest else {
        return Ok(());
    };

    for (alias, target) in &manifest.aliases {
        let sample = resolve_alias_target(
            alias,
            target,
            &manifest.aliases,
            bank,
            manifest_path,
            &mut BTreeSet::new(),
        )?;
        bank.insert_entry(alias, sample);
    }

    Ok(())
}

fn apply_manifest_regions(
    bank: &mut SampleBank,
    manifest: Option<&SampleManifest>,
    manifest_path: &Path,
) -> Result<(), SampleBankError> {
    let Some(manifest) = manifest else {
        return Ok(());
    };

    for (region_name, region) in &manifest.regions {
        let entry = resolve_region_target(
            region_name,
            region,
            &manifest.regions,
            bank,
            manifest_path,
            &mut BTreeSet::from([region_name.clone()]),
        )?;
        bank.insert_entry(region_name, entry);
    }

    Ok(())
}

fn resolve_alias_target(
    alias: &str,
    target: &str,
    aliases: &BTreeMap<String, String>,
    bank: &SampleBank,
    manifest_path: &Path,
    visiting: &mut BTreeSet<String>,
) -> Result<SampleEntry, SampleBankError> {
    if let Some(sample) = bank.samples.get(target) {
        return Ok(sample.clone());
    }
    if !visiting.insert(target.to_owned()) {
        return Err(SampleBankError::ManifestAliasCycle {
            path: manifest_path.display().to_string().into_boxed_str(),
            alias: alias.to_owned().into_boxed_str(),
            target: target.to_owned().into_boxed_str(),
        });
    }
    if let Some(next_target) = aliases.get(target) {
        return resolve_alias_target(alias, next_target, aliases, bank, manifest_path, visiting);
    }

    Err(SampleBankError::ManifestAliasTarget {
        path: manifest_path.display().to_string().into_boxed_str(),
        alias: alias.to_owned().into_boxed_str(),
        target: target.to_owned().into_boxed_str(),
    })
}

fn resolve_region_target(
    region_name: &str,
    region: &SampleRegion,
    regions: &BTreeMap<String, SampleRegion>,
    bank: &SampleBank,
    manifest_path: &Path,
    visiting: &mut BTreeSet<String>,
) -> Result<SampleEntry, SampleBankError> {
    let base = if let Some(entry) = bank.samples.get(region.token.as_str()) {
        entry.clone()
    } else if let Some(next_region) = regions.get(region.token.as_str()) {
        if !visiting.insert(region.token.clone()) {
            return Err(SampleBankError::ManifestRegionCycle {
                path: manifest_path.display().to_string().into_boxed_str(),
                region: region_name.to_owned().into_boxed_str(),
                target: region.token.clone().into_boxed_str(),
            });
        }
        let resolved = resolve_region_target(
            region_name,
            next_region,
            regions,
            bank,
            manifest_path,
            visiting,
        );
        visiting.remove(region.token.as_str());
        resolved?
    } else {
        return Err(SampleBankError::ManifestRegionTarget {
            path: manifest_path.display().to_string().into_boxed_str(),
            region: region_name.to_owned().into_boxed_str(),
            target: region.token.clone().into_boxed_str(),
        });
    };

    Ok(base.compose_region(region))
}

fn compose_relative_slice(
    base_start: f64,
    base_end: f64,
    relative_start: f64,
    relative_end: f64,
) -> (f64, f64) {
    let range = base_end - base_start;
    (
        range.mul_add(relative_start, base_start),
        range.mul_add(relative_end, base_start),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodeFailureMode {
    Strict,
    CollectErrors,
}

#[derive(Debug)]
struct SampleDirectoryScan {
    bank: SampleBank,
    errors: Vec<SampleLibraryScanError>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct SampleFileFingerprint {
    path: PathBuf,
    len: u64,
    modified_nanos: u128,
}

fn run_sample_library_watcher(
    directory: &Path,
    mut inventory: Vec<SampleFileFingerprint>,
    poll_interval: Duration,
    reload_tx: &mpsc::Sender<SampleLibraryReload>,
    stop_rx: &mpsc::Receiver<()>,
) {
    let mut revision = 0_u64;
    loop {
        match stop_rx.recv_timeout(poll_interval) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        let next_inventory = match sample_library_inventory(directory) {
            Ok(next_inventory) => next_inventory,
            Err(error) => {
                revision = revision.saturating_add(1);
                let reload = SampleLibraryReload::new(
                    revision,
                    SampleBank::load_builtin(),
                    vec![SampleLibraryScanError::from_bank_error(&error)],
                );
                if reload_tx.send(reload).is_err() {
                    break;
                }
                continue;
            }
        };

        if next_inventory == inventory {
            continue;
        }
        inventory = next_inventory;
        revision = revision.saturating_add(1);

        let reload = match load_sample_bank_from_directory_lossy(directory) {
            Ok(scan) => SampleLibraryReload::new(revision, scan.bank, scan.errors),
            Err(error) => SampleLibraryReload::new(
                revision,
                SampleBank::load_builtin(),
                vec![SampleLibraryScanError::from_bank_error(&error)],
            ),
        };
        if reload_tx.send(reload).is_err() {
            break;
        }
    }
}

fn collect_supported_sample_paths(directory: &Path) -> Result<Vec<PathBuf>, SampleBankError> {
    let mut paths = Vec::new();
    collect_supported_sample_paths_into(directory, directory, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_supported_sample_paths_into(
    root: &Path,
    directory: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<(), SampleBankError> {
    let entries = read_sorted_directory(directory)?;
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| directory_io_error(root, &source))?;
        if file_type.is_dir() {
            collect_supported_sample_paths_into(root, &path, paths)?;
        } else if file_type.is_file() && is_supported_sample_path(&path) {
            paths.push(path);
        }
    }
    Ok(())
}

fn sample_library_inventory(
    directory: &Path,
) -> Result<Vec<SampleFileFingerprint>, SampleBankError> {
    let mut fingerprints = Vec::new();
    collect_sample_library_inventory_into(directory, directory, &mut fingerprints)?;
    let manifest_path = directory.join(SAMPLE_MANIFEST_FILE);
    if manifest_path.is_file() {
        fingerprints.push(fingerprint_path(directory, &manifest_path)?);
    }
    fingerprints.sort();
    Ok(fingerprints)
}

fn collect_sample_library_inventory_into(
    root: &Path,
    directory: &Path,
    fingerprints: &mut Vec<SampleFileFingerprint>,
) -> Result<(), SampleBankError> {
    let entries = read_sorted_directory(directory)?;
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| directory_io_error(root, &source))?;
        if file_type.is_dir() {
            collect_sample_library_inventory_into(root, &path, fingerprints)?;
        } else if file_type.is_file() && is_supported_sample_path(&path) {
            fingerprints.push(fingerprint_path(root, &path)?);
        }
    }
    Ok(())
}

fn read_sorted_directory(directory: &Path) -> Result<Vec<fs::DirEntry>, SampleBankError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|source| directory_io_error(directory, &source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| directory_io_error(directory, &source))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    Ok(entries)
}

fn directory_io_error(path: &Path, source: &std::io::Error) -> SampleBankError {
    SampleBankError::DirectoryIo {
        path: path.display().to_string().into_boxed_str(),
        message: readable_io_message(source).into_boxed_str(),
    }
}

fn readable_io_message(source: &std::io::Error) -> String {
    match source.kind() {
        std::io::ErrorKind::NotFound => "file not found".to_owned(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
        _ => source.to_string(),
    }
}

fn fingerprint_path(root: &Path, path: &Path) -> Result<SampleFileFingerprint, SampleBankError> {
    let metadata = path
        .metadata()
        .map_err(|source| directory_io_error(root, &source))?;
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(system_time_nanos)
        .unwrap_or(0);
    Ok(SampleFileFingerprint {
        path: path
            .strip_prefix(root)
            .unwrap_or(path)
            .components()
            .filter_map(component_to_path_buf)
            .collect(),
        len: metadata.len(),
        modified_nanos,
    })
}

fn system_time_nanos(time: SystemTime) -> Option<u128> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_nanos())
}

fn component_to_path_buf(component: Component<'_>) -> Option<PathBuf> {
    match component {
        Component::Normal(value) => Some(PathBuf::from(value)),
        Component::CurDir | Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
            None
        }
    }
}

fn handle_decode_error(
    path: &Path,
    source: SampleError,
    mode: DecodeFailureMode,
    errors: &mut Vec<SampleLibraryScanError>,
) -> Result<(), SampleBankError> {
    match mode {
        DecodeFailureMode::Strict => Err(SampleBankError::Decode {
            path: path.display().to_string().into_boxed_str(),
            source,
        }),
        DecodeFailureMode::CollectErrors => {
            errors.push(SampleLibraryScanError::from_decode(path, &source));
            Ok(())
        }
    }
}

fn inferred_token_from_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let mut components = Vec::new();
    if let Some(parent) = relative.parent() {
        for component in parent.components() {
            if let Component::Normal(value) = component {
                components.push(sanitize_token_component(value.to_str()?));
            }
        }
    }
    components.push(sanitize_token_component(path.file_stem()?.to_str()?));
    Some(components.join("/"))
}

fn sanitize_token_component(raw: &str) -> String {
    let mut component = String::with_capacity(raw.len());
    let mut previous_was_separator = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            component.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            component.push('_');
            previous_was_separator = true;
        }
    }
    let component = component.trim_matches('_');
    if component.is_empty() {
        "sample".to_owned()
    } else {
        component.to_owned()
    }
}

fn is_top_level_sample_path(root: &Path, path: &Path) -> bool {
    path.parent().is_some_and(|parent| parent == root)
}
