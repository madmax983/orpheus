#!/bin/bash
awk '
  /^ *fn / || /^ *pub fn / || /^ *pub\(crate\) fn / {
    if (in_func) {
      if (lines > 50) print file ":" start_line " - " func_name " (" lines " lines)";
    }
    in_func = 1
    start_line = NR
    lines = 0
    func_name = $0
  }
  in_func { lines++ }
  /^}/ && in_func {
    # simple heuristic
  }
' $(find crates -name "*.rs")
