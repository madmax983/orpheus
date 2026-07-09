sed -i 's/writeln!(stderr, \"{}\", format_args!(\"{} {msg}\", \"\[Warn\]\".yellow().bold()))/writeln!(stderr, \"{} {msg}\", \"\[Warn\]\".yellow().bold())/g' crates/orpheus-lang/src/repl.rs
sed -i 's/writeln!(stdout, \"{}\", format_args!(\"{} {msg}\", \"\\u{2713}\".green()))/writeln!(stdout, \"{} {msg}\", \"\\u{2713}\".green())/g' crates/orpheus-lang/src/repl.rs
sed -i 's/writeln!(stderr, \"{}\", format_args!(\"{} {msg}\", \"\\u{2717}\".red().bold()))/writeln!(stderr, \"{} {msg}\", \"\\u{2717}\".red().bold())/g' crates/orpheus-lang/src/repl.rs
sed -i 's/writeln!(stdout, \"{}\", format_args!(\"{} {message}\", \"\\u{2713}\".green()))/writeln!(stdout, \"{} {message}\", \"\\u{2713}\".green())/g' crates/orpheus-lang/src/repl.rs
sed -i 's/writeln!(stderr, \"{}\", format_args!(\"{} {message}\", \"\\u{2717}\".red().bold()))/writeln!(stderr, \"{} {message}\", \"\\u{2717}\".red().bold())/g' crates/orpheus-lang/src/repl.rs
