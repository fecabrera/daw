use daw_core::*;
use daw_engine::{RenderPlan, Renderer};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;
thread_local! { static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) }; static ALLOCATIONS: Cell<usize> = const { Cell::new(0) }; }
struct CountingAllocator;
// SAFETY: This allocator forwards unchanged layouts and pointers to System.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        TRACK_ALLOCATIONS.with(|enabled| {
            if enabled.get() {
                ALLOCATIONS.with(|count| count.set(count.get() + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
fn plan(waveform: Waveform, rate: u32) -> RenderPlan {
    let mut project = Project {
        sample_rate_hz: rate,
        ..Project::default()
    };
    let track = project.add_midi_track().unwrap();
    project.tracks[0].instrument.as_mut().unwrap().waveform = waveform;
    let clip = project.add_midi_pattern(track, 0).unwrap();
    project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: vec![
                    MidiNote {
                        id: Id::from_u128(1),
                        start_tick: 0,
                        length_ticks: 960,
                        pitch: 69,
                        velocity: 127,
                    },
                    MidiNote {
                        id: Id::from_u128(2),
                        start_tick: 480,
                        length_ticks: 1440,
                        pitch: 72,
                        velocity: 100,
                    },
                ],
            },
        })
        .unwrap();
    project.tracks[0].clips[0].length_frames = project.midi_frame(3840);
    RenderPlan {
        project,
        audio: HashMap::new(),
        stretched_audio: HashMap::new(),
    }
}
#[test]
fn oscillator_output_is_finite_nonzero_and_independent_of_block_size_at_all_rates() {
    for rate in SAMPLE_RATES {
        for waveform in Waveform::ALL {
            let plan = plan(waveform, rate);
            let count = rate as usize;
            let mut expected = vec![[0.0; 2]; count];
            Renderer::new(plan.clone()).render_offline(&mut expected);
            assert!(
                expected
                    .iter()
                    .all(|s| s.iter().all(|x| x.is_finite() && x.abs() < 1.0))
            );
            assert!(expected.iter().any(|s| s[0].abs() > 0.05));
            for size in BUFFER_SIZES {
                let mut renderer = Renderer::new(plan.clone());
                let mut actual = vec![[0.0; 2]; count];
                for block in actual.chunks_mut(size as usize) {
                    renderer.render_offline(block);
                }
                assert_eq!(actual, expected);
            }
        }
    }
}
#[test]
fn playback_matches_export_after_start_ramp_and_pause_preserves_voice_state() {
    let plan = plan(Waveform::Sine, 48_000);
    let mut live = Renderer::new(plan.clone());
    live.play();
    let mut offline = Renderer::new(plan.clone());
    let mut expected = [[0.0; 2]; 500];
    offline.render_offline(&mut expected);
    for (index, sample) in expected.iter().enumerate() {
        let actual = live.next_sample();
        if index >= 240 {
            assert_eq!(actual, *sample);
        }
    }
    live.pause();
    let position = live.playhead;
    for _ in 0..300 {
        live.next_sample();
    }
    assert_eq!(live.playhead, position);
    live.play();
    for _ in 0..300 {
        live.next_sample();
    }
    let mut continuation = [[0.0; 2]; 300];
    offline.render_offline(&mut continuation);
    assert_eq!(live.next_sample(), {
        let mut s = [[0.0; 2]; 1];
        offline.render_offline(&mut s);
        s[0]
    });
    live.seek(0);
    live.stop();
    assert!(!live.playing);
}
#[test]
fn callbacks_note_events_seek_loop_and_renderer_replacement_do_not_allocate() {
    for waveform in Waveform::ALL {
        let mut plan = plan(waveform, 48_000);
        plan.project.transport.r#loop = Loop {
            enabled: true,
            start_frame: 0,
            end_frame: 48_000,
        };
        let mut renderer = Renderer::new(plan.clone());
        let mut updated = Renderer::new(plan);
        ALLOCATIONS.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
        renderer.play();
        for _ in 0..60_000 {
            renderer.next_sample();
        }
        renderer.pause();
        for _ in 0..512 {
            renderer.next_sample();
        }
        updated.inherit(&mut renderer);
        updated.seek(12_000);
        updated.play();
        for _ in 0..512 {
            updated.next_sample();
        }
        updated.stop();
        TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    }
}

#[test]
fn trimmed_clip_does_not_restart_a_release_tail_in_the_gap_and_mute_keeps_processing() {
    let mut original = plan(Waveform::Sine, 48_000);
    original.project.tracks[0].clips[0].length_frames = 18_000;
    let mut output = vec![[0.0; 2]; 19_000];
    Renderer::new(original).render_offline(&mut output);
    assert!(output[18_000..].iter().all(|s| *s == [0.0; 2]));
    let plan = plan(Waveform::Sine, 48_000);
    let mut running = Renderer::new(plan.clone());
    running.play();
    for _ in 0..1000 {
        running.next_sample();
    }
    let mut muted_plan = plan.clone();
    muted_plan.project.tracks[0].muted = true;
    let mut muted = Renderer::new(muted_plan);
    muted.inherit(&mut running);
    for _ in 0..4000 {
        muted.next_sample();
    }
    let mut unmuted = Renderer::new(plan.clone());
    unmuted.inherit(&mut muted);
    for _ in 0..1000 {
        unmuted.next_sample();
    }
    let mut expected = Renderer::new(plan);
    let mut samples = vec![[0.0; 2]; 6001];
    expected.render_offline(&mut samples);
    assert_eq!(unmuted.next_sample(), samples[6000]);
}

#[test]
fn polyphonic_playback_and_seek_mix_each_pitch_independently() {
    let mut chord = plan(Waveform::Sine, 48_000);
    let clip = chord.project.tracks[0].clips[0].id;
    let notes: Vec<_> = [69, 72, 76]
        .into_iter()
        .enumerate()
        .map(|(index, pitch)| MidiNote {
            id: Id::from_u128(index as u128 + 100),
            start_tick: 0,
            length_ticks: 1440,
            pitch,
            velocity: 100,
        })
        .collect();
    chord
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: notes.clone(),
            },
        })
        .unwrap();
    for start in [0, 12_000] {
        let mut combined = Renderer::new(chord.clone());
        combined.seek(start);
        let mut actual = vec![[0.0; 2]; 2000];
        combined.render_offline(&mut actual);
        let mut expected = vec![[0.0; 2]; 2000];
        for note in &notes {
            let mut single = chord.clone();
            single
                .project
                .edit(Edit::ReplaceMidiPattern {
                    clip_id: clip,
                    pattern: MidiPattern {
                        length_ticks: 3840,
                        notes: vec![note.clone()],
                    },
                })
                .unwrap();
            let mut renderer = Renderer::new(single);
            renderer.seek(start);
            let mut samples = vec![[0.0; 2]; 2000];
            renderer.render_offline(&mut samples);
            for (sum, sample) in expected.iter_mut().zip(samples) {
                for ch in 0..2 {
                    sum[ch] += sample[ch];
                }
            }
        }
        assert!(actual.iter().zip(expected).all(|(actual,expected)| (0..2).all(|ch| (actual[ch]-expected[ch]).abs()<1e-6)));
    }
}
