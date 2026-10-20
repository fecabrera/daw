use daw_core::*;
fn fixture() -> (Project, [Id; 4]) {
    let mut p = Project::default();
    let ids = std::array::from_fn(|_| p.add_track().unwrap());
    (p, ids)
}
fn group(p: &mut Project, ids: &[Id]) -> Id {
    let id = Id::new_v4();
    p.edit(Edit::GroupTracks {
        track_ids: ids.to_vec(),
        group_id: id,
        name: "Bus".into(),
    })
    .unwrap();
    id
}
#[test]
fn grouping_nested_moves_ungroup_and_delete_preserve_stable_children() {
    let (mut p, ids) = fixture();
    let inner = group(&mut p, &ids[0..2]);
    let outer = group(&mut p, &[inner, ids[3]]);
    assert_eq!(
        p.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
        [outer, inner, ids[0], ids[1], ids[3], ids[2]]
    );
    let original = p.clone();
    assert!(
        p.edit(Edit::MoveTrack {
            track_id: outer,
            group_id: Some(inner),
            index: 1
        })
        .is_err()
    );
    assert_eq!(p, original);
    p.edit(Edit::MoveTrack {
        track_id: ids[2],
        group_id: Some(inner),
        index: 4,
    })
    .unwrap();
    assert_eq!(p.tracks[4].group_id, Some(inner));
    p.edit(Edit::SetGroupCollapsed {
        track_id: inner,
        collapsed: true,
    })
    .unwrap();
    assert_eq!(p.visible_tracks(), [0, 1, 5]);
    p.edit(Edit::UngroupTrack { track_id: inner }).unwrap();
    assert!(p.tracks[1..].iter().all(|t| t.group_id == Some(outer)));
    p.edit(Edit::DeleteTrack(outer)).unwrap();
    assert!(p.tracks.is_empty());
}
#[test]
fn nonadjacent_grouping_and_cross_parent_extraction_are_atomic() {
    let (mut p, ids) = fixture();
    let first = group(&mut p, &ids[..2]);
    let second = group(&mut p, &ids[2..]);
    let common = group(&mut p, &[ids[0], ids[3]]);
    assert_eq!(
        p.tracks.iter().map(|t| t.id).collect::<Vec<_>>(),
        [common, ids[0], ids[3], first, ids[1], second, ids[2]]
    );
    p.validate().unwrap();
    let before = p.clone();
    assert!(
        p.edit(Edit::Batch(vec![
            Edit::SetTrackMuted {
                track_id: common,
                muted: true
            },
            Edit::MoveTrack {
                track_id: first,
                group_id: Some(ids[1]),
                index: 0
            }
        ]))
        .is_err()
    );
    assert_eq!(p, before);
}
#[test]
fn hierarchy_rejects_invalid_parents_split_subtrees_and_sources() {
    let (mut p, ids) = fixture();
    let bus = group(&mut p, &ids[..2]);
    for bad in [ids[2], Id::new_v4(), ids[0]] {
        let mut invalid = p.clone();
        invalid.tracks[1].group_id = Some(bad);
        assert!(invalid.validate().is_err());
    }
    let mut split = p.clone();
    split.tracks.swap(2, 3);
    assert!(split.validate().is_err());
    let mut source = p.clone();
    source.tracks[0].instrument = Some(InstrumentSettings::default());
    assert!(source.validate().is_err());
    let mut cycle = p.clone();
    cycle.tracks[0].group_id = Some(bus);
    assert!(cycle.validate().is_err());
}
#[test]
fn solo_opens_ancestor_path_without_overriding_child_mutes() {
    let (mut p, ids) = fixture();
    let bus = group(&mut p, &ids[..2]);
    p.edit(Edit::SetTrackMuted {
        track_id: bus,
        muted: true,
    })
    .unwrap();
    assert!(!p.audible(&p.tracks[1]));
    p.edit(Edit::SetTrackSoloed {
        track_id: ids[0],
        soloed: true,
    })
    .unwrap();
    assert_eq!(p.track_mix_audible(), [true, true, false, false, false]);
    assert!(p.audible(&p.tracks[1]));
    p.edit(Edit::SetTrackSoloed {
        track_id: ids[0],
        soloed: false,
    })
    .unwrap();
    p.edit(Edit::SetTrackSoloed {
        track_id: bus,
        soloed: true,
    })
    .unwrap();
    p.edit(Edit::SetTrackMuted {
        track_id: ids[1],
        muted: true,
    })
    .unwrap();
    assert_eq!(p.track_mix_audible(), [true, true, false, false, false]);
}
#[test]
fn copies_have_new_track_and_effect_ids_and_legacy_tracks_remain_flat() {
    let (mut p, ids) = fixture();
    let bus = group(&mut p, &ids[..2]);
    p.tracks[0].effects.push(EffectInstance::drive());
    let copy = Id::new_v4();
    p.edit(Edit::DuplicateTrack {
        track_id: bus,
        new_track_id: copy,
    })
    .unwrap();
    assert_eq!(p.tracks[3].id, copy);
    assert_eq!(p.tracks[4].group_id, Some(copy));
    assert_ne!(p.tracks[3].effects[0].id, p.tracks[0].effects[0].id);
    assert_ne!(p.tracks[4].id, ids[0]);
    let restored: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(restored, p);
    let (p, _) = fixture();
    let json = serde_json::to_value(&p).unwrap();
    assert!(json["tracks"][0].get("group_id").is_none());
    assert_eq!(serde_json::from_value::<Project>(json).unwrap(), p);
}
