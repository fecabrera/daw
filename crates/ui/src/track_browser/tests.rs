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
    let p = row(&app, g);
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
    let group = row(&app, g);
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
            let p = row(&app, g);
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
