# 🔭 Vantage: Spec for Safe NaN/Infinity Handling in Number Pattern Exports

## Context
When users generate dynamic number patterns (e.g., control values, algorithmic sequences), mathematical operations or corrupted inputs can result in invalid floating-point values like `NaN` (Not-a-Number), `+Infinity`, or `-Infinity`. Currently, if a user exports a sequence containing these values to CSV, JSON, HTML, or SVG reports, the system behavior might be unhandled, causing the output parser to fail downstream or panic during serialization.

## 👤 User Story
"As a Data Analyst or Composer exporting pattern metrics, I want the export tools to gracefully handle invalid numerical data (`NaN` or Infinity), so that my entire report doesn't crash or corrupt downstream analysis tools."

## ❓ The "So What?"
**What business problem does this solve?**
Unpredictable panics or malformed files destroy trust. By formally defining how we handle mathematical outliers in our reports, we ensure our tooling pipeline (from Orpheus to Python/Excel/DAW) is reliable and doesn't require users to manually sanitize their data first. Complexity is a cost, but failing silently or crashing is a liability.

## ✅ Acceptance Criteria
- **Formatting Standardization:** When exporting a `NumberPattern` containing `NaN`, the system must serialize it explicitly as a valid null/placeholder equivalent instead of panicking.
  - In JSON: output as `null` (since JSON spec does not support `NaN`).
  - In CSV/TXT: output as the string `"NaN"`.
- **Handling Infinities:**
  - `+Infinity` must be formatted as `"Infinity"` (or `null` in JSON).
  - `-Infinity` must be formatted as `"-Infinity"` (or `null` in JSON).
- **Graceful Degradation:** The presence of `NaN` or Infinity in a single event *must not* cause the export function to abort the file generation for the rest of the sequence.

## 🚫 Out of Scope
- Implementing automatic interpolation or "fixing" of `NaN` values within the pattern engine itself (Engineering's job to handle DSP limits).
- Changing how the Ratatui terminal UI displays `NaN` values (This spec strictly targets the `export_number_pattern_to_*` pipeline).
