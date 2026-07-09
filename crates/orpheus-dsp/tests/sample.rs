//! Integration tests for the static wav sample decoding, playback framing, bounds checking, and basic looping slice logic.
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use orpheus_dsp::{
    SampleBankError, SampleError, SampleLibraryReload, SampleLibraryWatcher,
    SampleLibraryWatcherConfig, load_builtin_sample_for_test, load_sample_bank_from_directory,
    load_wav_for_test,
};

static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn wav_loader_decodes_mono_f32_samples() {
    let sample = load_wav_for_test(fixture("kick.wav")).unwrap();

    assert!(!sample.frames.is_empty());
}

#[test]
fn wav_loader_rejects_non_mono_or_stereo_sources() {
    let path = temp_fixture("tri_channel.wav");
    let spec = hound::WavSpec {
        channels: 3,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&path, spec).unwrap();
    for sample in [0_i16, 0, 0] {
        writer.write_sample(sample).unwrap();
    }
    writer.finalize().unwrap();

    let error = load_wav_for_test(&path).unwrap_err();
    assert!(matches!(error, SampleError::UnsupportedChannelCount(_)));

    fs::remove_file(path).unwrap();
}

#[test]
fn built_in_drum_assets_decode_for_test_use() {
    for token in ["bd", "sn", "cp", "hh"] {
        let sample = load_builtin_sample_for_test(token).unwrap();
        assert!(
            !sample.frames.is_empty(),
            "builtin sample {token} was empty"
        );
    }
}

#[test]
fn sample_directory_scan_maps_common_aliases_to_builtin_tokens() {
    let directory = temp_directory("sample-aliases");
    write_wav(directory.join("kick.wav"), &[0.25, 0.0, 0.0, 0.0]);
    write_wav(directory.join("snare.wav"), &[0.5, 0.0, 0.0, 0.0]);

    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let available = bank.available_tokens();

    assert!(available.contains(&"bd".to_owned()));
    assert!(available.contains(&"sn".to_owned()));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sample_manifest_maps_explicit_tokens_and_aliases() {
    let directory = temp_directory("sample-manifest");
    write_wav(directory.join("vox.wav"), &[0.75, 0.0, 0.0, 0.0]);
    fs::write(
        directory.join("samples.ron"),
        "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n  aliases: {\n    \"vox\": \"vox_ah\",\n  },\n)\n",
    )
    .unwrap();

    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let direct = bank.get_by_token("vox_ah").unwrap();
    let alias = bank.get_by_token("vox").unwrap();

    assert_eq!(direct, alias);
    assert!((direct.frames()[0] - 0.75).abs() < f32::EPSILON);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sample_manifest_rejects_regions_targeting_unknown_tokens() {
    let directory = temp_directory("sample-region-errors");
    fs::write(
        directory.join("samples.ron"),
        concat!(
            "(\n",
            "  regions: {\n",
            "    \"ghost\": (\n",
            "      token: \"missing\",\n",
            "      start: 0.0,\n",
            "      end: 0.5,\n",
            "    ),\n",
            "  },\n",
            ")\n"
        ),
    )
    .unwrap();

    let error = load_sample_bank_from_directory(&directory).unwrap_err();

    assert!(matches!(
        error,
        SampleBankError::ManifestRegionTarget { .. }
    ));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sample_directory_scan_infers_token_names_from_filenames() {
    let directory = temp_directory("sample-inference");
    write_wav(directory.join("Vox_Ah.wav"), &[0.2, 0.0, 0.0, 0.0]);

    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let sample = bank.get_by_token("vox_ah").unwrap();

    assert!((sample.frames()[0] - 0.2).abs() < f32::EPSILON);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sample_directory_scan_recurses_and_uses_relative_token_paths() {
    let directory = temp_directory("sample-recursive");
    let drums = directory.join("drums");
    fs::create_dir_all(&drums).unwrap();
    write_wav(drums.join("Kick Main.wav"), &[0.33, 0.0, 0.0, 0.0]);

    let bank = load_sample_bank_from_directory(&directory).unwrap();
    let sample = bank.get_by_token("drums/kick_main").unwrap();

    assert!((sample.frames()[0] - 0.33).abs() < f32::EPSILON);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sample_library_watcher_loads_new_files_within_latency_budget() {
    let directory = temp_directory("sample-watcher-add");
    let mut watcher = watch_fast(&directory);
    let start = Instant::now();

    write_wav(directory.join("rim.wav"), &[0.44, 0.0, 0.0, 0.0]);
    let reload = wait_for_reload(&mut watcher, |reload| {
        reload.bank().get_by_token("rim").is_some()
    });

    assert!(start.elapsed() <= Duration::from_millis(500));
    assert!(reload.errors().is_empty());
    assert!(reload.tokens().contains(&"rim".to_owned()));

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sample_library_watcher_purges_deleted_samples() {
    let directory = temp_directory("sample-watcher-delete");
    write_wav(directory.join("loop.wav"), &[0.12, 0.0, 0.0, 0.0]);
    let mut watcher = watch_fast(&directory);

    fs::remove_file(directory.join("loop.wav")).unwrap();
    let reload = wait_for_reload(&mut watcher, |reload| {
        !reload.tokens().contains(&"loop".to_owned())
    });

    assert!(reload.bank().get_by_token("loop").is_none());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn sample_library_watcher_reports_corrupt_files_but_keeps_valid_samples() {
    let directory = temp_directory("sample-watcher-corrupt");
    let mut watcher = watch_fast(&directory);

    fs::write(directory.join("broken.wav"), b"not a wav").unwrap();
    write_wav(directory.join("good.wav"), &[0.66, 0.0, 0.0, 0.0]);
    let reload = wait_for_reload(&mut watcher, |reload| {
        reload.bank().get_by_token("good").is_some() && !reload.errors().is_empty()
    });

    assert!(reload.bank().get_by_token("broken").is_none());

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn temp_fixture_uses_system_temp_directory() {
    let path = temp_fixture("fixture.wav");

    assert!(path.starts_with(std::env::temp_dir()));
}

fn temp_fixture(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("orpheus-dsp-{}-{name}", unique_temp_suffix()))
}

fn temp_directory(name: &str) -> PathBuf {
    let directory = temp_fixture(name);
    fs::create_dir_all(&directory).unwrap();
    directory
}

fn watch_fast(directory: &std::path::Path) -> SampleLibraryWatcher {
    let config = SampleLibraryWatcherConfig {
        poll_interval: Duration::from_millis(10),
    };
    SampleLibraryWatcher::spawn(directory, config).unwrap()
}

fn wait_for_reload(
    watcher: &mut SampleLibraryWatcher,
    predicate: impl Fn(&SampleLibraryReload) -> bool,
) -> SampleLibraryReload {
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        while let Some(reload) = watcher.try_recv() {
            if predicate(&reload) {
                return reload;
            }
        }
        assert!(
            Instant::now() < deadline,
            "sample watcher did not reload in time"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn unique_temp_suffix() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}-{counter}")
}

fn write_wav(path: PathBuf, frames: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for sample in frames {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
}
