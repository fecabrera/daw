#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_chorus::Chorus as Drive;
use daw_chorus_clap as plugin_export;
include!("../../drive-clap/tests/host/support.rs");
#[path = "../../filter-clap/tests/host/cases.rs"]
mod cases;

#[test]
fn host_tempo_position_and_seeks_match_the_portable_clock() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let mut state = PluginState::default();
    state.set(P::RateMode, 1.0).unwrap();
    plugin.load(state);
    plugin.activate(48000, 512);
    let mut direct = Drive::new(
        ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 512,
        },
        &state.to_settings(),
    );
    let mut next_frame = 0;
    for (frame, tempo, playing) in [
        (48000, 90.0, true),
        (96000, 180.0, true),
        (12000, 110.0, true),
        (12000, 110.0, false),
        (12000, 110.0, false),
        (72000, 110.0, true),
    ] {
        let effective_frame = if playing { frame } else { next_frame };
        next_frame = effective_frame + 512;
        let mut left: Vec<_> = (0..512).map(|i| (i as f32 * 0.17).sin() * 0.4).collect();
        let mut right = left.iter().map(|v| -0.37 * v).collect::<Vec<_>>();
        let mut expected = (Vec::new(), Vec::new());
        for i in 0..512 {
            let output = direct.process_modulated_at(
                [left[i], right[i]],
                effective_frame + i as u64,
                tempo,
                [0.0; 8],
            );
            expected.0.push(output[0]);
            expected.1.push(output[1]);
        }
        let mut transport: clap_event_transport = unsafe { std::mem::zeroed() };
        transport.flags = CLAP_TRANSPORT_HAS_TEMPO | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE;
        if playing {
            transport.flags |= CLAP_TRANSPORT_IS_PLAYING;
        }
        transport.tempo = f64::from(tempo);
        transport.song_pos_seconds = (frame as f64 / 48000.0
            * daw_plugin_clap::sys::fixedpoint::CLAP_SECTIME_FACTOR as f64)
            .round() as i64;
        let actual = run_with_transport(
            &plugin,
            &mut left,
            &mut right,
            &EventList::new(&[]),
            false,
            0,
            &transport,
        );
        assert_samples(&actual, &expected);
    }
}
