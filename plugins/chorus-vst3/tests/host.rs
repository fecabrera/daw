#![allow(non_snake_case, dead_code, clippy::undocumented_unsafe_blocks)]
use daw_chorus::Chorus as Drive;
use daw_chorus_vst3 as plugin_export;
include!("../../drive-vst3/tests/host/support.rs");
#[path = "../../filter-vst3/tests/host/cases.rs"]
mod cases;
#[path = "../../test_support/transport.rs"]
mod transport;

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
    for (frame, tempo, playing, effective_frame) in transport::SEEK_AND_TEMPO {
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
    transport.sampleRate = 48000.0;
    for frame in transport::OFFLINE_FRAMES {
        let mut left = vec![0.4; 512];
        let mut right = vec![-0.2; 512];
        let mut expected = (Vec::new(), Vec::new());
        for i in 0..512 {
            let output =
                direct.process_modulated_at([left[i], right[i]], frame + i as u64, 90.0, [0.0; 8]);
            expected.0.push(output[0]);
            expected.1.push(output[1]);
        }
        transport.projectTimeSamples = frame as i64;
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
