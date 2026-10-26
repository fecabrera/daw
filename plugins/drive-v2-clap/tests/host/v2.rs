use super::*;
#[test]
fn all_v2_parameters_retarget_with_direct_dsp_parity() {
    let _serial = SERIAL.lock().unwrap();
    for rate in SUPPORTED_RATES {
        for hard in [0.0, 1.0] {
            let mut plugin = Plugin::new();
            let mut state = PluginState::default();
            state.set(P::ColorEnabled, 1.0).unwrap();
            state.set(P::Clip, hard).unwrap();
            state.set(P::InputGain, 6.0).unwrap();
            state.set(P::DryWet, 0.37).unwrap();
            plugin.load(state);
            plugin.activate(rate, 512);
            let mut drive = Drive::new(
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: 512,
                },
                &state.to_settings(),
            );
            let left: Vec<_> = (0..512).map(|i| (i as f32 * 0.11).sin() * 0.8).collect();
            let right: Vec<_> = left.iter().map(|v| -v * 0.3 + 0.07).collect();
            for p in P::ALL {
                let events = EventList::new(&[(0, p, p.info().maximum), (17, p, p.info().minimum)]);
                let expected = reference(&mut drive, &mut state, &left, &right, &events);
                let actual = run(
                    &plugin,
                    &mut left.clone(),
                    &mut right.clone(),
                    &events,
                    false,
                    0,
                );
                assert_samples(&actual, &expected);
            }
        }
    }
}
