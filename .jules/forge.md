**[Extracted validate_value method]
**Learning:** `validate_control_events` was a God function that violated `clippy::too_many_lines` with a massive match block handling every control pattern kind identically. It violated the Tell, Don't Ask principle.
**Action:** Extract the specific validation logic as a method (`validate_value(self, value)`) onto the `ControlPatternKind` enum itself.
