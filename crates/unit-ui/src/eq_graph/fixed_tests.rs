use super::*;
use daw_unit_core::EqSettings;
fn draw(
    ctx: &egui::Context,
    s: &mut EqSettings,
    events: Vec<egui::Event>,
    command: bool,
    live: bool,
) -> Bounds {
    let mut bounds = Bounds::default();
    let modifiers = egui::Modifiers {
        command,
        mac_cmd: command,
        ..Default::default()
    };
    let mut events = events;
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 400.0))),
            events,
            focused: true,
            ..Default::default()
        },
        |ui| {
            bounds = show_equalizer(
                ui,
                s,
                layout::unit_control_height(),
                true,
                FixedInput {
                    live_edits: live,
                    spectrum: None,
                    response: |_, _| std::array::from_fn(|_| vec![0.0; POINTS]),
                    effective: None,
                },
            );
        },
    );
    output.textures_delta.clear();
    bounds
}
fn button(p: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
#[test]
fn fixed_graph_edits_outer_frequency_gain_q_and_mid_gain_without_touching_legacy_fields() {
    for live in [false, true] {
        for index in 0..3 {
            let ctx = egui::Context::default();
            theme::configure(&ctx);
            crate::fonts::configure(&ctx);
            let mut settings = EqSettings::default();
            let before = settings.clone();
            let bounds = draw(&ctx, &mut settings, vec![], false, live);
            let start = bounds.nodes[index].center();
            draw(
                &ctx,
                &mut settings,
                vec![egui::Event::PointerMoved(start), button(start, true)],
                false,
                live,
            );
            let next = start + Vec2::new(20.0, -12.0);
            let b = draw(
                &ctx,
                &mut settings,
                vec![egui::Event::PointerMoved(next)],
                false,
                live,
            );
            assert!(b.preview.is_some());
            if !live {
                assert_eq!(settings, before);
            }
            // Change mode while held, then change Q without losing the previous edits.
            draw(&ctx, &mut settings, vec![], true, live);
            let end = next + Vec2::new(15.0, 0.0);
            draw(
                &ctx,
                &mut settings,
                vec![egui::Event::PointerMoved(end)],
                true,
                live,
            );
            draw(&ctx, &mut settings, vec![button(end, false)], true, live);
            assert!(settings.bands[index].gain_db > before.bands[index].gain_db);
            for other in 0..3 {
                if other != index {
                    assert_eq!(settings.bands[other], before.bands[other]);
                }
            }
            if index == 1 {
                assert_eq!(settings.bands[1].frequency_hz, before.bands[1].frequency_hz);
                assert_eq!(settings.bands[1].q, before.bands[1].q);
            } else {
                assert!(settings.bands[index].frequency_hz > before.bands[index].frequency_hz);
                assert!(settings.bands[index].q > before.bands[index].q);
            }
            let empty = bounds.plot.left_top() + Vec2::splat(6.0);
            draw(
                &ctx,
                &mut settings,
                vec![egui::Event::PointerMoved(empty), button(empty, true)],
                false,
                live,
            );
            assert_eq!(
                draw(&ctx, &mut settings, vec![button(empty, false)], false, live).selected,
                None
            );
            assert_eq!(
                draw(&ctx, &mut settings, vec![], false, live).selected,
                None
            );
        }
    }
}
