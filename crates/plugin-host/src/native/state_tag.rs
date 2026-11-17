//! Fixed atomic identity of the opaque state currently installed in a native
//! instance. State capture can refresh this identity without cloning audio DSP.
use super::*;
use daw_unit_core::HostedStateReference;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NativeStateTag([u64; 4]);
impl NativeStateTag {
    fn combine(component: [u8; 32], length: u64, controller: Option<([u8; 32], u64)>) -> Self {
        let mut hash = Sha256::new();
        hash.update(component);
        hash.update(length.to_le_bytes());
        if let Some((controller, length)) = controller.filter(|(_, length)| *length > 0) {
            hash.update(controller);
            hash.update(length.to_le_bytes());
        }
        let bytes = hash.finalize();
        Self(std::array::from_fn(|i| {
            u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().expect("digest word"))
        }))
    }
    pub fn from_state(state: &NativeState) -> Self {
        match state {
            NativeState::Clap(bytes) => {
                Self::combine(Sha256::digest(bytes).into(), bytes.len() as u64, None)
            }
            NativeState::Vst3 {
                component,
                controller,
            } => Self::combine(
                Sha256::digest(component).into(),
                component.len() as u64,
                Some((Sha256::digest(controller).into(), controller.len() as u64)),
            ),
        }
    }
    pub fn from_reference(state: &HostedStateReference) -> Option<Self> {
        fn decode(text: &str) -> Option<[u8; 32]> {
            if text.len() != 64 {
                return None;
            }
            let mut bytes = [0u8; 32];
            for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
                let digit = |b: u8| match b {
                    b'0'..=b'9' => Some(b - b'0'),
                    b'a'..=b'f' => Some(b - b'a' + 10),
                    _ => None,
                };
                bytes[index] = (digit(pair[0])? << 4) | digit(pair[1])?;
            }
            Some(bytes)
        }
        let controller = match &state.controller {
            Some(blob) => Some((decode(&blob.sha256)?, blob.length_bytes)),
            None => None,
        };
        Some(Self::combine(
            decode(&state.component.sha256)?,
            state.component.length_bytes,
            controller,
        ))
    }
}
pub(super) struct AtomicStateTag {
    sequence: AtomicU64,
    present: AtomicBool,
    words: [AtomicU64; 4],
}
impl AtomicStateTag {
    pub fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            present: AtomicBool::new(false),
            words: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
    pub fn store(&self, tag: Option<NativeStateTag>) {
        self.sequence.fetch_add(1, Ordering::SeqCst);
        if let Some(tag) = tag {
            for (word, value) in self.words.iter().zip(tag.0) {
                word.store(value, Ordering::SeqCst);
            }
        }
        self.present.store(tag.is_some(), Ordering::SeqCst);
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    pub fn load(&self) -> Option<NativeStateTag> {
        for _ in 0..2 {
            let first = self.sequence.load(Ordering::SeqCst);
            if first & 1 != 0 {
                continue;
            }
            let present = self.present.load(Ordering::SeqCst);
            let words = std::array::from_fn(|i| self.words[i].load(Ordering::SeqCst));
            if first == self.sequence.load(Ordering::SeqCst) {
                return present.then_some(NativeStateTag(words));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_and_manifest_tags_match_without_treating_edit_revisions_as_presets() {
        for state in [
            NativeState::Clap(vec![1, 2, 255]),
            NativeState::Vst3 {
                component: vec![0, 4],
                controller: vec![9],
            },
            NativeState::Vst3 {
                component: vec![1],
                controller: vec![],
            },
        ] {
            let mut bank = crate::StateBank::default();
            let reference = bank.capture(state.clone(), 7).unwrap();
            let tag = NativeStateTag::from_reference(&reference).unwrap();
            assert!(tag == NativeStateTag::from_state(&state));
            let mut later = reference;
            later.revision = 12;
            assert!(NativeStateTag::from_reference(&later) == Some(tag));
            let atomic = AtomicStateTag::new();
            atomic.store(Some(tag));
            assert!(atomic.load() == Some(tag));
            atomic.store(None);
            assert!(atomic.load().is_none());
        }
    }
}
