use daw_core::{ClipLoop, Edit, Id, MidiNote, MidiNoteSplit, MidiPattern, Project};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {
    static COUNT: Cell<bool> = const { Cell::new(false) };
    static BYTES: Cell<usize> = const { Cell::new(0) };
}
struct Allocator;
// SAFETY: All layouts and pointers are forwarded unchanged to System.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|on| {
            if on.get() {
                BYTES.with(|n| n.set(n.get() + layout.size()));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNT.with(|on| {
            if on.get() {
                BYTES.with(|n| n.set(n.get() + size));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn measured<T>(f: impl FnOnce() -> T) -> (T, usize) {
    BYTES.with(|n| n.set(0));
    COUNT.with(|n| n.set(true));
    let value = f();
    COUNT.with(|n| n.set(false));
    (value, BYTES.with(Cell::get))
}
fn project(length: u64, repeat: u64) -> (Project, Id) {
    let mut project = Project::default();
    let track = project.add_midi_track().unwrap();
    let id = project.add_midi_pattern(track, 0).unwrap();
    project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: id,
            pattern: MidiPattern {
                length_ticks: 960,
                notes: vec![MidiNote {
                    id: Id::new_v4(),
                    start_tick: 0,
                    length_ticks: 960,
                    pitch: 60,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    let clip = &mut project.tracks[0].clips[0];
    clip.length_frames = length;
    clip.repeat = Some(ClipLoop {
        length_frames: repeat,
        phase_frame: 0,
    });
    project.validate().unwrap();
    (project, id)
}
#[test]
fn geometry_split_storage_does_not_grow_with_loop_duration() {
    for mode in MidiNoteSplit::ALL {
        let mut allocations = Vec::new();
        for length in [480_000, 2_880_000_000] {
            let (project, id) = project(length, 1);
            let ((next, created), bytes) =
                measured(|| project.split_clips(&[id], length / 2, mode).unwrap());
            assert_eq!(next.assets, project.assets);
            assert_eq!(created.len(), 1);
            assert_eq!(next.tracks[0].clips.len(), 2);
            assert!(bytes < 64 * 1024, "geometry split allocated {bytes} bytes");
            allocations.push(bytes);
        }
        assert_eq!(allocations[0], allocations[1]);
    }
}
#[test]
fn excessive_unfolding_is_rejected_without_expansion_or_mutation() {
    let (project, id) = project(480_000, 2);
    let original = project.clone();
    for mode in MidiNoteSplit::ALL {
        let (result, bytes) = measured(|| project.split_clips(&[id], 240_001, mode));
        assert!(result.unwrap_err().0.contains("loop-occurrence budget"));
        assert!(bytes < 64 * 1024, "rejected split allocated {bytes} bytes");
        assert_eq!(project, original);
    }
}

#[test]
fn unfolding_budget_is_shared_by_the_selected_group() {
    let (mut project, id) = project(4098, 2);
    let mut second = project.tracks[0].clips[0].clone();
    second.id = Id::new_v4();
    let second_id = second.id;
    project.add_midi_track().unwrap();
    project.tracks[1].clips.push(second);
    project.validate().unwrap();
    for mode in MidiNoteSplit::ALL {
        assert!(project.split_clips(&[id], 1, mode).is_ok());
        let original = project.clone();
        assert!(
            project
                .split_clips(&[id, second_id], 1, mode)
                .unwrap_err()
                .0
                .contains("loop-occurrence budget")
        );
        assert_eq!(project, original);
    }
}
