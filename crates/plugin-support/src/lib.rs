//! Format-neutral ABI containment, atomic publication, and bounded text helpers.
use std::{
    ffi::c_char,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::atomic::{AtomicU64, Ordering},
};

/// No panic may escape a C callback. Unexpected panic payloads are deliberately
/// leaked, rather than retiring an arbitrary final owner on the audio thread.
/// Validated normal paths do not panic; this is a last-resort ABI containment.
pub fn boundary<T>(fallback: T, call: impl FnOnce() -> T) -> T {
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(value) => value,
        Err(payload) => {
            std::mem::forget(payload);
            fallback
        }
    }
}

/// Fixed-size atomic publication with one externally serialized writer. Readers
/// attempt once: a concurrent publication returns None, never a partial snapshot.
/// SeqCst on every word establishes one total order for the two version checks.
pub struct Snapshot<const N: usize> {
    version: AtomicU64,
    words: [AtomicU64; N],
}
impl<const N: usize> Snapshot<N> {
    pub fn new(words: [u64; N]) -> Self {
        Self {
            version: AtomicU64::new(0),
            words: words.map(AtomicU64::new),
        }
    }
    /// Caller must serialize writers. Returns the completed even revision.
    pub fn publish(&self, words: [u64; N]) -> u64 {
        self.version.fetch_add(1, Ordering::SeqCst);
        for (target, value) in self.words.iter().zip(words) {
            target.store(value, Ordering::SeqCst);
        }
        self.version.fetch_add(1, Ordering::SeqCst).wrapping_add(1)
    }
    pub fn read(&self) -> Option<(u64, [u64; N])> {
        let before = self.version.load(Ordering::SeqCst);
        if before & 1 != 0 {
            return None;
        }
        let words = std::array::from_fn(|i| self.words[i].load(Ordering::SeqCst));
        (before == self.version.load(Ordering::SeqCst)).then_some((before, words))
    }
}

/// Copy UTF-8 without splitting a code point. The destination always ends in NUL.
pub fn write_text(text: &str, destination: &mut [c_char]) -> bool {
    if destination.is_empty() {
        return false;
    }
    let mut length = text.len().min(destination.len() - 1);
    while !text.is_char_boundary(length) {
        length -= 1;
    }
    for (target, value) in destination.iter_mut().zip(text.bytes().take(length)) {
        *target = value as c_char;
    }
    destination[length] = 0;
    length == text.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_are_coherent_under_concurrent_publication() {
        let snapshot = std::sync::Arc::new(Snapshot::new([0; 8]));
        let reader = snapshot.clone();
        let thread = std::thread::spawn(move || {
            for _ in 0..100000 {
                if let Some((_, words)) = reader.read() {
                    assert!(words.iter().all(|v| *v == words[0]));
                }
            }
        });
        for value in 1..100000 {
            snapshot.publish([value; 8]);
        }
        thread.join().unwrap();
        assert_eq!(snapshot.read().unwrap().1, [99999; 8]);
    }
    #[test]
    fn text_copy_is_bounded_and_does_not_split_utf8() {
        let mut text = [42; 3];
        assert!(!write_text("−∞", &mut text));
        assert_eq!(text[0], 0);
    }
}
