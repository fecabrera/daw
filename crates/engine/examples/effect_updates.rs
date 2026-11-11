//! Synthetic preparation/handoff probe; no device or audible-latency claims.
use daw_core::{EffectInstance, EffectSettings, Project};
use daw_engine::{EffectUpdate, RenderPlan, Renderer};
use std::{hint::black_box, time::Instant};

fn median(values: &mut [u128]) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn main() {
    println!(
        "family,units,edit,full_prepare_ns,full_handoff_ns,patch_prepare_ns,patch_handoff_ns,new_processors"
    );
    for mixed in [false, true] {
        for count in [1, 8, 32, 128] {
            for reorder in [false, true] {
                if reorder && count == 1 {
                    continue;
                }
                let mut project = Project::default();
                project.add_track().unwrap();
                project.tracks[0].effects = (0..count)
                    .map(|i| {
                        if mixed && i % 16 == 15 {
                            EffectInstance::plate_reverb()
                        } else {
                            EffectInstance::drive()
                        }
                    })
                    .collect();
                let mut plan = RenderPlan {
                    project,
                    beat_tracks: Default::default(),
                    audio: Default::default(),
                    sampler_audio: Default::default(),
                    stretched_audio: Default::default(),
                };
                let mut full = Renderer::new(plan.clone());
                let mut active = Renderer::new(plan.clone());
                let mut times: [Vec<u128>; 4] = std::array::from_fn(|_| Vec::new());
                for step in 0..16 {
                    let mut desired = plan.clone();
                    if reorder {
                        desired.project.tracks[0].effects.swap(0, 1);
                    } else if let EffectSettings::Drive(s) =
                        &mut desired.project.tracks[0].effects[0].unit
                    {
                        s.gain = if step % 2 == 0 { 0.2 } else { 0.8 };
                    }
                    let start = Instant::now();
                    let mut candidate = Renderer::new(desired.clone());
                    times[0].push(start.elapsed().as_nanos());
                    let start = Instant::now();
                    candidate.inherit(&mut full);
                    times[1].push(start.elapsed().as_nanos());
                    full = candidate;
                    let start = Instant::now();
                    let mut update = EffectUpdate::prepare(&plan, desired.clone()).unwrap();
                    times[2].push(start.elapsed().as_nanos());
                    assert_eq!(update.prepared_processors(), 0);
                    let start = Instant::now();
                    active.apply_effect_update(&mut update);
                    times[3].push(start.elapsed().as_nanos());
                    black_box(&active);
                    plan = desired;
                }
                let [build, inherit, prepare, apply] = times.each_mut().map(|v| median(v));
                println!(
                    "{},{count},{},{build},{inherit},{prepare},{apply},0",
                    if mixed { "mixed-reverb" } else { "drive" },
                    if reorder { "reorder" } else { "control" }
                );
            }
        }
    }
}
