1. **Explore & Verify:** Run some bash commands.
2. **Draft Feature:** Create a Minecraft Command Block exporter (`minecraft_export.rs`), mapping `NumberPatternValue` into `/playsound` commands for Note Blocks (or standard Minecraft instruments).
   - Exporters are common in this codebase (`arduino_export`, `abc_export`, `gcode_export`, `scad_export`, `sonic_pi_export`, etc). This fits the R&D theme perfectly.
   - It will export Minecraft `.mcfunction` files! A "Minecraft Datapack Exporter".
   - It doesn't modify core logic, just additive.
3. **Draft the Implementation:**
   - Map Orpheus note ranges to Minecraft note block sounds and pitch values (0.5 to 2.0).
   - Write output to a `.mcfunction` file.
   - Use `export_number_pattern_to_minecraft` in a new module.
4. **Integrate:**
   - Add module to `crates/orpheus-lang/src/lib.rs`.
   - Ensure it compiles and tests pass.
