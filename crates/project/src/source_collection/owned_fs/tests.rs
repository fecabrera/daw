use super::*;
use std::io::Write;
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("daw-store-handles-{}", daw_core::Id::new_v4()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn no_hook(_: IoStep, _: &Path) -> io::Result<()> {
    Ok(())
}
#[test]
fn completed_files_and_directories_share_one_bounded_handle_budget() {
    let f = Fixture::new();
    let mut store = Store::new(&f.0).unwrap();
    store
        .reserve_directory(Path::new("assets/.private"), true, &mut no_hook)
        .unwrap();
    let raw = store
        .reserve_file(Path::new("assets/.private/raw.wav"), true, &mut no_hook)
        .unwrap()
        .unwrap();
    for i in 0..4 * MAX_RETAINED_HANDLES {
        let dir = PathBuf::from(format!("stems/group-{i}"));
        assert!(store.reserve_directory(&dir, false, &mut no_hook).unwrap());
        let index = store
            .reserve_file(&dir.join("source.wav"), false, &mut no_hook)
            .unwrap()
            .unwrap();
        store
            .file_mut(index)
            .unwrap()
            .write_all(b"complete")
            .unwrap();
        store.seal_file(index).unwrap();
        assert!(store.live_handles <= MAX_RETAINED_HANDLES);
    }
    assert!(
        store.files[raw].file.is_some(),
        "active private proof must stay pinned"
    );
    store.remove_temporary(raw).unwrap();
    assert!(store.files[raw].removed);
    for (index, entry) in store.files.iter().enumerate() {
        if !entry.removed {
            store.check_file(index).unwrap();
        }
    }
    store.mode = CleanupMode::Committed;
    assert!(store.cleanup().is_empty());
    assert!(!f.0.join("assets/.private").exists());
}
#[test]
fn rollback_retains_evicted_file_replacements_and_the_old_manifest() {
    let f = Fixture::new();
    fs::write(f.0.join("project.json"), b"old manifest").unwrap();
    let mut store = Store::new(&f.0).unwrap();
    for i in 0..2 * MAX_RETAINED_HANDLES {
        let index = store
            .reserve_file(
                Path::new(&format!("assets/sources/{i}.wav")),
                false,
                &mut no_hook,
            )
            .unwrap()
            .unwrap();
        store
            .file_mut(index)
            .unwrap()
            .write_all(b"complete")
            .unwrap();
        store.seal_file(index).unwrap();
    }
    let first = store.files[0].path.clone();
    assert!(store.files[0].file.is_none());
    fs::remove_file(&first).unwrap();
    fs::write(&first, b"foreign replacement").unwrap();
    let last = store.files.last().unwrap().path.clone();
    assert!(
        store
            .cleanup()
            .iter()
            .any(|w| w.contains("handle was released"))
    );
    assert_eq!(fs::read(first).unwrap(), b"foreign replacement");
    assert!(
        !last.exists(),
        "still-pinned reservations should roll back normally"
    );
    assert_eq!(fs::read(f.0.join("project.json")).unwrap(), b"old manifest");
}
#[test]
fn rollback_retains_an_evicted_directory_and_a_reused_private_path() {
    let f = Fixture::new();
    let mut store = Store::new(&f.0).unwrap();
    for i in 0..2 * MAX_RETAINED_HANDLES {
        store
            .reserve_directory(Path::new(&format!("group-{i}")), false, &mut no_hook)
            .unwrap();
    }
    let first = store.dirs[0].path.clone();
    assert!(store.dirs[0].file.is_none());
    fs::remove_dir(&first).unwrap();
    fs::create_dir(&first).unwrap();
    let temporary = Path::new("proof.wav");
    let index = store
        .reserve_file(temporary, true, &mut no_hook)
        .unwrap()
        .unwrap();
    store.remove_temporary(index).unwrap();
    fs::write(f.0.join(temporary), b"unrelated new file").unwrap();
    let warnings = store.cleanup();
    assert!(first.exists());
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("directory handle was released"))
    );
    assert_eq!(
        fs::read(f.0.join(temporary)).unwrap(),
        b"unrelated new file"
    );
}
