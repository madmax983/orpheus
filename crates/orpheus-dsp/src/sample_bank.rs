//! The `sample_bank` module manages collections of loaded audio samples.
//!
//! A `SampleBank` acts as an in-memory repository mapping string identifiers (like "bd" or "sn")
//! to fully decoded `DecodedSample` buffers, allowing the engine to quickly look up
//! and trigger audio events.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

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

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackSample {
    frames: Arc<[f32]>,
    sample_rate_hz: u32,
}

impl PlaybackSample {
    #[must_use]
    pub const fn frames(&self) -> &Arc<[f32]> {
        &self.frames
    }

    #[must_use]
    pub const fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
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

    #[must_use]
    pub fn get(&self, voice: VoiceKind) -> Option<&PlaybackSample> {
        self.get_by_token(voice.token())
    }

    #[must_use]
    pub fn get_by_token(&self, token: &str) -> Option<&PlaybackSample> {
        self.samples.get(token).map(|entry| &entry.sample)
    }

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
    #[error("failed to read sample directory `{path}`: {message}")]
    DirectoryIo { path: Box<str>, message: Box<str> },
    #[error("failed to read sample manifest `{path}`: {message}")]
    ManifestIo { path: Box<str>, message: Box<str> },
    #[error("failed to parse sample manifest `{path}`: {message}")]
    ManifestParse { path: Box<str>, message: Box<str> },
    #[error("sample manifest `{path}` aliases `{alias}` to unknown token `{target}`")]
    ManifestAliasTarget {
        path: Box<str>,
        alias: Box<str>,
        target: Box<str>,
    },
    #[error("sample manifest `{path}` contains an alias cycle at `{alias}` via `{target}`")]
    ManifestAliasCycle {
        path: Box<str>,
        alias: Box<str>,
        target: Box<str>,
    },
    #[error("sample manifest `{path}` region `{region}` targets unknown token `{target}`")]
    ManifestRegionTarget {
        path: Box<str>,
        region: Box<str>,
        target: Box<str>,
    },
    #[error("sample manifest `{path}` contains a region cycle at `{region}` via `{target}`")]
    ManifestRegionCycle {
        path: Box<str>,
        region: Box<str>,
        target: Box<str>,
    },
    #[error("failed to decode sample override `{path}`: {source}")]
    Decode {
        path: Box<str>,
        #[source]
        source: SampleError,
    },
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
    let directory = directory.as_ref();
    let display_path = directory.display().to_string();
    let mut entries = fs::read_dir(directory)
        .map_err(|source| SampleBankError::DirectoryIo {
            path: display_path.clone().into_boxed_str(),
            message: match source.kind() {
                std::io::ErrorKind::NotFound => "file not found".into(),
                std::io::ErrorKind::PermissionDenied => "permission denied".into(),
                _ => source.to_string().into_boxed_str(),
            },
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| SampleBankError::DirectoryIo {
            path: display_path.clone().into_boxed_str(),
            message: match source.kind() {
                std::io::ErrorKind::NotFound => "file not found".into(),
                std::io::ErrorKind::PermissionDenied => "permission denied".into(),
                _ => source.to_string().into_boxed_str(),
            },
        })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let manifest_path = directory.join(SAMPLE_MANIFEST_FILE);
    let manifest = load_optional_manifest(&manifest_path)?;

    let mut bank = SampleBank::load_builtin();
    let mut inferred_tokens = BTreeMap::<String, PlaybackSample>::new();
    let mut candidates: BTreeMap<&'static str, (u8, PlaybackSample)> = BTreeMap::new();

    for entry in entries {
        let path = entry.path();
        if !path.is_file() || !is_supported_wav_path(&path) {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let sample = load_wav_for_test(&path).map_err(|source| SampleBankError::Decode {
            path: path.display().to_string().into_boxed_str(),
            source,
        })?;
        let playback_sample: PlaybackSample = sample.into();
        inferred_tokens
            .entry(inferred_token_from_stem(stem))
            .or_insert_with(|| playback_sample.clone());
        if let Some((token, priority)) = token_from_stem(stem) {
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
    apply_manifest_tokens(&mut bank, manifest.as_ref(), directory)?;
    apply_manifest_regions(&mut bank, manifest.as_ref(), &manifest_path)?;
    apply_manifest_aliases(&mut bank, manifest.as_ref(), &manifest_path)?;

    Ok(bank)
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

fn is_supported_wav_path(path: &Path) -> bool {
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
) -> Result<(), SampleBankError> {
    let Some(manifest) = manifest else {
        return Ok(());
    };

    for (token, relative_path) in &manifest.tokens {
        let path = directory.join(relative_path);
        let sample = load_wav_for_test(&path).map_err(|source| SampleBankError::Decode {
            path: path.display().to_string().into_boxed_str(),
            source,
        })?;
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

fn inferred_token_from_stem(stem: &str) -> String {
    stem.to_ascii_lowercase()
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
