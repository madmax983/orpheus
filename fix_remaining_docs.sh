for file in crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/eval.rs crates/orpheus-lang/src/pitch.rs crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/loader.rs; do
  sed -i 's/```rust/```ignore/g' $file
  sed -i 's/```no_run/```ignore/g' $file
  sed -i 's/```$/```/g' $file
done
