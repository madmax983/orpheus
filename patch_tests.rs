#[cfg(test)]
mod explain_tests {
    use super::*;
    use crate::explain::Explain;

    #[test]
    fn explain_works_for_new_values() {
        let plugin_value = Value::PluginPattern(PluginPatternValue::new(
            orpheus_dsp::PluginTrackSource::new(orpheus_dsp::PluginDescriptor::vst3("Dummy")),
        ));
        let explanation = plugin_value.explain("my_plugin");
        assert!(explanation.contains("Plugin Instrument"));

        let pitch_class_value = Value::PitchClassSet(PitchClassSetValue::new(vec![0, 4, 7]).unwrap());
        let explanation = pitch_class_value.explain("my_chord");
        assert!(explanation.contains("Scale/Chord"));

        let arp_direction_value = Value::ArpDirection(ArpDirectionValue::Up);
        let explanation = arp_direction_value.explain("my_arp_dir");
        assert!(explanation.contains("Arpeggiator Direction"));
    }
}
