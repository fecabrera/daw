    use super::*;
    use egui::{Event, Modifiers, PointerButton, Pos2, Rect, Vec2};
    fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<Event>, focused: bool) {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(WIDTH as f32, HEIGHT as f32),
                )),
                events,
                focused,
                ..Default::default()
            },
            |ui| editor.show(ui),
        );
        output.textures_delta.clear();
    }
    fn pointer(p: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    fn drag(ctx: &egui::Context, editor: &mut Editor, slot: usize, release: bool) {
        let p = Pos2::new(
            [
                225.0, 45.0, 45.0, 135.0, 135.0, 135.0, 225.0, 225.0, 45.0, 115.0,
            ][slot],
            [
                96.0, 96.0, 156.0, 36.0, 96.0, 156.0, 36.0, 156.0, 36.0, 200.0,
            ][slot],
        );
        frame(
            ctx,
            editor,
            vec![Event::PointerMoved(p), pointer(p, true)],
            true,
        );
        let end = if slot == 9 {
            p
        } else {
            p - Vec2::new(
                0.0,
                if slot == 8 {
                    100.0
                } else if slot == 3 {
                    40.0
                } else if slot == 7 {
                    -18.0
                } else {
                    18.0
                },
            )
        };
        frame(ctx, editor, vec![Event::PointerMoved(end)], true);
        if release {
            frame(ctx, editor, vec![pointer(end, false)], true);
        }
    }
    fn setup() -> (egui::Context, Arc<Model>, Editor) {
        let ctx = egui::Context::default();
        fonts::configure(&ctx);
        theme::configure(&ctx);
        let model = Arc::new(Model::default());
        let editor = Editor::new(model.clone(), || {});
        (ctx, model, editor)
    }
    fn events(model: &Model) -> Vec<Edit> {
        std::iter::from_fn(|| model.pop()).collect()
    }
    #[test]
    fn ten_controls_emit_balanced_host_gestures_and_off_color_is_retained() {
        let (ctx, model, mut editor) = setup();
        frame(&ctx, &mut editor, vec![], true);
        drag(&ctx, &mut editor, 4, true);
        assert!(events(&model).is_empty());
        let mut state = PluginState::default();
        state.set(Parameter::ColorEnabled, 1.0).unwrap();
        model.publish(state);
        for (slot, p) in Parameter::ALL[1..].iter().copied().enumerate() {
            frame(&ctx, &mut editor, vec![], true);
            drag(&ctx, &mut editor, slot, true);
            let edits = events(&model);
            assert_eq!(edits.first(), Some(&Edit::Begin(p)), "{p:?}: {edits:?}");
            assert!(edits.iter().any(|e| matches!(e,Edit::Value(q,_)if *q==p)));
            assert_eq!(edits.last(), Some(&Edit::End(p)));
        }
        state.set(Parameter::ColorEnabled, 0.0).unwrap();
        state.set(Parameter::ColorStrength, 0.8).unwrap();
        model.publish(state);
        frame(&ctx, &mut editor, vec![], true);
        assert_eq!(editor.state.get(Parameter::ColorStrength), 0.8);
    }
    #[test]
    fn closing_and_focus_loss_end_drag_and_keyboard_edits_are_single_gestures() {
        let (ctx, model, mut editor) = setup();
        frame(&ctx, &mut editor, vec![], true);
        drag(&ctx, &mut editor, 1, false);
        assert_eq!(events(&model).first(), Some(&Edit::Begin(Parameter::Gain)));
        frame(&ctx, &mut editor, vec![], false);
        assert_eq!(events(&model), [Edit::End(Parameter::Gain)]);
        frame(&ctx, &mut editor, vec![Event::PointerGone], true);
        let p = Pos2::new(225.0, 96.0);
        frame(
            &ctx,
            &mut editor,
            vec![Event::PointerMoved(p), pointer(p, true)],
            true,
        );
        frame(&ctx, &mut editor, vec![pointer(p, false)], true);
        events(&model);
        frame(
            &ctx,
            &mut editor,
            vec![Event::Key {
                key: egui::Key::ArrowUp,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
            true,
        );
        let edits = events(&model);
        assert!(matches!(
            edits.as_slice(),
            [
                Edit::Begin(Parameter::Volume),
                Edit::Value(Parameter::Volume, _),
                Edit::End(Parameter::Volume)
            ]
        ));
        drag(&ctx, &mut editor, 2, false);
        events(&model);
        drop(editor);
        assert_eq!(events(&model), [Edit::End(Parameter::Tone)]);
    }
    #[test]
    fn full_ui_queue_preserves_batches_and_reserves_gesture_end() {
        let (ctx, model, mut editor) = setup();
        frame(&ctx, &mut editor, vec![], true);
        let filler = vec![Edit::Begin(Parameter::Bypass); CAPACITY - 2];
        assert!(model.push(&filler));
        assert!(!model.push(&[
            Edit::Begin(Parameter::Gain),
            Edit::Value(Parameter::Gain, 0.7),
            Edit::End(Parameter::Gain)
        ]));
        drag(&ctx, &mut editor, 1, true);
        assert_eq!(events(&model).len(), CAPACITY - 2);
        frame(&ctx, &mut editor, vec![], true);
        drag(&ctx, &mut editor, 1, false);
        events(&model);
        assert!(model.push(&vec![Edit::Begin(Parameter::Bypass); CAPACITY - 1]));
        drop(editor);
        let edits = events(&model);
        assert_eq!(edits.last(), Some(&Edit::End(Parameter::Gain)));
    }
    #[test]
    fn output_peak_reset_is_not_a_parameter_gesture() {
        use daw_unit_core::MeterStorage;
        let (ctx, model, mut editor) = setup();
        model.output_meter.store([1.5, 0.0, 0.0]);
        model.output_meter.initialize([0.25, 0.0, 0.0]);
        frame(&ctx, &mut editor, vec![], true);
        let pos = Pos2::new(225.0, 4.0);
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut editor,
                vec![Event::PointerMoved(pos), pointer(pos, pressed)],
                true,
            );
        }
        assert_eq!(model.output_meter.boost_peaks()[0].peak(), 0.0);
        assert_eq!(model.output_meter.readings()[0], 0.25);
        assert!(events(&model).is_empty());
        assert_eq!(editor.state, PluginState::default());
    }
