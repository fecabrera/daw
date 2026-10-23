use super::*;
#[test]
fn filter_catalog_and_shared_controls_omit_the_lfo_and_preserve_unit_actions() {
    for color in [settings::ColorTheme::Dark, settings::ColorTheme::Light] {
        let (mut app, track) = fixture();
        let ctx = context();
        app.set_settings(
            &ctx,
            settings::AppSettings {
                theme: color,
                ..Default::default()
            },
        );
        app.select_track(track, false);
        app.set_view_mode(ViewMode::Timeline, &ctx);
        frame(&mut app, &ctx, vec![]);
        let entry = app.effects_bounds.filter_entry.center();
        click(&mut app, &ctx, entry);
        click(&mut app, &ctx, entry);
        let shapes = frame_shapes(&mut app, &ctx, vec![], Vec2::new(1280.0, 800.0));
        assert_eq!(app.session.project.tracks[0].effects.len(), 1);
        let daw_core::EffectSettings::Filter(settings) =
            &app.session.project.tracks[0].effects[0].unit
        else {
            unreachable!()
        };
        assert_eq!(*settings, daw_core::FilterSettings::default());
        let b = &app.effects_bounds.filter_units[0];
        assert_eq!(b.sections[0].width(), 90.0);
        assert_eq!(b.sections[2].width(), 90.0);
        assert_eq!(b.sections[0].right(), b.sections[2].left());
        assert_eq!(b.sections[1], Rect::NOTHING);
        assert_eq!(b.lfo, Rect::NOTHING);
        assert!(b.knobs[0].iter().all(Rect::is_positive));
        assert!(b.knobs[3][0].is_positive() && b.knobs[3][1].is_positive());
        assert!(
            shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="Filter"))
        );
        assert!(
            !shapes
                .iter()
                .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text()=="LFO"))
        );
        let power = b.power.center();
        click(&mut app, &ctx, power);
        let daw_core::EffectSettings::Filter(settings) =
            &app.session.project.tracks[0].effects[0].unit
        else {
            unreachable!()
        };
        assert!(!settings.enabled);
        app.session.project.tracks[0].effects_compact = true;
        frame(&mut app, &ctx, vec![]);
        assert!(
            app.effects_bounds.filter_units[0]
                .knobs
                .iter()
                .flatten()
                .all(|r| *r == Rect::NOTHING)
        );
        assert_eq!(
            app.effects_bounds.filter_units[0].sections[1],
            Rect::NOTHING
        );
    }
}

fn click(app: &mut DawUi, ctx: &egui::Context, point: Pos2) {
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    frame(app, ctx, vec![button(point, false)]);
}
