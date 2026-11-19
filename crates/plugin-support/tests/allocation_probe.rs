use daw_plugin_support::allocation_probe::{Allocator, Region, assert_no_heap, probe};

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;

#[test]
fn regions_detect_heap_operations_and_clear_after_unwind() {
    let region = Region::begin();
    probe();
    assert_eq!(region.finish(), [1, 1]);

    assert!(
        std::panic::catch_unwind(|| assert_no_heap(|| panic!("interrupt measurement"))).is_err()
    );
    assert_eq!(assert_no_heap(|| 42), 42);
}

#[test]
fn region_observes_only_the_calling_thread() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let start = AtomicBool::new(false);
    let done = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let thread = scope.spawn(|| {
            while !start.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            probe();
            done.store(true, Ordering::Release);
        });
        let region = Region::begin();
        start.store(true, Ordering::Release);
        while !done.load(Ordering::Acquire) {
            std::hint::spin_loop();
        }
        let counts = region.finish();
        thread.join().unwrap();
        assert_eq!(counts, [0; 2]);
    });
}
