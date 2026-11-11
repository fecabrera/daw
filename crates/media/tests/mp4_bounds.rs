use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! { static GUARDED: Cell<bool> = const { Cell::new(false) }; }
struct Allocator;
fn check(size: usize) {
    if size > 64 * 1024 * 1024 && GUARDED.with(|v| v.get()) {
        // A regression must fail before the oversized allocation reaches the host.
        std::process::abort();
    }
}
// SAFETY: Layouts/pointers are forwarded unchanged to System. The guard stops
// before allocation and cannot return an invalid pointer.
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        check(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        check(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        check(size);
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
struct Guard;
impl Guard {
    fn new() -> Self {
        GUARDED.with(|v| v.set(true));
        Self
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        GUARDED.with(|v| v.set(false));
    }
}
fn fixture() -> Vec<u8> {
    include_bytes!("fixtures/tone-2.m4a").to_vec()
}
fn atom(bytes: &[u8], name: &[u8; 4]) -> usize {
    bytes.windows(4).position(|v| v == name).unwrap()
}
struct Scratch(std::path::PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("daw-mp4-bounds-{}", daw_core::Id::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn rejects(bytes: &[u8]) -> String {
    let dir = Scratch::new();
    let path = dir.0.join("input.m4a");
    std::fs::write(&path, bytes).unwrap();
    let _guard = Guard::new();
    daw_media::decode_audio_at_rate(&path, 48000)
        .err()
        .expect("malformed MP4 accepted")
        .to_string()
}
#[test]
fn tiny_packet_and_table_declarations_reject_before_large_allocation() {
    let original = fixture();
    for table in [b"stsz", b"stco", b"stsc", b"stts"] {
        let mut bytes = original.clone();
        let pos = atom(&bytes, table);
        let field = pos + if table == b"stsz" { 12 } else { 8 };
        bytes[field..field + 4].copy_from_slice(&0x30000000_u32.to_be_bytes());
        assert!(rejects(&bytes).contains("table"), "{table:?}");
    }
    let mut bytes = original;
    let pos = atom(&bytes, b"stsz");
    assert_eq!(&bytes[pos + 8..pos + 12], &[0; 4]);
    bytes[pos + 16..pos + 20].copy_from_slice(&0xc0000000_u32.to_be_bytes());
    let error = rejects(&bytes);
    assert!(
        error.contains("packet") || error.contains("extent"),
        "{error}"
    );
}
#[test]
fn atom_file_extents_and_aggregate_metadata_are_checked_before_parsing() {
    let mut bytes = fixture();
    let pos = atom(&bytes, b"moov");
    bytes[pos - 4..pos].copy_from_slice(&0xfffffff0_u32.to_be_bytes());
    assert!(rejects(&bytes).contains("extent"));
    let mut bytes = fixture();
    bytes[..4].copy_from_slice(&1_u32.to_be_bytes());
    bytes[8..16].copy_from_slice(&u64::MAX.to_be_bytes());
    assert!(rejects(&bytes).contains("extent"));
    let original = fixture();
    let pos = atom(&original, b"moov");
    let extent = symphonia::default::formats::IsoMp4Reader::MAX_METADATA_BYTES as usize + 8;
    let mut bytes = original[..pos - 4].to_vec();
    bytes.extend_from_slice(&(extent as u32).to_be_bytes());
    bytes.extend_from_slice(b"moov");
    bytes.resize(pos - 4 + extent, 0);
    assert!(rejects(&bytes).contains("metadata limit"));
}
#[test]
fn ordinary_m4a_retains_repeatable_pcm_and_channels() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone-2.m4a");
    let _guard = Guard::new();
    let first = daw_media::decode_audio_at_rate(&path, 48000).unwrap();
    let second = daw_media::decode_audio_at_rate(&path, 48000).unwrap();
    assert_eq!(first.samples, second.samples);
    assert_eq!(first.metadata.channels, 2);
    assert!(first.samples.iter().flatten().all(|v| v.is_finite()));
    assert!(first.samples.iter().any(|v| v[0] != v[1]));
}

fn boxed(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(payload);
    bytes
}
#[test]
fn related_chunk_index_and_fragment_tables_reject_invalid_extents() {
    let original = fixture();
    let mut bytes = original.clone();
    let pos = atom(&bytes, b"stco");
    bytes[pos..pos + 4].copy_from_slice(b"co64");
    bytes[pos + 8..pos + 12].copy_from_slice(&0x30000000_u32.to_be_bytes());
    assert!(rejects(&bytes).contains("table"));
    let mut bytes = original.clone();
    let pos = atom(&bytes, b"stsc");
    bytes[pos + 12..pos + 16].copy_from_slice(&0_u32.to_be_bytes());
    assert!(rejects(&bytes).contains("first chunk"));
    let mut payload = vec![1, 0, 0, 0]; // version 1
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&48000_u32.to_be_bytes());
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&u64::MAX.to_be_bytes());
    payload.extend_from_slice(&[0; 4]); // reserved/reference count
    let sidx = boxed(b"sidx", &payload);
    let pos = atom(&original, b"moov") - 4;
    let mut bytes = original[..pos].to_vec();
    bytes.extend_from_slice(&sidx);
    bytes.extend_from_slice(&original[pos..]);
    assert!(rejects(&bytes).contains("offset overflow"));
    let tfhd = boxed(b"tfhd", &[0, 0, 0, 0, 0, 0, 0, 1]);
    let trun = boxed(b"trun", &[0, 0, 2, 0, 0x30, 0, 0, 0]);
    let mut traf = tfhd;
    traf.extend_from_slice(&trun);
    let mut moof = boxed(b"mfhd", &[0, 0, 0, 0, 0, 0, 0, 1]);
    moof.extend_from_slice(&boxed(b"traf", &traf));
    let mut bytes = original;
    bytes.extend_from_slice(&boxed(b"moof", &moof));
    let error = rejects(&bytes);
    assert!(error.contains("fragment"), "{error}");
}
