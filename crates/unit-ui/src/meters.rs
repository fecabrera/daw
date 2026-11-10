//! Shared level rails for effect Out strips and band monitors.
use crate::theme;
use egui::{Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
pub const MIN_DBFS: f32 = -60.0;
/// DAW shells use separate stereo gap meters; standalone bodies retain output rails.
pub const HIDE_OUTPUT_LEVELS_TAG: &str = "hide-unit-output-levels";

/// Hide queued or stale readings as soon as the unit's saved power state is off.
pub fn unit_reading<T: Default>(enabled: bool, reading: T) -> T {
    if enabled { reading } else { T::default() }
}

/// Fit a four-point monitor above standard knob rows without changing unit height.
pub fn unit_monitor_rail(strip: Rect, margin: i8) -> Rect {
    Rect::from_min_size(
        strip.min + Vec2::new(f32::from(margin), crate::layout::UNIT_STRIP_VERTICAL_MARGIN),
        Vec2::new(strip.width() - 2.0 * f32::from(margin), 4.0),
    )
}

pub fn horizontal_rail(ui: &Ui, rail: Rect, progress: f32, color: egui::Color32) {
    ui.painter()
        .rect_filled(rail, 1.0, theme::palette(ui.ctx()).input);
    if progress > 0.0 {
        ui.painter().rect_filled(
            Rect::from_min_size(rail.min, Vec2::new(rail.width() * progress, rail.height())),
            1.0,
            color,
        );
    }
}

/// Disabled bands share the theme's secondary color for every monitor type.
pub fn band_color(ui: &Ui, enabled: bool, active: egui::Color32) -> egui::Color32 {
    if enabled {
        active
    } else {
        theme::palette(ui.ctx()).secondary
    }
}

/// Stereo peak level using the shared −60 to 0 dBFS scale; retain headroom in readings.
pub fn band_level_rail(ui: &mut Ui, rail: Rect, name: &str, peak: f32, enabled: bool) {
    let peak = if peak.is_finite() { peak.max(0.0) } else { 0.0 };
    let palette = theme::palette(ui.ctx());
    horizontal_rail(
        ui,
        rail,
        position_for_peak(peak),
        band_color(
            ui,
            enabled,
            if peak > 1.0 {
                palette.error
            } else {
                palette.meter
            },
        ),
    );
    let db = if peak > 0.0 {
        20.0 * peak.log10()
    } else {
        f32::NEG_INFINITY
    };
    let level = peak_text(peak);
    let response = ui.interact(rail, ui.id().with(("band-level", name)), Sense::hover());
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::WidgetType::ProgressIndicator,
            ui.is_enabled(),
            format!("{name} output level: {level} dBFS"),
        );
        info.value = db.is_finite().then_some(f64::from(db));
        info
    });
    response.on_hover_text(format!(
        "{name} output level: {level} dBFS (display range −60 to 0 dBFS)"
    ));
}

pub fn position_for_dbfs(db: f32) -> f32 {
    ((db - MIN_DBFS) / -MIN_DBFS).clamp(0.0, 1.0)
}

pub fn position_for_peak(peak: f32) -> f32 {
    if peak > 0.0 {
        position_for_dbfs(20.0 * peak.log10())
    } else {
        0.0
    }
}

pub fn peak_text(peak: f32) -> String {
    if peak > 0.0 {
        let db = 20.0 * peak.log10();
        if db > 0.0 {
            format!("+{db:.1}")
        } else {
            format!("{db:.1}")
        }
    } else {
        "−∞".into()
    }
}

/// Current linked-stereo level and the shared click-resettable sample peak.
#[derive(Clone, Copy)]
pub struct OutputLevel<'a> {
    current: f32,
    peak: Option<&'a daw_unit_core::PeakHold>,
    reset_request: Option<&'a std::sync::atomic::AtomicBool>,
    reset_mask: Option<(&'a std::sync::atomic::AtomicU64, u64)>,
    stereo: Option<&'a daw_unit_core::BandMeter<3>>,
}
impl<'a> OutputLevel<'a> {
    pub fn with_reset_request(mut self, request: &'a std::sync::atomic::AtomicBool) -> Self {
        self.reset_request = Some(request);
        self
    }
    pub fn with_reset_mask(mut self, request: &'a std::sync::atomic::AtomicU64, mask: u64) -> Self {
        self.reset_mask = Some((request, mask));
        self
    }
    pub fn new(current: f32, peak: Option<&'a daw_unit_core::PeakHold>) -> Self {
        Self {
            current,
            peak,
            reset_request: None,
            reset_mask: None,
            stereo: None,
        }
    }
    /// Linked Out-strip display shares reset with the separate channel indicators.
    pub fn linked(current: f32, meter: Option<&'a daw_unit_core::BandMeter<3>>) -> Self {
        let mut reading = Self::new(current, meter.map(|m| &m.boost_peaks()[0]));
        reading.stereo = meter;
        reading
    }
    fn reset(&self) {
        if let Some(peak) = self.peak {
            peak.reset_peak();
        }
        if let Some(meter) = self.stereo {
            for peak in &meter.boost_peaks()[1..] {
                peak.reset_peak();
            }
        }
    }
}
impl From<f32> for OutputLevel<'_> {
    fn from(current: f32) -> Self {
        Self::new(current, None)
    }
}

/// Shared one-point held-peak marker for output, gain-change, and reduction rails.
pub fn peak_marker(ui: &Ui, rail: Rect, x: f32) {
    ui.painter().vline(
        x.clamp(rail.left() + 0.5, rail.right() - 0.5),
        rail.y_range(),
        egui::Stroke::new(1.0, theme::palette(ui.ctx()).secondary),
    );
}

/// Vertical level-rail counterpart of the shared one-point held-peak marker.
pub fn vertical_peak_marker(ui: &Ui, rail: Rect, y: f32) {
    ui.painter().hline(
        rail.x_range(),
        y.clamp(rail.top() + 0.5, rail.bottom() - 0.5),
        egui::Stroke::new(1.0, theme::palette(ui.ctx()).secondary),
    );
}

/// Standard effect Out-strip monitor. Hide stale values immediately on bypass.
/// The rail occupies the existing top margin, so controls and strip size do not move.
pub fn effect_output_level<'a>(
    ui: &mut Ui,
    strip: Rect,
    margin: i8,
    reading: impl Into<OutputLevel<'a>>,
    enabled: bool,
) -> Rect {
    if ui
        .stack()
        .iter()
        .any(|node| node.tags().contains(HIDE_OUTPUT_LEVELS_TAG))
    {
        return Rect::NOTHING;
    }
    let rail = unit_monitor_rail(strip, margin);
    band_output_level(ui, rail, reading.into(), enabled, true, "Output");
    rail
}
/// The standard level rail and held peak, with inactive-band colors independent of unit power.
pub fn band_output_level(
    ui: &mut Ui,
    rail: Rect,
    reading: OutputLevel<'_>,
    enabled: bool,
    band_enabled: bool,
    name: &str,
) -> Response {
    output_level(ui, rail, reading, enabled, band_enabled, name, None)
}

/// Independent channels plus their shared linked peak for the existing Out strip.
#[derive(Clone, Copy)]
pub struct StereoOutputLevel<'a> {
    current: [f32; 2],
    meter: Option<&'a daw_unit_core::BandMeter<3>>,
}
impl<'a> StereoOutputLevel<'a> {
    pub fn new(current: [f32; 2], meter: Option<&'a daw_unit_core::BandMeter<3>>) -> Self {
        Self { current, meter }
    }
    fn reset(&self) {
        if let Some(meter) = self.meter {
            for peak in meter.boost_peaks() {
                peak.reset_peak();
            }
        }
    }
}

/// Stereo output rails centered in the existing sixteen-point unit gap.
pub fn unit_gap_rect(unit: Rect) -> Rect {
    Rect::from_center_size(
        egui::pos2(unit.right() + 8.0, unit.center().y),
        Vec2::new(8.0, 64.0),
    )
}

/// Paint without allocating space, retaining unit geometry and lane padding.
pub fn unit_gap_level(
    ui: &mut Ui,
    unit: Rect,
    id: egui::Id,
    reading: StereoOutputLevel<'_>,
    enabled: bool,
) -> Response {
    let bounds = unit_gap_rect(unit);
    if !enabled {
        reading.reset();
    }
    let channels: [Response; 2] = std::array::from_fn(|channel| {
        let rail = Rect::from_min_size(
            bounds.min + Vec2::new(channel as f32 * 5.0, 0.0),
            Vec2::new(3.0, 64.0),
        );
        output_level(
            ui,
            rail,
            OutputLevel::new(
                reading.current[channel],
                reading.meter.map(|m| &m.boost_peaks()[channel + 1]),
            ),
            enabled,
            true,
            if channel == 0 {
                "Left output"
            } else {
                "Right output"
            },
            Some(id.with(channel)),
        )
    });
    let response = ui.interact(bounds, id, Sense::click());
    if ui.is_enabled() && ui.input(|i| i.focused) {
        if response.is_pointer_button_down_on() || response.clicked() {
            response.request_focus();
        }
        if enabled && (response.clicked() || channels.iter().any(Response::clicked)) {
            reading.reset();
            ui.ctx().request_repaint();
        }
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            "Stereo output. Click to reset both channel peaks",
        )
    });
    let values = unit_reading(enabled, reading.current);
    let held = unit_reading(
        enabled,
        reading.meter.map_or([0.0; 2], |m| {
            [m.boost_peaks()[1].peak(), m.boost_peaks()[2].peak()]
        }),
    );
    response.on_hover_text(format!("Stereo output (dBFS). Left: {}, peak {}. Right: {}, peak {}. Click to reset both peaks. Display range −60 to 0 dBFS.", peak_text(values[0]), peak_text(held[0]), peak_text(values[1]), peak_text(held[1])))
}

fn output_level(
    ui: &mut Ui,
    rail: Rect,
    reading: OutputLevel<'_>,
    enabled: bool,
    band_enabled: bool,
    name: &str,
    vertical: Option<egui::Id>,
) -> Response {
    if !enabled {
        reading.reset();
    }
    let current = unit_reading(enabled, reading.current);
    let current = if current.is_finite() {
        current.max(0.0)
    } else {
        0.0
    };
    let held = unit_reading(enabled, reading.peak.map_or(0.0, |p| p.peak()));
    let palette = theme::palette(ui.ctx());
    let color = band_color(
        ui,
        band_enabled,
        if current > 1.0 {
            palette.error
        } else {
            palette.meter
        },
    );
    let progress = position_for_peak(current);
    if vertical.is_some() {
        ui.painter().rect_filled(rail, 1.0, palette.input);
        if progress > 0.0 {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    egui::pos2(rail.left(), rail.bottom() - rail.height() * progress),
                    rail.right_bottom(),
                ),
                1.0,
                color,
            );
        }
    } else {
        horizontal_rail(ui, rail, progress, color);
    }
    if held > 0.0 {
        if vertical.is_some() {
            vertical_peak_marker(
                ui,
                rail,
                rail.bottom() - rail.height() * position_for_peak(held),
            );
        } else {
            peak_marker(
                ui,
                rail,
                rail.left() + rail.width() * position_for_peak(held),
            );
        }
    }
    let response = ui.interact(
        rail,
        vertical.unwrap_or_else(|| ui.id().with(("effect-output-level", name))),
        Sense::click(),
    );
    if vertical.is_some()
        && ui.is_enabled()
        && ui.input(|i| i.focused)
        && (response.is_pointer_button_down_on() || response.clicked())
    {
        response.request_focus();
    }
    if enabled
        && response.clicked_by(egui::PointerButton::Primary)
        && ui.is_enabled()
        && ui.input(|i| i.focused)
    {
        reading.reset();
        if let Some(request) = reading.reset_request {
            request.store(true, std::sync::atomic::Ordering::Release);
        }
        if let Some((request, mask)) = reading.reset_mask {
            request.fetch_or(mask, std::sync::atomic::Ordering::Release);
        }
        ui.ctx().request_repaint();
    }
    let level = peak_text(current);
    let peak = peak_text(held);
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("{name} level: {level} dBFS. Held peak: {peak} dBFS. Click to reset peak"),
        );
        if current > 0.0 {
            info.value = Some(f64::from(20.0 * current.log10()));
        }
        info
    });
    response.on_hover_text(format!("{name} level: {level} dBFS. Held peak: {peak} dBFS. Click to reset peak. Display range −60 to 0 dBFS."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use daw_unit_core::{MeterStorage, PeakMeter};
    #[test]
    fn daw_policy_hides_output_only_and_standalone_keeps_it() {
        for hidden in [false, true] {
            let ctx = egui::Context::default();
            theme::configure(&ctx);
            crate::fonts::configure(&ctx);
            let strip = Rect::from_min_size(Pos2::ZERO, Vec2::new(90.0, 218.0));
            let band = unit_monitor_rail(strip.translate(Vec2::new(90.0, 0.0)), 10);
            let reduction = band.translate(Vec2::new(90.0, 0.0));
            let mut bounds = None;
            let mut out = ctx.run_ui(Default::default(), |ui| {
                let info = if hidden {
                    egui::UiStackInfo::default().with_tag(HIDE_OUTPUT_LEVELS_TAG)
                } else {
                    Default::default()
                };
                ui.scope_builder(egui::UiBuilder::new().ui_stack_info(info), |ui| {
                    let output = effect_output_level(ui, strip, 10, 0.5, true);
                    let band = band_output_level(ui, band, 0.5.into(), true, true, "Band");
                    let reduction = gain_reduction_rail(ui, reduction, 12.0.into());
                    bounds = Some((output, band.rect, reduction.rect));
                });
            });
            out.textures_delta.clear();
            let (output, band_bounds, reduction_bounds) = bounds.unwrap();
            assert_eq!(output.is_positive(), !hidden);
            assert_eq!(band_bounds, band);
            assert_eq!(reduction_bounds, reduction);
            for rail in [unit_monitor_rail(strip, 10), band, reduction] {
                let painted = out.shapes.iter().any(|s| {
                    matches!(&s.shape, egui::Shape::Rect(r)
                    if r.rect == rail && r.fill == theme::palette(&ctx).input)
                });
                assert_eq!(painted, !hidden || rail != unit_monitor_rail(strip, 10));
            }
        }
    }
    #[test]
    fn linked_output_rail_resets_both_stereo_channel_holds() {
        let ctx = egui::Context::default();
        theme::configure(&ctx);
        crate::fonts::configure(&ctx);
        let meter = daw_unit_core::BandMeter::<3>::default();
        meter.store([1.5, 1.5, 0.5]);
        let strip = Rect::from_min_size(Pos2::ZERO, Vec2::new(90.0, 218.0));
        let point = unit_monitor_rail(strip, 10).center();
        for phase in 0..3 {
            let events = if phase == 0 {
                vec![]
            } else {
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: phase == 1,
                        modifiers: Default::default(),
                    },
                ]
            };
            let mut out = ctx.run_ui(
                egui::RawInput {
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    effect_output_level(
                        ui,
                        strip,
                        10,
                        OutputLevel::linked(0.25, Some(&meter)),
                        true,
                    );
                },
            );
            out.textures_delta.clear();
            assert_eq!(
                meter.boost_peaks().each_ref().map(|p| p.peak()),
                if phase < 2 { [1.5, 1.5, 0.5] } else { [0.0; 3] }
            );
        }
    }
    #[test]
    fn gap_meter_keeps_layout_and_shared_peak_reset_for_all_unit_sizes() {
        for (width, height) in [(300.0, 218.0), (26.0, 218.0), (300.0, 42.0), (26.0, 42.0)] {
            for click_x in [1.5, 4.0, 6.5] {
                let ctx = egui::Context::default();
                theme::configure(&ctx);
                crate::fonts::configure(&ctx);
                let meter = daw_unit_core::BandMeter::<3>::default();
                meter.store([1.5, 1.5, 0.5]);
                let unit = Rect::from_min_size(egui::pos2(20.0, 13.0), Vec2::new(width, height));
                let expected = unit_gap_rect(unit);
                assert_eq!(expected.size(), Vec2::new(8.0, 64.0));
                assert_eq!(expected.center().y, unit.center().y);
                assert_eq!(expected.left() - unit.right(), 4.0);
                for phase in 0..4 {
                    let center = Pos2::new(expected.left() + click_x, expected.center().y);
                    let events = match phase {
                        1 | 2 => vec![
                            egui::Event::PointerMoved(center),
                            egui::Event::PointerButton {
                                pos: center,
                                button: egui::PointerButton::Primary,
                                pressed: phase == 1,
                                modifiers: Default::default(),
                            },
                        ],
                        _ => vec![],
                    };
                    if phase == 3 {
                        meter.store([2.0, 2.0, 0.5]);
                    }
                    let mut geometry = None;
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(800.0, 400.0),
                            )),
                            focused: true,
                            events,
                            ..Default::default()
                        },
                        |ui| {
                            let before = ui.min_rect();
                            let response = unit_gap_level(
                                ui,
                                unit,
                                egui::Id::new("gap"),
                                StereoOutputLevel::new([1.25, 0.125], Some(&meter)),
                                phase != 3,
                            );
                            geometry = Some((response.rect, before, ui.min_rect()));
                        },
                    );
                    output.textures_delta.clear();
                    let (rect, before, after) = geometry.unwrap();
                    assert_eq!(rect, expected);
                    assert_eq!(after, before, "meter must not expand the sixteen-point gap");
                    let rails: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|s| {
                            if let egui::Shape::Rect(r) = &s.shape
                                && r.fill == theme::palette(&ctx).input
                                && expected.contains_rect(r.rect)
                            {
                                Some(r.rect)
                            } else {
                                None
                            }
                        })
                        .collect();
                    assert_eq!(rails.len(), 2);
                    assert_eq!(rails[0].size(), Vec2::new(3.0, 64.0));
                    assert_eq!(rails[1].left() - rails[0].right(), 2.0);
                    if phase < 3 {
                        assert!(
                            output
                                .shapes
                                .iter()
                                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                        if r.rect == rails[0] && r.fill == theme::palette(&ctx).error))
                        );
                        assert!(
                            output
                                .shapes
                                .iter()
                                .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
                        if rails[1].contains_rect(r.rect) && r.fill == theme::palette(&ctx).meter))
                        );
                    }
                    assert_eq!(
                        meter.boost_peaks().each_ref().map(|p| p.peak()),
                        if phase < 2 { [1.5, 1.5, 0.5] } else { [0.0; 3] }
                    );
                    let markers: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|s| {
                            if let egui::Shape::LineSegment { points, stroke } = &s.shape {
                                Some((*points, *stroke))
                            } else {
                                None
                            }
                        })
                        .collect();
                    assert_eq!(markers.len(), 2 * usize::from(phase < 3));
                    if phase < 3 {
                        assert_ne!(markers[0].0[0].y, markers[1].0[0].y);
                    }
                    if let Some((points, stroke)) = markers.first() {
                        assert_eq!(points[0].y, points[1].y);
                        assert_eq!(stroke.color, theme::palette(&ctx).secondary);
                    }
                }
            }
        }
    }
    fn frame(
        ctx: &egui::Context,
        meter: &PeakMeter,
        current: f32,
        enabled: bool,
        events: Vec<egui::Event>,
    ) -> Vec<egui::epaint::ClippedShape> {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(90.0, 218.0),
                )),
                events,
                focused: true,
                ..Default::default()
            },
            |ui| {
                effect_output_level(
                    ui,
                    Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(90.0, 218.0)),
                    10,
                    OutputLevel::new(current, Some(meter.hold())),
                    enabled,
                );
            },
        );
        output.textures_delta.clear();
        std::mem::take(&mut output.shapes)
    }
    #[test]
    fn standard_output_rail_preserves_geometry_overload_and_bypass_in_all_themes() {
        for (base, palette) in [
            (egui::Theme::Dark, theme::DARK),
            (egui::Theme::Light, theme::LIGHT),
            (egui::Theme::Light, theme::MIST),
        ] {
            let ctx = egui::Context::default();
            crate::fonts::configure(&ctx);
            theme::configure(&ctx);
            theme::apply(&ctx, base, palette, theme::ACCENT);
            let meter = PeakMeter::default();
            meter.store(2.0);
            for accent in [theme::ACCENT, theme::PLUGIN_ACCENT] {
                theme::apply(&ctx, base, palette, accent);
                for (current, expected) in [
                    (0.5, palette.meter),
                    (1.0, palette.meter),
                    (1.01, palette.error),
                ] {
                    let shapes = frame(&ctx, &meter, current, true, vec![]);
                    let rail = unit_monitor_rail(
                        Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(90.0, 218.0)),
                        10,
                    );
                    let fill = Rect::from_min_size(
                        rail.min,
                        Vec2::new(rail.width() * position_for_peak(current), rail.height()),
                    );
                    assert!(shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == fill && r.fill == expected)));
                    assert!(!shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect.intersects(rail) && r.fill == accent)));
                }
            }
            let shapes = frame(&ctx, &meter, 2.0, true, vec![]);
            let rail = Rect::from_min_size(egui::pos2(10.0, 4.0), Vec2::new(70.0, 4.0));
            assert!(shapes.iter().any(
                |s| matches!(&s.shape,egui::Shape::Rect(r) if r.rect==rail && r.fill==palette.error)
            ));
            assert!(shapes.iter().any(|s|matches!(&s.shape,egui::Shape::LineSegment{points,stroke} if points[0].x==rail.right()-0.5 && stroke.color==palette.secondary)));
            let shapes = frame(&ctx, &meter, 2.0, false, vec![]);
            assert_eq!(meter.peak(), 0.0);
            assert!(
                !shapes
                    .iter()
                    .any(|s| matches!(&s.shape, egui::Shape::LineSegment { .. }))
            );
            assert!(!shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.rect.intersects(rail) && [palette.error, palette.meter, theme::ACCENT, theme::PLUGIN_ACCENT].contains(&r.fill))));
        }
    }
    #[test]
    fn band_levels_share_track_colors_independently_of_accent_and_keep_disabled_grey() {
        for (base, palette) in [
            (egui::Theme::Dark, theme::DARK),
            (egui::Theme::Light, theme::LIGHT),
            (egui::Theme::Light, theme::MIST),
        ] {
            for accent in [theme::ACCENT, theme::PLUGIN_ACCENT] {
                let ctx = egui::Context::default();
                crate::fonts::configure(&ctx);
                theme::configure(&ctx);
                theme::apply(&ctx, base, palette, accent);
                for (peak, enabled) in [
                    (0.5, true),
                    (1.0, true),
                    (1.01, true),
                    (0.5, false),
                    (1.01, false),
                ] {
                    let rail = Rect::from_min_size(egui::pos2(10.0, 4.0), Vec2::new(70.0, 4.0));
                    let mut output = ctx.run_ui(Default::default(), |ui| {
                        band_level_rail(ui, rail, "Band", peak, enabled)
                    });
                    let expected = if !enabled {
                        palette.secondary
                    } else if peak > 1.0 {
                        palette.error
                    } else {
                        palette.meter
                    };
                    let fill = Rect::from_min_size(
                        rail.min,
                        Vec2::new(rail.width() * position_for_peak(peak), rail.height()),
                    );
                    assert!(output.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == fill && r.fill == expected)));
                    output.textures_delta.clear();
                }
            }
        }
    }
    #[test]
    fn clicking_the_standard_rail_resets_only_the_shared_peak() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        theme::configure(&ctx);
        let meter = PeakMeter::default();
        meter.store(0.8);
        meter.initialize(0.2);
        frame(&ctx, &meter, 0.2, true, vec![]);
        let pos = egui::pos2(45.0, 4.0);
        for pressed in [true, false] {
            frame(
                &ctx,
                &meter,
                0.2,
                true,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        assert_eq!(meter.peak(), 0.0);
        assert_eq!(meter.reading(), 0.2);
    }
}

/// Signed band gain monitor. Center is unity; left is reduction, right is boost.
pub fn gain_change(
    ui: &mut Ui,
    rail: Rect,
    name: &str,
    reading: GainReduction<'_>,
    enabled: bool,
) -> Response {
    let db = reading.signed_current();
    let response = ui.interact(rail, ui.id().with(("gain-change", name)), Sense::click());
    reading.reset_on_click(ui, &response);
    let palette = theme::palette(ui.ctx());
    let painter = ui.painter();
    painter.rect_filled(rail, 1.0, palette.input);
    let x = rail.center().x + rail.width() * 0.5 * (db / 24.0).clamp(-1.0, 1.0);
    let fill = Rect::from_min_max(
        Pos2::new(x.min(rail.center().x), rail.top()),
        Pos2::new(x.max(rail.center().x), rail.bottom()),
    );
    painter.rect_filled(
        fill,
        1.0,
        band_color(
            ui,
            enabled,
            if db > 0.0 {
                palette.meter
            } else {
                theme::accent(ui.ctx())
            },
        ),
    );
    painter.vline(
        rail.center().x,
        rail.y_range(),
        Stroke::new(1.0, palette.secondary),
    );
    reading.paint_peak(ui, rail, rail.center().x, -rail.width() * 0.5);
    paint_peak_hold(
        ui,
        rail,
        rail.center().x,
        rail.width() * 0.5,
        reading.held_boost(),
    );
    let held = reading.held();
    let boost = reading.held_boost();
    reading.describe(ui, response, &format!("{name} gain change"), db,
        format!("{name} compression gain: {db:+.1} dB. Peak reduction: {held:.1} dB. Peak boost: {boost:.1} dB. Click to reset both peaks. Left: reduction; right: boost. Display ±24 dB; before Dry/Wet."))
}

#[derive(Clone, Copy)]
pub struct GainReduction<'a> {
    current_db: f32,
    peak: Option<&'a daw_unit_core::PeakHold>,
    boost: Option<&'a daw_unit_core::PeakHold>,
    enabled: bool,
}
impl<'a> GainReduction<'a> {
    pub fn new(current_db: f32, peak: Option<&'a daw_unit_core::PeakMeter>) -> Self {
        Self {
            current_db,
            peak: peak.map(daw_unit_core::PeakMeter::hold),
            boost: None,
            enabled: true,
        }
    }
    /// Use a held reading from a repeated-band telemetry cell.
    pub fn with_peak(mut self, peak: Option<&'a daw_unit_core::PeakHold>) -> Self {
        self.peak = peak;
        self
    }
    pub fn for_unit(mut self, enabled: bool) -> Self {
        self.current_db = unit_reading(enabled, self.current_db);
        self.enabled = enabled;
        if !enabled {
            for peak in [self.peak, self.boost].into_iter().flatten() {
                peak.reset_peak();
            }
        }
        self
    }
    fn current(self) -> f32 {
        self.signed_current().max(0.0)
    }
    fn signed_current(self) -> f32 {
        if self.current_db.is_finite() {
            self.current_db
        } else {
            0.0
        }
    }
    fn held(self) -> f32 {
        unit_reading(self.enabled, self.peak.map_or(0.0, |peak| peak.peak()))
    }
    fn held_boost(self) -> f32 {
        unit_reading(self.enabled, self.boost.map_or(0.0, |peak| peak.peak()))
    }
    fn interact(self, ui: &Ui, response: Response, description: &str) -> Response {
        self.reset_on_click(ui, &response);
        let db = self.current();
        let held = self.held();
        self.describe(ui, response, description, db, format!(
            "{description}: {db:.1} dB. Peak: {held:.1} dB. Click to reset peak; 0–24 dB display scale."
        ))
    }
    fn reset_on_click(self, ui: &Ui, response: &Response) {
        if response.clicked_by(egui::PointerButton::Primary)
            && ui.is_enabled()
            && ui.input(|input| input.focused)
            && self.enabled
        {
            for peak in [self.peak, self.boost].into_iter().flatten() {
                peak.reset_peak();
            }
        }
    }
    fn describe(
        self,
        ui: &Ui,
        response: Response,
        description: &str,
        db: f32,
        tooltip: String,
    ) -> Response {
        response.widget_info(|| {
            let mut info = egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!(
                    "{description}. Click to reset {}",
                    if self.boost.is_some() {
                        "both peaks"
                    } else {
                        "peak"
                    }
                ),
            );
            info.value = Some(f64::from(db));
            info
        });
        crate::control_capture::record(ui, &response);
        response.on_hover_text(tooltip)
    }
    fn paint_peak(self, ui: &Ui, rail: Rect, origin: f32, span: f32) {
        paint_peak_hold(ui, rail, origin, span, self.held());
    }
}
fn paint_peak_hold(ui: &Ui, rail: Rect, origin: f32, span: f32, held: f32) {
    if held > 0.0 {
        let x =
            (origin + span * (held / 24.0).min(1.0)).clamp(rail.left() + 0.5, rail.right() - 0.5);
        peak_marker(ui, rail, x);
    }
}
#[derive(Clone, Copy)]
pub struct BandGainChange<'a>([GainReduction<'a>; 3]);
impl<'a> BandGainChange<'a> {
    pub fn new(current: [f32; 3], meter: Option<&'a daw_unit_core::BandMeter<3>>) -> Self {
        Self(std::array::from_fn(|i| GainReduction {
            current_db: current[i],
            peak: meter.map(|m| &m.reduction_peaks()[i]),
            boost: meter.map(|m| &m.boost_peaks()[i]),
            enabled: true,
        }))
    }
    pub fn for_unit(self, enabled: bool) -> Self {
        Self(self.0.map(|reading| reading.for_unit(enabled)))
    }
    pub fn band(self, index: usize) -> GainReduction<'a> {
        self.0[index]
    }
}
impl From<[f32; 3]> for BandGainChange<'_> {
    fn from(current: [f32; 3]) -> Self {
        Self::new(current, None)
    }
}
impl From<f32> for GainReduction<'_> {
    fn from(current_db: f32) -> Self {
        Self::new(current_db, None)
    }
}

/// Compression before Dry/Wet. Click resets only the held maximum.
pub fn gain_reduction(ui: &mut Ui, reading: GainReduction<'_>) -> Response {
    gain_reduction_sized(ui, reading, crate::knobs::CHOICE_SIZE)
}
/// Shared reduction monitor with the compact control footprint.
pub fn gain_reduction_sized(ui: &mut Ui, reading: GainReduction<'_>, size: f32) -> Response {
    let db = reading.current();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let response = reading.interact(ui, response, "Gain reduction before Dry/Wet");
    let palette = theme::palette(ui.ctx());
    let painter = ui.painter();
    let rail = Rect::from_center_size(rect.center(), Vec2::new(rect.width() - 8.0, 8.0));
    painter.rect_filled(rail, 1.0, palette.input);
    let fill = Rect::from_min_size(
        rail.min,
        Vec2::new(rail.width() * (db / 24.0).min(1.0), rail.height()),
    );
    if fill.width() > 0.0 {
        painter.rect_filled(fill, 1.0, theme::accent(ui.ctx()));
    }
    reading.paint_peak(ui, rail, rail.left(), rail.width());
    painter.text(
        rect.center_top(),
        egui::Align2::CENTER_TOP,
        if db > 0.0 {
            format!("−{db:.1} dB")
        } else {
            "0.0 dB".into()
        },
        crate::fonts::numeric(9.0 * size / crate::knobs::CHOICE_SIZE),
        palette.text,
    );
    for (position, align, label) in [
        (rect.left_bottom(), egui::Align2::LEFT_BOTTOM, "0"),
        (rect.center_bottom(), egui::Align2::CENTER_BOTTOM, "GR"),
        (rect.right_bottom(), egui::Align2::RIGHT_BOTTOM, "24"),
    ] {
        painter.text(
            position,
            align,
            label,
            crate::fonts::numeric(9.0 * size / crate::knobs::CHOICE_SIZE),
            palette.secondary,
        );
    }
    response
}

/// Thin reduction monitor fits above standard knob rows without changing unit height.
pub fn gain_reduction_rail(ui: &mut Ui, rail: Rect, reading: GainReduction<'_>) -> Response {
    let db = reading.current();
    let response = ui.interact(rail, ui.id().with("limiter-reduction"), Sense::click());
    let response = reading.interact(ui, response, "Gain reduction");
    horizontal_rail(ui, rail, (db / 24.0).min(1.0), theme::accent(ui.ctx()));
    reading.paint_peak(ui, rail, rail.left(), rail.width());
    response
}

#[cfg(test)]
mod band_output_tests {
    use super::*;
    use daw_unit_core::{BandMeter, MeterStorage};
    #[test]
    fn band_level_hold_uses_shared_marker_grey_disabled_color_and_independent_click_reset() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        theme::configure(&ctx);
        let meter = BandMeter::<3>::default();
        meter.store([0.8, 6.0, 0.0]);
        meter.initialize([0.1, 3.0, 0.0]);
        let requests = std::sync::atomic::AtomicU64::new(0);
        let rail = Rect::from_min_size(egui::pos2(8.0, 4.0), egui::vec2(145.0, 4.0));
        for phase in 0..3 {
            let events = if phase == 0 {
                vec![]
            } else {
                vec![
                    egui::Event::PointerMoved(rail.center()),
                    egui::Event::PointerButton {
                        pos: rail.center(),
                        button: egui::PointerButton::Primary,
                        pressed: phase == 1,
                        modifiers: Default::default(),
                    },
                ]
            };
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(200.0, 100.0))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ui| {
                    band_output_level(
                        ui,
                        rail,
                        OutputLevel::new(meter.readings()[0], Some(&meter.boost_peaks()[0]))
                            .with_reset_mask(&requests, 1 << 63),
                        true,
                        false,
                        "Band 64",
                    );
                },
            );
            if phase == 0 {
                let expected = rail.left() + rail.width() * position_for_peak(0.8);
                assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::LineSegment{points,stroke} if (points[0].x-expected).abs()<0.01 && stroke.color==theme::palette(&ctx).secondary)));
                assert!(out.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Rect(r) if r.fill==theme::palette(&ctx).secondary)));
            }
            out.textures_delta.clear();
        }
        assert_eq!(meter.boost_peaks()[0].peak(), 0.0);
        assert_eq!(meter.readings(), [0.1, 3.0, 0.0]);
        assert_eq!(meter.boost_peaks()[1].peak(), 6.0);
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1 << 63);
    }
}
