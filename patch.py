import re

with open('crates/orpheus-dsp/src/effects/reverb.rs', 'r') as f:
    content = f.read()

content = content.replace("""    #[test]
    fn should_process_comb_filter() {
        let mut comb = CombState::new(4, 0.5, 0.2);

        // Frame 1
        let out1 = comb.process(1.0);
        assert_eq!(out1, 0.0);
        assert_eq!(comb.buffer[0], 0.5); // 0.0 + 0.5 * 1.0
        assert_eq!(comb.index, 1);

        // Advance to loop point
        let _ = comb.process(0.0);
        let _ = comb.process(0.0);
        let _ = comb.process(0.0);

        // Frame 5 (feedback occurs)
        let out5 = comb.process(0.0);
        assert_eq!(out5, 0.0); // previous input comes out
        // The filter store calculation:
        // 0.0 mul_add(1.0 - 0.2, 0.0) = 0.0
        // 0.0 mul_add(0.5, 0.0) = 0.0
        assert_eq!(comb.index, 1);
    }""", """    #[test]
    fn should_process_comb_filter() {
        let mut comb = CombState::new(4, 0.5, 0.2);

        // Frame 1
        let out1 = comb.process(1.0);
        assert_eq!(out1, 0.0);
        assert_eq!(comb.buffer[0], 0.5); // 0.0 + 0.5 * 1.0
        assert_eq!(comb.index, 1);

        // Advance to loop point
        let _ = comb.process(0.0);
        let _ = comb.process(0.0);
        let _ = comb.process(0.0);

        // Frame 5 (feedback occurs)
        let out5 = comb.process(0.0);
        assert_eq!(out5, 0.5); // previous input comes out
        assert_eq!(comb.index, 1);
    }""")

content = content.replace("""    #[test]
    fn should_process_allpass_filter() {
        let mut allpass = AllpassState::new(2, 0.5);

        // Frame 1
        let out1 = allpass.process(1.0);
        assert_eq!(out1, -1.0); // 0.0 - 1.0
        assert_eq!(allpass.buffer[0], 0.5); // 0.0 + 0.5 * 1.0
        assert_eq!(allpass.index, 1);

        // Frame 2
        let out2 = allpass.process(0.0);
        assert_eq!(out2, 0.0);
        assert_eq!(allpass.buffer[1], 0.0);
        assert_eq!(allpass.index, 0);

        // Frame 3 (feedback occurs)
        let out3 = allpass.process(0.0);
        assert_eq!(out3, 0.5); // buffered 0.5 - 0.0
    }""", """    #[test]
    fn should_process_allpass_filter() {
        let mut allpass = AllpassState::new(2, 0.5);

        // Frame 1
        let out1 = allpass.process(1.0);
        assert_eq!(out1, -1.0); // 0.0 - 1.0
        assert_eq!(allpass.buffer[0], 0.5); // 0.0 + 0.5 * 1.0
        assert_eq!(allpass.index, 1);

        // Frame 2
        let out2 = allpass.process(0.0);
        assert_eq!(out2, 0.0);
        assert_eq!(allpass.buffer[1], 0.0);
        assert_eq!(allpass.index, 0);

        // Frame 3 (feedback occurs)
        let out3 = allpass.process(0.0);
        assert_eq!(out3, 0.5); // buffered 0.5 - 0.0
    }""")

with open('crates/orpheus-dsp/src/effects/reverb.rs', 'w') as f:
    f.write(content)
