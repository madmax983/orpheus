So moving `EngineError` to `error.rs` might break the cycle.
Let's see if there is another circular dependency:
`sample_bank -> voice -> effects -> engine -> sample_bank`
`engine` uses `sample_bank::SampleBank`
`sample_bank` uses `voice::Voice`? Let's check `crates/orpheus-dsp/src/sample_bank.rs` for `voice` usage.
