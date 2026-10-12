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
    project.tracks[0].instrument.as_mut().unwrap().oscillators[0].waveform = waveform;
    // Existing oscillator tests isolate source/mixer behavior using the legacy gate.
    project.tracks[0].instrument.as_mut().unwrap().envelope = EnvelopeSettings::legacy();
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
        plan.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .oscillators
            .extend([
                OscillatorSettings {
                    waveform: Waveform::Triangle,
                    transpose: -12,
                    gain_db: -6.0,
                    muted: true,
                    ..Default::default()
                },
                OscillatorSettings {
                    waveform: Waveform::Saw,
                    transpose: 12,
                    gain_db: -18.0,
                    ..Default::default()
                },
            ]);
        plan.project.transport.r#loop = Loop {
            enabled: true,
            start_frame: 0,
            end_frame: 48_000,
        };
        let mut renderer = Renderer::new(plan.clone());
        plan.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .oscillators[0]
            .gain_db = -3.0;
        plan.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .oscillators[1]
            .muted = false;
        plan.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .oscillators[0]
            .transpose = 12;
        plan.project.tracks[0].instrument.as_mut().unwrap().envelope = EnvelopeSettings::default();
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
fn adsr_playback_matches_export_and_block_sizes_at_all_project_rates() {
    for rate in SAMPLE_RATES {
        let mut plan = plan(Waveform::Sine, rate);
        let envelope = &mut plan.project.tracks[0].instrument.as_mut().unwrap().envelope;
        *envelope = EnvelopeSettings::default();
        envelope.attack.shape = CurveShape::Logarithmic;
        envelope.attack.time_ms = 35.0;
        envelope.decay.shape = CurveShape::SCurve;
        envelope.sustain = 0.4;
        envelope.release.time_ms = 300.0;
        let mut expected = vec![[0.0; 2]; rate as usize];
        Renderer::new(plan.clone()).render_offline(&mut expected);
        for size in BUFFER_SIZES {
            let mut renderer = Renderer::new(plan.clone());
            let mut actual = vec![[0.0; 2]; expected.len()];
            for block in actual.chunks_mut(size as usize) {
                renderer.render_offline(block);
            }
            assert_eq!(actual, expected);
        }
        let mut live = Renderer::new(plan);
        live.play();
        for (frame, expected) in expected.iter().enumerate() {
            let actual = live.next_sample();
            if frame >= (rate / 200) as usize {
                assert_eq!(actual, *expected);
            }
        }
    }
}

#[test]
fn live_envelope_edits_preserve_oscillator_phase_and_current_envelope_progress() {
    use daw_envelope::Envelope;
    use daw_oscillator::Oscillator;
    use daw_unit_core::{ProcessingContext, SourceUnit};
    let mut plan = plan(Waveform::Sine, 48_000);
    plan.project
        .assets
        .last_mut()
        .unwrap()
        .midi_pattern
        .as_mut()
        .unwrap()
        .notes
        .truncate(1);
    let mut settings = EnvelopeSettings::default();
    settings.attack.time_ms = 200.0;
    plan.project.tracks[0].instrument.as_mut().unwrap().envelope = settings;
    let context = ProcessingContext {
        sample_rate_hz: 48_000,
        max_block_frames: 512,
    };
    let mut source = Oscillator::new(Waveform::Sine, context);
    source.reset();
    source.set_frequency(440.0);
    let mut envelope = Envelope::new(settings, context);
    envelope.note_on();
    let mut live = Renderer::new(plan.clone());
    live.play();
    for _ in 0..3000 {
        live.next_sample();
        source.next_sample();
        envelope.next_sample();
    }
    let gain = linear_gain(-12.0);
    let pan = daw_engine::pan_coefficients(0.0, true);
    for stage in 0..4 {
        match stage {
            0 => {
                settings.attack.time_ms = 100.0;
                settings.attack.shape = CurveShape::Exponential;
            }
            1 => {
                settings.attack.amount = 0.9;
                settings.decay.shape = CurveShape::SCurve;
            }
            2 => {
                settings.sustain = 0.25;
            }
            _ => {
                settings.release.time_ms = 1500.0;
            }
        }
        plan.project.tracks[0].instrument.as_mut().unwrap().envelope = settings;
        let mut next = Renderer::new(plan.clone());
        next.inherit(&mut live);
        live = next;
        envelope.set_settings(settings);
        for _ in 0..2000 {
            let expected = source.next_sample() * envelope.next_sample() * gain;
            let actual = live.next_sample();
            for ch in 0..2 {
                assert!((actual[ch] - expected * pan[ch]).abs() < 2e-6);
            }
        }
    }
}

#[test]
fn overlapping_release_tails_have_independent_note_envelopes() {
    let mut combined = plan(Waveform::Sine, 48_000);
    combined.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .envelope = EnvelopeSettings::default();
    let clip = combined.project.tracks[0].clips[0].id;
    let notes: Vec<_> = [0, 120, 240]
        .into_iter()
        .enumerate()
        .map(|(index, start_tick)| MidiNote {
            id: Id::from_u128(index as u128 + 50),
            start_tick,
            length_ticks: 60,
            pitch: 60 + index as u8 * 4,
            velocity: 100,
        })
        .collect();
    combined
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: notes.clone(),
            },
        })
        .unwrap();
    let mut actual = vec![[0.0; 2]; 20_000];
    Renderer::new(combined.clone()).render_offline(&mut actual);
    let mut expected = vec![[0.0; 2]; actual.len()];
    for note in notes {
        let mut single = combined.clone();
        single
            .project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![note],
                },
            })
            .unwrap();
        let mut samples = vec![[0.0; 2]; expected.len()];
        Renderer::new(single).render_offline(&mut samples);
        for (sum, sample) in expected.iter_mut().zip(samples) {
            for ch in 0..2 {
                sum[ch] += sample[ch];
            }
        }
    }
    assert!(
        actual
            .iter()
            .zip(expected)
            .all(|(a, b)| (0..2).all(|ch| (a[ch] - b[ch]).abs() < 1e-6))
    );
    assert!(
        actual[10_000..12_000]
            .iter()
            .any(|sample| sample[0].abs() > 1e-4)
    );
    assert!(actual[18_000..].iter().all(|sample| *sample == [0.0; 2]));
}

#[test]
fn live_pitch_edits_follow_a_continuous_source_without_restarting_notes() {
    use daw_oscillator::Oscillator;
    use daw_unit_core::{ProcessingContext, SourceUnit};
    for rate in SAMPLE_RATES {
        for waveform in Waveform::ALL {
            let mut edited = plan(waveform, rate);
            let gain = linear_gain(-12.0);
            let pan = daw_engine::pan_coefficients(0.0, true);
            let mut source = Oscillator::new(
                waveform,
                ProcessingContext {
                    sample_rate_hz: rate,
                    max_block_frames: edited.project.buffer_size_frames,
                },
            );
            source.reset();
            source.set_frequency(440.0);
            let mut live = Renderer::new(edited.clone());
            live.play();
            for _ in 0..4096 {
                live.next_sample();
                source.next_sample();
            }
            let mut frequency = 440.0;
            for pitch in [12, 24, -12, -48, 48, 0] {
                edited.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .oscillators[0]
                    .transpose = pitch;
                let mut next = Renderer::new(edited.clone());
                next.inherit(&mut live);
                live = next;
                let target = 440.0 * 2.0_f32.powf(f32::from(pitch) / 12.0);
                // Retarget an unfinished ramp, as a fast knob drag does.
                let ramp_frames = rate / 200;
                let frames = if pitch == -48 || pitch == 48 || pitch == 0 {
                    ramp_frames + 128
                } else {
                    53
                };
                for frame in 0..frames {
                    if frame < ramp_frames {
                        frequency += (target - frequency) / (ramp_frames - frame) as f32;
                    }
                    source.set_frequency(frequency);
                    let expected = source.next_sample() * gain;
                    let actual = live.next_sample();
                    for ch in 0..2 {
                        assert!(
                            (actual[ch] - expected * pan[ch]).abs() < 2e-6,
                            "{rate} Hz, {waveform:?}, pitch {pitch}: {} != {}",
                            actual[ch],
                            expected * pan[ch]
                        );
                    }
                }
                if frames >= ramp_frames {
                    assert_eq!(frequency, target);
                }
            }
        }
    }
}

#[test]
fn repeated_source_gain_edits_preserve_pitch_phase_and_other_sources() {
    for rate in SAMPLE_RATES {
        for waveform in Waveform::ALL {
            let mut edited = plan(waveform, rate);
            edited.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .oscillators
                .push(OscillatorSettings {
                    waveform: Waveform::Triangle,
                    transpose: 12,
                    gain_db: -18.0,
                    ..Default::default()
                });
            let mut first = edited.clone();
            first.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .oscillators[1]
                .muted = true;
            let mut second = edited.clone();
            second.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .oscillators[0]
                .muted = true;
            let mut first = Renderer::new(first);
            let mut second = Renderer::new(second);
            let mut live = Renderer::new(edited.clone());
            live.play();
            first.play();
            second.play();
            for _ in 0..4096 {
                live.next_sample();
                first.next_sample();
                second.next_sample();
            }
            let initial_gain = linear_gain(-12.0);
            let mut gain = initial_gain;
            let mut target = gain;
            let mut remaining = 0;
            let ramp_frames = rate / 200;
            // Changes arrive before earlier ramps finish, then include silence and unmute.
            for (db, muted) in [
                (-6.0, false),
                (-3.0, false),
                (-9.0, false),
                (-18.0, false),
                (SILENT_GAIN_DB, false),
                (0.0, false),
                (0.0, true),
                (-12.0, false),
            ] {
                let source = &mut edited.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .oscillators[0];
                source.gain_db = db;
                source.muted = muted;
                let next_target = if muted { 0.0 } else { linear_gain(db) };
                if next_target != target {
                    target = next_target;
                    remaining = ramp_frames;
                }
                let mut next = Renderer::new(edited.clone());
                next.inherit(&mut live);
                live = next;
                let frames = if db == SILENT_GAIN_DB || muted {
                    ramp_frames + 53
                } else {
                    53
                };
                for _ in 0..frames {
                    if remaining > 0 {
                        gain += (target - gain) / remaining as f32;
                        remaining -= 1;
                    }
                    let a = first.next_sample();
                    let b = second.next_sample();
                    let actual = live.next_sample();
                    for ch in 0..2 {
                        let expected = a[ch] * (gain / initial_gain) + b[ch];
                        assert!(
                            (actual[ch] - expected).abs() < 2e-6,
                            "{rate} Hz, {waveform:?}, {db} dB, mute {muted}: {} != {expected}",
                            actual[ch]
                        );
                    }
                }
            }
            for _ in 0..ramp_frames {
                if remaining > 0 {
                    gain += (target - gain) / remaining as f32;
                    remaining -= 1;
                }
                let actual = live.next_sample();
                let a = first.next_sample();
                let b = second.next_sample();
                for ch in 0..2 {
                    assert!((actual[ch] - a[ch] * (gain / initial_gain) - b[ch]).abs() < 2e-6);
                }
            }
            assert_eq!(gain, initial_gain);
        }
    }
}

#[test]
fn multiple_oscillators_sum_independent_sources_for_each_note_and_empty_is_silent() {
    let mut combined = plan(Waveform::Sine, 48_000);
    let settings = vec![
        OscillatorSettings::default(),
        OscillatorSettings {
            waveform: Waveform::Triangle,
            transpose: -12,
            gain_db: -6.0,
            ..Default::default()
        },
        OscillatorSettings {
            waveform: Waveform::Saw,
            transpose: 12,
            gain_db: -18.0,
            ..Default::default()
        },
    ];
    combined.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .oscillators = settings.clone();
    for start in [0, 18_000] {
        let mut expected = vec![[0.0; 2]; 36_000];
        for oscillator in &settings {
            let mut single = combined.clone();
            single.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .oscillators = vec![oscillator.clone()];
            let mut renderer = Renderer::new(single);
            renderer.seek(start);
            let mut samples = vec![[0.0; 2]; expected.len()];
            renderer.render_offline(&mut samples);
            for (sum, sample) in expected.iter_mut().zip(samples) {
                for ch in 0..2 {
                    sum[ch] += sample[ch];
                }
            }
        }
        let mut renderer = Renderer::new(combined.clone());
        renderer.seek(start);
        let mut actual = vec![[0.0; 2]; expected.len()];
        for block in actual.chunks_mut(128) {
            renderer.render_offline(block);
        }
        assert!(
            actual
                .iter()
                .zip(&expected)
                .all(|(a, e)| (0..2).all(|ch| (a[ch] - e[ch]).abs() < 1e-6))
        );
        let mut live = Renderer::new(combined.clone());
        live.seek(start);
        live.play();
        for (index, expected) in actual.iter().enumerate() {
            let sample = live.next_sample();
            if index >= 240 {
                assert_eq!(sample, *expected);
            }
        }
    }
    combined.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .oscillators
        .clear();
    let mut samples = vec![[1.0; 2]; 48_000];
    Renderer::new(combined).render_offline(&mut samples);
    assert!(samples.iter().all(|s| *s == [0.0; 2]));
}

#[test]
fn oscillator_mute_silences_only_its_source_and_unmute_restores_saved_gain() {
    let mut original = plan(Waveform::Sine, 48_000);
    let active = original.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .oscillators[0]
        .clone();
    original.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .oscillators
        .extend([
            OscillatorSettings {
                waveform: Waveform::Saw,
                gain_db: 6.0,
                muted: true,
                ..Default::default()
            },
            OscillatorSettings {
                waveform: Waveform::Square,
                transpose: -12,
                gain_db: 0.0,
                muted: true,
                ..Default::default()
            },
        ]);
    let mut reference = original.clone();
    reference.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .oscillators = vec![active];
    let mut expected = vec![[0.0; 2]; 48_000];
    Renderer::new(reference).render_offline(&mut expected);
    let mut actual = vec![[0.0; 2]; expected.len()];
    Renderer::new(original.clone()).render_offline(&mut actual);
    assert_eq!(actual, expected);
    let mut live = Renderer::new(original.clone());
    live.play();
    for (index, expected) in expected.iter().enumerate() {
        let sample = live.next_sample();
        if index >= 240 {
            assert_eq!(sample, *expected);
        }
    }
    let instrument = original.project.tracks[0].instrument.as_mut().unwrap();
    instrument.oscillators[0].muted = true;
    Renderer::new(original.clone()).render_offline(&mut actual);
    assert!(actual.iter().all(|s| *s == [0.0; 2]));
    let instrument = original.project.tracks[0].instrument.as_mut().unwrap();
    instrument.oscillators[0].muted = false;
    assert_eq!(instrument.oscillators[1].gain_db, 6.0);
    Renderer::new(original).render_offline(&mut actual);
    assert_eq!(actual, expected);
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
