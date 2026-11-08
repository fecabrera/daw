use daw_core::{BeatEdit, BeatSubtrack, Edit, Id};
use daw_project::Session;
use std::{fs, path::PathBuf};

struct Folder(PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn beat_folder_disclosure_survives_save_and_reopen_in_both_states() {
    let folder = Folder(std::env::temp_dir().join(format!("daw-beat-folder-{}", Id::new_v4())));
    let mut session = Session::default();
    let parent = session.project.add_beat_track().unwrap();
    let row = BeatSubtrack::new("Kick".into());
    let child = row.id;
    session
        .project
        .edit(Edit::Beat {
            track_id: parent,
            edit: BeatEdit::Add {
                subtrack: Box::new(row),
                index: 0,
            },
        })
        .unwrap();
    let clip = session.project.add_beat_pattern(parent, 0).unwrap();
    for collapsed in [false, true, false] {
        session
            .project
            .edit(Edit::SetGroupCollapsed {
                track_id: parent,
                collapsed,
            })
            .unwrap();
        let expected = session.project.clone();
        session.save(&folder.0).unwrap();
        session = Session::open_document(&folder.0)
            .unwrap()
            .into_editor_session();
        assert_eq!(session.project.tracks, expected.tracks);
        assert_eq!(session.project.tracks[0].group_collapsed, collapsed);
        assert_eq!(session.project.tracks[0].id, parent);
        assert_eq!(session.project.tracks[0].clips[0].id, clip);
        assert_eq!(session.project.beat(parent).unwrap().subtracks[0].id, child);
    }
}
