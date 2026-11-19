use crate::{drag::AxisLock, layout, rows, spectrum_graph, theme};
use daw_unit_core::{EnvelopeSettings, EnvelopeTiming};
use egui::{Align2, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};

const MIN_TIMED_WIDTH: f32 = 0.045;
const SUSTAIN_WIDTH: f32 = 0.20;
const SAMPLES_PER_STAGE: usize = 25;

#[derive(Clone, Copy)]
struct Drag {
    before: EnvelopeSettings,
    handle: usize,
    origin: Pos2,
    direction: Option<AxisLock>,
    plot: Rect,
    frame: u64,
}

fn times(settings: EnvelopeSettings) -> [f32; 3] {
    let [attack, decay] = match settings.timing {
        EnvelopeTiming::Time => [settings.attack.time_ms, settings.decay.time_ms],
        // Note-relative previews use a one-second reference note.
        EnvelopeTiming::Note => [settings.attack_note * 1000.0, settings.decay_note * 1000.0],
    };
    [attack, decay, settings.release.time_ms]
}

fn stage_edges(settings: EnvelopeSettings) -> [f32; 5] {
    let durations = times(settings);
    let total = durations.iter().sum::<f32>().max(1.0);
    let [attack, decay, release] = durations.map(|duration| {
        MIN_TIMED_WIDTH + (1.0 - SUSTAIN_WIDTH - 3.0 * MIN_TIMED_WIDTH) * duration / total
    });
    [0.0, attack, attack + decay, 1.0 - release, 1.0]
}

fn point(plot: Rect, x: f32, level: f32) -> Pos2 {
    egui::pos2(
        plot.left() + x * plot.width(),
        plot.bottom() - level * plot.height(),
    )
}

fn handles(plot: Rect, settings: EnvelopeSettings) -> [Pos2; 3] {
    let edges = stage_edges(settings);
    [
        point(plot, edges[1], 1.0),
        point(plot, edges[2], settings.sustain),
        point(plot, edges[3], settings.sustain),
    ]
}

fn preview(drag: &mut Drag, pointer: Pos2) -> EnvelopeSettings {
    let mut settings = drag.before;
    let pointer = if drag.handle == 1 {
        drag.direction
            .as_mut()
            .map_or(drag.origin, |direction| direction.update(pointer))
    } else {
        pointer
    };
    let delta = pointer - drag.origin;
    let time_scale = times(settings).iter().sum::<f32>().max(250.0)
        / ((1.0 - SUSTAIN_WIDTH - 3.0 * MIN_TIMED_WIDTH) * drag.plot.width());
    let time = delta.x * time_scale;
    let max = daw_unit_core::MAX_ENVELOPE_TIME_MS;
    match (drag.handle, settings.timing) {
        (0, EnvelopeTiming::Time) => {
            settings.attack.time_ms = (settings.attack.time_ms + time).clamp(0.0, max)
        }
        (0, EnvelopeTiming::Note) => {
            settings.attack_note = (settings.attack_note + time / 1000.0).clamp(0.0, 1.0)
        }
        (1, EnvelopeTiming::Time) => {
            settings.decay.time_ms = (settings.decay.time_ms + time).clamp(0.0, max)
        }
        (1, EnvelopeTiming::Note) => {
            settings.decay_note = (settings.decay_note + time / 1000.0).clamp(0.0, 1.0)
        }
        _ => settings.release.time_ms = (settings.release.time_ms - time).clamp(0.0, max),
    }
    if drag.handle == 1 {
        settings.sustain = (settings.sustain - delta.y / drag.plot.height()).clamp(0.0, 1.0);
    }
    settings
}

/// Edit and display the contour with a marker from the running envelope.
/// Returns the transient settings while a drag is in progress.
pub fn show(
    ui: &mut Ui,
    dimensions: (bool, f32),
    settings: &mut EnvelopeSettings,
    enabled: bool,
    live: Option<[f32; 2]>,
) -> Option<EnvelopeSettings> {
    let (compact, height) = dimensions;
    let id = ui.id().with("envelope-plot-drag");
    let frame = ui.ctx().cumulative_frame_nr();
    let mut drag = ui.data_mut(|data| data.get_temp::<Drag>(id));
    let blocked = compact || crate::drag::cancelled(ui);
    if blocked
        || drag.is_some_and(|d| d.before != *settings || crate::drag::skipped_frame(d.frame, frame))
    {
        drag = None;
    }
    let mut pending = None;
    layout::unit_strip_with_width(
        ui,
        height,
        spectrum_graph::WIDTH,
        egui::UiBuilder::new(),
        |ui| {
            let chart = spectrum_graph::chart(ui, height, compact);
            if ui.is_rect_visible(chart) {
                let (painter, grid) = spectrum_graph::surface(ui, chart);
                let plot = chart.shrink2(if compact {
                    spectrum_graph::COMPACT_PLOT_INSET
                } else {
                    Vec2::new(24.0, 17.0)
                });
                let nodes = handles(plot, *settings);
                let mut selected = None;
                if !compact {
                    for (index, name) in ["Attack", "Decay and Sustain", "Release"]
                        .into_iter()
                        .enumerate()
                    {
                        // Keep neighboring handles reachable even when a stage is short.
                        let mut hit = Rect::from_center_size(nodes[index], Vec2::splat(17.0));
                        if let Some(left) = index.checked_sub(1) {
                            hit.min.x = hit.min.x.max((nodes[left].x + nodes[index].x) * 0.5);
                        }
                        if let Some(right) = nodes.get(index + 1) {
                            hit.max.x = hit.max.x.min((right.x + nodes[index].x) * 0.5);
                        }
                        let response = ui
                            .interact(hit, id.with(index), Sense::click_and_drag())
                            .on_hover_cursor(egui::CursorIcon::Grab)
                            .on_hover_text(name);
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::DragValue,
                                response.enabled(),
                                name,
                            )
                        });
                        if !blocked
                            && drag.is_none()
                            && response.is_pointer_button_down_on()
                            && let Some(origin) = ui.input(|input| input.pointer.press_origin())
                            && ui.input(|input| input.pointer.primary_pressed())
                        {
                            response.request_focus();
                            drag = Some(Drag {
                                before: *settings,
                                handle: index,
                                origin,
                                direction: None,
                                plot,
                                frame,
                            });
                        }
                        if index == 1
                            && response.drag_started()
                            && let Some(active) = drag.as_mut().filter(|active| active.handle == 1)
                        {
                            active.direction = Some(AxisLock::new(active.origin, true));
                        }
                        if response.has_focus() {
                            selected = Some(index);
                        }
                        crate::control_capture::record(ui, &response);
                    }
                }
                let selected = if blocked {
                    None
                } else {
                    drag.as_ref().map(|active| active.handle).or(selected)
                };
                let mut view = *settings;
                if let Some(active) = drag.as_mut() {
                    active.frame = frame;
                    let (pointer, released, down) = ui.input(|input| {
                        (
                            input.pointer.interact_pos(),
                            input.pointer.primary_released(),
                            input.pointer.primary_down(),
                        )
                    });
                    if let Some(pointer) = pointer {
                        view = preview(active, pointer);
                    }
                    if released {
                        *settings = view;
                    }
                    if down {
                        pending = Some(view);
                    } else {
                        drag = None;
                    }
                }
                let edges = stage_edges(view);
                let stage_point = |stage: usize, progress: f32, level: f32| -> Pos2 {
                    point(
                        plot,
                        egui::lerp(edges[stage]..=edges[stage + 1], progress),
                        level,
                    )
                };
                for y in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    painter.hline(plot.x_range(), point(plot, 0.0, y).y, grid);
                }
                for stage in 0..4 {
                    painter.vline(stage_point(stage, 0.0, 0.0).x, plot.y_range(), grid);
                }
                painter.vline(plot.right(), plot.y_range(), grid);
                if !compact {
                    let labels = theme::palette(ui.ctx()).secondary;
                    for (level, label) in [(0.0, "0%"), (0.5, "50%"), (1.0, "100%")] {
                        painter.text(
                            point(plot, 0.0, level) - Vec2::new(4.0, 0.0),
                            Align2::RIGHT_CENTER,
                            label,
                            FontId::proportional(9.0),
                            labels,
                        );
                    }
                    for (stage, label) in ["A", "D", "S", "R"].into_iter().enumerate() {
                        painter.text(
                            stage_point(stage, 0.5, 0.0) + Vec2::new(0.0, 4.0),
                            Align2::CENTER_TOP,
                            label,
                            FontId::proportional(9.0),
                            labels,
                        );
                    }
                }
                let sustain = view.sustain;
                let mut points = Vec::with_capacity(4 * SAMPLES_PER_STAGE);
                for stage in 0..4 {
                    for sample in 0..SAMPLES_PER_STAGE {
                        let t = sample as f32 / (SAMPLES_PER_STAGE - 1) as f32;
                        let level = match stage {
                            0 => view.attack.shape.progress(t, view.attack.amount, false),
                            1 => egui::lerp(
                                1.0..=sustain,
                                view.decay.shape.progress(t, view.decay.amount, true),
                            ),
                            2 => sustain,
                            _ => {
                                sustain
                                    * (1.0
                                        - view.release.shape.progress(t, view.release.amount, true))
                            }
                        };
                        points.push(stage_point(stage, t, level));
                    }
                }
                let gray = theme::palette(ui.ctx()).secondary;
                let accent = theme::accent(ui.ctx());
                let affected = match selected.filter(|_| enabled) {
                    Some(0) => 0..1,
                    Some(1) => 1..3,
                    Some(2) => 3..4,
                    _ => 0..0,
                };
                let (highlighted, tail) = if affected.is_empty() {
                    (Vec::new(), Vec::new())
                } else {
                    let tail = points.split_off(affected.end * SAMPLES_PER_STAGE);
                    (points.split_off(affected.start * SAMPLES_PER_STAGE), tail)
                };
                for (segment, color) in [(points, gray), (highlighted, accent), (tail, gray)] {
                    if !segment.is_empty() {
                        painter.add(Shape::line(
                            segment,
                            Stroke::new(spectrum_graph::CURVE_STROKE_WIDTH, color),
                        ));
                    }
                }
                if !compact {
                    for (index, node) in handles(plot, view).into_iter().enumerate() {
                        spectrum_graph::band_handle(
                            ui,
                            &painter,
                            node,
                            5.5,
                            selected == Some(index),
                            enabled,
                        );
                    }
                }
                if let Some([position, level]) = live.filter(|[position, level]| {
                    enabled && (0.0..=4.0).contains(position) && (0.0..=1.0).contains(level)
                }) {
                    let stage = (position as usize).min(3);
                    painter.circle_filled(
                        stage_point(stage, position - stage as f32, level),
                        3.0,
                        accent,
                    );
                }
            }
            if !compact {
                rows::centered(ui, layout::COMPACT_BUTTON_SIZE, |ui| {
                    rows::unit_label(ui, "Shape")
                });
            }
        },
    );
    ui.data_mut(|data| {
        if let Some(drag) = drag {
            data.insert_temp(id, drag);
        } else {
            data.remove::<Drag>(id);
        }
    });
    pending
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_previews_commits_and_cancels() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        let mut settings = EnvelopeSettings::default();
        let before = settings;
        let run = |settings: &mut EnvelopeSettings, events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    show(ui, (false, 190.0), settings, true, None);
                },
            );
            output.textures_delta.clear();
            let handles = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    Shape::Circle(circle)
                        if circle.radius == 5.5 && circle.stroke == Stroke::NONE =>
                    {
                        Some(circle.center)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let accent = theme::accent(&ctx);
            let highlighted = output.shapes.iter().find_map(|shape| match &shape.shape {
                Shape::Path(path)
                    if path.stroke
                        == Stroke::new(spectrum_graph::CURVE_STROKE_WIDTH, accent).into() =>
                {
                    Some(path.points.len())
                }
                _ => None,
            });
            (handles, highlighted)
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let (handles, highlighted) = run(&mut settings, vec![]);
        assert_eq!(highlighted, None);
        let attack = handles[0];
        assert_eq!(
            run(&mut settings, vec![egui::Event::PointerMoved(attack)]).1,
            None
        );
        let longer = attack + Vec2::new(30.0, 0.0);
        run(
            &mut settings,
            vec![egui::Event::PointerMoved(attack), button(attack, true)],
        );
        let (preview, highlighted) = run(&mut settings, vec![egui::Event::PointerMoved(longer)]);
        assert_eq!(highlighted, Some(SAMPLES_PER_STAGE));
        assert_eq!(settings, before);
        assert!(preview[0].x > attack.x);
        run(&mut settings, vec![button(longer, false)]);
        assert!(settings.attack.time_ms > before.attack.time_ms);
        assert_eq!(settings.decay, before.decay);

        let after_attack = settings;
        let sustain = run(&mut settings, vec![]).0[1];
        run(
            &mut settings,
            vec![egui::Event::PointerMoved(sustain), button(sustain, true)],
        );
        let lower = sustain + Vec2::new(5.0, 20.0);
        let (preview, highlighted) = run(&mut settings, vec![egui::Event::PointerMoved(lower)]);
        assert_eq!(highlighted, Some(2 * SAMPLES_PER_STAGE));
        assert!(preview[1].y > sustain.y);
        assert_eq!(preview[1].x, sustain.x);
        let below_tolerance = lower + Vec2::new(11.0, 4.0);
        let (vertical, _) = run(
            &mut settings,
            vec![egui::Event::PointerMoved(below_tolerance)],
        );
        assert!(vertical[1].y > preview[1].y);
        assert_eq!(vertical[1].x, sustain.x);
        let past_tolerance = lower + Vec2::new(14.0, 6.0);
        let (horizontal, _) = run(
            &mut settings,
            vec![egui::Event::PointerMoved(past_tolerance)],
        );
        assert_eq!(horizontal[1].y, vertical[1].y);
        assert!(horizontal[1].x > sustain.x);
        run(
            &mut settings,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        run(&mut settings, vec![button(past_tolerance, false)]);
        assert_eq!(settings, after_attack);

        let release = run(&mut settings, vec![]).0[2];
        run(
            &mut settings,
            vec![egui::Event::PointerMoved(release), button(release, true)],
        );
        let (nodes, highlighted) = run(&mut settings, vec![button(release, false)]);
        assert_eq!(highlighted, Some(SAMPLES_PER_STAGE));
        assert_eq!(nodes[2], release);
        assert_eq!(settings, after_attack);
    }

    #[test]
    fn marker_uses_the_reported_level_and_hides_when_idle_or_bypassed() {
        let ctx = egui::Context::default();
        crate::fonts::configure(&ctx);
        let marker = |enabled, live| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                show(
                    ui,
                    (false, 190.0),
                    &mut EnvelopeSettings::default(),
                    enabled,
                    live,
                );
            });
            output.textures_delta.clear();
            output.shapes.iter().find_map(|shape| match &shape.shape {
                Shape::Circle(circle) if circle.radius == 3.0 => Some(circle.center),
                _ => None,
            })
        };
        let high = marker(true, Some([1.5, 0.8])).unwrap();
        let low = marker(true, Some([1.5, 0.2])).unwrap();
        assert_eq!(high.x, low.x);
        assert!(high.y < low.y);
        assert_eq!(marker(true, None), None);
        assert_eq!(marker(false, Some([1.5, 0.8])), None);
    }
}
