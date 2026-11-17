//! Nonblocking descriptor readiness for native editor main-thread services.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct PollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}
unsafe extern "C" {
    fn poll(descriptors: *mut PollFd, count: usize, timeout: i32) -> i32;
}
pub(super) fn ready(descriptors: &mut [PollFd]) -> bool {
    (unsafe { poll(descriptors.as_mut_ptr(), descriptors.len(), 0) }) >= 0
}
