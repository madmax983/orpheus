sed -i 's/for (step, val_opt) in grid.iter().enumerate().take(total_steps) {/for (step, val_opt) in grid.iter().enumerate().take(total_steps) {/g' crates/orpheus-lang/src/tracker.rs
sed -i 's/if let Some(ref val) = grid\[step\] {/if let Some(val) = val_opt {/g' crates/orpheus-lang/src/tracker.rs
