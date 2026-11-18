use daw_core::*;
use daw_engine::{RenderPlan, Renderer};
use std::cell::Cell;
#[path = "support/allocations.rs"]
mod allocations;
use allocations::{ALLOCATIONS, CountingAllocator, TRACK_ALLOCATIONS};

#[global_allocator]
static ALLOCATOR: CountingAllocator<true> = CountingAllocator;
#[path = "support/midi_plan.rs"]
mod midi_fixture;
#[path = "support/tone.rs"]
mod tone;
#[path = "support/tone_parity.rs"]
mod tone_parity;
use tone::plan;
fn configured() -> EffectInstance {
    let mut e = EffectInstance::harmonic_lift();
    let EffectSettings::HarmonicLift(s) = &mut e.unit else {
        unreachable!()
    };
    s.bands[2].enabled = true;
    s.bands[2].gain_db = 9.0;
    s.drive = 0.2;
    s.push = 0.2;
    e
}
#[test]
fn live_offline_blocks_seek_and_latency_match_on_tracks_and_master_at_every_rate() {
    tone_parity::check(configured, 49);
}
#[test]
fn inherit_modulation_and_quality_updates_do_not_allocate_or_retire_on_callback() {
    let mut p = plan(48000, false);
    let e = configured();
    let id = e.id;
    let band = e.unit.band_ids().next().unwrap();
    p.project.tracks[0].effects.push(e);
    let mut old = Renderer::new(p.clone());
    old.play();
    for _ in 0..512 {
        old.next_sample();
    }
    let EffectSettings::HarmonicLift(s) = &mut p.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    s.quality = HarmonicQuality::Hq;
    s.bands[0].enabled = true;
    s.level_db = -3.0;
    let mut next = Renderer::new(p);
    ALLOCATIONS.with(|c| c.set(0));
    TRACK_ALLOCATIONS.with(|c| c.set(true));
    next.inherit(&mut old);
    for _ in 0..512 {
        assert!(next.next_sample().iter().all(|v| v.is_finite()));
    }
    TRACK_ALLOCATIONS.with(|c| c.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert!(
        next.modulation_snapshot(daw_engine::ModulationTarget::EffectBand { effect: id, band })
            .is_some()
    );
}

fn midi_plan(rate: u32) -> RenderPlan {
    midi_fixture::plan(Waveform::Sine, rate)
}

#[test]
fn active_targeted_routes_and_changed_route_amounts_stay_allocation_free() {
    let mut p = midi_plan(48000);
    let instrument = p.project.tracks[0].instrument.as_mut().unwrap();
    instrument.main.macro_module.add();
    instrument.main.macro_module.controls[0].settings.value = 0.5;
    let source = ModulationSourceId {
        layer: instrument.main.id,
        unit: instrument.main.macro_module.controls[0].id,
    };
    let mut effect = configured();
    let EffectSettings::HarmonicLift(s) = &mut effect.unit else {
        unreachable!()
    };
    let mut r = Modulation::new(ModulationParameter::EqBandGain);
    r.target = Some(s.bands[0].id);
    r.source = Some(source);
    r.amount = 12.0;
    s.modulations.push(r);
    instrument.layers[0].effects.push(effect);
    p.project.validate().unwrap();
    let mut old = Renderer::new(p.clone());
    old.play();
    for _ in 0..2048 {
        old.next_sample();
    }
    let EffectSettings::HarmonicLift(s) =
        &mut p.project.tracks[0].instrument.as_mut().unwrap().layers[0].effects[0].unit
    else {
        unreachable!()
    };
    s.modulations[0].amount = -12.0;
    let mut next = Renderer::new(p);
    ALLOCATIONS.with(|c| c.set(0));
    TRACK_ALLOCATIONS.with(|c| c.set(true));
    next.inherit(&mut old);
    for _ in 0..1024 {
        next.next_sample();
    }
    TRACK_ALLOCATIONS.with(|c| c.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
}
