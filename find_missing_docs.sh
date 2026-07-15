#!/bin/bash
for f in $(find crates -name "*.rs"); do
  awk '
    BEGIN { has_doc=0; }
    /^\s*\/\/\// { has_doc=1; }
    /^\s*#\[/ { /* keep has_doc as is */ }
    /^\s*pub fn/ {
      if (!has_doc) {
        print FILENAME ":" NR " " $0
      }
      has_doc=0;
    }
    /^\s*$/ { has_doc=0; }
    !/^\s*\/\/\// && !/^\s*#\[/ && !/^\s*pub fn/ && !/^\s*$/ { has_doc=0; }
  ' "$f"
done
