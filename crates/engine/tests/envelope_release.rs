use daw_core::*;
use daw_engine::{ENVELOPE_PLOT_POSITION, ModulationTarget, RenderPlan, Renderer};

#[path = "support/allocations.rs"]
mod allocations;
#[path = "support/midi_plan.rs"]
mod midi_fixture;

#[global_allocator]
static ALLOCATOR: allocations::CountingAllocator<false> = allocations::CountingAllocator;

fn note(id: u128, start_tick: u32, end_tick: u32, pitch: u8) -> MidiNote {
    MidiNote {
        id: Id::from_u128(id),
        start_tick,
        length_ticks: end_tick - start_tick,
        pitch,
        velocity: 127,
    }
}

fn release_plan(
    rate: u32,
    mode: EnvelopeMode,
    notes: Vec<MidiNote>,
) -> (RenderPlan, Id, ModulationTarget) {
    let mut plan = midi_fixture::plan(Waveform::Sine, rate);
    let track = &mut plan.project.tracks[0];
    let id = track.id;
    let clip_id = track.clips[0].id;
    let layer = &mut track.instrument.as_mut().unwrap().layers[0];
    let envelope = &mut layer.envelopes[0];
    envelope.mode = mode;
    envelope.settings.attack.time_ms = 0.0;
    envelope.settings.release.time_ms = 330.0;
    let meter = ModulationTarget::Envelope {
        track: id,
        layer: layer.id,
        unit: envelope.id,
    };
    plan.project
        .edit(Edit::ReplaceMidiPattern {
            clip_id,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes,
            },
        })
        .unwrap();
    (plan, id, meter)
}

fn stage(renderer: &Renderer, meter: ModulationTarget) -> f32 {
    renderer.modulation_snapshot(meter).unwrap()[ENVELOPE_PLOT_POSITION.start]
}

fn advance(renderer: &mut Renderer, frames: u64) {
    for _ in 0..frames {
        renderer.next_sample();
    }
}

fn configure_live(renderer: &mut Renderer, track: Id) {
    renderer.prepare_live_midi();
    renderer.live_midi_route(track, true);
}

fn assert_close(actual: [f32; 2], expected: [f32; 2]) {
    for (a, b) in actual.into_iter().zip(expected) {
        assert!((a - b).abs() < 0.00002, "{actual:?} != {expected:?}");
    }
}

#[test]
fn off_for_skipped_pre_loop_attack_cannot_release_a_held_note() {
    for rate in SAMPLE_RATES {
        for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
            for compensated in [false, true] {
                let (mut plan, _, meter) = release_plan(
                    rate,
                    mode,
                    vec![note(1, 20, 300, 45), note(2, 180, 650, 72)],
                );
                let start = plan.project.midi_frame(100);
                let end = plan.project.midi_frame(800);
                let check = plan.project.midi_frame(390);
                let after_off = plan.project.midi_frame(720);
                plan.project.transport.r#loop = Loop {
                    enabled: true,
                    start_frame: start,
                    end_frame: end,
                };
                if compensated {
                    plan.project.tracks[0]
                        .effects
                        .push(EffectInstance::limiter());
                }
                let mut renderer = Renderer::new(plan);
                renderer.play();
                advance(&mut renderer, end - start + check - start);
                assert_eq!(
                    stage(&renderer, meter),
                    2.5,
                    "{rate} {mode:?} {compensated}"
                );
                advance(&mut renderer, after_off - check);
                assert!(
                    stage(&renderer, meter) >= 3.0,
                    "{rate} {mode:?} {compensated}"
                );
            }
        }
    }
}

#[test]
fn loop_and_gap_do_not_readmit_earlier_released_voices() {
    for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
        for (wrap, already_releasing) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let earlier = note(1, 20, 140, 45);
            let final_note = note(2, 80, 400, 72);
            let make = |notes| {
                let (mut plan, _, _) = release_plan(48_000, mode, notes);
                let start = plan.project.midi_frame(192);
                let length = plan
                    .project
                    .midi_frame(if already_releasing { 520 } else { 240 });
                let boundary = start + length;
                let clip = &mut plan.project.tracks[0].clips[0];
                clip.start_frame = start;
                if !wrap {
                    clip.length_frames = length;
                }
                plan.project.transport.r#loop = Loop {
                    enabled: true,
                    start_frame: 0,
                    end_frame: if wrap { boundary } else { boundary + start },
                };
                (Renderer::new(plan), boundary)
            };
            let (mut actual, boundary) = make(vec![earlier, final_note.clone()]);
            let (mut reference, _) = make(vec![final_note]);
            actual.play();
            reference.play();
            advance(&mut actual, boundary + 511);
            advance(&mut reference, boundary + 511);
            let mut peak = 0.0_f32;
            for _ in 0..997 {
                let sample = actual.next_sample();
                peak = peak.max(sample[0].abs());
                assert_close(sample, reference.next_sample());
            }
            assert!(
                peak > 0.01,
                "{mode:?} wrap={wrap} releasing={already_releasing}"
            );
        }
    }
}

#[test]
fn live_shared_tail_survives_both_edges_of_an_empty_clip() {
    for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
        let (mut with_clip, track, meter) = release_plan(48_000, mode, vec![]);
        with_clip.project.tracks[0].clips[0].start_frame = 2701;
        with_clip.project.tracks[0].clips[0].length_frames = 2303;
        let mut without_clip = with_clip.clone();
        without_clip.project.tracks[0].clips.clear();
        let mut actual = Renderer::new(with_clip);
        let mut reference = Renderer::new(without_clip);
        let token = Id::from_u128(99);
        for renderer in [&mut actual, &mut reference] {
            configure_live(renderer, track);
            renderer.play();
            assert!(renderer.live_midi_note(token, Some((69, 127))));
            advance(renderer, 1749);
            renderer.live_midi_note(token, None);
        }
        let mut peak = 0.0_f32;
        for _ in 0..5501 {
            let sample = actual.next_sample();
            peak = peak.max(sample[0].abs());
            assert_close(sample, reference.next_sample());
        }
        assert!(peak > 0.01);
        assert_eq!(stage(&actual, meter), stage(&reference, meter));
        assert!(stage(&actual, meter) >= 3.0);
    }
}

#[test]
fn idle_live_route_keeps_paused_shared_contour_and_resume_samples_frozen() {
    for rate in SAMPLE_RATES {
        for mode in [EnvelopeMode::Retrigger, EnvelopeMode::Legato] {
            for release in [false, true] {
                let (mut plan, track, meter) = release_plan(rate, mode, vec![note(1, 0, 240, 69)]);
                let release_start = plan.project.midi_frame(240);
                let instrument = plan.project.tracks[0].instrument.as_mut().unwrap();
                instrument.main.macro_module.add();
                let knob = &mut instrument.main.macro_module.controls[0];
                knob.settings.value = 0.4;
                let mut route = Modulation::new(if release {
                    ModulationParameter::EnvelopeRelease
                } else {
                    ModulationParameter::EnvelopeAttack
                });
                route.source = Some(ModulationSourceId {
                    layer: instrument.main.id,
                    unit: knob.id,
                });
                route.amount = 0.6;
                let layer = &mut instrument.layers[0];
                layer.envelopes[0].settings.attack.time_ms = 90.0;
                layer.envelopes[0].modulations.push(route);
                let other = plan.project.add_midi_track().unwrap();
                let mut actual = Renderer::new(plan.clone());
                let mut reference = Renderer::new(plan);
                for renderer in [&mut actual, &mut reference] {
                    configure_live(renderer, track);
                    renderer.play();
                    advance(
                        renderer,
                        if release {
                            release_start + 317
                        } else {
                            u64::from(rate / 50) + 13
                        },
                    );
                }
                let before = stage(&actual, meter);
                assert_eq!(before >= 3.0, release);
                let frame = actual.playhead;
                actual.pause();
                actual.live_midi_route(track, false);
                actual.live_midi_route(other, true);
                let token = Id::from_u128(100);
                actual.live_midi_note(token, Some((69, 100)));
                actual.live_midi_route(track, true);
                allocations::ALLOCATIONS.with(|count| count.set(0));
                allocations::TRACK_ALLOCATIONS.with(|track| track.set(true));
                advance(&mut actual, u64::from(rate / 2) + 7);
                allocations::TRACK_ALLOCATIONS.with(|track| track.set(false));
                assert_eq!(allocations::ALLOCATIONS.with(std::cell::Cell::get), 0);
                assert_eq!(actual.playhead, frame);
                assert_eq!(stage(&actual, meter), before);
                assert!(actual.live_envelope_snapshot(meter).is_none());
                actual.play();
                for _ in 0..509 {
                    assert_close(actual.next_sample(), reference.next_sample());
                }
                assert_eq!(stage(&actual, meter), stage(&reference, meter));
            }
        }
    }
}

#[test]
fn live_shape_includes_per_note_reserve_and_clears_after_off() {
    for mode in [
        EnvelopeMode::PerNote,
        EnvelopeMode::Retrigger,
        EnvelopeMode::Legato,
    ] {
        for playing in [false, true] {
            let (plan, track, meter) = release_plan(48_000, mode, vec![]);
            let mut renderer = Renderer::new(plan);
            configure_live(&mut renderer, track);
            if playing {
                renderer.play();
            }
            let token = Id::from_u128(101);
            assert!(renderer.live_midi_note(token, Some((69, 127))));
            advance(&mut renderer, 1021);
            assert_eq!(stage(&renderer, meter), 2.5);
            assert!(renderer.live_midi_note(token, None));
            advance(&mut renderer, 509);
            let snapshot = renderer.live_envelope_snapshot(meter).unwrap();
            assert!((3.0..4.0).contains(&snapshot[ENVELOPE_PLOT_POSITION.start]));
            assert_eq!(
                stage(&renderer, meter),
                snapshot[ENVELOPE_PLOT_POSITION.start]
            );
            advance(&mut renderer, 16_021);
            assert!(renderer.live_envelope_snapshot(meter).is_none());
        }
    }
}

#[test]
fn shape_keeps_the_first_scheduled_voice_when_a_live_voice_also_releases() {
    let (plan, track, meter) =
        release_plan(48_000, EnvelopeMode::PerNote, vec![note(1, 0, 480, 60)]);
    let mut renderer = Renderer::new(plan);
    configure_live(&mut renderer, track);
    renderer.play();
    advance(&mut renderer, 501);
    let token = Id::from_u128(102);
    assert!(renderer.live_midi_note(token, Some((69, 127))));
    advance(&mut renderer, 509);
    assert!(renderer.live_midi_note(token, None));
    advance(&mut renderer, 127);
    assert_eq!(stage(&renderer, meter), 2.5);
    assert!(renderer.live_envelope_snapshot(meter).unwrap()[ENVELOPE_PLOT_POSITION.start] >= 3.0);
}
