sed -i 's/write!(file, " {:4} |", padded)?;/write!(file, " {padded:4} |")?;/g' crates/orpheus-lang/src/tracker.rs
