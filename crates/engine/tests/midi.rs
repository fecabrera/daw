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
    project.tracks[0].instrument = Some(OscillatorSettings::default().into());
    project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[0].waveform = waveform;
    // Existing oscillator tests isolate source/mixer behavior using the legacy gate.
    project.tracks[0].instrument.as_mut().unwrap().layers[0].envelope = EnvelopeSettings::legacy();
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
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0]
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
        let mut second = InstrumentLayerSettings::default();
        second.oscillators.push(OscillatorSettings::default());
        second.has_envelope = true;
        plan.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers
            .push(second);
        let mut renderer = Renderer::new(plan.clone());
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[0].gain_db = -3.0;
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[1].muted = false;
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[0].transpose = 12;
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelope =
            EnvelopeSettings::default();
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[1].oscillators[0].gain_db =
            -18.0;
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[1]
            .envelope
            .release
            .time_ms = 400.0;
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].gain_db = -9.0;
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].pan = -0.6;
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[1].soloed = true;
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
fn instrument_layers_sum_independent_envelopes_and_sources_and_empty_tracks_are_silent() {
    let mut combined = plan(Waveform::Sine, 48_000);
    let instrument = combined.project.tracks[0].instrument.as_mut().unwrap();
    instrument.layers[0].envelope.release.time_ms = 250.0;
    let mut second = InstrumentLayerSettings::default();
    second.oscillators.push(OscillatorSettings {
        waveform: Waveform::Triangle,
        transpose: -12,
        gain_db: -18.0,
        ..Default::default()
    });
    second.has_envelope = true;
    second.envelope.attack.time_ms = 35.0;
    second.envelope.sustain = 0.35;
    second.envelope.release.time_ms = 900.0;
    instrument.layers.push(second);
    instrument.layers.push(InstrumentLayerSettings::default());
    for start in [0, 18_000, 49_000] {
        let mut expected = vec![[0.0; 2]; 48_000];
        for layer in &combined.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers
        {
            let mut single = combined.clone();
            single.project.tracks[0].instrument.as_mut().unwrap().layers = vec![layer.clone()];
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
        for block in actual.chunks_mut(127) {
            renderer.render_offline(block);
        }
        assert!(
            actual
                .iter()
                .zip(expected)
                .all(|(a, e)| (0..2).all(|ch| (a[ch] - e[ch]).abs() < 1e-6))
        );
        assert!(actual.iter().any(|sample| sample[0].abs() > 1e-5));
    }
    combined.project.tracks[0].instrument = Some(InstrumentSettings::default());
    let mut samples = vec![[1.0; 2]; 96_000];
    Renderer::new(combined).render_offline(&mut samples);
    assert!(samples.iter().all(|s| *s == [0.0; 2]));
}

#[test]
fn adsr_playback_matches_export_and_block_sizes_at_all_project_rates() {
    for rate in SAMPLE_RATES {
        let mut plan = plan(Waveform::Sine, rate);
        let envelope = &mut plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelope;
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
    plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelope = settings;
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
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelope = settings;
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
        .layers[0]
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
                edited.project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[0]
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
            edited.project.tracks[0].instrument.as_mut().unwrap().layers[0]
                .oscillators
                .push(OscillatorSettings {
                    waveform: Waveform::Triangle,
                    transpose: 12,
                    gain_db: -18.0,
                    ..Default::default()
                });
            let mut first = edited.clone();
            first.project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[1].muted =
                true;
            let mut second = edited.clone();
            second.project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators[0].muted =
                true;
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
                let source = &mut edited.project.tracks[0].instrument.as_mut().unwrap().layers[0]
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
        .layers[0]
        .oscillators = settings.clone();
    for start in [0, 18_000] {
        let mut expected = vec![[0.0; 2]; 36_000];
        for oscillator in &settings {
            let mut single = combined.clone();
            single.project.tracks[0].instrument.as_mut().unwrap().layers[0].oscillators =
                vec![oscillator.clone()];
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
        .layers[0]
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
        .layers[0]
        .oscillators[0]
        .clone();
    original.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
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
        .layers[0]
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
    instrument.layers[0].oscillators[0].muted = true;
    Renderer::new(original.clone()).render_offline(&mut actual);
    assert!(actual.iter().all(|s| *s == [0.0; 2]));
    let instrument = original.project.tracks[0].instrument.as_mut().unwrap();
    instrument.layers[0].oscillators[0].muted = false;
    assert_eq!(instrument.layers[0].oscillators[1].gain_db, 6.0);
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

#[test]
fn layer_renaming_reordering_and_collapse_preserve_live_voices_and_offline_audio() {
    let mut original = plan(Waveform::Sine, 48_000);
    let instrument = original.project.tracks[0].instrument.as_mut().unwrap();
    let mut layer = daw_core::InstrumentLayerSettings {
        name: "Bass".into(),
        ..Default::default()
    };
    layer.oscillators.push(OscillatorSettings {
        waveform: Waveform::Triangle,
        transpose: -12,
        gain_db: -18.0,
        ..Default::default()
    });
    layer.has_envelope = true;
    layer.envelope.attack.time_ms = 500.0;
    instrument.layers.push(layer);
    let mut live = Renderer::new(original.clone());
    let mut reference = Renderer::new(original.clone());
    live.play();
    reference.play();
    for _ in 0..4000 {
        assert_eq!(live.next_sample(), reference.next_sample());
    }
    let mut edited = original.clone();
    let layers = &mut edited.project.tracks[0].instrument.as_mut().unwrap().layers;
    layers[0].name = "Renamed lead".into();
    layers[0].collapsed = true;
    layers[1].collapsed = true;
    layers[0].oscillator_collapsed = true;
    layers[0].envelope_collapsed = true;
    layers[1].envelope_collapsed = true;
    layers.reverse();
    let mut replacement = Renderer::new(edited.clone());
    replacement.inherit(&mut live);
    for _ in 0..4000 {
        assert_eq!(replacement.next_sample(), reference.next_sample());
    }
    let mut original_audio = vec![[0.0; 2]; 12000];
    let mut edited_audio = original_audio.clone();
    Renderer::new(original).render_offline(&mut original_audio);
    Renderer::new(edited).render_offline(&mut edited_audio);
    assert_eq!(original_audio, edited_audio);
}

#[test]
fn layer_mixing_controls_route_stereo_and_share_track_solo_precedence() {
    let original = plan(Waveform::Sine, 48_000);
    let mut centered = vec![[0.0; 2]; 8000];
    Renderer::new(original.clone()).render_offline(&mut centered);
    for (muted, soloed, gain_db, pan) in [
        (false, false, 0.0, 0.0),
        (false, false, -6.0, -1.0),
        (false, false, 6.0, 1.0),
        (true, false, 0.0, 0.0),
        (true, true, -3.0, -0.5),
        (false, false, SILENT_GAIN_DB, 0.0),
    ] {
        let mut changed = original.clone();
        let layer = &mut changed.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        layer.muted = muted;
        layer.soloed = soloed;
        layer.gain_db = gain_db;
        layer.pan = pan;
        let mut actual = centered.clone();
        Renderer::new(changed.clone()).render_offline(&mut actual);
        let pan = daw_engine::pan_coefficients(pan, true);
        let center = daw_engine::pan_coefficients(0.0, true);
        for (value, source) in actual.iter().zip(&centered) {
            for ch in 0..2 {
                let expected = if mix_audible(muted, soloed, soloed) {
                    source[ch] * linear_gain(gain_db) * pan[ch] / center[ch]
                } else {
                    0.0
                };
                assert!((value[ch] - expected).abs() < 1e-6);
            }
        }
        let mut live = Renderer::new(changed);
        live.play();
        for (index, expected) in actual.iter().enumerate() {
            let live = live.next_sample();
            let ramp = ((index + 1) as f32 / 240.0).min(1.0);
            assert!((0..2).all(|ch| (live[ch] - expected[ch] * ramp).abs() < 1e-5));
        }
    }
    // Solo excludes peer layers in this instrument, without soloing another track.
    let mut combined = original.clone();
    let instrument = combined.project.tracks[0].instrument.as_mut().unwrap();
    let mut peer = instrument.layers[0].clone();
    peer.id = Id::new_v4();
    peer.oscillators[0].waveform = Waveform::Triangle;
    peer.gain_db = -6.0;
    peer.pan = 1.0;
    instrument.layers[0].soloed = true;
    instrument.layers.push(peer);
    let mut actual = centered.clone();
    Renderer::new(combined.clone()).render_offline(&mut actual);
    assert_eq!(actual, centered);
    combined.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[1]
        .soloed = true;
    let mut single_peer = combined.clone();
    single_peer.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers
        .remove(0);
    let mut expected_peer = centered.clone();
    Renderer::new(single_peer).render_offline(&mut expected_peer);
    Renderer::new(combined).render_offline(&mut actual);
    assert!(actual.iter().zip(centered.iter().zip(expected_peer)).all(
        |(actual, (first, second))| {
            (0..2).all(|ch| (actual[ch] - first[ch] - second[ch]).abs() < 1e-6)
        }
    ));
}

#[test]
fn live_layer_gain_pan_mute_and_solo_edits_preserve_voices_and_ramp_without_allocations() {
    let original = plan(Waveform::Sine, 48_000);
    let mut current = Renderer::new(original.clone());
    current.play();
    for _ in 0..4000 {
        current.next_sample();
    }
    for (gain, pan, muted, soloed) in [
        (-6.0, -0.5, false, false),
        (0.0, 1.0, false, true),
        (0.0, 1.0, true, false),
        (0.0, 0.0, false, false),
    ] {
        let mut changed = original.clone();
        let layer = &mut changed.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        layer.gain_db = gain;
        layer.pan = pan;
        layer.muted = muted;
        layer.soloed = soloed;
        let coefficients = |layer: &InstrumentLayerSettings| {
            let gain = if mix_audible(layer.muted, layer.soloed, layer.soloed) {
                linear_gain(layer.gain_db)
            } else {
                0.0
            };
            let pan = daw_engine::pan_coefficients(layer.pan, true);
            let center = daw_engine::pan_coefficients(0.0, true);
            std::array::from_fn::<_, 2, _>(|ch| gain * pan[ch] / center[ch])
        };
        let old_gain = coefficients(
            &current.plan.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers[0],
        );
        let new_gain = coefficients(layer);
        let mut neutral = Renderer::new(original.clone());
        neutral.play();
        for _ in 0..current.playhead {
            neutral.next_sample();
        }
        let mut replacement = Renderer::new(changed.clone());
        let mut reference = Renderer::new(changed);
        reference.play();
        for _ in 0..current.playhead {
            reference.next_sample();
        }
        ALLOCATIONS.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
        replacement.inherit(&mut current);
        for step in 1..=240 {
            let actual = replacement.next_sample();
            let source = neutral.next_sample();
            reference.next_sample();
            let progress = step as f32 / 240.0;
            assert!(
                (0..2).all(|ch| {
                    let gain = old_gain[ch] + (new_gain[ch] - old_gain[ch]) * progress;
                    (actual[ch] - source[ch] * gain).abs() < 1e-6
                }),
                "Active layer controls must ramp over 5 ms without resetting the source"
            );
        }
        for _ in 0..3000 {
            let actual = replacement.next_sample();
            let expected = reference.next_sample();
            assert!((0..2).all(|ch| (actual[ch] - expected[ch]).abs() < 1e-6));
        }
        TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        current = replacement;
    }
}

#[test]
fn unit_enable_states_silence_sources_bypass_adsr_and_match_playback_export() {
    for rate in [44_100, 48_000] {
        let gate = plan(Waveform::Sine, rate);
        let mut wet = gate.clone();
        wet.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelope = EnvelopeSettings {
            attack: EnvelopeStageSettings {
                time_ms: 200.0,
                ..EnvelopeSettings::default().attack
            },
            sustain: 0.25,
            release: EnvelopeStageSettings {
                time_ms: 700.0,
                ..EnvelopeSettings::default().release
            },
            ..Default::default()
        };
        let count = rate as usize;
        let mut gate_audio = vec![[0.0; 2]; count];
        Renderer::new(gate.clone()).render_offline(&mut gate_audio);
        for (oscillator_enabled, envelope_enabled) in
            [(false, true), (false, false), (true, false), (true, true)]
        {
            let mut changed = wet.clone();
            let layer = &mut changed.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0];
            layer.oscillator_enabled = oscillator_enabled;
            layer.envelope_enabled = envelope_enabled;
            let mut audio = vec![[0.0; 2]; count];
            Renderer::new(changed.clone()).render_offline(&mut audio);
            if !oscillator_enabled {
                assert!(audio.iter().all(|frame| *frame == [0.0; 2]));
            } else if !envelope_enabled {
                assert_eq!(audio, gate_audio, "Bypass uses the legacy note gate");
            } else {
                assert!(
                    audio
                        .iter()
                        .zip(&gate_audio)
                        .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
                );
            }
            let mut live = Renderer::new(changed);
            live.play();
            for (frame, expected) in audio.into_iter().enumerate() {
                let actual = live.next_sample();
                if frame >= rate as usize / 200 {
                    assert_eq!(actual, expected);
                }
            }
        }
        // Disabling one source unit must not silence another layer.
        let mut combined = gate.clone();
        let mut peer = combined.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .clone();
        peer.id = Id::new_v4();
        let layers = &mut combined.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers;
        layers[0].oscillator_enabled = false;
        layers.push(peer);
        let mut audio = vec![[0.0; 2]; count];
        Renderer::new(combined).render_offline(&mut audio);
        assert_eq!(audio, gate_audio);
    }
}

#[test]
fn live_unit_toggles_ramp_without_resetting_source_or_adsr_and_do_not_allocate() {
    let gate = plan(Waveform::Sine, 48_000);
    let mut wet = gate.clone();
    wet.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelope = EnvelopeSettings {
        attack: EnvelopeStageSettings {
            time_ms: 200.0,
            ..EnvelopeSettings::default().attack
        },
        sustain: 0.4,
        ..Default::default()
    };
    let mut current = Renderer::new(wet.clone());
    let mut wet_reference = Renderer::new(wet.clone());
    let mut gate_reference = Renderer::new(gate);
    current.play();
    wet_reference.play();
    gate_reference.play();
    for _ in 0..4000 {
        assert_eq!(current.next_sample(), wet_reference.next_sample());
        gate_reference.next_sample();
    }
    let mut gain = 1.0;
    let mut mix = 1.0;
    for (oscillator_enabled, envelope_enabled) in [
        (true, false),
        (false, false),
        (true, false),
        (true, true),
        (false, true),
        (true, true),
    ] {
        let mut changed = wet.clone();
        let layer = &mut changed.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers[0];
        layer.oscillator_enabled = oscillator_enabled;
        layer.envelope_enabled = envelope_enabled;
        let next_gain = f32::from(oscillator_enabled);
        let next_mix = f32::from(envelope_enabled);
        let mut replacement = Renderer::new(changed);
        ALLOCATIONS.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
        replacement.inherit(&mut current);
        for step in 1..=740 {
            let actual = replacement.next_sample();
            let wet_sample = wet_reference.next_sample();
            let gate_sample = gate_reference.next_sample();
            let progress = (step as f32 / 240.0).min(1.0);
            let gain = gain + (next_gain - gain) * progress;
            let mix = mix + (next_mix - mix) * progress;
            for ch in 0..2 {
                let expected = (gate_sample[ch] + (wet_sample[ch] - gate_sample[ch]) * mix) * gain;
                assert!(
                    (actual[ch] - expected).abs() < 1e-6,
                    "Unit toggle must retain phase and ADSR progress and ramp over 5 ms"
                );
            }
        }
        TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        gain = next_gain;
        mix = next_mix;
        current = replacement;
    }
}

#[test]
fn midi_track_eq_uses_the_aggregated_instrument_bus_in_playback_and_export() {
    for rate in SAMPLE_RATES {
        let mut source = plan(Waveform::Sine, rate);
        let mut dry = vec![[0.0; 2]; 4096];
        Renderer::new(source.clone()).render_offline(&mut dry);
        let mut effect = EffectInstance::equalizer();
        let EffectSettings::Eq3(settings) = &mut effect.unit else {
            unreachable!()
        };
        settings.bands[1].frequency_hz = 440.0;
        settings.bands[1].gain_db = 12.0;
        source.project.tracks[0].effects.push(effect.clone());
        let mut wet = vec![[0.0; 2]; 4096];
        Renderer::new(source.clone()).render_offline(&mut wet);
        let power = |samples: &[[f32; 2]]| {
            samples[2000..]
                .iter()
                .map(|sample| sample[0] * sample[0])
                .sum::<f32>()
        };
        assert!(power(&wet) > power(&dry) * 4.0);
        let mut live = Renderer::new(source.clone());
        live.play();
        for (frame, sample) in wet.iter().enumerate() {
            let output = live.next_sample();
            if frame > rate as usize / 200 {
                for ch in 0..2 {
                    assert!((output[ch] - sample[ch]).abs() < 1e-6);
                }
            }
        }
        let EffectSettings::Eq3(settings) = &mut source.project.tracks[0].effects[0].unit else {
            unreachable!()
        };
        settings.enabled = false;
        let mut bypass = vec![[0.0; 2]; 4096];
        Renderer::new(source.clone()).render_offline(&mut bypass);
        assert_eq!(bypass, dry);
        let EffectSettings::Eq3(settings) = &mut source.project.tracks[0].effects[0].unit else {
            unreachable!()
        };
        settings.enabled = true;
        settings.bands[1].enabled = false;
        let mut cut = vec![[0.0; 2]; 4096];
        Renderer::new(source).render_offline(&mut cut);
        assert!(power(&cut) < power(&dry) * 0.5);
    }
}

#[test]
fn midi_track_drive_processes_the_instrument_bus_in_playback_and_export() {
    for rate in SAMPLE_RATES {
        let mut source = plan(Waveform::Sine, rate);
        let mut dry = vec![[0.0; 2]; 4096];
        Renderer::new(source.clone()).render_offline(&mut dry);
        let mut effect = EffectInstance::drive();
        let EffectSettings::Drive(settings) = &mut effect.unit else {
            unreachable!()
        };
        settings.gain = 0.8;
        settings.color_shape = DriveColorShape::MidHump;
        source.project.tracks[0].effects.push(effect);
        let mut wet = vec![[0.0; 2]; 4096];
        Renderer::new(source.clone()).render_offline(&mut wet);
        assert!(
            wet.iter()
                .zip(&dry)
                .any(|(wet, dry)| (wet[0] - dry[0]).abs() > 0.01)
        );
        let mut live = Renderer::new(source.clone());
        live.play();
        for (frame, sample) in wet.iter().enumerate() {
            let output = live.next_sample();
            if frame > rate as usize / 200 {
                for channel in 0..2 {
                    assert!((output[channel] - sample[channel]).abs() < 1e-6);
                }
            }
        }
        let EffectSettings::Drive(settings) = &mut source.project.tracks[0].effects[0].unit else {
            unreachable!()
        };
        settings.enabled = false;
        let mut bypass = vec![[0.0; 2]; 4096];
        Renderer::new(source).render_offline(&mut bypass);
        assert_eq!(bypass, dry);
    }
}
