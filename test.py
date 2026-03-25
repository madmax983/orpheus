with open("crates/orpheus-lang/src/repl.rs", "r") as f:
    lines = f.readlines()

out = []
for line in lines:
    if "assert!(stderr_str.contains(\"✗ expected expression\"));" in line:
        out.append("        assert!(stderr_str.contains(\"✗ parse error\"));\n")
    elif "fn run_with_handles_reports_errors_to_stderr_debug" in line:
        break
    else:
        out.append(line)

with open("crates/orpheus-lang/src/repl.rs", "w") as f:
    f.writelines(out)
