use egui::{Event, Rect, Vec2};

pub fn scenarios(variants: usize) -> impl Iterator<Item = (u32, f32, usize)> {
    (0..3).flat_map(move |appearance| {
        [1.0, 1.5, 2.0]
            .into_iter()
            .flat_map(move |scale| (0..variants).map(move |variant| (appearance, scale, variant)))
    })
}

pub fn context(appearance: u32, scale: f32) -> egui::Context {
    let ctx = egui::Context::default();
    daw_unit_ui::fonts::configure(&ctx);
    daw_unit_ui::theme::configure(&ctx);
    daw_unit_ui::theme::apply(
        &ctx,
        if appearance == 0 {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        },
        if appearance == 2 {
            daw_unit_ui::theme::MIST
        } else {
            daw_unit_ui::theme::LIGHT
        },
        daw_unit_ui::theme::PLUGIN_ACCENT,
    );
    ctx.set_pixels_per_point(scale);
    ctx.all_styles_mut(|style| style.animation_time = 0.0);
    ctx
}
pub fn paint(
    ctx: &egui::Context,
    size: Vec2,
    draw: impl FnMut(&mut egui::Ui),
) -> Vec<egui::ClippedPrimitive> {
    ctx.tessellate(
        frame(ctx, size, vec![Event::PointerGone], None, draw),
        ctx.pixels_per_point(),
    )
}

pub fn frame(
    ctx: &egui::Context,
    size: Vec2,
    events: Vec<Event>,
    time: Option<f64>,
    draw: impl FnMut(&mut egui::Ui),
) -> Vec<egui::epaint::ClippedShape> {
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            focused: true,
            events,
            time,
            ..Default::default()
        },
        draw,
    );
    out.textures_delta.clear();
    out.shapes
}

pub fn primary_button(pos: egui::Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

pub fn compare(
    appearance: u32,
    scale: f32,
    variant: usize,
    size: Vec2,
    actual: impl FnMut(&mut egui::Ui),
    mut expected: impl FnMut(&mut egui::Ui),
) {
    let actual = paint(&context(appearance, scale), size, actual);
    let expected = paint(&context(appearance, scale), size, |ui| {
        ui.painter().rect_filled(
            ui.max_rect(),
            0.0,
            daw_unit_ui::theme::unit_separator(ui.ctx()),
        );
        expected(ui);
    });
    assert_eq!(
        actual.len(),
        expected.len(),
        "primitive count: appearance={appearance}, scale={scale}, snapshot={variant}"
    );
    for (i, (a, b)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(
            a.clip_rect, b.clip_rect,
            "clip {i}: appearance={appearance}, scale={scale}, snapshot={variant}"
        );
        match (&a.primitive, &b.primitive) {
            (egui::epaint::Primitive::Mesh(a), egui::epaint::Primitive::Mesh(b)) => {
                assert_eq!(a.texture_id, b.texture_id, "texture {i}");
                assert_eq!(
                    a.indices, b.indices,
                    "triangles {i}: appearance={appearance}, scale={scale}, snapshot={variant}"
                );
                assert_eq!(a.vertices.len(), b.vertices.len(), "vertices {i}");
                for (n, (a, b)) in a.vertices.iter().zip(&b.vertices).enumerate() {
                    assert!(
                        a.pos.distance(b.pos) < 0.001
                            && a.uv.distance(b.uv) < 0.001
                            && a.color == b.color,
                        "vertex {i}/{n}: actual={a:?}, builtin={b:?}, appearance={appearance}, scale={scale}, snapshot={variant}"
                    );
                }
            }
            _ => panic!("unexpected paint callback in unit body"),
        }
    }
}
