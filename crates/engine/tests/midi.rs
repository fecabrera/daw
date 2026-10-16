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
    project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].settings =
        EnvelopeSettings::legacy();
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
        second.install_amplitude_envelope();
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
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].settings =
            EnvelopeSettings::default();
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[1].oscillators[0].gain_db =
            -18.0;
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[1].envelopes[0]
            .settings
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
    instrument.layers[0].envelopes[0].settings.release.time_ms = 250.0;
    let mut second = InstrumentLayerSettings::default();
    second.oscillators.push(OscillatorSettings {
        waveform: Waveform::Triangle,
        transpose: -12,
        gain_db: -18.0,
        ..Default::default()
    });
    second.install_amplitude_envelope();
    second.envelopes[0].settings.attack.time_ms = 35.0;
    second.envelopes[0].settings.sustain = 0.35;
    second.envelopes[0].settings.release.time_ms = 900.0;
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
        let envelope = &mut plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes
            [0]
        .settings;
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
    plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].settings = settings;
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
        plan.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].settings =
            settings;
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
        .envelopes[0]
        .settings = EnvelopeSettings::default();
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
    layer.install_amplitude_envelope();
    layer.envelopes[0].settings.attack.time_ms = 500.0;
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
    layers[0].envelopes[0].collapsed = true;
    layers[0].amplitude.modulation_open = true;
    layers[1].envelopes[0].collapsed = true;
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
        wet.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].settings =
            EnvelopeSettings {
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
            layer.envelopes[0].enabled = envelope_enabled;
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
    wet.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0].settings =
        EnvelopeSettings {
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
        layer.envelopes[0].enabled = envelope_enabled;
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

#[test]
fn midi_track_auto_filter_processes_the_instrument_bus_in_playback_and_export() {
    for rate in SAMPLE_RATES {
        let mut source = plan(Waveform::Sine, rate);
        let mut dry = vec![[0.0; 2]; 4096];
        Renderer::new(source.clone()).render_offline(&mut dry);
        let mut effect = EffectInstance::auto_filter();
        let EffectSettings::AutoFilter(settings) = &mut effect.unit else {
            unreachable!()
        };
        settings.frequency_hz = 100.0;
        settings.stereo_phase_degrees = -90.0;
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
        let EffectSettings::AutoFilter(settings) = &mut source.project.tracks[0].effects[0].unit
        else {
            unreachable!()
        };
        settings.enabled = false;
        let mut bypass = vec![[0.0; 2]; 4096];
        Renderer::new(source).render_offline(&mut bypass);
        assert_eq!(bypass, dry);
    }
}

#[test]
fn midi_compressor_processes_the_instrument_bus_and_matches_export() {
    for rate in SAMPLE_RATES {
        let dry = plan(Waveform::Sine, rate);
        let mut compressed = dry.clone();
        let mut effect = EffectInstance::compressor();
        let EffectSettings::Compressor(settings) = &mut effect.unit else {
            unreachable!()
        };
        settings.threshold_db = -36.0;
        settings.attack_ms = 0.1;
        settings.detection = CompressorDetection::Rms;
        compressed.project.tracks[0].effects.push(effect);
        let frames = rate as usize / 5;
        let mut expected = vec![[0.0; 2]; frames];
        Renderer::new(compressed.clone()).render_offline(&mut expected);
        let mut untouched = vec![[0.0; 2]; frames];
        Renderer::new(dry).render_offline(&mut untouched);
        assert!(
            expected
                .iter()
                .zip(untouched)
                .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
        );
        let mut live = Renderer::new(compressed);
        live.play();
        for (frame, expected) in expected.iter().enumerate() {
            let actual = live.next_sample();
            if frame >= rate as usize / 200 {
                for ch in 0..2 {
                    assert!((actual[ch] - expected[ch]).abs() < 1e-6);
                }
            }
        }
    }
}

#[test]
fn limiter_alignment_preserves_midi_notes_and_loop_timing() {
    for rate in [44_100, 96_000] {
        let mut dry = plan(Waveform::Sine, rate);
        let mut wet = dry.clone();
        let mut effect = EffectInstance::limiter();
        let EffectSettings::Limiter(s) = &mut effect.unit else {
            unreachable!()
        };
        s.enabled = false;
        wet.project.tracks[0].effects.push(effect);
        let mut expected = vec![[0.0; 2]; 5000];
        let mut actual = expected.clone();
        Renderer::new(dry.clone()).render_offline(&mut expected);
        Renderer::new(wet.clone()).render_offline(&mut actual);
        assert_eq!(actual, expected);
        for p in [&mut dry.project, &mut wet.project] {
            p.transport.r#loop.enabled = true;
            p.transport.r#loop.start_frame = 0;
            p.transport.r#loop.end_frame = 500;
        }
        let mut a = Renderer::new(dry);
        let mut b = Renderer::new(wet);
        a.play();
        b.play();
        a.ramp = 1.0;
        b.ramp = 1.0;
        for i in 0..2000 {
            assert_eq!(a.next_sample(), b.next_sample(), "{rate}: {i}");
        }
    }
}

#[test]
fn adding_a_zero_latency_track_keeps_the_existing_live_pitch_phase() {
    let initial = plan(Waveform::Sine, 48_000);
    let mut changed = initial.clone();
    changed.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0]
        .oscillators[0]
        .transpose = 7;
    let prepare = || {
        let mut old = Renderer::new(initial.clone());
        old.play();
        old.ramp = 1.0;
        for _ in 0..4000 {
            old.next_sample();
        }
        let mut next = Renderer::new(changed.clone());
        next.inherit(&mut old);
        for _ in 0..1000 {
            next.next_sample();
        }
        next
    };
    let mut current = prepare();
    let mut reference = prepare();
    changed.project.add_track().unwrap();
    let mut added = Renderer::new(changed);
    added.inherit(&mut current);
    for i in 0..1000 {
        let actual = added.next_sample();
        let expected = reference.next_sample();
        if i > 250 {
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn unconnected_generic_envelopes_preserve_audio_and_live_voices_when_added_or_edited() {
    let original = plan(Waveform::Triangle, 48_000);
    let mut live = Renderer::new(original.clone());
    let mut reference = Renderer::new(original.clone());
    live.play();
    reference.play();
    for _ in 0..4000 {
        assert_eq!(live.next_sample(), reference.next_sample());
    }
    let mut edited = original.clone();
    let layer = &mut edited.project.tracks[0].instrument.as_mut().unwrap().layers[0];
    layer.envelopes[1].present = true;
    layer.envelopes[1].settings.attack.time_ms = 432.0;
    layer.envelopes[1].settings.sustain = 0.23;
    layer.envelopes[1].settings.release.time_ms = 777.0;
    layer.envelopes[1].enabled = true;
    layer.envelopes[1].collapsed = true;
    layer.envelopes.push(EnvelopeInstance::default());
    let mut replacement = Renderer::new(edited.clone());
    replacement.inherit(&mut live);
    ALLOCATIONS.with(|count| count.set(0));
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
    for _ in 0..4000 {
        assert_eq!(replacement.next_sample(), reference.next_sample());
    }
    TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
    ALLOCATIONS.with(|count| assert_eq!(count.get(), 0));
    let mut before = vec![[0.0; 2]; 12000];
    let mut after = before.clone();
    Renderer::new(original).render_offline(&mut before);
    Renderer::new(edited).render_offline(&mut after);
    assert_eq!(before, after);
}

#[test]
fn shared_envelope_modulates_auto_filter_with_playback_export_and_live_edit_parity() {
    for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
        let mut source = plan(Waveform::Saw, 48_000);
        let l = &mut source.project.tracks[0].instrument.as_mut().unwrap().layers[0];
        l.envelopes[1].present = true;
        l.envelopes[1].mode = mode;
        l.envelopes[1].settings.attack.time_ms = 400.0;
        let id = ModulationSourceId {
            layer: l.id,
            unit: l.envelopes[1].id,
        };
        let mut effect = EffectInstance::auto_filter();
        let EffectSettings::AutoFilter(f) = &mut effect.unit else {
            panic!()
        };
        f.lfo_enabled = false;
        f.frequency_hz = 100.0;
        let mut route = Modulation::new(ModulationParameter::Frequency);
        route.source = Some(id);
        route.amount = 5.0;
        f.modulations.push(route);
        source.project.tracks[0].effects.push(effect);
        source.project.validate().unwrap();
        let mut wet = vec![[0.0; 2]; 32000];
        Renderer::new(source.clone()).render_offline(&mut wet);
        let mut live = Renderer::new(source.clone());
        live.play();
        ALLOCATIONS.with(|c| c.set(0));
        TRACK_ALLOCATIONS.with(|e| e.set(true));
        for (i, s) in wet.iter().enumerate() {
            let actual = live.next_sample();
            if i > 240 {
                assert!((actual[0] - s[0]).abs() < 1e-6);
            }
        }
        TRACK_ALLOCATIONS.with(|e| e.set(false));
        ALLOCATIONS.with(|c| assert_eq!(c.get(), 0));
        let mut random = [[0.0; 2]; 64];
        source.render(13000, &mut random);
        assert_eq!(&random[..], &wet[13000..13064]);
        let mut dry = source.clone();
        let EffectSettings::AutoFilter(f) = &mut dry.project.tracks[0].effects[0].unit else {
            panic!()
        };
        f.modulations.clear();
        let mut unmodulated = vec![[0.0; 2]; 32000];
        Renderer::new(dry).render_offline(&mut unmodulated);
        assert!(
            wet.iter()
                .zip(&unmodulated)
                .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
        );
        let mut edited = source.clone();
        edited.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[1]
            .settings
            .attack
            .time_ms = 100.0;
        let EffectSettings::AutoFilter(f) = &mut edited.project.tracks[0].effects[0].unit else {
            panic!()
        };
        f.modulations[0].amount = 2.0;
        let mut replacement = Renderer::new(edited);
        ALLOCATIONS.with(|c| c.set(0));
        TRACK_ALLOCATIONS.with(|e| e.set(true));
        replacement.inherit(&mut live);
        for _ in 0..512 {
            assert!(replacement.next_sample().iter().all(|s| s.is_finite()));
        }
        TRACK_ALLOCATIONS.with(|e| e.set(false));
        ALLOCATIONS.with(|c| assert_eq!(c.get(), 0));
    }
}

#[test]
fn shared_amplitude_releases_the_last_notes_without_reviving_earlier_notes() {
    for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
        let mut source = plan(Waveform::Sine, 48_000);
        let clip = source.project.tracks[0].clips[0].id;
        let first = MidiNote {
            id: Id::from_u128(71),
            start_tick: 0,
            length_ticks: 96,
            pitch: 48,
            velocity: 127,
        };
        let second = MidiNote {
            id: Id::from_u128(72),
            start_tick: 48,
            length_ticks: 96,
            pitch: 72,
            velocity: 127,
        };
        source
            .project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![first, second.clone()],
                },
            })
            .unwrap();
        let e = &mut source.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0];
        e.mode = mode;
        e.settings.attack.time_ms = 0.0;
        e.settings.decay.time_ms = 0.0;
        e.settings.sustain = 1.0;
        e.settings.release.time_ms = 500.0;
        let end = source.project.midi_frame(144) as usize;
        let mut both = vec![[0.0; 2]; end + 25_000];
        Renderer::new(source.clone()).render_offline(&mut both);
        let mut only_last = source.clone();
        only_last
            .project
            .edit(Edit::ReplaceMidiPattern {
                clip_id: clip,
                pattern: MidiPattern {
                    length_ticks: 3840,
                    notes: vec![second],
                },
            })
            .unwrap();
        let mut reference = vec![[0.0; 2]; both.len()];
        Renderer::new(only_last).render_offline(&mut reference);
        assert!(
            both[end + 12_000..end + 16_000]
                .iter()
                .any(|s| s[0].abs() > 0.01),
            "Shared release must outlive the legacy 5 ms gate"
        );
        for (a, b) in both[end + 500..].iter().zip(&reference[end + 500..]) {
            assert!(
                (a[0] - b[0]).abs() < 1e-6,
                "An earlier note must not restart during the shared release"
            );
        }
        assert!(both[end + 24_100..].iter().all(|s| s[0].abs() < 1e-6));
        for amount in [0.0, 0.25, 0.5] {
            let mut partial = source.clone();
            partial.project.tracks[0]
                .instrument
                .as_mut()
                .unwrap()
                .layers[0]
                .amplitude
                .modulations[0]
                .amount = amount;
            let mut audio = vec![[0.0; 2]; both.len()];
            Renderer::new(partial).render_offline(&mut audio);
            for (actual, full) in audio[end + 500..].iter().zip(&both[end + 500..]) {
                assert!(
                    (actual[0] - full[0] * amount).abs() < 1e-6,
                    "Partial-depth shared release must fade to zero without a step"
                );
            }
        }
    }
}

#[test]
fn modulation_telemetry_tracks_processed_note_values_and_preserves_audio_without_allocations() {
    use daw_engine::ModulationTarget;
    let mut plan = plan(Waveform::Saw, 48_000);
    let track = plan.project.tracks[0].id;
    let instrument = plan.project.tracks[0].instrument.as_mut().unwrap();
    let layer = &mut instrument.layers[0];
    layer.install_amplitude_envelope();
    let envelope = &mut layer.envelopes[0];
    envelope.mode = EnvelopeMode::Legato;
    envelope.settings.attack = EnvelopeStageSettings {
        time_ms: 100.0,
        shape: CurveShape::Linear,
        amount: 0.5,
    };
    envelope.settings.decay.time_ms = 0.0;
    envelope.settings.sustain = 1.0;
    let source = ModulationSourceId {
        layer: layer.id,
        unit: envelope.id,
    };
    layer.amplitude.modulations[0].source = Some(source);
    let amplitude = ModulationTarget::Amplitude {
        track,
        layer: layer.id,
    };
    let mut effect = EffectInstance::auto_filter();
    let filter = ModulationTarget::Effect(effect.id);
    let EffectSettings::AutoFilter(settings) = &mut effect.unit else {
        panic!()
    };
    settings.lfo_enabled = false;
    settings.frequency_hz = 1000.0;
    let mut route = Modulation::new(ModulationParameter::Frequency);
    route.source = Some(source);
    route.amount = 2.0;
    settings.modulations.push(route);
    plan.project.tracks[0].effects.push(effect);
    let mut observed = Renderer::new(plan.clone());
    let mut reference = Renderer::new(plan.clone());
    observed.play();
    reference.play();
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    for _ in 0..2400 {
        assert_eq!(observed.next_sample(), reference.next_sample());
        assert!(
            observed
                .modulation_values(filter)
                .unwrap()
                .iter()
                .all(|v| v.is_finite())
        );
        assert!(observed.modulation_values(amplitude).unwrap()[0].is_finite());
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    let values = observed.modulation_values(filter).unwrap();
    assert!((1900.0..2100.0).contains(&values[0]), "{values:?}");
    assert_eq!(values[0], values[1]);
    let gain = observed.modulation_values(amplitude).unwrap()[0];
    assert!((-6.2..-5.8).contains(&gain), "{gain}");
    let mut edited = plan;
    edited.project.tracks[0].instrument.as_mut().unwrap().layers[0]
        .amplitude
        .modulation_open = true;
    let mut next = Renderer::new(edited);
    next.inherit(&mut observed);
    for _ in 0..2600 {
        assert_eq!(next.next_sample(), reference.next_sample());
    }
    assert!(next.modulation_values(amplitude).unwrap()[0].abs() < 1e-4);
    assert!((next.modulation_values(filter).unwrap()[0] - 4000.0).abs() < 2.0);
    next.pause();
    let held = next.modulation_values(amplitude);
    next.next_sample();
    assert_eq!(next.modulation_values(amplitude), held);
    next.stop();
    assert_eq!(
        next.modulation_values(amplitude).unwrap()[0],
        SILENT_GAIN_DB
    );
    assert!(
        next.modulation_values(ModulationTarget::Amplitude {
            track,
            layer: Id::new_v4()
        })
        .is_none()
    );
}

#[test]
fn drive_modulation_has_playback_export_seek_and_live_edit_parity_without_allocations() {
    for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
        let mut source = plan(Waveform::Saw, 48_000);
        let l = &mut source.project.tracks[0].instrument.as_mut().unwrap().layers[0];
        l.envelopes[1].present = true;
        l.envelopes[1].mode = mode;
        l.envelopes[1].settings.attack.time_ms = 400.0;
        let id = ModulationSourceId {
            layer: l.id,
            unit: l.envelopes[1].id,
        };
        let mut effect = EffectInstance::drive();
        let EffectSettings::Drive(f) = &mut effect.unit else {
            panic!()
        };
        f.gain = 0.0;
        f.color_shape = DriveColorShape::MidHump;
        let mut route = Modulation::new(ModulationParameter::DriveGain);
        route.source = Some(id);
        route.amount = 1.0;
        f.modulations.push(route);
        source.project.tracks[0].effects.push(effect);
        source.project.validate().unwrap();
        let mut wet = vec![[0.0; 2]; 32000];
        Renderer::new(source.clone()).render_offline(&mut wet);
        let target = daw_engine::ModulationTarget::Effect(source.project.tracks[0].effects[0].id);
        let mut live = Renderer::new(source.clone());
        live.play();
        ALLOCATIONS.with(|c| c.set(0));
        TRACK_ALLOCATIONS.with(|e| e.set(true));
        for (i, s) in wet.iter().enumerate() {
            let actual = live.next_sample();
            if i > 240 {
                assert!((actual[0] - s[0]).abs() < 1e-6);
            }
        }
        assert!(
            live.modulation_values(target)
                .unwrap()
                .iter()
                .all(|v| v.is_finite())
        );
        assert!(live.modulation_values(target).unwrap()[1] > 0.1);
        TRACK_ALLOCATIONS.with(|e| e.set(false));
        ALLOCATIONS.with(|c| assert_eq!(c.get(), 0));
        let mut random = [[0.0; 2]; 64];
        source.render(13000, &mut random);
        assert_eq!(&random[..], &wet[13000..13064]);
        let mut dry = source.clone();
        let EffectSettings::Drive(f) = &mut dry.project.tracks[0].effects[0].unit else {
            panic!()
        };
        f.modulations.clear();
        let mut unmodulated = vec![[0.0; 2]; 32000];
        Renderer::new(dry).render_offline(&mut unmodulated);
        assert!(
            wet.iter()
                .zip(&unmodulated)
                .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
        );
        let mut edited = source.clone();
        edited.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[1]
            .settings
            .attack
            .time_ms = 100.0;
        let EffectSettings::Drive(f) = &mut edited.project.tracks[0].effects[0].unit else {
            panic!()
        };
        f.modulations[0].amount = -0.5;
        let mut replacement = Renderer::new(edited);
        ALLOCATIONS.with(|c| c.set(0));
        TRACK_ALLOCATIONS.with(|e| e.set(true));
        replacement.inherit(&mut live);
        for _ in 0..512 {
            assert!(replacement.next_sample().iter().all(|s| s.is_finite()));
        }
        TRACK_ALLOCATIONS.with(|e| e.set(false));
        ALLOCATIONS.with(|c| assert_eq!(c.get(), 0));
    }
}

#[test]
fn portable_modulation_protocol_preserves_all_destination_apis_without_callback_allocation() {
    use daw_amplitude::Amplitude;
    use daw_auto_filter::AutoFilter;
    use daw_drive::Drive;
    use daw_envelope::Envelope;
    use daw_unit_core::{
        EffectUnit, ModulationContext, ModulationDestination, ModulationOutput, ModulationRoute,
        ModulationSource, ProcessingContext, process_modulation,
    };
    for rate in SAMPLE_RATES {
        let context = ProcessingContext {
            sample_rate_hz: rate,
            max_block_frames: 32,
        };
        let source = ModulationSourceId {
            layer: Id::new_v4(),
            unit: Id::new_v4(),
        };
        let mut envelope = Envelope::new(EnvelopeSettings::default(), context);
        let mut output = ModulationOutput::new(source, true, context);
        assert!(envelope.descriptor().provides_modulation());
        assert!(!envelope.descriptor().consumes_modulation());
        assert_eq!(
            Envelope::MODULATION_OUTPUT,
            daw_unit_core::ENVELOPE_MODULATION_OUTPUT
        );
        let amp_settings = AmplitudeSettings {
            present: true,
            ..Default::default()
        };
        let mut amp = Amplitude::new(&amp_settings, context);
        let mut amp_reference = Amplitude::new(&amp_settings, context);
        let mut filter = AutoFilter::new(context, &AutoFilterSettings::default());
        let mut filter_reference = AutoFilter::new(context, &AutoFilterSettings::default());
        let mut drive = Drive::new(context, &DriveSettings::default());
        let mut drive_reference = Drive::new(context, &DriveSettings::default());
        for descriptor in [amp.descriptor(), filter.descriptor(), drive.descriptor()] {
            assert!(descriptor.consumes_modulation());
            assert!(!descriptor.provides_modulation());
        }
        let routes = |parameters: &[ModulationParameter], amounts: &[f32]| {
            parameters
                .iter()
                .zip(amounts)
                .map(|(parameter, amount)| {
                    let mut settings = Modulation::new(*parameter);
                    settings.source = Some(source);
                    settings.amount = *amount;
                    ModulationRoute::new(settings, rate)
                })
                .collect::<Vec<_>>()
        };
        let filter_amounts = [2.0, 0.5, -3.0, -0.25];
        let drive_amounts = [-3.0, 0.15, -0.1, 0.2, 0.5];
        let mut amp_routes = routes(Amplitude::MODULATION_PARAMETERS, &[1.0]);
        let mut filter_routes = routes(AutoFilter::MODULATION_PARAMETERS, &filter_amounts);
        let mut drive_routes = routes(Drive::MODULATION_PARAMETERS, &drive_amounts);
        envelope.note_on();
        ALLOCATIONS.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|enabled| enabled.set(true));
        for frame in 0..rate / 20 {
            if frame == rate / 40 {
                envelope.note_off();
                output.set_enabled(false);
            }
            let cached = [output.sample(&mut envelope, Default::default())];
            let input = [0.2 * (frame as f32 * 0.035).sin(), 0.1];
            let c = ModulationContext {
                frame: u64::from(frame),
                ..Default::default()
            };
            // All three consume one cached output through the same public entry point.
            assert_eq!(
                process_modulation(&mut filter, &mut filter_routes, input, &[], &cached, c),
                filter_reference.process_modulated_at(
                    input,
                    c.frame,
                    c.tempo_bpm,
                    filter_amounts.map(|a| cached[0].value * (a * cached[0].weight))
                )
            );
            assert_eq!(
                process_modulation(&mut drive, &mut drive_routes, input, &[], &cached, c),
                drive_reference.process_modulated(
                    input,
                    drive_amounts.map(|a| cached[0].value * (a * cached[0].weight))
                )
            );
            let factor = 1.0 + cached[0].weight * (cached[0].value - 1.0);
            assert_eq!(
                process_modulation(&mut amp, &mut amp_routes, input[0], &[], &cached, c),
                amp_reference.process(input[0], factor, 1.0)
            );
        }
        TRACK_ALLOCATIONS.with(|enabled| enabled.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    }
}

fn lfo_plan(restart: LfoRestart, polarity: LfoPolarity) -> RenderPlan {
    let mut p = plan(Waveform::Saw, 48_000);
    let layer = &mut p.project.tracks[0].instrument.as_mut().unwrap().layers[0];
    let mut lfo = LfoInstance::default();
    lfo.settings.restart = restart;
    lfo.settings.polarity = polarity;
    lfo.settings.rate_mode = LfoRateMode::Hz;
    lfo.settings.rate_hz = 3.0;
    lfo.settings.smooth_ms = 7.0;
    let source = ModulationSourceId {
        layer: layer.id,
        unit: lfo.id,
    };
    layer.lfos.push(lfo);
    let mut route = Modulation::new(ModulationParameter::Amplitude);
    route.source = Some(source);
    route.amount = 0.8;
    layer.amplitude.modulations.push(route);
    let mut filter = EffectInstance::auto_filter();
    let EffectSettings::AutoFilter(f) = &mut filter.unit else {
        panic!()
    };
    f.lfo_enabled = false;
    f.frequency_hz = 1500.0;
    let mut route = Modulation::new(ModulationParameter::Frequency);
    route.source = Some(source);
    route.amount = 2.0;
    f.modulations.push(route);
    let mut drive = EffectInstance::drive();
    let EffectSettings::Drive(f) = &mut drive.unit else {
        panic!()
    };
    let mut route = Modulation::new(ModulationParameter::DriveGain);
    route.source = Some(source);
    route.amount = 0.3;
    f.modulations.push(route);
    p.project.tracks[0].effects.extend([filter, drive]);
    p.project.validate().unwrap();
    p
}
#[test]
fn lfo_fanout_has_live_export_block_seek_and_loop_parity_without_extending_note_lifetime() {
    for restart in LfoRestart::ALL {
        for polarity in [LfoPolarity::Unipolar, LfoPolarity::Bipolar] {
            let p = lfo_plan(restart, polarity);
            let mut expected = vec![[0.0; 2]; 52_000];
            Renderer::new(p.clone()).render_offline(&mut expected);
            assert!(expected.iter().flatten().all(|x| x.is_finite()));
            assert!(expected[1000..].iter().any(|s| s[0].abs() > 0.01));
            // Continuous modulation cannot hold a voice after its contour and gate end.
            assert!(expected[50_000..].iter().flatten().all(|x| *x == 0.0));
            let mut live = Renderer::new(p.clone());
            live.play();
            ALLOCATIONS.with(|v| v.set(0));
            TRACK_ALLOCATIONS.with(|v| v.set(true));
            for (i, s) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if i > 240 {
                    assert_eq!(actual, *s, "{restart:?} {polarity:?} {i}");
                }
            }
            live.pause();
            let position = live.playhead;
            for _ in 0..300 {
                live.next_sample();
            }
            assert_eq!(live.playhead, position);
            live.seek(12_345);
            TRACK_ALLOCATIONS.with(|v| v.set(false));
            assert_eq!(ALLOCATIONS.with(Cell::get), 0);
            let mut blocked = vec![[0.0; 2]; expected.len()];
            let mut renderer = Renderer::new(p.clone());
            for block in blocked.chunks_mut(137) {
                renderer.render_offline(block);
            }
            assert_eq!(blocked, expected);
            let mut random = [[0.0; 2]; 128];
            p.render(13_000, &mut random);
            assert_eq!(&random[..], &expected[13_000..13_128]);
            let mut loop_plan = p.clone();
            loop_plan.project.transport.r#loop = Loop {
                enabled: true,
                start_frame: 0,
                end_frame: 24_000,
            };
            let mut looping = Renderer::new(loop_plan);
            looping.play();
            looping.ramp = 1.0;
            let mut first = vec![[0.0; 2]; 24_000];
            for sample in &mut first {
                *sample = looping.next_sample();
            }
            // The existing loop crossfade and effect history settle after the boundary.
            for (i, sample) in first.iter().enumerate() {
                let actual = looping.next_sample();
                assert!(actual.iter().all(|s| s.is_finite()));
                if i > 2400 {
                    assert!((actual[0] - sample[0]).abs() < 1e-5, "loop {i}");
                }
            }
        }
    }
}
#[test]
fn adding_and_retargeting_lfos_retains_note_phase_and_callback_storage() {
    let p = lfo_plan(LfoRestart::Retrigger, LfoPolarity::Bipolar);
    let mut initial = p.clone();
    let layer = &mut initial.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.lfos.clear();
    layer.amplitude.modulations.pop();
    initial.project.tracks[0].effects.clear();
    let mut live = Renderer::new(initial);
    let mut reference = Renderer::new(p.clone());
    live.play();
    reference.play();
    for _ in 0..13_000 {
        live.next_sample();
        reference.next_sample();
    }
    let mut added = Renderer::new(p.clone());
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    added.inherit(&mut live);
    for _ in 0..2000 {
        added.next_sample();
        reference.next_sample();
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    // The newly added LFO uses the latest note origin, rather than transport frame zero.
    let target = daw_engine::ModulationTarget::Effect(p.project.tracks[0].effects[0].id);
    assert_eq!(
        added.modulation_values(target),
        reference.modulation_values(target)
    );
    let mut edited = p.clone();
    let lfo = &mut edited.project.tracks[0].instrument.as_mut().unwrap().layers[0].lfos[0];
    lfo.settings.wave = LfoWave::Square;
    lfo.settings.rate_hz = 7.0;
    lfo.settings.phase_degrees = 90.0;
    lfo.enabled = false;
    let mut off = Renderer::new(edited.clone());
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    off.inherit(&mut added);
    for _ in 0..1000 {
        assert!(off.next_sample().iter().all(|s| s.is_finite()));
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert!((off.modulation_values(target).unwrap()[0] - 1500.0).abs() < 0.1);
    edited.project.tracks[0].instrument.as_mut().unwrap().layers[0].lfos[0].enabled = true;
    let mut on = Renderer::new(edited);
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    on.inherit(&mut off);
    on.seek(0);
    for _ in 0..1000 {
        assert!(on.next_sample().iter().all(|s| s.is_finite()));
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn main_bus_mixes_the_stereo_layer_sum_before_track_effects_and_matches_live_export_at_all_rates() {
    for rate in SAMPLE_RATES {
        let mut base = plan(Waveform::Sine, rate);
        let mut second = InstrumentLayerSettings {
            pan: 0.3,
            ..Default::default()
        };
        second.oscillators.push(OscillatorSettings {
            waveform: Waveform::Triangle,
            transpose: 12,
            ..Default::default()
        });
        base.project.tracks[0]
            .instrument
            .as_mut()
            .unwrap()
            .layers
            .push(second);
        let mut changed = base.clone();
        let main = &mut changed.project.tracks[0].instrument.as_mut().unwrap().main;
        main.gain_db = -6.0;
        main.pan = -1.0;
        let mut original = vec![[0.0; 2]; 1000];
        let mut actual = original.clone();
        Renderer::new(base).render_offline(&mut original);
        Renderer::new(changed.clone()).render_offline(&mut actual);
        let factor = linear_gain(-6.0) * 2.0_f32.sqrt();
        for (a, b) in actual.iter().zip(&original) {
            assert!((a[0] - b[0] * factor).abs() < 1e-6);
            assert!(a[1].abs() < 1e-8);
        }
        let mut live = Renderer::new(changed);
        live.play();
        for (i, expected) in actual.iter().enumerate() {
            let s = live.next_sample();
            if i > rate as usize / 200 {
                assert_eq!(s, *expected);
            }
        }
    }
}
#[test]
fn main_modulators_use_shared_destinations_and_live_bus_edits_retain_voice_state_without_allocation()
 {
    let mut p = lfo_plan(LfoRestart::Legato, LfoPolarity::Unipolar);
    let original_sources = p.clone();
    let instrument = p.project.tracks[0].instrument.as_mut().unwrap();
    let lfo = instrument.layers[0].lfos.remove(0);
    let old = ModulationSourceId {
        layer: instrument.layers[0].id,
        unit: lfo.id,
    };
    let source = ModulationSourceId {
        layer: instrument.main.id,
        unit: lfo.id,
    };
    instrument.main.lfos.push(lfo);
    for route in &mut instrument.layers[0].amplitude.modulations {
        if route.source == Some(old) {
            route.source = Some(source);
        }
    }
    for effect in &mut p.project.tracks[0].effects {
        match &mut effect.unit {
            EffectSettings::AutoFilter(f) => f
                .modulations
                .iter_mut()
                .for_each(|r| r.source = Some(source)),
            EffectSettings::Drive(f) => f
                .modulations
                .iter_mut()
                .for_each(|r| r.source = Some(source)),
            _ => {}
        }
    }
    p.project.validate().unwrap();
    let mut original_audio = vec![[0.0; 2]; 16000];
    let mut main_audio = original_audio.clone();
    Renderer::new(original_sources).render_offline(&mut original_audio);
    Renderer::new(p.clone()).render_offline(&mut main_audio);
    assert_eq!(
        main_audio, original_audio,
        "Moving the shared source to Main preserves fan-out to all destinations"
    );
    // Isolate the live bus ramp from nonlinear downstream effects.
    p.project.tracks[0].effects.clear();
    let mut live = Renderer::new(p.clone());
    let mut reference = Renderer::new(p.clone());
    live.play();
    reference.play();
    for _ in 0..13000 {
        assert_eq!(live.next_sample(), reference.next_sample());
    }
    let mut changed = p.clone();
    changed.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .main
        .gain_db = -6.0;
    let mut next = Renderer::new(changed);
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    next.inherit(&mut live);
    for i in 0..1000 {
        let a = next.next_sample();
        let b = reference.next_sample();
        if i > 240 {
            assert!((a[0] - b[0] * linear_gain(-6.0)).abs() < 1e-6);
        }
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    // Main envelopes use the same shared note lifecycle and can fan out to layer amplitude.
    let instrument = p.project.tracks[0].instrument.as_mut().unwrap();
    let envelope = EnvelopeInstance {
        mode: EnvelopeMode::Legato,
        settings: EnvelopeSettings {
            release: EnvelopeStageSettings {
                time_ms: 10.0,
                ..EnvelopeSettings::default().release
            },
            ..Default::default()
        },
        ..Default::default()
    };
    let source = ModulationSourceId {
        layer: instrument.main.id,
        unit: envelope.id,
    };
    instrument.main.envelopes.push(envelope);
    let mut route = Modulation::new(ModulationParameter::Amplitude);
    route.source = Some(source);
    route.amount = 1.0;
    instrument.layers[0].amplitude.modulations.push(route);
    p.project.validate().unwrap();
    let mut expected = vec![[0.0; 2]; 52000];
    Renderer::new(p.clone()).render_offline(&mut expected);
    assert!(expected[50_000..].iter().flatten().all(|s| *s == 0.0));
    let mut live = Renderer::new(p);
    live.play();
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    for (i, b) in expected.iter().enumerate() {
        let a = live.next_sample();
        if i > 240 {
            assert_eq!(a, *b);
        }
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

#[test]
fn main_gain_precedes_nonlinear_track_effects() {
    let mut p = plan(Waveform::Saw, 48000);
    let mut drive = EffectInstance::drive();
    let EffectSettings::Drive(settings) = &mut drive.unit else {
        panic!()
    };
    settings.gain = 0.8;
    p.project.tracks[0].effects.push(drive);
    let mut bus = p.clone();
    bus.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .main
        .gain_db = -12.0;
    let mut layer = p.clone();
    layer.project.tracks[0].instrument.as_mut().unwrap().layers[0].gain_db = -12.0;
    let mut expected = vec![[0.0; 2]; 3000];
    let mut actual = expected.clone();
    let mut post_effect = expected.clone();
    Renderer::new(layer).render_offline(&mut expected);
    Renderer::new(bus).render_offline(&mut actual);
    Renderer::new(p).render_offline(&mut post_effect);
    assert!(
        actual
            .iter()
            .zip(&expected)
            .all(|(a, b)| (a[0] - b[0]).abs() < 1e-6)
    );
    assert!(
        actual
            .iter()
            .zip(&post_effect)
            .any(|(a, b)| (a[0] - b[0] * linear_gain(-12.0)).abs() > 0.01)
    );
}

fn pitch_plan(rate: u32, shared: bool) -> (RenderPlan, [daw_engine::ModulationTarget; 2]) {
    let mut p = plan(Waveform::Sine, rate);
    let track = p.project.tracks[0].id;
    let instrument = p.project.tracks[0].instrument.as_mut().unwrap();
    instrument.layers[0].oscillators.push(OscillatorSettings {
        transpose: -12,
        ..Default::default()
    });
    let source = if shared {
        let mut lfo = LfoInstance::default();
        lfo.settings.rate_mode = LfoRateMode::Hz;
        lfo.settings.rate_hz = 3.0;
        lfo.settings.polarity = LfoPolarity::Bipolar;
        lfo.settings.restart = LfoRestart::Retrigger;
        let source = ModulationSourceId {
            layer: instrument.main.id,
            unit: lfo.id,
        };
        instrument.main.lfos.push(lfo);
        source
    } else {
        let l = &mut instrument.layers[0];
        let e = &mut l.envelopes[1];
        e.present = true;
        e.settings.attack.time_ms = 400.0;
        e.settings.attack.shape = CurveShape::Linear;
        e.settings.decay.time_ms = 0.0;
        e.settings.sustain = 1.0;
        ModulationSourceId {
            layer: l.id,
            unit: e.id,
        }
    };
    let layer = &mut instrument.layers[0];
    let targets = std::array::from_fn(|index| {
        let mut route = Modulation::new(ModulationParameter::Pitch);
        route.source = Some(source);
        route.target = Some(layer.oscillators[index].id);
        route.amount = if index == 0 { 12.0 } else { -6.0 };
        layer.oscillator_modulations.push(route);
        daw_engine::ModulationTarget::Oscillator {
            track,
            layer: layer.id,
            oscillator: layer.oscillators[index].id,
        }
    });
    p.project.validate().unwrap();
    (p, targets)
}

#[test]
fn oscillator_pitch_targets_share_local_envelopes_and_main_lfos_with_live_export_and_seek_parity() {
    for rate in SAMPLE_RATES {
        for shared in [false, true] {
            let (p, targets) = pitch_plan(rate, shared);
            let count = (rate as f32 * 0.4) as usize;
            let mut expected = vec![[0.0; 2]; count];
            Renderer::new(p.clone()).render_offline(&mut expected);
            let mut live = Renderer::new(p.clone());
            live.play();
            for (i, sample) in expected.iter().enumerate() {
                let actual = live.next_sample();
                if i >= (rate / 200) as usize {
                    assert_eq!(actual, *sample);
                }
            }
            let a = live.modulation_values(targets[0]).unwrap();
            let b = live.modulation_values(targets[1]).unwrap();
            if shared {
                assert!((a[0] - a[1]).abs() < 1e-4);
                assert!((a[0] + 2.0 * (b[0] + 12.0)).abs() < 1e-4, "{a:?} {b:?}");
                assert!(a[0].abs() > 0.5);
            } else {
                // Overlapping notes have different contour ages, not a single bus pitch.
                assert!(a[1] - a[0] > 7.0, "{a:?}");
                assert!((a[0] + 2.0 * (b[1] + 12.0)).abs() < 1e-4, "{a:?} {b:?}");
                assert!((a[1] + 2.0 * (b[0] + 12.0)).abs() < 1e-4, "{a:?} {b:?}");
            }
            let mut blocked = vec![[0.0; 2]; count];
            let mut renderer = Renderer::new(p.clone());
            for block in blocked.chunks_mut(127) {
                renderer.render_offline(block);
            }
            assert_eq!(blocked, expected);
            let start = count as u64 - 1000;
            let mut sought = [[0.0; 2]; 128];
            p.render(start, &mut sought);
            assert_eq!(&sought[..], &expected[start as usize..start as usize + 128]);
            live.pause();
            let held = live.modulation_values(targets[0]).unwrap();
            for _ in 0..300 {
                live.next_sample();
            }
            assert_eq!(
                &live.modulation_values(targets[0]).unwrap()[..2],
                &held[..2]
            );
            live.stop();
            assert!(live.modulation_values(targets[0]).unwrap()[0].is_nan());
            // Continuous or pitch-only contour sources cannot extend the voice's amplitude gate.
            let mut tail = [[0.0; 2]; 128];
            p.render(u64::from(rate) * 3 / 2, &mut tail);
            assert!(tail.iter().all(|s| *s == [0.0; 2]));
        }
    }
}

#[test]
fn pitch_routing_retargets_by_oscillator_id_without_resetting_phase_or_allocating() {
    let (p, targets) = pitch_plan(48_000, true);
    let mut current = Renderer::new(p.clone());
    let mut reference = Renderer::new(p.clone());
    current.play();
    reference.play();
    for _ in 0..13000 {
        current.next_sample();
        reference.next_sample();
    }
    let mut reordered = p.clone();
    let layer = &mut reordered.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    layer.oscillators.reverse();
    layer.oscillator_modulations.reverse();
    layer.oscillator_modulation_open = true;
    let mut next = Renderer::new(reordered.clone());
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    next.inherit(&mut current);
    for _ in 0..1500 {
        assert_eq!(next.next_sample(), reference.next_sample());
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(
        &next.modulation_values(targets[0]).unwrap()[..2],
        &reference.modulation_values(targets[0]).unwrap()[..2]
    );
    // Disable routing, preserving the base transpose, voices, and source phase.
    let layer = &mut reordered.project.tracks[0]
        .instrument
        .as_mut()
        .unwrap()
        .layers[0];
    for route in &mut layer.oscillator_modulations {
        route.enabled = false;
    }
    let mut off = Renderer::new(reordered);
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    off.inherit(&mut next);
    for _ in 0..2000 {
        assert!(off.next_sample().iter().all(|v| v.is_finite()));
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert!(off.modulation_values(targets[0]).unwrap()[0].abs() < 1e-4);
    assert!((off.modulation_values(targets[1]).unwrap()[0] + 12.0).abs() < 1e-4);
    assert_eq!(
        off.plan.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers[0]
            .oscillators[1]
            .transpose,
        0
    );
}

#[test]
fn pitch_modulation_repeats_deterministically_at_loop_boundaries_without_callback_allocation() {
    let (mut p, _) = pitch_plan(48_000, true);
    p.project.transport.r#loop = Loop {
        enabled: true,
        start_frame: 0,
        end_frame: 24000,
    };
    let mut live = Renderer::new(p);
    live.play();
    live.ramp = 1.0;
    let mut first = vec![[0.0; 2]; 24000];
    for sample in &mut first {
        *sample = live.next_sample();
    }
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    for (i, expected) in first.iter().enumerate() {
        let actual = live.next_sample();
        if i > 2400 {
            assert!((actual[0] - expected[0]).abs() < 1e-5, "{i}");
        }
    }
    live.seek(12000);
    for _ in 0..1000 {
        assert!(live.next_sample().iter().all(|v| v.is_finite()));
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}

fn modulated_lfo_plan() -> RenderPlan {
    let mut p = lfo_plan(LfoRestart::Retrigger, LfoPolarity::Bipolar);
    let i = p.project.tracks[0].instrument.as_mut().unwrap();
    let layer = &mut i.layers[0];
    let e = &mut layer.envelopes[0];
    e.present = true;
    e.mode = EnvelopeMode::Legato;
    e.settings.attack.time_ms = 100.0;
    e.settings.sustain = 1.0;
    let source = ModulationSourceId {
        layer: layer.id,
        unit: e.id,
    };
    let lfo = &mut layer.lfos[0];
    lfo.settings.depth = 0.0;
    lfo.modulations.push(Modulation {
        source: Some(source),
        amount: 1.0,
        ..Modulation::new(ModulationParameter::LfoDepth)
    });
    let mut provider = LfoInstance::default();
    provider.settings.rate_mode = LfoRateMode::Hz;
    provider.settings.rate_hz = 0.5;
    lfo.modulations.push(Modulation {
        source: Some(ModulationSourceId {
            layer: i.main.id,
            unit: provider.id,
        }),
        amount: 2.0,
        ..Modulation::new(ModulationParameter::LfoRate)
    });
    i.main.lfos.push(provider);
    p.project.validate().unwrap();
    p
}
#[test]
fn modulated_lfos_match_live_export_blocks_and_graph_order_without_callback_allocations() {
    let p = modulated_lfo_plan();
    let mut expected = vec![[0.0; 2]; 12_000];
    Renderer::new(p.clone()).render_offline(&mut expected);
    let mut blocked = vec![[0.0; 2]; expected.len()];
    let mut blocks = Renderer::new(p.clone());
    for block in blocked.chunks_mut(137) {
        blocks.render_offline(block);
    }
    assert_eq!(expected, blocked);
    let mut random = [[0.0; 2]; 64];
    p.render(7000, &mut random);
    assert_eq!(&random[..], &expected[7000..7064]);
    let i = p.project.tracks[0].instrument.as_ref().unwrap();
    let key = daw_engine::ModulationTarget::Lfo {
        track: p.project.tracks[0].id,
        layer: i.layers[0].id,
        unit: i.layers[0].lfos[0].id,
    };
    let mut live = Renderer::new(p.clone());
    live.play();
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    for (frame, expected) in expected.iter().enumerate() {
        let sample = live.next_sample();
        if frame > 240 {
            assert_eq!(sample, *expected);
        }
        assert!(live.modulation_values(key).unwrap()[0].is_finite());
    }
    live.pause();
    let paused = live.playhead;
    for _ in 0..100 {
        live.next_sample();
    }
    assert_eq!(live.playhead, paused);
    live.seek(4000);
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    let values = blocks.modulation_values(key).unwrap();
    assert!(values[0] > 0.9 && values[1] > 3.0, "{values:?}");
    // A direct seek resets accumulated frequency history, consistently on every run.
    let mut sought_a = Renderer::new(p.clone());
    let mut sought_b = Renderer::new(p.clone());
    sought_a.seek(7000);
    sought_b.seek(7000);
    let mut a = [[0.0; 2]; 600];
    let mut b = a;
    sought_a.render_offline(&mut a);
    sought_b.render_offline(&mut b);
    assert_eq!(a, b);
    // Moving the provider before the destination in saved layout changes no samples.
    let mut reordered = p.clone();
    let i = reordered.project.tracks[0].instrument.as_mut().unwrap();
    let provider = i.main.lfos.remove(0);
    let old = ModulationSourceId {
        layer: i.main.id,
        unit: provider.id,
    };
    let new = ModulationSourceId {
        layer: i.layers[0].id,
        unit: provider.id,
    };
    i.layers[0].lfos[0].modulations[1].source = Some(new);
    i.layers[0].lfos.insert(0, provider);
    assert_ne!(old, new);
    let mut moved = vec![[0.0; 2]; expected.len()];
    Renderer::new(reordered).render_offline(&mut moved);
    assert_eq!(moved, expected);
    let mut loop_plan = p;
    loop_plan.project.transport.r#loop = Loop {
        enabled: true,
        start_frame: 0,
        end_frame: 12_000,
    };
    let mut looped = Renderer::new(loop_plan);
    looped.play();
    looped.ramp = 1.0;
    let mut first = vec![[0.0; 2]; 12_000];
    for sample in &mut first {
        *sample = looped.next_sample();
    }
    for (frame, sample) in first.iter().enumerate() {
        let actual = looped.next_sample();
        if frame > 2400 {
            assert!((actual[0] - sample[0]).abs() < 1e-5, "loop {frame}");
        }
    }
}
#[test]
fn live_lfo_depth_rate_and_connection_edits_retain_voices_and_prepared_storage() {
    let p = modulated_lfo_plan();
    let mut live = Renderer::new(p.clone());
    live.play();
    for _ in 0..4000 {
        live.next_sample();
    }
    let mut edited = p.clone();
    let lfo = &mut edited.project.tracks[0].instrument.as_mut().unwrap().layers[0].lfos[0];
    lfo.settings.rate_hz = 5.0;
    lfo.settings.depth = 0.2;
    lfo.modulations[0].amount = 0.5;
    lfo.modulations[1].enabled = false;
    let mut next = Renderer::new(edited);
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    next.inherit(&mut live);
    for _ in 0..2000 {
        assert!(next.next_sample().into_iter().all(f32::is_finite));
    }
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(next.playhead, 6000);
    let i = next.plan.project.tracks[0].instrument.as_ref().unwrap();
    let key = daw_engine::ModulationTarget::Lfo {
        track: next.plan.project.tracks[0].id,
        layer: i.layers[0].id,
        unit: i.layers[0].lfos[0].id,
    };
    let values = next.modulation_values(key).unwrap();
    assert!(
        (values[0] - 0.7).abs() < 0.0001 && (values[1] - 5.0).abs() < 0.0001,
        "{values:?}"
    );
}

#[test]
fn note_relative_envelopes_share_live_export_blocks_and_allocation_free_retargeting() {
    for mode in EnvelopeMode::ALL {
        let mut p = plan(Waveform::Saw, 48_000);
        let e = &mut p.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0];
        e.mode = mode;
        e.settings = EnvelopeSettings {
            timing: EnvelopeTiming::Note,
            attack_note: 0.5,
            decay_note: 0.25,
            ..Default::default()
        };
        p.project.validate().unwrap();
        let mut expected = vec![[0.0; 2]; 60_000];
        Renderer::new(p.clone()).render_offline(&mut expected);
        let mut blocked = expected.clone();
        let mut blocks = Renderer::new(p.clone());
        for block in blocked.chunks_mut(137) {
            blocks.render_offline(block);
        }
        assert_eq!(expected, blocked);
        let mut random = [[0.0; 2]; 64];
        p.render(15_000, &mut random);
        assert_eq!(&random[..], &expected[15_000..15_064]);
        let mut live = Renderer::new(p.clone());
        live.play();
        ALLOCATIONS.with(|v| v.set(0));
        TRACK_ALLOCATIONS.with(|v| v.set(true));
        for (frame, sample) in expected.iter().enumerate() {
            let actual = live.next_sample();
            if frame >= 240 {
                assert_eq!(actual, *sample);
            }
        }
        live.seek(6000);
        for _ in 0..1000 {
            live.next_sample();
        }
        TRACK_ALLOCATIONS.with(|v| v.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        p.project.tracks[0].instrument.as_mut().unwrap().layers[0].envelopes[0]
            .settings
            .attack_note = 0.75;
        let mut updated = Renderer::new(p);
        ALLOCATIONS.with(|v| v.set(0));
        TRACK_ALLOCATIONS.with(|v| v.set(true));
        updated.inherit(&mut live);
        for _ in 0..1000 {
            assert!(updated.next_sample().into_iter().all(f32::is_finite));
        }
        TRACK_ALLOCATIONS.with(|v| v.set(false));
        assert_eq!(ALLOCATIONS.with(Cell::get), 0);
        assert_eq!(updated.playhead, 8000);
    }
}
