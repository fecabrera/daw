#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_auto_filter::AutoFilter as Drive;
use daw_auto_filter_vst3 as plugin_export;
include!("../../drive-vst3/tests/host/support.rs");
#[path = "../../filter-vst3/tests/host/cases.rs"]
mod cases;

#[test]
fn host_tempo_position_and_seeks_match_the_portable_clock() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let mut state = PluginState::default();
    state.set(P::RateMode, 1.0).unwrap();
    plugin.load(state);
    plugin.activate(48000, 512, 0);
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
            let output = direct.process_at([left[i], right[i]], effective_frame + i as u64, tempo);
            expected.0.push(output[0]);
            expected.1.push(output[1]);
        }
        let mut transport: ProcessContext = unsafe { std::mem::zeroed() };
        transport.state = ProcessContext_::StatesAndFlags_::kTempoValid as _;
        if playing {
            let flag: u32 = ProcessContext_::StatesAndFlags_::kPlaying as _;
            transport.state |= flag;
        }
        transport.tempo = f64::from(tempo);
        transport.projectTimeSamples = frame as i64;
        transport.sampleRate = 48000.0;
        let actual = run_with_transport(
            &plugin,
            &mut left,
            &mut right,
            &[],
            0,
            false,
            0,
            &mut transport,
        );
        assert_samples(&actual, &expected);
    }
}

#[test]
fn offline_context_uses_the_host_position_without_a_playing_flag() {
    let _serial = SERIAL.lock().unwrap();
    let mut plugin = Plugin::new();
    let mut state = PluginState::default();
    state.set(P::RateMode, 1.0).unwrap();
    plugin.load(state);
    plugin.activate(48000, 512, 2);
    let mut direct = Drive::new(
        ProcessingContext {
            sample_rate_hz: 48000,
            max_block_frames: 512,
        },
        &state.to_settings(),
    );
    let mut transport: ProcessContext = unsafe { std::mem::zeroed() };
    transport.state = ProcessContext_::StatesAndFlags_::kTempoValid as _;
    transport.tempo = 90.0;
    transport.projectTimeSamples = 48000;
    transport.sampleRate = 48000.0;
    for block in 0..4 {
        let mut left = vec![0.4; 512];
        let mut right = vec![-0.2; 512];
        let mut expected = (Vec::new(), Vec::new());
        for i in 0..512 {
            let output =
                direct.process_at([left[i], right[i]], 48000 + block * 512 + i as u64, 90.0);
            expected.0.push(output[0]);
            expected.1.push(output[1]);
        }
        transport.projectTimeSamples = 48000 + block as i64 * 512;
        let actual = run_with_transport(
            &plugin,
            &mut left,
            &mut right,
            &[],
            2,
            false,
            0,
            &mut transport,
        );
        assert_samples(&actual, &expected);
    }
}
