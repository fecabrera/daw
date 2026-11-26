use super::*;
use daw_core::{BeatEdit, BeatSubtrack};

fn setup(color: settings::ColorTheme) -> (DawUi, egui::Context, Id, Vec<Id>) {
    let mut app = DawUi::default();
    let parent = app.session.project.add_beat_track().unwrap();
    let rows: Vec<_> = ["Kick", "Snare"]
        .into_iter()
        .map(|name| BeatSubtrack::new(name.into()))
        .collect();
    let ids = rows.iter().map(|r| r.id).collect();
    for (index, row) in rows.into_iter().enumerate() {
        app.session
            .project
            .edit(Edit::Beat {
                track_id: parent,
                edit: BeatEdit::Add {
                    subtrack: Box::new(row),
                    index,
                },
            })
            .unwrap();
    }
    app.session
        .project
        .edit(Edit::SetGroupCollapsed {
            track_id: parent,
            collapsed: false,
        })
        .unwrap();
    let ctx = context();
    app.set_settings(
        &ctx,
        settings::AppSettings {
            theme: color,
            ..Default::default()
        },
    );
    app.set_view_mode(ViewMode::Mixer, &ctx);
    frame(&mut app, &ctx, vec![]);
    (app, ctx, parent, ids)
}
fn click(app: &mut DawUi, ctx: &egui::Context, pos: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(pos), button(pos, true)],
    );
    frame(app, ctx, vec![button(pos, false)]);
}
fn text(app: &mut DawUi, ctx: &egui::Context, label: &str, within: Rect) -> Pos2 {
    frame_shapes(app, ctx, vec![], Vec2::new(1280.0, 800.0))
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                let p = t.pos + t.galley.size() * 0.5;
                within.contains(p).then_some(p)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {label} in {within:?}"))
}

#[test]
fn beat_mixer_children_share_group_gutters_destinations_and_aligned_output_controls() {
    for color in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        let (mut app, ctx, parent, rows) = setup(color);
        let group = app.session.project.add_group_track().unwrap();
        app.session
            .project
            .edit(Edit::MoveTrack {
                track_id: parent,
                group_id: Some(group),
                index: 1,
            })
            .unwrap();
        let before = app.session.project.clone();
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        let header_text = if color == settings::ColorTheme::Light {
            theme::LIGHT.text
        } else {
            egui::Color32::WHITE
        };
        for name in ["Kick", "Snare"] {
            assert!(shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.text() == name
                    && text.galley.job.sections.iter().all(|section| section.format.color == header_text)
            )));
        }
        let rect = app.mixer_bounds[&parent];
        assert_eq!(app.mixer_bounds[&rows[0]].left(), rect.right());
        assert_eq!(
            app.mixer_bounds[&rows[1]].left(),
            app.mixer_bounds[&rows[0]].right()
        );
        assert_eq!(
            app.mixer_gutter_height(),
            2.0 * crate::mixer::GROUP_GUTTER_HEIGHT
        );
        let parent_color = theme::clip_colors(
            app.session
                .project
                .tracks
                .iter()
                .find(|t| t.id == parent)
                .unwrap()
                .color,
            None,
        )
        .header;
        for id in rows {
            let strip = app.mixer_bounds[&id];
            let gutter = Rect::from_min_size(
                Pos2::new(
                    strip.left(),
                    strip.top() + crate::mixer::GROUP_GUTTER_HEIGHT,
                ),
                Vec2::new(strip.width(), crate::mixer::GROUP_GUTTER_HEIGHT),
            );
            assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == gutter && r.fill == parent_color)));
            assert_eq!(
                app.mixer_faders[&Some(id)].top(),
                app.mixer_faders[&Some(parent)].top()
            );
            assert_eq!(
                app.mixer_faders[&Some(id)].bottom(),
                app.mixer_faders[&None].bottom()
            );
            text(&mut app, &ctx, "Beat 1", strip);
        }
        assert_eq!(app.session.project, before);
        assert!(!app.dirty);
        assert!(app.error.is_none());
    }
}

#[test]
fn beat_mixer_collapse_uses_saved_disclosure_without_losing_children_or_playback_data() {
    let (mut app, ctx, parent, rows) = setup(settings::ColorTheme::Dark);
    let children = app.session.project.beat(parent).unwrap().clone();
    app.edit(Edit::SetGroupCollapsed {
        track_id: parent,
        collapsed: true,
    });
    frame(&mut app, &ctx, vec![]);
    assert!(rows.iter().all(|id| !app.mixer_bounds.contains_key(id)));
    assert_eq!(app.session.project.beat(parent).unwrap(), &children);
    let restored: daw_core::Project =
        serde_json::from_str(&serde_json::to_string(&app.session.project).unwrap()).unwrap();
    assert!(
        restored
            .tracks
            .iter()
            .find(|t| t.id == parent)
            .unwrap()
            .group_collapsed
    );
    app.perform_history_action(HistoryAction::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(rows.iter().all(|id| app.mixer_bounds.contains_key(id)));
    app.perform_history_action(HistoryAction::Redo, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(rows.iter().all(|id| !app.mixer_bounds.contains_key(id)));
}

#[test]
fn beat_mixer_controls_edit_only_the_real_lane_and_follow_existing_history() {
    let (mut app, ctx, parent, rows) = setup(settings::ColorTheme::Dark);
    let original = app.session.project.clone();
    let fader = app.mixer_faders[&Some(rows[0])];
    let start = fader.center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), button(start, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(Pos2::new(
            start.x,
            fader.top() + 8.0,
        ))],
    );
    frame(
        &mut app,
        &ctx,
        vec![button(Pos2::new(start.x, fader.top() + 8.0), false)],
    );
    let changed = app.session.project.beat(parent).unwrap().subtracks[0]
        .instrument
        .main
        .gain_db;
    assert!(changed > 0.0);
    assert_eq!(
        app.session.project.beat(parent).unwrap().subtracks[1],
        original.beat(parent).unwrap().subtracks[1]
    );
    assert_eq!(
        app.session.project.tracks[0].gain_db,
        original.tracks[0].gain_db
    );
    assert_eq!(app.selected_mixer_beat_row().unwrap().1.id, rows[0]);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert_eq!(
        app.session.project.beat(parent).unwrap().subtracks[0]
            .instrument
            .main
            .gain_db,
        0.0
    );
    let strip = app.mixer_bounds[&rows[0]];
    let mute = text(&mut app, &ctx, "M", strip);
    click(&mut app, &ctx, mute);
    assert!(app.session.project.beat(parent).unwrap().subtracks[0].muted);
    assert!(!app.session.project.tracks[0].muted);
    assert!(!app.session.project.beat(parent).unwrap().subtracks[1].muted);
    app.perform_track_action(TrackAction::Solo, &ctx);
    assert!(app.session.project.beat(parent).unwrap().subtracks[0].soloed);
    assert!(!app.session.project.tracks[0].soloed);
}

#[test]
fn beat_mixer_selection_targets_lane_effects_and_parent_click_restores_track_scope() {
    let (mut app, ctx, parent, rows) = setup(settings::ColorTheme::Dark);
    let before = app.session.project.clone();
    let child = app.mixer_bounds[&rows[1]];
    click(
        &mut app,
        &ctx,
        Pos2::new(child.center().x, child.top() + 160.0),
    );
    let row = &app.session.project.beat(parent).unwrap().subtracks[1];
    assert_eq!(
        app.effects_target(),
        Some(effects_panel::EffectTarget::Track(row.id))
    );
    assert!(!app.track_selected(parent));
    assert!(app.selected_tracks.is_empty());
    let parent_rect = app.mixer_bounds[&parent];
    click(
        &mut app,
        &ctx,
        Pos2::new(parent_rect.center().x, parent_rect.top() + 160.0),
    );
    assert_eq!(
        app.effects_target(),
        Some(effects_panel::EffectTarget::Track(parent))
    );
    assert!(app.selected_mixer_beat_row().is_none());
    assert_eq!(app.session.project, before);
    assert!(!app.dirty);
}

#[test]
fn beat_mixer_tracks_current_lane_order_and_removal_without_stale_strip_ids() {
    let (mut app, ctx, parent, rows) = setup(settings::ColorTheme::Light);
    app.edit(Edit::Beat {
        track_id: parent,
        edit: BeatEdit::Reorder {
            ids: vec![rows[1]],
            index: 0,
        },
    });
    frame(&mut app, &ctx, vec![]);
    assert!(app.mixer_bounds[&rows[1]].left() < app.mixer_bounds[&rows[0]].left());
    let child = app.mixer_bounds[&rows[1]];
    click(
        &mut app,
        &ctx,
        Pos2::new(child.center().x, child.top() + 18.0),
    );
    app.perform_track_action(TrackAction::Delete, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.mixer_bounds.contains_key(&rows[1]));
    assert!(app.mixer_bounds.contains_key(&rows[0]));
    assert!(app.session.project.beat(parent).is_some());
    app.perform_history_action(HistoryAction::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.mixer_bounds.contains_key(&rows[1]));
}

#[test]
fn beat_mixer_group_selection_shortcuts_do_not_touch_the_parent_or_unselected_lanes() {
    let (mut app, ctx, parent, rows) = setup(settings::ColorTheme::Dark);
    app.select_track(parent, false);
    app.navigate_beat_owner(parent, Some(rows[0]), false);
    app.navigate_beat_owner(parent, Some(rows[1]), true);
    app.perform_track_action(TrackAction::Mute, &ctx);
    assert!(
        app.session
            .project
            .beat(parent)
            .unwrap()
            .subtracks
            .iter()
            .all(|r| r.muted)
    );
    assert!(!app.session.project.tracks[0].muted);
    app.perform_history_action(HistoryAction::Undo, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(
        app.session
            .project
            .beat(parent)
            .unwrap()
            .subtracks
            .iter()
            .all(|r| !r.muted)
    );
    app.navigate_beat_owner(parent, Some(rows[1]), true);
    assert_eq!(app.selected_mixer_beat_row().unwrap().1.id, rows[0]);
    app.perform_track_action(TrackAction::Solo, &ctx);
    assert!(app.session.project.beat(parent).unwrap().subtracks[0].soloed);
    assert!(!app.session.project.beat(parent).unwrap().subtracks[1].soloed);
    app.perform_track_action(TrackAction::Rename, &ctx);
    assert!(app.beat_renaming(rows[0]));
    assert!(app.track_name_edit.is_none());
}

#[test]
fn beat_mixer_overflow_keeps_master_and_returns_pinned_and_controls_within_strips() {
    let (mut app, ctx, parent, _) = setup(settings::ColorTheme::Mist);
    for index in 2..16 {
        app.session
            .project
            .edit(Edit::Beat {
                track_id: parent,
                edit: BeatEdit::Add {
                    subtrack: Box::new(BeatSubtrack::new(format!("Lane {index}"))),
                    index,
                },
            })
            .unwrap();
    }
    let returns = app.session.project.add_return_track().unwrap();
    for size in [Vec2::new(640.0, 480.0), Vec2::new(1280.0, 800.0)] {
        frame_sized(&mut app, &ctx, vec![], size);
        assert_eq!(app.mixer_bounds[&returns].right(), app.master_bounds.left());
        assert_eq!(
            app.master_bounds.right(),
            size.x - crate::timeline_toolbar::WIDTH
        );
        for row in &app.session.project.beat(parent).unwrap().subtracks {
            let strip = app.mixer_bounds[&row.id];
            assert!(strip.contains_rect(app.mixer_faders[&Some(row.id)]));
        }
        assert!(app.lane_bounds.is_empty());
        assert!(app.error.is_none());
    }
}
