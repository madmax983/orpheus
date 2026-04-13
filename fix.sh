sed -i '$d' src/main.rs
cat << 'INNER_EOF' >> src/main.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn startup_path_from_args_handles_empty_args() {
        let args: Vec<OsString> = vec![];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Run(None) => (),
            _ => panic!("Expected Run(None)"),
        }
    }

    #[test]
    fn startup_path_from_args_handles_help_flag() {
        let args = vec![OsString::from("--help")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Help => (),
            _ => panic!("Expected Help"),
        }

        let args = vec![OsString::from("-h")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Help => (),
            _ => panic!("Expected Help"),
        }
    }

    #[test]
    fn startup_path_from_args_handles_version_flag() {
        let args = vec![OsString::from("--version")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Version => (),
            _ => panic!("Expected Version"),
        }

        let args = vec![OsString::from("-V")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Version => (),
            _ => panic!("Expected Version"),
        }
    }

    #[test]
    fn startup_path_from_args_handles_file_path() {
        let args = vec![OsString::from("test.ode")];
        let action = startup_path_from_args(args).unwrap();
        match action {
            CliAction::Run(Some(path)) => assert_eq!(path, PathBuf::from("test.ode")),
            _ => panic!("Expected Run(Some)"),
        }
    }

    #[test]
    fn startup_path_from_args_rejects_unknown_flags() {
        let args = vec![OsString::from("--unknown")];
        let error = startup_path_from_args(args).unwrap_err();
        assert!(error.to_string().contains("unexpected argument"));
    }

    #[test]
    fn startup_path_from_args_rejects_multiple_args() {
        let args = vec![OsString::from("file1.ode"), OsString::from("file2.ode")];
        let error = startup_path_from_args(args).unwrap_err();
        assert!(error.to_string().contains("usage: orpheus [path/to/song.ode]"));
    }
}
INNER_EOF
cargo clippy --all-targets --all-features -- -D warnings
