#[cfg(test)]
mod tests {
    use crate::midi_input::update_from_message;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_havoc_midi_update_proptest(msg in proptest::collection::vec(any::<u8>(), 0..20)) {
            update_from_message(&msg);
        }
    }
}
