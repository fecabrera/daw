use super::instrument_catalog::{click, key};
use super::*;
fn setup(theme: settings::ColorTheme) -> (DawUi, egui::Context, Id) {
    let mut app = DawUi::default();
    let track = app.session.project.add_midi_track().unwrap();
    app.select_track(track, false);
    let ctx = context();
    app.set_settings(
        &ctx,
        settings::AppSettings {
            theme,
            ..Default::default()
        },
    );
    app.set_view_mode(ViewMode::Instrument, &ctx);
    frame(&mut app, &ctx, vec![]);
    (app, ctx, track)
}
#[test]
fn main_stays_below_scrolled_layers_and_reuses_mix_controls_in_both_themes() {
    for theme in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, ctx, _) = setup(theme);
        let main = app.instrument_ui.main_bounds.row;
        let label = app.instrument_ui.main_bounds.label;
        assert_eq!(main.height(), ROW_HEIGHT);
        assert_eq!(label.width(), TRACK_WIDTH);
        assert!(app.instrument_ui.main_bounds.collapse.right() < label.right());
        assert!(
            app.instrument_ui
                .main_bounds
                .mix
                .iter()
                .all(|r| label.contains_rect(*r)),
            "Main mix controls must stay inside its layer panel"
        );
        assert!(
            app.instrument_ui
                .main_bounds
                .mix
                .iter()
                .all(|r| r.top() >= app.instrument_ui.main_bounds.collapse.bottom() + 4.0)
        );
        let instrument = app.session.project.tracks[0].instrument.as_mut().unwrap();
        instrument
            .layers
            .extend((0..8).map(|_| daw_core::InstrumentLayerSettings::default()));
        frame(&mut app, &ctx, vec![]);
        let viewport = app.instrument_catalog_bounds.layer_viewport;
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(viewport.center()),
                egui::Event::MouseWheel {
                    phase: egui::TouchPhase::Move,
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, -500.0),
                    modifiers: Default::default(),
                },
            ],
        );
        for _ in 0..10 {
            frame(&mut app, &ctx, vec![]);
        }
        assert!(app.instrument_catalog_bounds.layer_scroll > 0.0);
        assert_eq!(app.instrument_ui.main_bounds.row, main);
        assert!(app.instrument_catalog_bounds.layer_viewport.bottom() <= main.top() + 1.0);
        let pan = app.instrument_ui.main_bounds.mix[2].center();
        click(&mut app, &ctx, pan, false);
        key(&mut app, &ctx, egui::Key::ArrowUp, false, false);
        assert!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .main
                .pan
                > 0.0
        );
        assert!(app.dirty);
        let ids: Vec<_> = app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .layers
            .iter()
            .map(|l| l.id)
            .collect();
        let collapse = app.instrument_ui.main_bounds.collapse.center();
        click(&mut app, &ctx, collapse, false);
        assert!(
            !app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .main
                .collapsed
        );
        assert_eq!(app.instrument_ui.main_bounds.row.bottom(), main.bottom());
        assert_eq!(
            app.instrument_ui.main_bounds.row.height(),
            panels::unit_height(false)
        );
        let bounds = &app.instrument_ui.main_bounds;
        assert!(bounds.mix.iter().all(|r| bounds.label.contains_rect(*r)));
        assert!(
            bounds
                .mix
                .iter()
                .all(|r| r.top() >= bounds.collapse.bottom() + 4.0)
        );
        assert_eq!(
            app.session.project.tracks[0]
                .instrument
                .as_ref()
                .unwrap()
                .layers
                .iter()
                .map(|l| l.id)
                .collect::<Vec<_>>(),
            ids
        );
    }
}
#[test]
fn main_accepts_shared_modulators_from_catalog_and_rejects_audio_components() {
    let (mut app, ctx, _) = setup(settings::ColorTheme::Dark);
    let point = app.instrument_ui.main_bounds.label.min + Vec2::new(25.0, 15.0);
    click(&mut app, &ctx, point, false);
    let main = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .main
        .id;
    assert_eq!(app.instrument_ui.selected_layer, Some(main));
    let toggle = app.instrument_catalog_bounds.toggle.center();
    click(&mut app, &ctx, toggle, false);
    for index in [0, 1, 2, 3] {
        let point = app.instrument_catalog_bounds.entries[index].center();
        click(&mut app, &ctx, point, false);
        key(&mut app, &ctx, egui::Key::Enter, false, false);
    }
    let instrument = app.session.project.tracks[0].instrument.as_ref().unwrap();
    assert!(instrument.layers[0].oscillators.is_empty() && !instrument.layers[0].amplitude.present);
    assert_eq!(instrument.main.envelopes.len(), 1);
    assert_eq!(instrument.main.lfos.len(), 1);
    assert_ne!(
        instrument.main.envelopes[0].mode,
        daw_core::EnvelopeMode::PerNote
    );
    let sources = crate::modulation_panel::sources(Some(instrument));
    assert!(sources.iter().any(|s| s.label == "Main · Envelope 1"));
    assert!(sources.iter().any(|s| s.label == "Main · LFO 1"));
    let point = app.instrument_catalog_bounds.entries[2].center();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point + Vec2::new(20.0, 0.0))],
    );
    let end = app.instrument_ui.main_bounds.row.center();
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert_eq!(app.instrument_catalog_bounds.drop_target, Some(main));
    frame(&mut app, &ctx, vec![button(end, false)]);
    assert_eq!(
        app.session.project.tracks[0]
            .instrument
            .as_ref()
            .unwrap()
            .main
            .lfos
            .len(),
        2
    );
    let layer = app.session.project.tracks[0]
        .instrument
        .as_ref()
        .unwrap()
        .layers[0]
        .id;
    app.instrument_ui.selected_layer = Some(layer);
    let point = app.instrument_ui.main_bounds.lfos[0].center();
    click(&mut app, &ctx, point, false);
    assert_eq!(app.instrument_ui.selected_layer, Some(main));
    app.session.project.validate().unwrap();
}
