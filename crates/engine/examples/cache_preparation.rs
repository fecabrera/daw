//! Synthetic exact-history comparison. Run with tools/performance_audit.py --suite cache.
#[path = "../tests/support/cache_project.rs"]
mod fixture;
use daw_core::EffectInstance;
use daw_engine::{CacheProfile, PreparationMode, TransportCache, TransportState};
use serde_json::json;
use std::{hint::black_box, time::Instant};

fn main() {
    let repetitions = std::env::args()
        .nth(2)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(7)
        .max(3);
    for (name, tracks, effects, master, frames, buffer) in [
        ("single", 1, 1, 0, 48000, 512),
        ("balanced2", 2, 1, 0, 48000, 512),
        ("balanced8", 8, 1, 0, 48000, 512),
        ("voices8", 8, 0, 0, 48000, 512),
        ("skew8", 8, 0, 0, 48000, 512),
        ("master8", 8, 0, 4, 48000, 512),
        ("short2", 2, 1, 0, 128, 512),
        ("audio8", 1, 0, 0, 48000, 512),
        ("buffer32", 4, 1, 0, 48000, 32),
        ("buffer4096", 4, 1, 0, 48000, 4096),
    ] {
        let mut plan = fixture::midi_project(48000, tracks, effects);
        plan.project.buffer_size_frames = buffer;
        if name == "audio8" {
            plan.project.tracks[0].instrument.as_mut().unwrap().layers[0]
                .oscillators
                .clear();
            fixture::add_audio(&mut plan, 8, 0, false);
        }
        if name == "skew8" {
            for _ in 0..8 {
                plan.project.tracks[0]
                    .effects
                    .push(EffectInstance::saturator());
            }
        }
        for _ in 0..master {
            plan.project
                .master
                .effects
                .push(EffectInstance::saturator());
        }
        let modes = [
            PreparationMode::Serial,
            PreparationMode::Parallel,
            PreparationMode::Auto,
        ];
        let mut cold: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
        let mut warm: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::new());
        let mut stats = [daw_engine::PreparationStats::default(); 3];
        let mut checkpoint_bytes = [0_u64; 3];
        // Alternate execution order to reduce a fixed serial/parallel thermal bias.
        for run in 0..=repetitions {
            for slot in 0..3 {
                let mode_index = (slot + run) % 3;
                let mode = modes[mode_index];
                let state = TransportState {
                    frame: frames,
                    playing: true,
                    playback_start: Some(0),
                    ..Default::default()
                };
                let start = Instant::now();
                let mut cache =
                    TransportCache::with_mode(plan.clone(), CacheProfile::Balanced, mode);
                let mut renderer = cache.prepare(state, || false).unwrap();
                let elapsed = start.elapsed().as_secs_f64() * 1e6;
                black_box(renderer.next_sample());
                let start = Instant::now();
                let restored = cache
                    .prepare(
                        TransportState {
                            frame: frames.saturating_sub(123),
                            ..state
                        },
                        || false,
                    )
                    .unwrap();
                let restored_time = start.elapsed().as_secs_f64() * 1e6;
                black_box(restored);
                stats[mode_index] = cache.preparation_stats();
                checkpoint_bytes[mode_index] = cache.memory_usage().checkpoint_bytes;
                if run > 0 {
                    cold[mode_index].push(elapsed);
                    warm[mode_index].push(restored_time);
                }
            }
        }
        for (index, mode) in modes.iter().enumerate() {
            for (phase, values) in [("cold", &mut cold[index]), ("warm", &mut warm[index])] {
                values.sort_by(f64::total_cmp);
                println!(
                    "{}",
                    json!({"suite":"cache", "case":format!("{name}/{mode:?}/{phase}"), "samples":values.len(),
                    "median_us":values[values.len()/2], "p95_us":values[(values.len()*95).div_ceil(100).saturating_sub(1)],
                    "max_us":values[values.len()-1], "latencies_us":values,
                    "facts":{"rate":48000,"tracks":plan.project.tracks.len(),"frames":frames,"project_buffer":buffer,
                        "workers":stats[index].workers,"parallel_blocks":stats[index].parallel_blocks,
                        "serial_blocks":stats[index].serial_blocks,"parallel_bytes":stats[index].parallel_bytes,
                        "checkpoint_bytes":checkpoint_bytes[index],"cold_includes_construction_and_thread_start":true,
                        "reference":"same exact renderer and ordered mix","native_audio_device":false}})
                );
            }
        }
    }
}
