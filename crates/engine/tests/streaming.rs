use daw_core::*;
use daw_engine::{RenderPlan, Renderer};
use daw_media::{DiskAudioWriter, STREAM_CHUNK_FRAMES};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    time::{Duration, Instant},
};
#[path = "support/cache_project.rs"]
mod support;
thread_local! { static COUNT: Cell<Option<usize>> = const { Cell::new(None) }; }
struct Allocator;
// SAFETY: Allocations are forwarded to System with unchanged pointers/layouts.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNT.with(|c| {
            if let Some(n) = c.get() {
                c.set(Some(n + 1));
            }
        });
        unsafe { System.realloc(p, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn plan(rate: u32, effects: usize) -> (RenderPlan, RenderPlan) {
    let mut resident = support::midi_project(rate, 0, 0);
    let id = support::add_audio(&mut resident, 1, effects, false);
    resident.project.tracks[0].clips[0].repeat = None;
    let mut streamed = resident.clone();
    let audio = &resident.audio[&id];
    let mut writer = DiskAudioWriter::new(audio.frame_count()).unwrap();
    writer.write(&audio.samples).unwrap();
    streamed
        .audio
        .insert(id, writer.finish(rate, audio.metadata.clone()).unwrap());
    (resident, streamed)
}
#[test]
fn disk_pcm_matches_resident_offline_and_live_with_seeks_loops_and_latency() {
    for rate in SAMPLE_RATES {
        for effects in [0, 1] {
            let (mut resident, mut streamed) = plan(rate, effects);
            for start in [0, STREAM_CHUNK_FRAMES as u64 - 19, 509, u64::from(rate)] {
                let mut a = Renderer::new(resident.clone());
                let mut b = Renderer::new(streamed.clone());
                let mut expected = vec![[0.; 2]; 1024];
                let mut actual = expected.clone();
                a.render_offline_at(start, &mut expected);
                b.render_offline_at(start, &mut actual);
                assert_eq!(actual, expected, "offline {rate} {effects} {start}");
                assert!(b.source_error().is_none());
            }
            let region = Loop {
                enabled: true,
                start_frame: 513,
                end_frame: u64::from(rate) + 3000,
            };
            resident.project.transport.r#loop = region.clone();
            streamed.project.transport.r#loop = region;
            let mut expected = Renderer::new(resident);
            let mut actual = Renderer::new(streamed);
            actual.prepare_streaming().unwrap();
            for start in [0, u64::from(rate) - 10, 400] {
                COUNT.with(|c| c.set(Some(0)));
                actual.seek(start);
                actual.play();
                assert_eq!(COUNT.with(|c| c.replace(None)), Some(0));
                expected.seek(start);
                expected.play();
                let deadline = Instant::now() + Duration::from_secs(15);
                for _ in 0..rate / 2 + 5000 {
                    let sample = loop {
                        COUNT.with(|c| c.set(Some(0)));
                        let sample = actual.next_sample();
                        assert_eq!(COUNT.with(|c| c.replace(None)), Some(0));
                        if !actual.buffering() {
                            break sample;
                        }
                        assert!(Instant::now() < deadline, "disk read did not complete");
                        std::thread::sleep(Duration::from_millis(1));
                    };
                    assert_eq!(
                        sample,
                        expected.next_sample(),
                        "live {rate} {effects} at {}",
                        actual.playhead
                    );
                    assert_eq!(actual.playhead, expected.playhead);
                }
            }
        }
    }
}

#[test]
fn crossfades_stream_both_sources_without_callback_allocations_and_match_resident_audio() {
    for mask in 0..4 {
        for left in [false, true] {
            let rate = 48_000;
            let (mut resident, mut streamed) = plan(rate, 0);
            for plan in [&mut resident, &mut streamed] {
                let clips = &mut plan.project.tracks[0].clips;
                let mut first = clips[0].clone();
                first.length_frames = 24_000;
                let mut second = first.clone();
                second.id = Id::new_v4();
                second.start_frame = first.end();
                second.source_offset_frame += first.length_frames;
                if mask & 1 != 0 {
                    first.repeat = Some(ClipLoop {
                        length_frames: 257,
                        phase_frame: 193,
                    });
                }
                if mask & 2 != 0 {
                    second.repeat = Some(ClipLoop {
                        length_frames: 389,
                        phase_frame: 71,
                    });
                }
                first.channels = ClipChannels {
                    left: true,
                    right: false,
                    inverted: true,
                    swapped: true,
                };
                second.channels = ClipChannels {
                    left: false,
                    right: true,
                    inverted: false,
                    swapped: false,
                };
                let fade = ClipFade {
                    length_frames: 256,
                    midpoint: 0.75,
                    midpoint_position: 0.35,
                    crossfade: true,
                };
                if left {
                    second.fades.fade_in = fade;
                } else {
                    first.fades.fade_out = fade;
                }
                *clips = vec![first, second];
                plan.project.transport.r#loop = Loop {
                    enabled: true,
                    start_frame: 23_700,
                    end_frame: 24_300,
                };
            }
            let mut expected = Renderer::new(resident);
            let mut actual = Renderer::new(streamed);
            actual.prepare_streaming().unwrap();
            for start in [23_700, 24_050] {
                actual.seek(start);
                actual.play();
                expected.seek(start);
                expected.play();
                let deadline = Instant::now() + Duration::from_secs(15);
                for _ in 0..900 {
                    let sample = loop {
                        COUNT.with(|c| c.set(Some(0)));
                        let sample = actual.next_sample();
                        assert_eq!(COUNT.with(|c| c.replace(None)), Some(0));
                        if !actual.buffering() {
                            break sample;
                        }
                        assert!(Instant::now() < deadline);
                        std::thread::sleep(Duration::from_millis(1));
                    };
                    assert_eq!(sample, expected.next_sample());
                    assert_eq!(actual.playhead, expected.playhead);
                }
            }
        }
    }
}
