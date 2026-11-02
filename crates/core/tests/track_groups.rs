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

#[test]
fn empty_group_creation_is_an_independent_root_with_normal_membership_and_storage() {
    let (mut project, ids) = fixture();
    group(&mut project, &ids[..2]);
    let original = project.clone();
    let id = project.add_group_track().unwrap();
    assert_eq!(&project.tracks[..original.tracks.len()], &original.tracks);
    let bus = project.tracks.last().unwrap();
    assert_eq!(bus.id, id);
    assert_eq!(bus.name, "Group 2");
    assert_eq!(bus.kind, TrackKind::Group);
    assert!(bus.group_id.is_none());
    assert!(!bus.group_collapsed && !bus.muted && !bus.soloed);
    assert!(bus.clips.is_empty() && bus.effects.is_empty());
    assert!(bus.instrument.is_none() && bus.beat.is_none());
    assert_eq!((bus.gain_db, bus.pan), (0.0, 0.0));
    let index = project.tracks.len() - 1;
    project
        .edit(Edit::MoveTrack {
            track_id: ids[2],
            group_id: Some(id),
            index,
        })
        .unwrap();
    project.validate().unwrap();
    assert_eq!(project.tracks.last().unwrap().group_id, Some(id));
    let restored: Project =
        serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
    assert_eq!(restored, project);
}

#[test]
fn invalid_project_rejects_empty_group_creation_without_mutation() {
    let (mut project, _) = fixture();
    project.tracks[0].group_id = Some(Id::new_v4());
    let original = project.clone();
    assert!(project.add_group_track().is_err());
    assert_eq!(project, original);
}

#[test]
fn creating_each_track_kind_in_a_group_is_atomic_and_preserves_the_surrounding_tree() {
    for kind in [
        TrackKind::Audio,
        TrackKind::Midi,
        TrackKind::Beat,
        TrackKind::Group,
    ] {
        let (mut project, ids) = fixture();
        let parent = group(&mut project, &ids[..2]);
        let original = project.clone();
        let slot = project.track_subtree(parent).unwrap().end;
        let id = project.add_track_in_group(parent, kind).unwrap();
        assert_eq!(&project.tracks[..slot], &original.tracks[..slot]);
        assert_eq!(&project.tracks[slot + 1..], &original.tracks[slot..]);
        let track = &project.tracks[slot];
        assert_eq!(
            (track.id, track.kind, track.group_id),
            (id, kind, Some(parent))
        );
        assert!(track.clips.is_empty());
        assert_eq!(project.track_subtree(parent).unwrap().end, slot + 1);
        project.validate().unwrap();
        let restored: Project =
            serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        assert_eq!(restored, project);
        for target in [ids[2], Id::new_v4()] {
            let before = project.clone();
            assert!(project.add_track_in_group(target, kind).is_err());
            assert_eq!(project, before);
        }
        let mut invalid = original.clone();
        invalid.tracks.last_mut().unwrap().group_id = Some(Id::new_v4());
        let before = invalid.clone();
        assert!(invalid.add_track_in_group(parent, kind).is_err());
        assert_eq!(invalid, before);
    }
}
