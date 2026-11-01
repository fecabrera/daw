use super::*;
fn fixture() -> (DawUi, egui::Context, [Id; 6]) {
    let mut app = DawUi::default();
    let p = &mut app.session.project;
    let a = p.add_track().unwrap();
    let m = p.add_midi_track().unwrap();
    let b = p.add_track().unwrap();
    let n = p.add_midi_track().unwrap();
    let c = p.add_track().unwrap();
    let g = Id::new_v4();
    p.edit(Edit::GroupTracks {
        track_ids: vec![b, n],
        group_id: g,
        name: "Group 1".into(),
    })
    .unwrap();
    for (id, name) in [
        (a, "Audio 1"),
        (m, "MIDI 1"),
        (b, "Audio 2"),
        (n, "MIDI 2"),
        (c, "Audio 3"),
    ] {
        p.tracks.iter_mut().find(|t| t.id == id).unwrap().name = name.into();
    }
    app.view_mode = ViewMode::Instrument;
    app.select_track(m, false);
    let ctx = egui::Context::default();
    theme::configure(&ctx);
    fonts::configure(&ctx);
    icons::configure(&ctx);
    (app, ctx, [a, m, g, b, n, c])
}
fn frame(
    app: &mut DawUi,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280., 800.))),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
    out.shapes
}
fn click(app: &mut DawUi, ctx: &egui::Context, p: Pos2) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    frame(app, ctx, vec![]);
}
fn key(app: &mut DawUi, ctx: &egui::Context, key: egui::Key) {
    frame(
        app,
        ctx,
        vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    frame(
        app,
        ctx,
        vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
}
fn row(app: &DawUi, id: Id) -> Pos2 {
    app.track_browser
        .bounds
        .rows
        .iter()
        .find(|(i, _)| *i == id)
        .unwrap()
        .1
        .center()
}
fn open(app: &mut DawUi, ctx: &egui::Context) {
    frame(app, ctx, vec![]);
    let p = app.track_browser.bounds.toggle.center();
    click(app, ctx, p);
}
#[test]
fn browser_has_saved_order_and_independent_nested_group_disclosures() {
    let (mut app, _, [a, m, g, b, n, c]) = fixture();
    let p = &mut app.session.project;
    p.edit(Edit::SetGroupCollapsed {
        track_id: g,
        collapsed: true,
    })
    .unwrap();
    let inner = Id::new_v4();
    p.edit(Edit::GroupTracks {
        track_ids: vec![n],
        group_id: inner,
        name: "Nested".into(),
    })
    .unwrap();
    let original = p.clone();
    let state = &mut app.track_browser;
    state.reconcile(p);
    assert_eq!(
        state.rows(p).iter().map(|r| r.id).collect::<Vec<_>>(),
        [a, m, g, c]
    );
    state.open.insert(g);
    assert_eq!(
        state
            .rows(p)
            .iter()
            .map(|r| (r.id, r.depth))
            .collect::<Vec<_>>(),
        [(a, 0), (m, 0), (g, 0), (b, 1), (inner, 1), (c, 0)]
    );
    state.open.insert(inner);
    assert_eq!(state.rows(p).iter().find(|r| r.id == n).unwrap().depth, 2);
    assert_eq!(*p, original);
    state.open.remove(&g);
    state.open.insert(g);
    assert!(state.open.contains(&inner));
}
#[test]
fn toolbar_opens_tracks_and_switches_catalogs_without_project_edits() {
    let (mut app, ctx, _) = fixture();
    let original = app.session.project.clone();
    open(&mut app, &ctx);
    assert!(app.track_browser.visible);
    assert_eq!(
        app.track_browser.bounds.panel.width(),
        panels::ControlPanel::WIDTH
    );
    assert!(!app.instrument_catalog_visible && !app.effects_catalog_visible());
    let component = app.instrument_catalog_bounds.toggle.center();
    click(&mut app, &ctx, component);
    assert!(app.instrument_catalog_visible && !app.track_browser.visible);
    let toggle = app.track_browser.bounds.toggle.center();
    click(&mut app, &ctx, toggle);
    assert!(!app.instrument_catalog_visible && app.track_browser.visible);
    let effects = app.effects_bounds.library_toggle.center();
    click(&mut app, &ctx, effects);
    assert!(app.effects_catalog_visible() && !app.track_browser.visible);
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
    app.view_mode = ViewMode::Timeline;
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.track_browser.bounds.toggle, Rect::NOTHING);
}
#[test]
fn click_selects_midi_and_audio_targets_and_group_expansion_is_not_an_edit() {
    let (mut app, ctx, [a, _, g, _, n, _]) = fixture();
    let original = app.session.project.clone();
    open(&mut app, &ctx);
    let p = app.track_browser.bounds.disclosure[&g].center();
    click(&mut app, &ctx, p);
    assert!(app.track_browser.open.contains(&g));
    let p = row(&app, n);
    click(&mut app, &ctx, p);
    assert_eq!(app.selected_track, Some(n));
    assert_eq!(app.instrument_ui.track, Some(n));
    assert_eq!(
        app.effects_target(),
        Some(effects_panel::EffectTarget::Track(n))
    );
    let p = row(&app, a);
    click(&mut app, &ctx, p);
    assert_eq!(app.selected_track, Some(a));
    assert_eq!(app.view_mode, ViewMode::Instrument);
    assert_eq!(
        app.effects_target(),
        Some(effects_panel::EffectTarget::Track(a))
    );
    assert_eq!(app.effect_catalog_target(), app.effects_target());
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
}
#[test]
fn keyboard_navigation_stays_in_visible_tree_and_reconciles_renames_and_deletion() {
    let (mut app, ctx, [a, m, g, _, n, _]) = fixture();
    open(&mut app, &ctx);
    let p = row(&app, a);
    click(&mut app, &ctx, p);
    key(&mut app, &ctx, egui::Key::ArrowUp);
    assert_eq!(app.selected_track, Some(a));
    key(&mut app, &ctx, egui::Key::ArrowDown);
    assert_eq!(app.selected_track, Some(m));
    key(&mut app, &ctx, egui::Key::ArrowDown);
    assert_eq!(app.track_browser.cursor, Some(g));
    key(&mut app, &ctx, egui::Key::ArrowRight);
    assert!(app.track_browser.open.contains(&g));
    key(&mut app, &ctx, egui::Key::ArrowDown);
    key(&mut app, &ctx, egui::Key::ArrowDown);
    assert_eq!(app.selected_track, Some(n));
    key(&mut app, &ctx, egui::Key::ArrowLeft);
    assert_eq!(app.track_browser.cursor, Some(g));
    key(&mut app, &ctx, egui::Key::ArrowLeft);
    assert!(!app.track_browser.open.contains(&g));
    app.session
        .project
        .tracks
        .iter_mut()
        .find(|t| t.id == g)
        .unwrap()
        .name = "Renamed café".into();
    let rows = app.track_browser.rows(&app.session.project);
    assert!(rows.iter().any(|r| r.name == "Renamed café"));
    app.session.project.tracks.clear();
    frame(&mut app, &ctx, vec![]);
    assert!(app.track_browser.open.is_empty() && app.track_browser.bounds.rows.is_empty());
}
#[test]
fn disabled_browser_ignores_selection_and_disclosure_input() {
    let (mut app, ctx, [a, m, g, _, _, _]) = fixture();
    open(&mut app, &ctx);
    let audio = row(&app, a);
    let group = app.track_browser.bounds.disclosure[&g].center();
    let (_tx, rx) = mpsc::channel();
    app.job = Some(rx);
    click(&mut app, &ctx, audio);
    click(&mut app, &ctx, group);
    assert_eq!(app.selected_track, Some(m));
    assert!(!app.track_browser.open.contains(&g));
}
#[test]
fn rows_reuse_catalog_insets_in_every_theme_and_display_scale() {
    for theme in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1., 1.5] {
            let (mut app, ctx, [a, _, g, b, _, _]) = fixture();
            ctx.set_pixels_per_point(scale);
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme,
                    ..Default::default()
                },
            );
            open(&mut app, &ctx);
            let p = app.track_browser.bounds.disclosure[&g].center();
            click(&mut app, &ctx, p);
            let rows = &app.track_browser.bounds.rows;
            let first = rows.iter().find(|(id, _)| *id == a).unwrap().1;
            let child = rows.iter().find(|(id, _)| *id == b).unwrap().1;
            assert_eq!(first.left(), child.left());
            assert_eq!(first.width(), child.width());
            assert!(first.left() > app.track_browser.bounds.panel.left());
            assert!(first.right() < app.track_browser.bounds.panel.right());
            assert!((rows[1].1.top() - first.bottom() - menus::ITEM_SPACING).abs() < 0.01);
        }
    }
}

#[test]
fn editor_browser_shares_groups_and_selects_audio_midi_and_empty_tracks() {
    let (mut app, ctx, [a, m, g, b, n, _]) = fixture();
    let first = app.session.project.add_midi_pattern(m, 0).unwrap();
    let later = app.session.project.add_midi_pattern(n, 96_000).unwrap();
    let earliest = app.session.project.add_midi_pattern(n, 0).unwrap();
    app.select_clip(first, m, false);
    app.set_view_mode(ViewMode::Edit, &ctx);
    let original = app.session.project.clone();
    open(&mut app, &ctx);
    assert!(app.tracks_panel_visible());
    assert!(
        !app.track_browser.visible,
        "Instrument visibility is independent"
    );
    let group = app.track_browser.bounds.disclosure[&g].center();
    click(&mut app, &ctx, group);
    let midi = row(&app, n);
    click(&mut app, &ctx, midi);
    assert_eq!(app.editor_track().unwrap().id, n);
    assert_eq!(app.selected_clip, Some(earliest));
    assert!(app.midi_editor.clip_bounds.contains_key(&later));
    assert_eq!(
        app.effects_target(),
        Some(effects_panel::EffectTarget::Track(n))
    );
    let audio = row(&app, b);
    click(&mut app, &ctx, audio);
    assert_eq!(app.editor_track().unwrap().id, b);
    assert!(app.selected_clip.is_none() && app.selected_clips.is_empty());
    let top = row(&app, a);
    click(&mut app, &ctx, top);
    key(&mut app, &ctx, egui::Key::ArrowDown);
    assert_eq!(app.selected_clip, Some(first));
    assert_eq!(app.editor_track().unwrap().id, m);
    assert_eq!(app.view_mode, ViewMode::Edit);
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
}

#[test]
fn editor_browser_catalogs_are_exclusive_and_visibility_is_per_view() {
    let (mut app, ctx, _) = fixture();
    app.set_view_mode(ViewMode::Edit, &ctx);
    open(&mut app, &ctx);
    let effects = app.effects_bounds.library_toggle.center();
    click(&mut app, &ctx, effects);
    assert!(app.effects_catalog_visible() && !app.tracks_panel_visible());
    let tracks = app.track_browser.bounds.toggle.center();
    click(&mut app, &ctx, tracks);
    assert!(app.tracks_panel_visible() && !app.effects_catalog_visible());
    let chords = app.chords.bounds.toggle.center();
    click(&mut app, &ctx, chords);
    assert!(app.chords_visible() && !app.tracks_panel_visible());
    let tracks = app.track_browser.bounds.toggle.center();
    click(&mut app, &ctx, tracks);
    assert!(app.tracks_panel_visible() && !app.chords_visible());
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.tracks_panel_visible());
    app.set_view_mode(ViewMode::Edit, &ctx);
    frame(&mut app, &ctx, vec![]);
    assert!(app.tracks_panel_visible());
}

fn rect(app: &DawUi, id: Id) -> Rect {
    app.track_browser
        .bounds
        .rows
        .iter()
        .find(|(owner, _)| *owner == id)
        .unwrap()
        .1
}
fn pointer(app: &mut DawUi, ctx: &egui::Context, point: Pos2, pressed: bool) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ],
    );
    if !pressed {
        frame(app, ctx, vec![]);
    }
}
fn beat_fixture() -> (DawUi, egui::Context, Id, [Id; 3], Id, Id) {
    let (mut app, ctx, _) = fixture();
    let parent = app.session.project.add_beat_track().unwrap();
    let mut ids = Vec::new();
    for name in ["Kick", "Snare", "Hat"] {
        let row = daw_core::BeatSubtrack::new(name.into());
        ids.push(row.id);
        app.session
            .project
            .edit(Edit::Beat {
                track_id: parent,
                edit: daw_core::BeatEdit::Add {
                    subtrack: Box::new(row),
                    index: ids.len() - 1,
                },
            })
            .unwrap();
    }
    let other = app.session.project.add_beat_track().unwrap();
    let row = daw_core::BeatSubtrack::new("Other".into());
    let other_row = row.id;
    app.session
        .project
        .edit(Edit::Beat {
            track_id: other,
            edit: daw_core::BeatEdit::Add {
                subtrack: Box::new(row),
                index: 0,
            },
        })
        .unwrap();
    app.select_track(parent, false);
    open(&mut app, &ctx);
    (app, ctx, parent, [ids[0], ids[1], ids[2]], other, other_row)
}

#[test]
fn compact_controls_keep_catalog_row_sizes_and_right_alignment_for_every_owner() {
    for color in [
        settings::ColorTheme::Dark,
        settings::ColorTheme::Light,
        settings::ColorTheme::Mist,
    ] {
        for scale in [1.0, 1.5, 2.0] {
            let (mut app, ctx, parent, ids, _, _) = beat_fixture();
            ctx.set_pixels_per_point(scale);
            app.set_settings(
                &ctx,
                settings::AppSettings {
                    theme: color,
                    ..Default::default()
                },
            );
            let shapes = frame(&mut app, &ctx, vec![]);
            let font = egui::TextStyle::Small.resolve(&ctx.style_of(ctx.theme()));
            let expected = ctx
                .fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap("Ag".into(), font, egui::Color32::WHITE)
                        .size()
                        .y
                })
                .max(icons::TOOLBAR_ICON_SIZE)
                + 2.0 * f32::from(menus::ITEM_PADDING);
            let right = app.track_browser.bounds.mix[&parent][3].right();
            for id in std::iter::once(parent).chain(ids) {
                let row = rect(&app, id);
                assert!((row.height() - expected).abs() < 0.01);
                let controls = app.track_browser.bounds.mix[&id];
                for control in controls {
                    assert!(row.contains_rect(control));
                }
                assert_eq!(controls[0].size(), Vec2::splat(16.0));
                assert_eq!(controls[1].size(), Vec2::splat(16.0));
                assert_eq!(controls[2].size(), Vec2::splat(16.0));
                assert_eq!(controls[3].size(), Vec2::splat(16.0));
                for control in &controls[2..] {
                    // Scroll clipping intentionally hides controls beyond the viewport.
                    if !app.track_browser.bounds.panel.contains_rect(*control) {
                        continue;
                    }
                    let mut painted = false;
                    for shape in &shapes {
                        let bounds = shape.shape.visual_bounding_rect();
                        if matches!(shape.shape, egui::Shape::Path(_) | egui::Shape::Circle(_))
                            && bounds.width() > 8.0
                            && bounds.height() > 8.0
                            && control.contains(bounds.center())
                        {
                            assert!(
                                shape.clip_rect.contains_rect(bounds),
                                "{bounds:?} clipped by {:?}",
                                shape.clip_rect
                            );
                            painted = true;
                        }
                    }
                    assert!(painted, "knob must be visible");
                }
                assert!((controls[3].right() - right).abs() < 0.01);
                assert!(
                    controls
                        .windows(2)
                        .all(|pair| pair[0].right() <= pair[1].left())
                );
            }
        }
    }
}

#[test]
fn row_controls_edit_the_canonical_bus_without_changing_navigation_or_starting_drags() {
    let (mut app, ctx, parent, ids, _, _) = beat_fixture();
    let output = app.session.project.beat(parent).unwrap().output.id;
    for id in [parent, ids[1]] {
        let mute = app.track_browser.bounds.mix[&id][0].center();
        click(&mut app, &ctx, mute);
        if id == parent {
            assert!(
                app.session
                    .project
                    .tracks
                    .iter()
                    .find(|track| track.id == id)
                    .unwrap()
                    .muted
            );
        } else {
            assert!(app.session.project.beat_subtrack(id).unwrap().1.muted);
        }
        assert_eq!(app.selected_track, Some(parent));
        assert_eq!(app.selected_instrument_owner(), Some(output));
        assert!(app.track_drag.is_none() && !app.beat_dragging());
        let gain = app.track_browser.bounds.mix[&id][3].center();
        pointer(&mut app, &ctx, gain, true);
        let moved = gain - Vec2::new(0.0, 24.0);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(moved)]);
        pointer(&mut app, &ctx, moved, false);
        let gain = if id == parent {
            app.session
                .project
                .tracks
                .iter()
                .find(|track| track.id == id)
                .unwrap()
                .gain_db
        } else {
            app.session
                .project
                .instrument_settings(id)
                .unwrap()
                .main
                .gain_db
        };
        assert!(gain > 0.0);
        assert!(app.track_drag.is_none() && !app.beat_dragging());
    }
    assert!(app.dirty);
}

#[test]
fn selecting_subtrack_opens_shared_layers_and_parent_opens_only_final_output() {
    let (mut app, ctx, parent, ids, _, _) = beat_fixture();
    let output = app.session.project.beat(parent).unwrap().output.id;
    let original = app.session.project.clone();
    assert_eq!(app.selected_instrument_owner(), Some(output));
    assert!(app.midi_editor.layer_bounds.is_empty());
    let point = row(&app, ids[1]);
    click(&mut app, &ctx, point);
    assert_eq!(app.selected_track, Some(parent));
    assert_eq!(app.selected_instrument_owner(), Some(ids[1]));
    let layer = app
        .session
        .project
        .instrument_settings(ids[1])
        .unwrap()
        .layers[0]
        .id;
    assert!(app.midi_editor.layer_bounds.contains_key(&layer));
    let point = row(&app, parent);
    click(&mut app, &ctx, point);
    assert_eq!(app.selected_instrument_owner(), Some(output));
    assert!(app.midi_editor.layer_bounds.is_empty());
    assert_eq!(app.session.project, original);
    assert!(!app.dirty);
}

#[test]
fn browser_track_moves_use_release_only_group_transactions_in_both_views() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        let (mut app, ctx, [a, _, group, _, _, c]) = fixture();
        app.view_mode = view;
        open(&mut app, &ctx);
        let original = app.session.project.clone();
        let start = row(&app, c);
        let end = rect(&app, a).left_top() + Vec2::new(45.0, 1.0);
        pointer(&mut app, &ctx, start, true);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(app.session.project, original);
        pointer(&mut app, &ctx, end, false);
        assert_eq!(app.session.project.tracks[0].id, c);
        let start = row(&app, c);
        let end = row(&app, group);
        pointer(&mut app, &ctx, start, true);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        pointer(&mut app, &ctx, end, false);
        assert_eq!(
            app.session
                .project
                .tracks
                .iter()
                .find(|track| track.id == c)
                .unwrap()
                .group_id,
            Some(group)
        );
        let disclosure = app.track_browser.bounds.disclosure[&group].center();
        click(&mut app, &ctx, disclosure);
        let start = row(&app, c);
        let end = Pos2::new(
            app.track_browser.bounds.panel.left() + 10.0,
            app.track_browser.bounds.panel.bottom() - 8.0,
        );
        pointer(&mut app, &ctx, start, true);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        pointer(&mut app, &ctx, end, false);
        assert_eq!(
            app.session
                .project
                .tracks
                .iter()
                .find(|track| track.id == c)
                .unwrap()
                .group_id,
            None
        );
        assert_eq!(app.session.project.tracks.last().unwrap().id, c);
        app.session.project.validate().unwrap();
    }
}

#[test]
fn browser_subtrack_reorder_rejects_other_beats_and_project_rows() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        let (mut app, ctx, parent, ids, other, other_row) = beat_fixture();
        if view == ViewMode::Edit {
            app.view_mode = view;
            app.set_tracks_panel_visible(true);
            frame(&mut app, &ctx, vec![]);
        }
        let start = row(&app, ids[2]);
        let end = rect(&app, ids[0]).left_top() + Vec2::new(60.0, 1.0);
        let original = app.session.project.clone();
        pointer(&mut app, &ctx, start, true);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert_eq!(app.session.project, original);
        pointer(&mut app, &ctx, end, false);
        assert_eq!(
            app.session
                .project
                .beat(parent)
                .unwrap()
                .subtracks
                .iter()
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            [ids[2], ids[0], ids[1]]
        );
        for destination in [other, other_row, app.session.project.tracks[0].id] {
            let original = app.session.project.clone();
            let start = row(&app, ids[0]);
            let end = row(&app, destination);
            pointer(&mut app, &ctx, start, true);
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
            pointer(&mut app, &ctx, end, false);
            assert_eq!(app.session.project, original);
            assert!(!app.beat_dragging() && app.track_drag.is_none());
        }
    }
}

#[test]
fn browser_drag_cancellation_and_no_op_release_do_not_edit_the_project() {
    for cancel in ["escape", "focus", "outside", "revision", "click"] {
        let (mut app, ctx, [a, _, _, _, _, c]) = fixture();
        open(&mut app, &ctx);
        let original = app.session.project.clone();
        let start = row(&app, c);
        let end = rect(&app, a).left_top() + Vec2::new(50.0, 1.0);
        pointer(&mut app, &ctx, start, true);
        if cancel != "click" {
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        }
        match cancel {
            "escape" => key(&mut app, &ctx, egui::Key::Escape),
            "focus" => {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1280.0, 800.0),
                        )),
                        focused: false,
                        events: vec![egui::Event::WindowFocused(false)],
                        ..Default::default()
                    },
                    |ui| app.show(ui),
                );
                output.textures_delta.clear();
            }
            "revision" => {
                app.change_revision += 1;
                frame(&mut app, &ctx, vec![]);
            }
            _ => {}
        }
        let release = if cancel == "outside" {
            Pos2::new(1000.0, 700.0)
        } else if cancel == "click" {
            start
        } else {
            end
        };
        pointer(&mut app, &ctx, release, false);
        assert_eq!(app.session.project, original, "{cancel}");
        assert!(!app.dirty && app.track_drag.is_none());
    }
}

#[test]
fn browser_moves_complete_group_subtrees_and_rejects_cycles() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        let (mut app, ctx, [a, _, group, b, n, _]) = fixture();
        let nested = Id::new_v4();
        app.session
            .project
            .edit(Edit::GroupTracks {
                track_ids: vec![b],
                group_id: nested,
                name: "Nested".into(),
            })
            .unwrap();
        app.view_mode = view;
        app.set_tracks_panel_visible(true);
        frame(&mut app, &ctx, vec![]);
        let start = row(&app, group);
        let end = rect(&app, a).left_top() + Vec2::new(8.0, 1.0);
        pointer(&mut app, &ctx, start, true);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        pointer(&mut app, &ctx, end, false);
        assert_eq!(
            &app.session.project.tracks[..4]
                .iter()
                .map(|t| t.id)
                .collect::<Vec<_>>(),
            &[group, nested, b, n]
        );
        assert_eq!(app.session.project.tracks[1].group_id, Some(group));
        assert_eq!(app.session.project.tracks[2].group_id, Some(nested));
        let disclosure = app.track_browser.bounds.disclosure[&group].center();
        click(&mut app, &ctx, disclosure);
        let original = app.session.project.clone();
        let start = row(&app, group);
        let end = row(&app, nested);
        pointer(&mut app, &ctx, start, true);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        pointer(&mut app, &ctx, end, false);
        assert_eq!(app.session.project, original);
        app.session.project.validate().unwrap();
    }
}

#[test]
fn editor_subtrack_navigation_retains_the_active_pattern_and_disclosure_retains_owner() {
    let (mut app, ctx, parent, ids, _, _) = beat_fixture();
    let clip = app.session.project.add_beat_pattern(parent, 0).unwrap();
    app.select_clip(clip, parent, false);
    app.view_mode = ViewMode::Edit;
    app.set_tracks_panel_visible(true);
    frame(&mut app, &ctx, vec![]);
    let before = app.session.project.clone();
    let point = row(&app, ids[1]);
    click(&mut app, &ctx, point);
    assert_eq!(app.selected_clip, Some(clip));
    assert_eq!(app.selected_track, Some(parent));
    assert_eq!(app.selected_instrument_owner(), Some(ids[1]));
    let disclosure = app.track_browser.bounds.disclosure[&parent].center();
    click(&mut app, &ctx, disclosure);
    assert_eq!(app.selected_instrument_owner(), Some(ids[1]));
    assert!(
        !app.track_browser
            .bounds
            .rows
            .iter()
            .any(|(id, _)| *id == ids[1])
    );
    assert_eq!(app.session.project, before);
}

#[test]
fn shared_tracks_plus_creates_and_selects_midi_in_instrument_and_editor() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        let (mut app, ctx, [_, midi, _, _, _, _]) = fixture();
        let clip = app.session.project.add_midi_pattern(midi, 0).unwrap();
        app.select_clip(clip, midi, false);
        app.view_mode = view;
        app.set_tracks_panel_visible(true);
        frame(&mut app, &ctx, vec![]);
        let original = app.session.project.clone();
        let add = app.track_browser.bounds.add.center();
        click(&mut app, &ctx, add);
        let shapes = frame(&mut app, &ctx, vec![]);
        let point = shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "Add MIDI track" => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                _ => None,
            })
            .expect("MIDI creation menu item");
        pointer(&mut app, &ctx, point, true);
        assert_eq!(app.session.project, original);
        pointer(&mut app, &ctx, point, false);
        let added = app.session.project.tracks.last().unwrap();
        assert_eq!(app.session.project.tracks.len(), original.tracks.len() + 1);
        assert_eq!(
            &app.session.project.tracks[..original.tracks.len()],
            &original.tracks
        );
        assert_eq!(added.kind, TrackKind::Midi);
        assert!(added.clips.is_empty());
        assert_eq!(app.selected_track, Some(added.id));
        if view == ViewMode::Edit {
            assert_eq!(app.editor_track().unwrap().id, added.id);
            assert!(app.selected_clip.is_none());
        } else {
            assert_eq!(app.selected_clip, Some(clip));
        }
        assert!(app.dirty);
        assert_eq!(app.view_mode, view);
        app.session.project.validate().unwrap();
    }
}

#[test]
fn normal_track_presses_do_not_disable_the_workspace_before_the_drag_threshold() {
    let (mut app, ctx, [a, _, _, _, _, c]) = fixture();
    open(&mut app, &ctx);
    let original = app.session.project.clone();
    let selected = app.selected_track;
    let start = row(&app, c);
    pointer(&mut app, &ctx, start, true);
    frame(&mut app, &ctx, vec![]);
    assert!(app.track_browser.has_gesture());
    assert!(app.track_drag.is_none());
    assert!(app.view_switch_enabled());
    assert_eq!(app.selected_track, selected);
    key(&mut app, &ctx, egui::Key::Escape);
    assert_eq!(app.selected_track, selected);
    pointer(&mut app, &ctx, start, false);
    let start = row(&app, c);
    pointer(&mut app, &ctx, start, true);
    let end = rect(&app, a).left_top() + Vec2::new(50.0, 1.0);
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.track_drag, Some(c));
    assert!(!app.view_switch_enabled());
    key(&mut app, &ctx, egui::Key::Escape);
    pointer(&mut app, &ctx, end, false);
    assert_eq!(app.session.project, original);
}

#[test]
fn leaving_an_armed_browser_press_cancels_it_without_blocking_shortcuts() {
    for change_view in [false, true] {
        let (mut app, ctx, [a, _, _, _, _, _]) = fixture();
        open(&mut app, &ctx);
        let original = app.session.project.clone();
        let start = row(&app, a);
        pointer(&mut app, &ctx, start, true);
        assert!(app.track_browser.has_gesture());
        if change_view {
            app.set_view_mode(ViewMode::Edit, &ctx);
        } else {
            app.set_tracks_panel_visible(false);
        }
        assert!(!app.track_browser.has_gesture());
        pointer(&mut app, &ctx, start, false);
        assert!(app.edit_shortcuts_available(&ctx));
        assert_eq!(app.session.project, original);
    }
}

#[test]
fn project_track_and_group_names_share_inline_commit_and_cancel_in_both_views() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        for kind in [
            TrackKind::Audio,
            TrackKind::Midi,
            TrackKind::Group,
            TrackKind::Beat,
        ] {
            let (mut app, ctx, [a, m, g, _, _, _]) = fixture();
            let id = match kind {
                TrackKind::Audio => a,
                TrackKind::Midi => m,
                TrackKind::Group => g,
                TrackKind::Beat => app.session.project.add_beat_track().unwrap(),
            };
            app.view_mode = view;
            app.set_tracks_panel_visible(true);
            frame(&mut app, &ctx, vec![]);
            let point = row(&app, id);
            click(&mut app, &ctx, point);
            click(&mut app, &ctx, point);
            assert!(ctx.text_edit_focused(), "{view:?} {kind:?}");
            let before = app.session.project.clone();
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::Text("New 🎵 name".into())],
            );
            assert_eq!(app.session.project, before);
            key(&mut app, &ctx, egui::Key::Enter);
            let renamed = app
                .session
                .project
                .tracks
                .iter()
                .find(|t| t.id == id)
                .unwrap();
            assert_eq!(renamed.name, "New 🎵 name");
            assert_eq!(renamed.kind, kind);
            assert!(app.dirty);
            app.dirty = false;
            let point = row(&app, id);
            click(&mut app, &ctx, point);
            click(&mut app, &ctx, point);
            frame(&mut app, &ctx, vec![egui::Event::Text("Discard".into())]);
            let before = app.session.project.clone();
            key(&mut app, &ctx, egui::Key::Escape);
            assert_eq!(app.session.project, before);
            assert!(!app.dirty);
            assert!(!ctx.text_edit_focused());
            app.session.project.validate().unwrap();
        }
    }
}

#[test]
fn project_rows_open_rename_over_mix_controls_without_editing_the_mix() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        for group in [false, true] {
            let (mut app, ctx, [a, _, g, _, _, _]) = fixture();
            let id = if group { g } else { a };
            app.view_mode = view;
            app.set_tracks_panel_visible(true);
            frame(&mut app, &ctx, vec![]);
            let original = app.session.project.clone();
            let point = app.track_browser.bounds.mix[&id][0].center();
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Secondary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            let shapes = frame(&mut app, &ctx, vec![]);
            assert_eq!(app.session.project, original);
            let rename = shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Text(t) if t.galley.text() == "Rename" => {
                        Some(t.pos + t.galley.size() * 0.5)
                    }
                    _ => None,
                })
                .unwrap();
            click(&mut app, &ctx, rename);
            assert!(ctx.text_edit_focused());
            frame(&mut app, &ctx, vec![egui::Event::Text("Discard".into())]);
            app.set_tracks_panel_visible(false);
            frame(&mut app, &ctx, vec![]);
            assert!(app.track_name_edit.is_none());
            assert_eq!(app.session.project, original);
            assert!(!app.dirty);
        }
    }
}

fn secondary(app: &mut DawUi, ctx: &egui::Context, point: Pos2) -> Vec<egui::epaint::ClippedShape> {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    frame(app, ctx, vec![])
}
fn menu_point(shapes: &[egui::epaint::ClippedShape], label: &str) -> Pos2 {
    shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing menu item {label}"))
}

#[test]
fn plus_and_blank_panel_menus_share_all_creation_actions_and_commit_on_release() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        for context in [false, true] {
            for action in [
                TrackAction::Add,
                TrackAction::AddMidi,
                TrackAction::AddBeat,
                TrackAction::AddGroup,
                TrackAction::AddSubtrack,
            ] {
                if context && action == TrackAction::AddSubtrack {
                    continue;
                }
                let (mut app, ctx, parent, _, other, _) = beat_fixture();
                let clip = app.session.project.add_beat_pattern(parent, 0).unwrap();
                app.select_clip(clip, parent, false);
                app.view_mode = view;
                app.set_tracks_panel_visible(true);
                frame(&mut app, &ctx, vec![]);
                let original = app.session.project.clone();
                let shapes = if context {
                    let point =
                        app.track_browser.bounds.panel.left_bottom() + Vec2::new(20.0, -20.0);
                    secondary(&mut app, &ctx, point)
                } else {
                    let plus = app.track_browser.bounds.add.center();
                    click(&mut app, &ctx, plus);
                    frame(&mut app, &ctx, vec![])
                };
                for item in [
                    "Add audio track",
                    "Add MIDI track",
                    "Add beat track",
                    "Add group",
                ] {
                    menu_point(&shapes, item);
                }
                if context {
                    assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Add subtrack")));
                } else {
                    menu_point(&shapes, "Add subtrack");
                }
                assert_eq!(app.session.project, original);
                let target = menu_point(&shapes, action.label());
                pointer(&mut app, &ctx, target, true);
                assert_eq!(app.session.project, original);
                pointer(&mut app, &ctx, target, false);
                if action == TrackAction::AddSubtrack {
                    let beat = app.session.project.beat(parent).unwrap();
                    assert_eq!(
                        beat.subtracks.len(),
                        original.beat(parent).unwrap().subtracks.len() + 1
                    );
                    assert_eq!(app.session.project.beat(other), original.beat(other));
                    assert_eq!(app.selected_track, Some(parent));
                    assert_eq!(app.beat_ui.primary, beat.subtracks.last().map(|row| row.id));
                    assert_eq!(app.selected_clip, Some(clip));
                } else {
                    assert_eq!(app.session.project.tracks.len(), original.tracks.len() + 1);
                    assert_eq!(
                        &app.session.project.tracks[..original.tracks.len()],
                        &original.tracks
                    );
                    let added = app.session.project.tracks.last().unwrap();
                    let expected = match action {
                        TrackAction::Add => TrackKind::Audio,
                        TrackAction::AddMidi => TrackKind::Midi,
                        TrackAction::AddBeat => TrackKind::Beat,
                        TrackAction::AddGroup => TrackKind::Group,
                        _ => unreachable!(),
                    };
                    assert_eq!(added.kind, expected);
                    assert_eq!(app.selected_track, Some(added.id));
                    if action == TrackAction::AddGroup {
                        assert!(
                            added.group_id.is_none()
                                && added.clips.is_empty()
                                && added.instrument.is_none()
                        );
                    }
                    if view == ViewMode::Edit {
                        assert_eq!(app.editor_track().unwrap().id, added.id);
                        assert!(app.selected_clip.is_none());
                    }
                }
                assert!(app.dirty, "{view:?} {context} {action:?}");
                assert_eq!(app.view_mode, view);
                app.session.project.validate().unwrap();
            }
        }
    }
}

#[test]
fn beat_row_add_subtrack_targets_the_clicked_collapsed_parent_in_both_views() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        let (mut app, ctx, parent, _, other, _) = beat_fixture();
        let audio = app.session.project.tracks[0].id;
        app.select_track(audio, false);
        app.selected_clip = None;
        app.view_mode = view;
        app.set_tracks_panel_visible(true);
        app.track_browser.open.remove(&parent);
        frame(&mut app, &ctx, vec![]);
        let original = app.session.project.clone();
        let target = app.track_browser.bounds.mix[&parent][0].center();
        let shapes = secondary(&mut app, &ctx, target);
        menu_point(&shapes, "Rename");
        let add = menu_point(&shapes, "Add subtrack");
        assert_eq!(app.selected_track, Some(audio));
        assert_eq!(app.session.project, original);
        pointer(&mut app, &ctx, add, true);
        assert_eq!(app.session.project, original);
        pointer(&mut app, &ctx, add, false);
        assert_eq!(app.selected_track, Some(parent));
        assert_eq!(
            app.session.project.beat(parent).unwrap().subtracks.len(),
            original.beat(parent).unwrap().subtracks.len() + 1
        );
        assert_eq!(app.session.project.beat(other), original.beat(other));
        assert!(app.track_browser.open.contains(&parent));
        let child = app
            .session
            .project
            .beat(parent)
            .unwrap()
            .subtracks
            .last()
            .unwrap()
            .id;
        assert_eq!(app.selected_instrument_owner(), Some(child));
        assert!(
            app.track_browser
                .bounds
                .rows
                .iter()
                .any(|(id, _)| *id == child)
        );
        app.session.project.validate().unwrap();
    }
}

#[test]
fn empty_panel_group_creation_and_dismissal_preserve_existing_data() {
    for create in [false, true] {
        let (mut app, ctx, _) = fixture();
        app.session.project.tracks.clear();
        app.selected_track = None;
        app.selected_tracks.clear();
        open(&mut app, &ctx);
        let original = app.session.project.clone();
        let point = app.track_browser.bounds.panel.center();
        let shapes = secondary(&mut app, &ctx, point);
        let add = menu_point(&shapes, "Add group");
        assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Add subtrack")));
        if create {
            click(&mut app, &ctx, add);
            assert_eq!(app.session.project.tracks.len(), 1);
            assert_eq!(app.session.project.tracks[0].kind, TrackKind::Group);
            assert_eq!(app.session.project.tracks[0].name, "Group 1");
            assert!(app.dirty);
        } else {
            key(&mut app, &ctx, egui::Key::Escape);
            assert_eq!(app.session.project, original);
            assert!(!app.dirty);
        }
    }
}

#[test]
fn menubar_group_and_subtrack_actions_use_shared_guards_and_editor_owner() {
    let creation: Vec<_> = TrackAction::ALL
        .iter()
        .take(5)
        .map(|action| action.label())
        .collect();
    assert_eq!(
        creation,
        [
            "Add audio track",
            "Add MIDI track",
            "Add beat track",
            "Add group",
            "Add subtrack"
        ]
    );
    assert!(TrackAction::AddGroup.starts_menu_group());
    assert!(!TrackAction::AddSubtrack.starts_menu_group());
    let (mut app, ctx, parent, _, other, _) = beat_fixture();
    let original = app.session.project.clone();
    app.select_track(app.session.project.tracks[0].id, false);
    assert!(!app.track_action_enabled(TrackAction::AddSubtrack, &ctx));
    app.perform_track_action(TrackAction::AddSubtrack, &ctx);
    assert_eq!(app.session.project, original);
    let clip = app.session.project.add_beat_pattern(parent, 0).unwrap();
    app.select_clip(clip, parent, false);
    // Retained Editor clip owns the displayed beat, independent of Instrument selection.
    app.select_track(other, false);
    app.view_mode = ViewMode::Edit;
    frame(&mut app, &ctx, vec![]);
    let original = app.session.project.clone();
    assert!(app.track_action_enabled(TrackAction::AddSubtrack, &ctx));
    app.perform_track_action(TrackAction::AddSubtrack, &ctx);
    assert_eq!(
        app.session.project.beat(parent).unwrap().subtracks.len(),
        original.beat(parent).unwrap().subtracks.len() + 1
    );
    assert_eq!(app.session.project.beat(other), original.beat(other));
    app.settings_open = true;
    let original = app.session.project.clone();
    for action in [TrackAction::AddGroup, TrackAction::AddSubtrack] {
        assert!(!app.track_action_enabled(action, &ctx));
        app.perform_track_action(action, &ctx);
    }
    assert_eq!(app.session.project, original);
}

#[test]
fn timeline_beat_context_uses_clicked_parent_when_audio_is_primary_in_a_group_selection() {
    let (mut app, ctx, parent, _, other, _) = beat_fixture();
    let audio = app.session.project.tracks[0].id;
    app.view_mode = ViewMode::Timeline;
    app.select_track(audio, false);
    app.selected_tracks.insert(parent);
    frame(&mut app, &ctx, vec![]);
    let lane = app.lane_bounds[&parent];
    let point = Pos2::new(lane.left() - 100.0, lane.center().y);
    let original = app.session.project.clone();
    let shapes = secondary(&mut app, &ctx, point);
    assert_eq!(app.selected_track, Some(audio));
    let add = menu_point(&shapes, "Add subtrack");
    assert_eq!(app.session.project, original);
    click(&mut app, &ctx, add);
    assert_eq!(
        app.session.project.beat(parent).unwrap().subtracks.len(),
        original.beat(parent).unwrap().subtracks.len() + 1
    );
    assert_eq!(app.session.project.beat(other), original.beat(other));
    assert_eq!(app.selected_track, Some(parent));
}

#[test]
fn group_context_creates_audio_midi_beat_and_nested_groups_inside_the_clicked_parent() {
    for view in [ViewMode::Instrument, ViewMode::Edit] {
        for action in [
            TrackAction::Add,
            TrackAction::AddMidi,
            TrackAction::AddBeat,
            TrackAction::AddGroup,
        ] {
            let (mut app, ctx, [_, _, parent, _, _, _]) = fixture();
            let beat = app.session.project.add_beat_track().unwrap();
            app.select_track(beat, false);
            app.view_mode = view;
            app.set_tracks_panel_visible(true);
            frame(&mut app, &ctx, vec![]);
            let original = app.session.project.clone();
            let slot = original.track_subtree(parent).unwrap().end;
            let point = app.track_browser.bounds.mix[&parent][0].center();
            let shapes = secondary(&mut app, &ctx, point);
            for label in [
                "Rename",
                "Add audio track",
                "Add MIDI track",
                "Add beat track",
                "Add group",
            ] {
                menu_point(&shapes, label);
            }
            assert!(!shapes.iter().any(|shape| matches!(&shape.shape, egui::Shape::Text(text) if text.galley.text() == "Add subtrack")));
            assert_eq!(app.selected_track, Some(beat));
            assert_eq!(app.session.project, original);
            let add = menu_point(&shapes, action.label());
            pointer(&mut app, &ctx, add, true);
            assert_eq!(app.session.project, original);
            pointer(&mut app, &ctx, add, false);
            assert_eq!(
                &app.session.project.tracks[..slot],
                &original.tracks[..slot]
            );
            assert_eq!(
                &app.session.project.tracks[slot + 1..],
                &original.tracks[slot..]
            );
            let added = &app.session.project.tracks[slot];
            assert_eq!(added.group_id, Some(parent));
            let expected = match action {
                TrackAction::Add => TrackKind::Audio,
                TrackAction::AddMidi => TrackKind::Midi,
                TrackAction::AddBeat => TrackKind::Beat,
                TrackAction::AddGroup => TrackKind::Group,
                _ => unreachable!(),
            };
            assert_eq!(added.kind, expected);
            assert_eq!(app.selected_track, Some(added.id));
            assert!(app.track_browser.open.contains(&parent));
            assert!(
                app.track_browser
                    .bounds
                    .rows
                    .iter()
                    .any(|(id, _)| *id == added.id)
            );
            assert!(app.dirty);
            app.session.project.validate().unwrap();
            let created = app.session.project.clone();
            app.perform_history_action(HistoryAction::Undo, &ctx);
            assert_eq!(app.session.project, original);
            app.perform_history_action(HistoryAction::Redo, &ctx);
            assert_eq!(app.session.project, created);
        }
    }
}
