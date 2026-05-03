use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use orpheus_lang::export_number_pattern_to_tracker;
use orpheus_lang::export_sample_pattern_to_tracker;
use orpheus_lang::render_ascii_number_roll;
use orpheus_lang::render_ascii_roll;

#[test]
fn test_havoc_tracker_sample_oom() {
    let source = "pattern = bd sn cp";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

    // We expect this to fail gracefully rather than attempting to allocate gigabytes.
    // The internal limit bounds `total_steps` (cycle_count * steps_per_cycle).
    // `100_000 * 16 = 1.6 million > 100_000`, so it fails cleanly without OOMing.
    let result = export_sample_pattern_to_tracker(pattern, "test_havoc.trk", 100_000);
    assert!(
        result.is_err(),
        "Should have failed due to maximum step limit"
    );
}

#[test]
fn test_havoc_tracker_number_oom() {
    let source = "pattern = 1 2 3";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

    let result = export_number_pattern_to_tracker(pattern, "test_havoc.trk", 100_000);
    assert!(
        result.is_err(),
        "Should have failed due to maximum step limit"
    );
}

#[test]
fn test_havoc_ascii_roll_sample_oom() {
    let source = "pattern = bd sn cp";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

    let result = render_ascii_roll("pattern", pattern, 100_000, 16);
    assert!(
        result.is_err(),
        "Should have failed due to maximum step limit"
    );
}

#[test]
fn test_havoc_ascii_roll_number_oom() {
    let source = "pattern = 1 2 3";
    let module = eval_module(source, ReplMode::Loose).unwrap();
    let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

    let result = render_ascii_number_roll("pattern", pattern, 100_000, 16);
    assert!(
        result.is_err(),
        "Should have failed due to maximum step limit"
    );
}
