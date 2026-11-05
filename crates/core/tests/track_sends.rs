use daw_core::*;
fn fixture() -> (Project, Id, Id) {
    let mut p = Project::default();
    let source = p.add_track().unwrap();
    let ret = p.add_return_track().unwrap();
    (p, source, ret)
}
fn connect(p: &mut Project, s: Id, r: Id) {
    p.edit(Edit::SetTrackSends {
        track_id: s,
        sends: vec![TrackSend::new(r)],
    })
    .unwrap();
}
#[test]
fn defaults_persistence_duplicates_and_transactional_removal() {
    assert!(Project::default().tracks.is_empty());
    let (mut p, s, r) = fixture();
    connect(&mut p, s, r);
    let send = p.tracks[0].sends[0].clone();
    assert_eq!(send.gain_db, SILENT_GAIN_DB);
    assert!(send.enabled);
    assert_eq!(send.tap, SendTap::PostFader);
    let saved = serde_json::to_string(&p).unwrap();
    let restored: Project = serde_json::from_str(&saved).unwrap();
    assert_eq!(p, restored);
    restored.validate().unwrap();
    let copy = Id::new_v4();
    p.edit(Edit::DuplicateTrack {
        track_id: s,
        new_track_id: copy,
    })
    .unwrap();
    assert_ne!(p.tracks[1].sends[0].id, send.id);
    assert_eq!(p.tracks[1].sends[0].return_id, r);
    let retcopy = Id::new_v4();
    p.edit(Edit::DuplicateTrack {
        track_id: r,
        new_track_id: retcopy,
    })
    .unwrap();
    assert!(
        p.tracks
            .iter()
            .find(|t| t.id == retcopy)
            .unwrap()
            .sends
            .is_empty()
    );
    assert!(
        p.tracks
            .iter()
            .flat_map(|t| &t.sends)
            .all(|s| s.return_id == r)
    );
    let before = p.clone();
    p.edit(Edit::DeleteTrack(r)).unwrap();
    assert!(p.tracks.iter().all(|t| t.sends.is_empty()));
    assert_eq!(before.tracks[0].sends[0], send);
    let mut legacy = serde_json::to_value(Project::default()).unwrap();
    legacy["tracks"] = serde_json::to_value(&before.tracks[..1]).unwrap();
    legacy["tracks"][0].as_object_mut().unwrap().remove("sends");
    let legacy: Project = serde_json::from_value(legacy).unwrap();
    assert!(legacy.tracks[0].sends.is_empty());
    legacy.validate().unwrap();
}
#[test]
fn invalid_graphs_fail_atomically_and_returns_reorder_without_redirecting() {
    let (mut p, s, r) = fixture();
    connect(&mut p, s, r);
    let original = p.clone();
    for sends in [
        vec![TrackSend::new(s)],
        vec![TrackSend::new(r), TrackSend::new(r)],
        vec![TrackSend {
            gain_db: f32::NAN,
            ..TrackSend::new(r)
        }],
        vec![TrackSend {
            id: Id::nil(),
            ..TrackSend::new(r)
        }],
    ] {
        assert!(p.edit(Edit::SetTrackSends { track_id: s, sends }).is_err());
        assert_eq!(p, original);
    }
    assert!(
        p.edit(Edit::SetTrackSends {
            track_id: r,
            sends: vec![TrackSend::new(r)]
        })
        .is_err()
    );
    assert_eq!(p, original);
    assert!(
        p.edit(Edit::GroupTracks {
            track_ids: vec![s, r],
            group_id: Id::new_v4(),
            name: "Bad".into()
        })
        .is_err()
    );
    assert_eq!(p, original);
    let r2 = p.add_return_track().unwrap();
    p.edit(Edit::ReorderReturns {
        return_ids: vec![r2, r],
    })
    .unwrap();
    assert_eq!(p.tracks[0].id, s);
    assert_eq!(p.tracks[0].sends[0].return_id, r);
    let original = p.clone();
    assert!(
        p.edit(Edit::ReorderReturns {
            return_ids: vec![r, r]
        })
        .is_err()
    );
    assert_eq!(p, original);
}
#[test]
fn mute_does_not_gate_sends_and_source_and_return_solos_have_separate_domains() {
    let (mut p, a, r) = fixture();
    let b = p.add_track().unwrap();
    connect(&mut p, a, r);
    connect(&mut p, b, r);
    p.tracks[0].muted = true;
    assert_eq!(p.track_send_audible(), [true, false, true]);
    assert_eq!(p.track_mix_audible(), [false, true, true]);
    p.tracks[0].soloed = true;
    assert_eq!(p.track_send_audible(), [true, false, false]);
    assert_eq!(p.track_mix_audible(), [true, true, false]);
    p.tracks[0].soloed = false;
    p.tracks[1].soloed = true;
    assert_eq!(p.track_send_audible(), [true, false, true]);
    assert_eq!(p.track_mix_audible(), [false, true, false]);
    let r2 = p.add_return_track().unwrap();
    p.tracks[0].soloed = true;
    assert!(!p.track_mix_audible()[3]);
    assert_eq!(p.tracks[3].id, r2);
    let group = Id::new_v4();
    p.edit(Edit::GroupTracks {
        track_ids: vec![a, b],
        group_id: group,
        name: "Group".into(),
    })
    .unwrap();
    p.tracks[0].muted = true;
    let mask = p.track_send_audible();
    assert!(mask[0]);
    assert!(mask[1]);
    assert!(!mask[2]);
}
