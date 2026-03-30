import re

with open('crates/orpheus-lang/src/export.rs', 'r') as f:
    content = f.read()

# Find the last closing brace of the file
last_brace_idx = content.rfind('}')

if last_brace_idx != -1:
    tests = """
    #[test]
    fn export_sample_pattern_to_csv_returns_error_for_zero_cycles() {
        let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_cycles_sample.csv");
        let result = super::export_sample_pattern_to_csv(pattern, &path, 0);
        assert!(result.is_err());
    }

    #[test]
    fn export_number_pattern_to_csv_returns_error_for_zero_cycles() {
        let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_cycles_number.csv");
        let result = super::export_number_pattern_to_csv(pattern, &path, 0);
        assert!(result.is_err());
    }

    #[test]
    fn export_sample_pattern_to_json_returns_error_for_zero_cycles() {
        let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_cycles_sample.json");
        let result = super::export_sample_pattern_to_json(pattern, &path, 0);
        assert!(result.is_err());
    }

    #[test]
    fn export_number_pattern_to_json_returns_error_for_zero_cycles() {
        let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_cycles_number.json");
        let result = super::export_number_pattern_to_json(pattern, &path, 0);
        assert!(result.is_err());
    }

    #[test]
    fn render_sample_pattern_to_file_with_bank_returns_error_for_zero_cycles() {
        let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_cycles_render.wav");
        let bank = orpheus_dsp::SampleBank::load_builtin();
        let result = super::render_sample_pattern_to_file_with_bank(pattern, &path, 0, &bank);
        assert!(result.is_err());
    }
"""
    new_content = content[:last_brace_idx] + tests + "\n}\n"
    with open('crates/orpheus-lang/src/export.rs', 'w') as f:
        f.write(new_content)
