use daw_core::*;
use daw_engine::{RenderPlan, Renderer};
use std::cell::Cell;
#[path = "support/allocations.rs"]
mod allocations;
use allocations::{ALLOCATIONS, CountingAllocator, TRACK_ALLOCATIONS};

#[global_allocator]
static ALLOCATOR: CountingAllocator<true> = CountingAllocator;
use std::sync::Arc;
#[path = "support/midi_plan.rs"]
mod midi_fixture;
#[path = "support/tone.rs"]
mod tone;
#[path = "support/tone_parity.rs"]
mod tone_parity;
use tone::plan;
fn configured() -> EffectInstance {
    let mut e = EffectInstance::exciter();
    let EffectSettings::Exciter(s) = &mut e.unit else {
        unreachable!()
    };
    for (i, b) in s.bands.iter_mut().enumerate() {
        b.mix = 0.4;
        b.drive_db = 18.0;
        b.mode = ExciterMode::ALL[i % 4];
    }
    e
}
#[test]
fn live_offline_blocks_seek_and_latency_match_on_tracks_and_master_at_every_rate() {
    tone_parity::check(configured, 32);
}
#[test]
fn live_solo_never_changes_a_new_offline_renderer_and_callback_updates_do_not_allocate_or_retire() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut p = plan(48000, false);
    let effect = configured();
    let id = effect.id;
    let band = effect.unit.band_ids().next().unwrap();
    p.project.tracks[0].effects.push(effect);
    let encoded = serde_json::to_string(&p.project).unwrap();
    let mut expected = vec![[0.0; 2]; 8192];
    Renderer::new(p.clone()).render_offline(&mut expected);
    let mut live = Renderer::new(p.clone());
    let solo = Arc::new(AtomicBool::new(true));
    live.bind_exciter_solo(id, band, solo.clone());
    live.play();
    let mut heard = vec![[0.0; 2]; 2048];
    for v in &mut heard {
        *v = live.next_sample();
    }
    assert!(
        heard[512..]
            .iter()
            .zip(&expected[512..2048])
            .any(|(a, b)| (a[0] - b[0]).abs() > 0.01)
    );
    let mut exported = vec![[0.0; 2]; 8192];
    Renderer::new(p.clone()).render_offline(&mut exported);
    assert_eq!(exported, expected);
    assert_eq!(serde_json::to_string(&p.project).unwrap(), encoded);
    let EffectSettings::Exciter(s) = &mut p.project.tracks[0].effects[0].unit else {
        unreachable!()
    };
    s.bands[1].drive_db = 24.0;
    s.bands[1].mode = ExciterMode::Tube;
    s.level_db = -3.0;
    let mut next = Renderer::new(p);
    next.bind_exciter_solo(id, band, solo.clone());
    ALLOCATIONS.with(|c| c.set(0));
    TRACK_ALLOCATIONS.with(|c| c.set(true));
    next.inherit(&mut live);
    solo.store(false, Ordering::Relaxed);
    for _ in 0..1024 {
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
fn every_instrument_bus_receives_the_same_live_only_solo_and_output_meter_binding() {
    use std::sync::atomic::AtomicBool;
    for location in 0..4 {
        let mut p = midi_plan(48000);
        let e = configured();
        let id = e.id;
        let band = e.unit.band_ids().next().unwrap();
        let t = &mut p.project.tracks[0];
        match location {
            0 => t.instrument.as_mut().unwrap().layers[0].effects.push(e),
            1 => t.instrument.as_mut().unwrap().main.effects.push(e),
            2 => t.effects.push(e),
            _ => p.project.master.effects.push(e),
        }
        p.project.validate().unwrap();
        let mut expected = vec![[0.0; 2]; 4096];
        Renderer::new(p.clone()).render_offline(&mut expected);
        let mut soloed = Renderer::new(p.clone());
        soloed.bind_exciter_solo(id, band, Arc::new(AtomicBool::new(true)));
        let meter = Arc::new(daw_unit_core::BandMeter::<3>::default());
        soloed.set_band_peak_meter(id, meter.clone());
        let mut heard = vec![[0.0; 2]; 4096];
        soloed.render_offline(&mut heard);
        let energy = |v: &[[f32; 2]]| {
            v[2048..]
                .iter()
                .map(|s| f64::from(s[0]).powi(2))
                .sum::<f64>()
        };
        assert!(energy(&expected) > 0.01);
        assert!(
            energy(&heard) < energy(&expected) * 0.01,
            "location {location}"
        );
        assert!(
            f32::from_bits(meter.current()[0].load(std::sync::atomic::Ordering::Relaxed)) > 0.0
        );
        let mut exported = vec![[0.0; 2]; 4096];
        Renderer::new(p).render_offline(&mut exported);
        assert_eq!(exported, expected);
    }
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
    let EffectSettings::Exciter(s) = &mut effect.unit else {
        unreachable!()
    };
    let mut r = Modulation::new(ModulationParameter::SaturatorDrive);
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
    let EffectSettings::Exciter(s) =
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
