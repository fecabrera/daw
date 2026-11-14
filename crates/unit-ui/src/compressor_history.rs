//! Read-only background paint. It creates no interaction regions over curve handles.
use daw_unit_core::{LEVEL_HISTORY_BINS, LevelHistoryReading};
use egui::{Rect, Ui};
pub(super) fn paint(ui: &Ui, plot: Rect, history: &LevelHistoryReading) {
    let painter = ui.painter_at(plot);
    let color = crate::theme::palette(ui.ctx()).secondary;
    let count = history.count.min(LEVEL_HISTORY_BINS);
    // egui-wgpu cannot render a mesh with no indices (including a single point).
    // Idle tracks and the first audio bucket must produce no history primitives.
    if count < 2 {
        return;
    }
    // Warm-up is right-aligned too: uncollected time remains empty on the left.
    let x = |i: usize| {
        plot.left()
            + plot.width() * (LEVEL_HISTORY_BINS - count + i) as f32
                / (LEVEL_HISTORY_BINS - 1) as f32
    };
    let bin = |i: usize| {
        let oldest = if count == LEVEL_HISTORY_BINS {
            history.position % LEVEL_HISTORY_BINS
        } else {
            0
        };
        history.bins[(oldest + i) % LEVEL_HISTORY_BINS]
    };
    let input_y = |value: f32| {
        let db = 20.0 * value.max(1e-20).log10();
        plot.bottom() - plot.height() * ((db + 72.0) / 84.0).clamp(0.0, 1.0)
    };
    let reduction_y = |value: f32| plot.top() + plot.height() * (value / 24.0).clamp(0.0, 1.0);
    let mut area = egui::Mesh::default();
    let mut reduction = Vec::with_capacity(count);
    for i in 0..count {
        let value = bin(i);
        let top = egui::pos2(x(i), input_y(value[0]));
        area.colored_vertex(top, color.gamma_multiply(0.16));
        area.colored_vertex(egui::pos2(x(i), plot.bottom()), color.gamma_multiply(0.16));
        if i > 0 {
            let a = (2 * (i - 1)) as u32;
            area.add_triangle(a, a + 1, a + 2);
            area.add_triangle(a + 1, a + 3, a + 2);
        }
        reduction.push(egui::pos2(x(i), reduction_y(value[1])));
    }
    painter.add(egui::Shape::mesh(area));
    painter.add(egui::Shape::line(
        reduction,
        egui::Stroke::new(1.0, color.gamma_multiply(0.55)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mesh(history: LevelHistoryReading) -> egui::Mesh {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            paint(
                ui,
                Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(209.0, 104.0)),
                &history,
            );
        });
        output.textures_delta.clear();
        output
            .shapes
            .into_iter()
            .find_map(|s| match s.shape {
                egui::Shape::Mesh(mesh) => Some((*mesh).clone()),
                _ => None,
            })
            .unwrap()
    }
    #[test]
    fn idle_and_first_audio_bucket_emit_no_empty_gpu_mesh() {
        let ctx = egui::Context::default();
        for count in [0, 1, 2, LEVEL_HISTORY_BINS] {
            let history = LevelHistoryReading {
                count,
                position: count % LEVEL_HISTORY_BINS,
                bins: [[0.5, 6.0]; LEVEL_HISTORY_BINS],
            };
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint(
                    ui,
                    Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(199.0, 84.0)),
                    &history,
                );
            });
            output.textures_delta.clear();
            if count < 2 {
                assert!(output.shapes.is_empty());
            }
            let primitives = ctx.tessellate(output.shapes, 1.0);
            for primitive in primitives {
                if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                    assert!(
                        !mesh.vertices.is_empty(),
                        "empty vertex slice at {count} samples"
                    );
                    assert!(
                        !mesh.indices.is_empty(),
                        "empty index slice at {count} samples"
                    );
                }
            }
        }
    }
    #[test]
    fn latest_point_stays_at_right_edge_during_warmup_and_after_wrap() {
        let mut history = LevelHistoryReading {
            count: 2,
            position: 2,
            ..Default::default()
        };
        history.bins[0] = [0.1, 3.0];
        history.bins[1] = [1.0, 9.0];
        let partial = mesh(history);
        assert_eq!(partial.vertices[0].pos, egui::pos2(208.0, 52.0));
        assert_eq!(partial.vertices[2].pos, egui::pos2(209.0, 32.0));
        history.count = LEVEL_HISTORY_BINS;
        history.position = 1;
        history.bins[0] = [1.0, 9.0]; // Most recent bin sits before the ring's next write position.
        history.bins[1] = [0.1, 3.0];
        let full = mesh(history);
        assert_eq!(full.vertices[0].pos, egui::pos2(10.0, 52.0));
        assert_eq!(
            full.vertices[2 * (LEVEL_HISTORY_BINS - 1)].pos,
            egui::pos2(209.0, 32.0)
        );
        // Ring wrap is continuous in time; no sweep-head gap remains in the fill.
        assert_eq!(full.indices.len(), 6 * (LEVEL_HISTORY_BINS - 1));
    }
}
