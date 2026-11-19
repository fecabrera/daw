use daw_plugin_support::allocation_probe::Allocator;
pub use daw_plugin_support::allocation_probe::assert_no_heap as measured;

#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
