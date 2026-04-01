RUSTFLAGS="-D warnings" cargo clippy -p orpheus-lang --all-targets --all-features
cargo test
