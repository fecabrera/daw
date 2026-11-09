//! Shared audio envelope controls for Timeline, Editor and Clip settings.
use super::*;
use daw_core::ClipFade;

pub(super) const WAVEFORM_TOP: f32 = 30.0;

fn selected_fade(fades: &mut daw_core::ClipFades, left: bool) -> &mut daw_core::ClipFade {
    if left {
        &mut fades.fade_in
    } else {
        &mut fades.fade_out
    }
}
pub(super) const HANDLE_CENTER: f32 = 26.0;
pub(super) const HANDLE_RADIUS: f32 = 3.0;
pub(super) const HANDLE_TOP: f32 = HANDLE_CENTER - HANDLE_RADIUS;
pub(super) const FADE_SHADE: egui::Color32 = egui::Color32::from_black_alpha(45);
const DARK_INK: egui::Color32 = egui::Color32::from_rgb(32, 32, 32);

fn visual_position(fade: daw_core::ClipFade) -> f32 {
    if fade.crossfade {
        1.0 - fade.midpoint_position
    } else {
        fade.midpoint_position
    }
}
fn visual_level(fade: daw_core::ClipFade) -> f32 {
    fade.midpoint
}
fn visual_gain(fade: daw_core::ClipFade, progress: f32) -> f32 {
    if fade.crossfade {
        fade.opposing_gain(1.0 - progress)
    } else {
        fade.gain(progress)
    }
}
/// Align half level with the waveform center while retaining the handle-top
/// endpoint. The smooth correction vanishes at silence and full level.
fn gain_y(block: Rect, top: f32, gain: f32) -> f32 {
    let waveform_top = (block.top() + WAVEFORM_TOP).min(block.bottom());
    block.bottom() - gain * (block.bottom() - top)
        + 2.0 * (waveform_top - top) * gain * (1.0 - gain)
}
fn points(block: Rect, fade: daw_core::ClipFade, left: bool, zoom: f32, rate: u32) -> (Pos2, Pos2) {
    let anchor = if left { block.left() } else { block.right() };
    let direction = if left { 1.0 } else { -1.0 } * if fade.crossfade { -1.0 } else { 1.0 };
    let width = fade.length_frames as f64 / f64::from(rate) * f64::from(zoom);
    let center = (block.top() + HANDLE_CENTER).min(block.bottom());
    let top = center - HANDLE_RADIUS;
    let endpoint = Pos2::new(anchor + direction * width as f32, center);
    let midpoint = Pos2::new(
        anchor + (endpoint.x - anchor) * visual_position(fade),
        gain_y(block, top, visual_level(fade)),
    );
    (endpoint, midpoint)
}
fn curve_points(block: Rect, fade: daw_core::ClipFade, left: bool, endpoint: Pos2) -> Vec<Pos2> {
    curve_points_for_gain(block, fade, left, endpoint, false)
}
fn curve_points_for_gain(
    block: Rect,
    fade: ClipFade,
    left: bool,
    endpoint: Pos2,
    companion: bool,
) -> Vec<Pos2> {
    let anchor = if left { block.left() } else { block.right() };
    let knot = visual_position(fade);
    // Include the control point exactly, even for steep curves near the ends.
    (0..=32)
        .map(|i| {
            let t = if i <= 16 {
                knot * i as f32 / 16.0
            } else {
                knot + (1.0 - knot) * (i - 16) as f32 / 16.0
            };
            let gain = visual_gain(fade, t);
            Pos2::new(
                anchor + (endpoint.x - anchor) * t,
                gain_y(
                    block,
                    endpoint.y - HANDLE_RADIUS,
                    if companion { fade.gain(1.0 - t) } else { gain },
                ),
            )
        })
        .collect()
}
fn shade_above(painter: &egui::Painter, curve: &[Pos2], top: f32) {
    let mut mesh = egui::Mesh::default();
    let shade = FADE_SHADE;
    for pair in curve.windows(2) {
        let (left, right) = if pair[0].x < pair[1].x {
            (pair[0], pair[1])
        } else {
            (pair[1], pair[0])
        };
        let index = mesh.vertices.len() as u32;
        for point in [Pos2::new(left.x, top), Pos2::new(right.x, top), right, left] {
            mesh.colored_vertex(point, shade);
        }
        mesh.add_triangle(index, index + 1, index + 2);
        mesh.add_triangle(index, index + 2, index + 3);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Two independent attenuation masks give clear, single- and double-shaded
/// regions. Keep both in one mesh so the host paints the crossover once.
fn shade_crossfade(
    painter: &egui::Painter,
    curve: &[Pos2],
    companion: &[Pos2],
    top: f32,
    bottom: f32,
) {
    let mut mesh = egui::Mesh::default();
    for (own, other) in curve.windows(2).zip(companion.windows(2)) {
        let (a, b, c, d) = (own[0], own[1], other[0], other[1]);
        for points in [
            [Pos2::new(c.x, top), Pos2::new(d.x, top), d, c],
            [a, b, Pos2::new(b.x, bottom), Pos2::new(a.x, bottom)],
        ] {
            let index = mesh.vertices.len() as u32;
            for point in points {
                mesh.colored_vertex(point, FADE_SHADE);
            }
            mesh.add_triangle(index, index + 1, index + 2);
            mesh.add_triangle(index, index + 2, index + 3);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_curve(painter: &egui::Painter, curve: Vec<Pos2>) {
    painter.add(egui::Shape::line(curve, Stroke::new(1.0, DARK_INK)));
}
/// Both signals share the same editable time/gain crossing.
/// Draw the companion without extra shading or independent edit handles.
fn paint_fade_curves(painter: &egui::Painter, curve: Vec<Pos2>, companion: Option<Vec<Pos2>>) {
    if let Some(companion) = companion {
        paint_curve(painter, companion);
    }
    paint_curve(painter, curve);
}

fn paint_guide(painter: &egui::Painter, handle: Pos2, bottom: f32) {
    painter.line_segment(
        [
            Pos2::new(handle.x, handle.y + HANDLE_RADIUS),
            Pos2::new(handle.x, bottom),
        ],
        Stroke::new(1.0, DARK_INK),
    );
}

fn paint_crossover_guides(painter: &egui::Painter, waveform: Rect) {
    // Keep the full one-point stroke inside the crossover's host clip.
    let inset = 0.5_f32.min(waveform.width() * 0.5);
    for x in [waveform.left() + inset, waveform.right() - inset] {
        painter.line_segment(
            [
                Pos2::new(x, waveform.top()),
                Pos2::new(x, waveform.bottom()),
            ],
            Stroke::new(1.0, DARK_INK),
        );
    }
}

// Zero-duration handles have no curve endpoint to preserve. Keep the complete
// square inside the viewport when the clip starts/ends at the viewport edge.
fn visible_handle(mut center: Pos2, fade: ClipFade, viewport: Rect) -> Pos2 {
    if fade.length_frames == 0
        && viewport.width() >= HANDLE_RADIUS * 2.0
        && center.x >= viewport.left()
        && center.x <= viewport.right()
    {
        center.x = center.x.clamp(
            viewport.left() + HANDLE_RADIUS,
            viewport.right() - HANDLE_RADIUS,
        );
    }
    center
}
fn paint_handle(painter: &egui::Painter, center: Pos2) {
    let rect = Rect::from_center_size(center, Vec2::splat(HANDLE_RADIUS * 2.0));
    painter.rect(
        rect,
        1.0,
        egui::Color32::TRANSPARENT,
        Stroke::new(1.0, DARK_INK),
        StrokeKind::Inside,
    );
}
fn paint_midpoint(painter: &egui::Painter, center: Pos2) {
    painter.circle(
        center,
        HANDLE_RADIUS,
        egui::Color32::TRANSPARENT,
        Stroke::new(1.0, DARK_INK),
    );
}

impl DawUi {
    fn audio_clip(&self, clip: &Clip) -> bool {
        self.session
            .project
            .assets
            .iter()
            .any(|a| a.id == clip.asset_id && a.is_audio())
    }
    pub(super) fn fade_drag_value(&self, drag: &Drag, ctx: &egui::Context) -> daw_core::ClipFades {
        self.fade_drag_value_for(drag, &drag.clip, ctx)
    }
    fn fade_drag_value_for(
        &self,
        drag: &Drag,
        clip: &Clip,
        ctx: &egui::Context,
    ) -> daw_core::ClipFades {
        let mut fades = clip.fades;
        let ClipDragMode::Fade {
            left,
            shape,
            height,
            frames_per_point,
        } = drag.mode
        else {
            return fades;
        };
        let pointer = ctx.input(|i| i.pointer.latest_pos()).unwrap_or(drag.origin);
        if pointer == drag.origin {
            return fades;
        }
        let edge = selected_fade(&mut fades, left);
        if shape {
            if edge.length_frames == 0 {
                return fades;
            }
            let vertical = (pointer.y - drag.origin.y) / height.max(1) as f32;
            edge.midpoint =
                (edge.midpoint - vertical).clamp(ClipFade::MIN_LEVEL, ClipFade::MAX_LEVEL);
            let horizontal = f64::from(pointer.x - drag.origin.x) * frames_per_point
                / (if left {
                    drag.clip.fades.fade_in
                } else {
                    drag.clip.fades.fade_out
                })
                .length_frames
                .max(1) as f64
                * if left { 1.0 } else { -1.0 };
            edge.midpoint_position = (f64::from(edge.midpoint_position) + horizontal).clamp(
                f64::from(ClipFade::MIN_POSITION),
                f64::from(ClipFade::MAX_POSITION),
            ) as f32;
        } else {
            let old = edge.length_frames as f64 * if edge.crossfade { -1.0 } else { 1.0 };
            let delta = f64::from(pointer.x - drag.origin.x)
                * frames_per_point
                * if left { 1.0 } else { -1.0 };
            let signed = old + delta;
            edge.crossfade = signed < 0.0;
            edge.length_frames = (signed.abs().round() as u64).min(
                self.session
                    .project
                    .clip_fade_limit(clip.id, left, edge.crossfade),
            );
            if edge.length_frames == 0 {
                edge.crossfade = false;
            }
        }
        fades
    }
    pub(super) fn fade_drag_edits(&self, drag: &Drag, ctx: &egui::Context) -> Vec<Edit> {
        let ClipDragMode::Fade { left, .. } = drag.mode else {
            return Vec::new();
        };
        let targets: Vec<_> = if drag.clips.is_empty() {
            vec![drag.clip.clone()]
        } else {
            drag.clips.iter().map(|c| c.clip.clone()).collect()
        };
        self.fade_group_edits(&targets, left, |clip| {
            let mut fades = if clip.id == drag.clip.id {
                self.fade_drag_value(drag, ctx)
            } else {
                self.fade_drag_value_for(drag, clip, ctx)
            };
            *selected_fade(&mut fades, left)
        })
    }
    /// Resolve competing neighbor envelopes against each preceding edit, so a
    /// later target cannot restore an opposite edge cleared by an earlier one.
    fn fade_group_edits(
        &self,
        targets: &[Clip],
        left: bool,
        value: impl Fn(&Clip) -> ClipFade,
    ) -> Vec<Edit> {
        let mut project = self.session.project.clone();
        let mut commands = Vec::new();
        for target in targets {
            let Some(current) = project
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .find(|c| c.id == target.id)
            else {
                continue;
            };
            let mut fades = current.fades;
            *selected_fade(&mut fades, left) = value(target);
            if fades == current.fades {
                continue;
            }
            let edit = Edit::SetClipFades {
                clip_id: target.id,
                fades,
            };
            if project.edit(edit.clone()).is_err() {
                return vec![edit];
            }
            commands.push(edit);
        }
        commands
    }
    pub(super) fn visible_fades(&self, clip: &Clip, ctx: &egui::Context) -> daw_core::ClipFades {
        let Some(drag) = self
            .drag
            .as_ref()
            .filter(|d| matches!(d.mode, ClipDragMode::Fade { .. }))
        else {
            return clip.fades;
        };
        let target = if drag.clip.id == clip.id {
            Some(&drag.clip)
        } else {
            drag.clips
                .iter()
                .find(|c| c.clip.id == clip.id)
                .map(|c| &c.clip)
        };
        let mut fades = target.map_or(clip.fades, |target| {
            self.fade_drag_value_for(drag, target, ctx)
        });
        let ClipDragMode::Fade { left, .. } = drag.mode else {
            return fades;
        };
        // Neighbor cleanup is also visible during previews, including unselected neighbors.
        let targets: Vec<_> = if drag.clips.is_empty() {
            vec![&drag.clip]
        } else {
            drag.clips.iter().map(|c| &c.clip).collect()
        };
        for other in targets {
            if other.id == clip.id {
                continue;
            }
            let same_track = self.session.project.tracks.iter().any(|t| {
                t.clips.iter().any(|c| c.id == clip.id) && t.clips.iter().any(|c| c.id == other.id)
            });
            if !same_track
                || !(if left {
                    other.start_frame == clip.end()
                } else {
                    other.end() == clip.start_frame
                })
            {
                continue;
            }
            let mut next = self.fade_drag_value_for(drag, other, ctx);
            let next = *selected_fade(&mut next, left);
            let mut old = other.fades;
            if next != *selected_fade(&mut old, left) && next.length_frames > 0 {
                let opposite = selected_fade(&mut fades, !left);
                if opposite.crossfade || next.crossfade {
                    *opposite = ClipFade::default();
                }
            }
        }
        fades
    }
    pub(super) fn paint_clip_fades(&self, painter: &egui::Painter, block: Rect, clip: &Clip) {
        self.paint_clip_fade_layer(painter, block, clip, false);
    }
    pub(super) fn paint_clip_fade_background(
        &self,
        painter: &egui::Painter,
        block: Rect,
        clip: &Clip,
    ) {
        self.paint_clip_fade_layer(painter, block, clip, true);
    }
    fn paint_clip_fade_layer(
        &self,
        painter: &egui::Painter,
        block: Rect,
        clip: &Clip,
        background: bool,
    ) {
        if !self
            .session
            .project
            .tracks
            .iter()
            .any(|t| t.clips.iter().any(|c| c.id == clip.id))
        {
            return;
        }
        let fades = self.visible_fades(clip, painter.ctx());
        let rate = self.session.project.sample_rate_hz;
        for (left, mut fade) in [(true, fades.fade_in), (false, fades.fade_out)] {
            fade.length_frames = fade.length_frames.min(self.session.project.clip_fade_limit(
                clip.id,
                left,
                fade.crossfade,
            ));
            let (endpoint, _) = points(block, fade, left, self.view_zoom(), rate);
            if fade.length_frames > 0 {
                let curve = curve_points(block, fade, left, endpoint);
                if background {
                    if fade.crossfade {
                        let companion = curve_points_for_gain(block, fade, left, endpoint, true);
                        shade_crossfade(
                            painter,
                            &curve,
                            &companion,
                            (block.top() + clip_paint::HEADER_HEIGHT).min(block.bottom()),
                            block.bottom(),
                        );
                    } else {
                        shade_above(painter, &curve, endpoint.y - HANDLE_RADIUS);
                    }
                    continue;
                }
                let anchor = if left { block.left() } else { block.right() };
                let crossover = Rect::from_two_pos(
                    Pos2::new(
                        anchor,
                        (block.top() + clip_paint::HEADER_HEIGHT).min(block.bottom()),
                    ),
                    Pos2::new(endpoint.x, block.bottom()),
                );
                let hovered = painter.ctx().pointer_hover_pos().is_some_and(|pos| {
                    crossover.contains(pos) && painter.clip_rect().contains(pos)
                });
                if fade.crossfade {
                    if hovered {
                        paint_crossover_guides(painter, crossover);
                    }
                } else {
                    paint_guide(painter, endpoint, block.bottom());
                }
                let dragging = self.drag.as_ref().is_some_and(|drag| (drag.clip.id == clip.id || drag.clips.iter().any(|c| c.clip.id == clip.id))
                    && matches!(drag.mode, ClipDragMode::Fade { left: edge, .. } if edge == left));
                if !fade.crossfade || hovered || dragging {
                    paint_fade_curves(
                        painter,
                        curve,
                        fade.crossfade
                            .then(|| curve_points_for_gain(block, fade, left, endpoint, true)),
                    );
                }
            }
        }
    }
    /// Controls use the lane clip, not the clip body, and are drawn last so
    /// their squares/circles remain whole at shared boundaries in either order.
    pub(super) fn crossfade_click_owner(
        &self,
        track_id: Id,
        clip: &Clip,
        block: Rect,
        pos: Pos2,
        ctx: &egui::Context,
    ) -> Option<Clip> {
        if !block.contains(pos) {
            return None;
        }
        let track = self
            .session
            .project
            .tracks
            .iter()
            .find(|track| track.id == track_id)?;
        for other in &track.clips {
            let fades = self.visible_fades(other, ctx);
            for (left, fade) in [(true, fades.fade_in), (false, fades.fade_out)] {
                if !fade.crossfade
                    || !(if left {
                        other.start_frame == clip.end()
                    } else {
                        other.end() == clip.start_frame
                    })
                {
                    continue;
                }
                let length = fade
                    .length_frames
                    .min(self.session.project.clip_fade_limit(other.id, left, true));
                let width = self.session.project.seconds(length) as f32 * self.view_zoom();
                if length > 0
                    && (if left {
                        pos.x >= block.right() - width
                    } else {
                        pos.x < block.left() + width
                    })
                {
                    return Some(other.clone());
                }
            }
        }
        None
    }
    fn clip_fade_handles_enabled(&self, ctx: &egui::Context) -> bool {
        !ctx.input(|input| clip_copy_modifier(input.modifiers))
            || self
                .drag
                .as_ref()
                .is_some_and(|drag| matches!(drag.mode, ClipDragMode::Fade { .. }))
    }

    pub(super) fn paint_clip_fade_controls(
        &self,
        painter: &egui::Painter,
        block: Rect,
        clip: &Clip,
    ) {
        if !self.audio_clip(clip)
            || !self.clip_fade_handles_enabled(painter.ctx())
            || !self
                .session
                .project
                .tracks
                .iter()
                .any(|track| track.clips.iter().any(|c| c.id == clip.id))
        {
            return;
        }
        let visual = self.clip_fade_header_rect(block, clip, painter.ctx());
        let hovered = painter
            .ctx()
            .pointer_hover_pos()
            .is_some_and(|pos| visual.contains(pos));
        let dragging = self.drag.as_ref().is_some_and(|drag| {
            (drag.clip.id == clip.id || drag.clips.iter().any(|c| c.clip.id == clip.id))
                && matches!(drag.mode, ClipDragMode::Fade { .. })
        });
        if !hovered && !dragging {
            return;
        }
        let fades = self.visible_fades(clip, painter.ctx());
        for (left, mut fade) in [(true, fades.fade_in), (false, fades.fade_out)] {
            fade.length_frames = fade.length_frames.min(self.session.project.clip_fade_limit(
                clip.id,
                left,
                fade.crossfade,
            ));
            let (handle, midpoint) = points(
                block,
                fade,
                left,
                self.view_zoom(),
                self.session.project.sample_rate_hz,
            );
            if fade.length_frames > 0 {
                paint_midpoint(painter, midpoint);
            }
            paint_handle(painter, visible_handle(handle, fade, painter.clip_rect()));
        }
    }
    pub(super) fn paint_neighbor_fade_controls(
        &self,
        painter: &egui::Painter,
        block: Rect,
        clip: &Clip,
    ) {
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|track| track.clips.iter().any(|c| c.id == clip.id))
        else {
            return;
        };
        let painter =
            painter.with_clip_rect(block.expand(HANDLE_RADIUS).intersect(painter.clip_rect()));
        for other in &track.clips {
            if other.id == clip.id
                || !(other.end() == clip.start_frame || other.start_frame == clip.end())
            {
                continue;
            }
            let delta = (other.start_frame as f64 - clip.start_frame as f64)
                / f64::from(self.session.project.sample_rate_hz)
                * f64::from(self.view_zoom());
            let rect = Rect::from_min_size(
                block.min + Vec2::new(delta as f32, 0.0),
                Vec2::new(
                    self.session.project.seconds(other.length_frames) as f32 * self.view_zoom(),
                    block.height(),
                ),
            );
            self.paint_clip_fade_controls(&painter, rect, other);
        }
    }
    /// Header ownership follows the audible source extension, without changing
    /// the fixed logical clip ranges used by editing and playback.
    pub(super) fn clip_fade_header_rect(
        &self,
        block: Rect,
        clip: &Clip,
        ctx: &egui::Context,
    ) -> Rect {
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|track| track.clips.iter().any(|c| c.id == clip.id))
        else {
            return block;
        };
        let width = |owner: &Clip, left: bool| {
            let mut fades = self.visible_fades(owner, ctx);
            let fade = selected_fade(&mut fades, left);
            if !fade.crossfade {
                return 0.0;
            }
            let frames = fade
                .length_frames
                .min(self.session.project.clip_fade_limit(owner.id, left, true));
            self.session.project.seconds(frames) as f32 * self.view_zoom()
        };
        let mut rect = block;
        rect.min.x -= width(clip, true);
        rect.max.x += width(clip, false);
        for other in &track.clips {
            if other.end() == clip.start_frame {
                rect.min.x += width(other, false);
            }
            if other.start_frame == clip.end() {
                rect.max.x -= width(other, true);
            }
        }
        rect
    }
    pub(super) fn paint_neighbor_crossfades(
        &self,
        painter: &egui::Painter,
        block: Rect,
        clip: &Clip,
        background: bool,
    ) {
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| t.clips.iter().any(|c| c.id == clip.id))
        else {
            return;
        };
        for other in &track.clips {
            let fades = self.visible_fades(other, painter.ctx());
            if (other.end() == clip.start_frame && fades.fade_out.crossfade)
                || (other.start_frame == clip.end() && fades.fade_in.crossfade)
            {
                let delta = (other.start_frame as f64 - clip.start_frame as f64)
                    / f64::from(self.session.project.sample_rate_hz)
                    * f64::from(self.view_zoom());
                let rect = Rect::from_min_size(
                    block.min + Vec2::new(delta as f32, 0.0),
                    Vec2::new(
                        self.session.project.seconds(other.length_frames) as f32 * self.view_zoom(),
                        block.height(),
                    ),
                );
                let painter = painter.with_clip_rect(painter.clip_rect().intersect(block));
                let colors = theme::clip_colors(track.color, other.color);
                let colors = if other.muted {
                    colors.dimmed(painter.ctx())
                } else {
                    colors
                };
                if background {
                    let header = self.clip_fade_header_rect(rect, other, painter.ctx());
                    clip_paint::paint_header(
                        &painter,
                        header,
                        &other.name,
                        colors,
                        false,
                        self.clip_selected(other.id),
                    );
                }
                self.paint_clip_fade_layer(&painter, rect, other, background);
            }
        }
    }
    /// Resolve preview curves once per paint, rather than searching per pixel.
    pub(super) fn waveform_fade_gain(
        &self,
        clip: &Clip,
        ctx: &egui::Context,
    ) -> impl Fn(u64) -> f32 + use<> {
        let mut fades = self.visible_fades(clip, ctx);
        for (left, fade) in [(true, &mut fades.fade_in), (false, &mut fades.fade_out)] {
            fade.length_frames = fade.length_frames.min(self.session.project.clip_fade_limit(
                clip.id,
                left,
                fade.crossfade,
            ));
        }
        let mut neighbors = [None; 2];
        if let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|t| t.clips.iter().any(|c| c.id == clip.id))
        {
            for other in &track.clips {
                let other_fades = self.visible_fades(other, ctx);
                for (left, mut fade) in [(true, other_fades.fade_in), (false, other_fades.fade_out)]
                {
                    if !fade.crossfade
                        || !(if left {
                            other.start_frame == clip.end()
                        } else {
                            other.end() == clip.start_frame
                        })
                    {
                        continue;
                    }
                    fade.length_frames = fade
                        .length_frames
                        .min(self.session.project.clip_fade_limit(other.id, left, true));
                    if fade.length_frames > 0 {
                        neighbors[usize::from(left)] = Some((left, fade));
                    }
                }
            }
        }
        let length = clip.length_frames;
        move |local| {
            let mut gain = 1.0_f32;
            for (left, fade) in [(true, fades.fade_in), (false, fades.fade_out)] {
                if !fade.crossfade && fade.length_frames > 0 {
                    let position = if left {
                        local
                    } else {
                        length.saturating_sub(1).saturating_sub(local)
                    };
                    gain = gain.min(fade.gain(position as f32 / fade.length_frames as f32));
                }
            }
            for (left, fade) in neighbors.into_iter().flatten() {
                let active = if left {
                    local >= length.saturating_sub(fade.length_frames)
                } else {
                    local < fade.length_frames
                };
                let position = if left {
                    local.saturating_sub(length.saturating_sub(fade.length_frames))
                } else {
                    fade.length_frames.saturating_sub(1).saturating_sub(local)
                };
                if active {
                    gain *= fade.opposing_gain(
                        position as f32 / fade.length_frames.saturating_sub(1).max(1) as f32,
                    );
                }
            }
            gain
        }
    }

    /// Draw the source samples actually used beyond the owner's logical edge.
    /// Each host paints its own span, so source continuations do not depend on
    /// saved clip order and never cover the neighbor's later curves.
    pub(super) fn paint_neighbor_crossfade_waveforms(
        &self,
        painter: &egui::Painter,
        block: Rect,
        clip: &Clip,
    ) {
        let Some(track) = self
            .session
            .project
            .tracks
            .iter()
            .find(|track| track.clips.iter().any(|c| c.id == clip.id))
        else {
            return;
        };
        for other in &track.clips {
            let fades = self.visible_fades(other, painter.ctx());
            for (left, fade) in [(true, fades.fade_in), (false, fades.fade_out)] {
                if !fade.crossfade
                    || !(if left {
                        other.start_frame == clip.end()
                    } else {
                        other.end() == clip.start_frame
                    })
                {
                    continue;
                }
                let length = fade
                    .length_frames
                    .min(self.session.project.clip_fade_limit(other.id, left, true));
                if length == 0 {
                    continue;
                }
                let cached = self.session.audio_for_clip(other);
                let Some(audio) = cached.or_else(|| self.session.audio.get(&other.asset_id)) else {
                    continue;
                };
                let mut visual = other.clone();
                if left {
                    visual.start_frame -= length;
                    if let Some(repeat) = &mut visual.repeat {
                        *repeat = repeat.shifted(-i128::from(length));
                    } else {
                        visual.source_offset_frame -= length;
                    }
                }
                visual.length_frames += length;
                let delta = (visual.start_frame as f64 - clip.start_frame as f64)
                    / f64::from(self.session.project.sample_rate_hz)
                    * f64::from(self.view_zoom());
                let rect = Rect::from_min_size(
                    block.min + Vec2::new(delta as f32, 0.0),
                    Vec2::new(
                        self.session.project.seconds(visual.length_frames) as f32
                            * self.view_zoom(),
                        block.height(),
                    ),
                );
                let body = Rect::from_min_max(
                    Pos2::new(rect.left(), (rect.top() + WAVEFORM_TOP).min(rect.bottom())),
                    rect.max,
                );
                let painter = painter.with_clip_rect(block.intersect(painter.clip_rect()));
                waveforms::paint_peaks_with_gain(
                    &painter,
                    body,
                    audio,
                    &visual,
                    f64::from(self.session.project.sample_rate_hz) / f64::from(self.view_zoom()),
                    theme::clip_colors(track.color, other.color),
                    if cached.is_none() {
                        other.stretch
                    } else {
                        None
                    },
                    |local| {
                        let position = if left {
                            local
                        } else {
                            visual.length_frames.saturating_sub(1).saturating_sub(local)
                        };
                        fade.gain(position as f32 / length.saturating_sub(1).max(1) as f32)
                    },
                );
            }
        }
    }

    pub(super) fn clip_fade_interaction(
        &mut self,
        ui: &mut egui::Ui,
        track: Id,
        clip: &Clip,
        block: Rect,
        lane: Rect,
    ) -> bool {
        if !self.audio_clip(clip) || !self.clip_fade_handles_enabled(ui.ctx()) {
            return false;
        }
        if let Some(drag) = &self.drag
            && let ClipDragMode::Fade { shape, .. } = drag.mode
        {
            ui.ctx().set_cursor_icon(if shape {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::ResizeHorizontal
            });
            return true;
        }
        if !self.clip_actions_available(ui.ctx())
            || !ui.is_enabled()
            || egui::Popup::is_any_open(ui.ctx())
        {
            return false;
        }
        let pointer = ui.input(|i| i.pointer.latest_pos());
        for (left, fade) in [(true, clip.fades.fade_in), (false, clip.fades.fade_out)] {
            let (endpoint, midpoint) = points(
                block,
                fade,
                left,
                self.view_zoom(),
                self.session.project.sample_rate_hz,
            );
            let endpoint = visible_handle(endpoint, fade, lane);
            for (shape, point) in [(true, midpoint), (false, endpoint)] {
                if shape && fade.length_frames == 0 {
                    continue;
                }
                let band = Rect::from_min_max(
                    Pos2::new(
                        lane.left(),
                        block.top() + if shape { HANDLE_TOP } else { 22.0 },
                    ),
                    Pos2::new(
                        lane.right(),
                        if shape {
                            block.bottom()
                        } else {
                            block.top() + WAVEFORM_TOP
                        },
                    ),
                );
                let hit = Rect::from_center_size(point, Vec2::splat(10.0))
                    .intersect(lane)
                    .intersect(band);
                if !pointer.is_some_and(|p| hit.contains(p)) {
                    continue;
                }
                ui.ctx().set_cursor_icon(if shape {
                    egui::CursorIcon::Grab
                } else {
                    egui::CursorIcon::ResizeHorizontal
                });
                let response = ui.interact(
                    hit,
                    egui::Id::new((clip.id, "fade", left, shape)),
                    Sense::drag(),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::slider(
                        ui.is_enabled(),
                        if shape {
                            f64::from(visual_level(fade))
                        } else {
                            fade.length_frames as f64
                        },
                        if left { "Fade in" } else { "Fade out" },
                    )
                });
                response.on_hover_text(if shape {
                    "Drag the midpoint horizontally and vertically to shape the fade"
                } else {
                    "Drag inward to fade; outward to crossfade adjacent audio"
                });
                if ui.input(|i| i.pointer.primary_pressed()) {
                    let origin = pointer.unwrap();
                    if !self.clip_selected(clip.id) {
                        self.select_clip(clip.id, track, false);
                    }
                    self.portion = None;
                    let clips = self
                        .selected_clip_snapshots()
                        .into_iter()
                        .filter(|entry| self.audio_clip(&entry.clip))
                        .collect();
                    self.content_clip_drag = self.midi_editor.content_active;
                    self.drag = Some(Drag {
                        clip: clip.clone(),
                        track,
                        mode: ClipDragMode::Fade {
                            left,
                            shape,
                            height: (block.height() - HANDLE_TOP).max(1.0).round() as u32,
                            frames_per_point: f64::from(self.session.project.sample_rate_hz)
                                / f64::from(self.view_zoom()),
                        },
                        origin,
                        duplicate: false,
                        linked_boundary: false,
                        clips,
                    });
                }
                return true;
            }
        }
        false
    }
    pub(super) fn clip_fade_settings(
        &mut self,
        ui: &mut egui::Ui,
        clip: &Clip,
        panel: &panels::ControlPanel,
        label_width: f32,
        commands: &mut Vec<Edit>,
    ) {
        if !self.audio_clip(clip) {
            return;
        }
        let preview = self.visible_fades(clip, ui.ctx());
        if self
            .selected_clip_snapshots()
            .iter()
            .filter(|entry| self.audio_clip(&entry.clip))
            .count()
            > 1
        {
            ui.small("Fade values show one clip; changes apply to selected audio clips.");
        }
        for left in [true, false] {
            ui.add(egui::Separator::default().spacing(12.0));
            rows::centered(ui, panels::ControlPanel::content_row_height(ui), |ui| {
                ui.label(
                    egui::RichText::new(if left { "Fade in" } else { "Fade out" })
                        .small()
                        .color(theme::palette(ui.ctx()).secondary),
                );
            });
            let mut fades = preview;
            let edge = selected_fade(&mut fades, left);
            let mut changed = false;
            let mut mode_changed = false;
            panel.field_row(
                ui,
                label_width,
                |ui| {
                    ui.small("Mode");
                },
                |ui| {
                    let name = if edge.crossfade { "Crossfade" } else { "Fade" };
                    menus::Dropdown::from_id_salt((clip.id, "fade-mode", left))
                        .selected_text(egui::RichText::new(name).small())
                        .small_items()
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (value, label) in [(false, "Fade"), (true, "Crossfade")] {
                                if ui
                                    .add_enabled(
                                        !value
                                            || self
                                                .session
                                                .project
                                                .clip_fade_limit(clip.id, left, true)
                                                > 0,
                                        egui::Button::selectable(edge.crossfade == value, label),
                                    )
                                    .clicked()
                                {
                                    edge.crossfade = value;
                                    mode_changed = true;
                                    changed = true;
                                    ui.close();
                                }
                            }
                        });
                },
            );
            let maximum = self
                .session
                .project
                .clip_fade_limit(clip.id, left, edge.crossfade);
            let mut ms = edge.length_frames.min(maximum) as f64 * 1000.0
                / f64::from(self.session.project.sample_rate_hz);
            if panel
                .field_row(
                    ui,
                    label_width,
                    |ui| {
                        ui.small("Duration");
                    },
                    |ui| {
                        clip_settings::setting_number(
                            ui,
                            ui.make_persistent_id((clip.id, "fade-duration", left)),
                            &mut ms,
                            0.0..=maximum as f64 * 1000.0
                                / f64::from(self.session.project.sample_rate_hz),
                            1.0,
                            " ms",
                        )
                    },
                )
                .inner
                .changed()
            {
                edge.length_frames = self.session.project.frames(ms / 1000.0).min(maximum);
                changed = true;
            }
            if changed {
                edge.length_frames = edge.length_frames.min(maximum);
            }
            let mut shape = f64::from((visual_level(*edge) - 0.5) * 200.0);
            if panel
                .field_row(
                    ui,
                    label_width,
                    |ui| {
                        ui.small("Shape");
                    },
                    |ui| {
                        ui.add_enabled_ui(edge.length_frames > 0, |ui| {
                            clip_settings::setting_number(
                                ui,
                                ui.make_persistent_id((clip.id, "fade-shape", left)),
                                &mut shape,
                                -70.0..=70.0,
                                1.0,
                                "%",
                            )
                        })
                        .inner
                    },
                )
                .inner
                .changed()
            {
                let level =
                    (shape as f32 / 200.0 + 0.5).clamp(ClipFade::MIN_LEVEL, ClipFade::MAX_LEVEL);
                edge.midpoint = level.clamp(ClipFade::MIN_LEVEL, ClipFade::MAX_LEVEL);
                changed = true;
            }
            let mut position = f64::from(visual_position(*edge) * 100.0);
            if panel
                .field_row(
                    ui,
                    label_width,
                    |ui| {
                        ui.small("Position");
                    },
                    |ui| {
                        ui.add_enabled_ui(edge.length_frames > 0, |ui| {
                            clip_settings::setting_number(
                                ui,
                                ui.make_persistent_id((clip.id, "fade-position", left)),
                                &mut position,
                                15.0..=85.0,
                                0.5,
                                "%",
                            )
                        })
                        .inner
                        .on_hover_text(
                            "Midpoint position from the clip edge to the duration handle",
                        )
                    },
                )
                .inner
                .changed()
            {
                let value =
                    (position as f32 / 100.0).clamp(ClipFade::MIN_POSITION, ClipFade::MAX_POSITION);
                edge.midpoint_position = (if edge.crossfade { 1.0 - value } else { value })
                    .clamp(ClipFade::MIN_POSITION, ClipFade::MAX_POSITION);
                changed = true;
            }
            if changed {
                edge.length_frames = edge.length_frames.min(self.session.project.clip_fade_limit(
                    clip.id,
                    left,
                    edge.crossfade,
                ));
                let next = *edge;
                let mut original = preview;
                let original = *selected_fade(&mut original, left);
                let targets: Vec<_> = self
                    .selected_clip_snapshots()
                    .into_iter()
                    .filter(|entry| self.audio_clip(&entry.clip))
                    .map(|entry| entry.clip)
                    .collect();
                let targets = if targets.is_empty() {
                    vec![clip.clone()]
                } else {
                    targets
                };
                commands.extend(self.fade_group_edits(&targets, left, |target| {
                    let mut old = target.fades;
                    let mut value = *selected_fade(&mut old, left);
                    if mode_changed {
                        value.crossfade = next.crossfade;
                    }
                    let delta = i128::from(next.length_frames) - i128::from(original.length_frames);
                    value.length_frames = (i128::from(value.length_frames) + delta).clamp(
                        0,
                        i128::from(self.session.project.clip_fade_limit(
                            target.id,
                            left,
                            value.crossfade,
                        )),
                    ) as u64;
                    value.midpoint = (value.midpoint + (next.midpoint - original.midpoint))
                        .clamp(ClipFade::MIN_LEVEL, ClipFade::MAX_LEVEL);
                    value.midpoint_position = (value.midpoint_position
                        + (visual_position(next) - visual_position(original))
                            * if value.crossfade { -1.0 } else { 1.0 })
                    .clamp(ClipFade::MIN_POSITION, ClipFade::MAX_POSITION);
                    value
                }));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_duration_handle_stays_whole_at_viewport_edges() {
        let viewport = Rect::from_min_size(Pos2::new(100.0, 40.0), Vec2::new(300.0, 160.0));
        for x in [viewport.left(), viewport.right()] {
            let center = visible_handle(Pos2::new(x, 66.0), ClipFade::default(), viewport);
            assert!(viewport.contains_rect(Rect::from_center_size(center, Vec2::splat(6.0))));
        }
    }

    #[test]
    fn curve_meets_handle_top_and_two_axis_knot_in_both_directions_and_modes() {
        let block = Rect::from_min_size(Pos2::new(150.0, 40.0), Vec2::new(300.0, 160.0));
        for left in [false, true] {
            for crossfade in [false, true] {
                for position in [0.15, 0.35, 0.8, 0.85] {
                    let fade = daw_core::ClipFade {
                        length_frames: 100,
                        midpoint: 0.7,
                        midpoint_position: position,
                        crossfade,
                    };
                    let (handle, midpoint) = points(block, fade, left, 100.0, 100);
                    let curve = curve_points(block, fade, left, handle);
                    assert_eq!(handle.y, block.top() + HANDLE_CENTER);
                    assert_eq!(
                        curve.last(),
                        Some(&Pos2::new(handle.x, handle.y - HANDLE_RADIUS))
                    );
                    assert!((curve[16] - midpoint).length() < 0.0001);
                    assert_eq!(curve.first().unwrap().y, block.bottom());
                }
            }
        }
    }

    #[test]
    fn grey_fill_covers_only_the_area_above_the_fade_curve() {
        let ctx = egui::Context::default();
        let block = Rect::from_min_size(Pos2::new(150.0, 40.0), Vec2::new(300.0, 160.0));
        for left in [false, true] {
            for crossfade in [false, true] {
                let fade = daw_core::ClipFade {
                    length_frames: 100,
                    midpoint: 0.7,
                    midpoint_position: 0.35,
                    crossfade,
                };
                let (handle, _) = points(block, fade, left, 100.0, 100);
                let curve = curve_points(block, fade, left, handle);
                let mut output = ctx.run_ui(Default::default(), |ui| {
                    shade_above(ui.painter(), &curve, handle.y - HANDLE_RADIUS)
                });
                let mesh = output
                    .shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::Shape::Mesh(mesh) => Some(mesh),
                        _ => None,
                    })
                    .expect("grey overlay mesh");
                assert!(mesh.is_valid());
                let anchor = if left { block.left() } else { block.right() };
                for vertex in &mesh.vertices {
                    assert!(vertex.color.a() > 0 && vertex.color.a() < 255);
                    assert!(vertex.pos.y >= handle.y - HANDLE_RADIUS);
                    let progress = (vertex.pos.x - anchor) / (handle.x - anchor);
                    assert!((0.0..=1.0).contains(&progress));
                    let curve_y =
                        gain_y(block, handle.y - HANDLE_RADIUS, visual_gain(fade, progress));
                    assert!(
                        vertex.pos.y <= curve_y + 0.0001,
                        "fill must not extend below the curve"
                    );
                }
                output.textures_delta.clear();
            }
        }
    }
    #[test]
    fn crossfade_shading_keeps_one_side_clear_and_doubles_the_other() {
        let ctx = egui::Context::default();
        let block = Rect::from_min_size(Pos2::new(150.0, 40.0), Vec2::new(300.0, 160.0));
        let coverage = |mesh: &egui::Mesh, point: Pos2| {
            mesh.indices
                .chunks_exact(3)
                .filter(|triangle| {
                    let [a, b, c] = [
                        mesh.vertices[triangle[0] as usize].pos,
                        mesh.vertices[triangle[1] as usize].pos,
                        mesh.vertices[triangle[2] as usize].pos,
                    ];
                    let cross = |u: egui::Vec2, v: egui::Vec2| u.x * v.y - u.y * v.x;
                    let area = cross(b - a, c - a);
                    if area.abs() < 0.00001 {
                        return false;
                    }
                    let signs = [
                        cross(b - a, point - a),
                        cross(c - b, point - b),
                        cross(a - c, point - c),
                    ];
                    signs.iter().all(|v| *v >= -0.00001) || signs.iter().all(|v| *v <= 0.00001)
                })
                .count()
        };
        for left in [false, true] {
            let fade = ClipFade {
                length_frames: 100,
                midpoint: 0.7,
                midpoint_position: 0.3,
                crossfade: true,
            };
            let (handle, _) = points(block, fade, left, 100.0, 100);
            let curve = curve_points(block, fade, left, handle);
            let companion = curve_points_for_gain(block, fade, left, handle, true);
            let mut output = ctx.run_ui(Default::default(), |ui| {
                shade_crossfade(
                    ui.painter(),
                    &curve,
                    &companion,
                    handle.y - HANDLE_RADIUS,
                    block.bottom(),
                )
            });
            let mesh = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) => Some(mesh),
                    _ => None,
                })
                .unwrap();
            for (index, expected) in [(4, 0), (28, 2)] {
                let a = curve[index].lerp(curve[index + 1], 0.37);
                let b = companion[index].lerp(companion[index + 1], 0.37);
                assert_eq!(
                    coverage(mesh, Pos2::new(a.x, (a.y + b.y) * 0.5)),
                    expected,
                    "the original side stays clear and the extension side receives two masks"
                );
                assert_eq!(
                    coverage(
                        mesh,
                        Pos2::new(a.x, (handle.y - HANDLE_RADIUS + a.y.min(b.y)) * 0.5)
                    ),
                    1
                );
                assert_eq!(
                    coverage(mesh, Pos2::new(a.x, (block.bottom() + a.y.max(b.y)) * 0.5)),
                    1
                );
            }
            output.textures_delta.clear();
        }
    }

    #[test]
    fn half_level_midpoint_is_centered_in_the_waveform() {
        let block = Rect::from_min_size(Pos2::new(150.0, 40.0), Vec2::new(300.0, 160.0));
        for left in [false, true] {
            for crossfade in [false, true] {
                let fade = ClipFade {
                    length_frames: 100,
                    crossfade,
                    ..Default::default()
                };
                let (handle, midpoint) = points(block, fade, left, 100.0, 100);
                assert_eq!(
                    midpoint.y,
                    (block.top() + WAVEFORM_TOP + block.bottom()) * 0.5
                );
                assert_eq!(curve_points(block, fade, left, handle)[16], midpoint);
            }
        }
    }

    #[test]
    fn crossfades_draw_both_curves_through_the_time_and_gain_control() {
        let ctx = egui::Context::default();
        let block = Rect::from_min_size(Pos2::new(150.0, 40.0), Vec2::new(300.0, 160.0));
        for left in [false, true] {
            for crossfade in [false, true] {
                let fade = ClipFade {
                    length_frames: 100,
                    midpoint: 0.7,
                    midpoint_position: 0.3,
                    crossfade,
                };
                let (handle, _) = points(block, fade, left, 100.0, 100);
                let top = handle.y - HANDLE_RADIUS;
                if crossfade {
                    let (_, knot_point) = points(block, fade, left, 100.0, 100);
                    let companion = curve_points_for_gain(block, fade, left, handle, true);
                    assert!((companion[16].y - knot_point.y).abs() < 0.0001);
                    assert!((companion[16].x - knot_point.x).abs() < 0.0001);
                }
                let curve = curve_points(block, fade, left, handle);
                let output = ctx.run_ui(Default::default(), |ui| {
                    paint_fade_curves(
                        ui.painter(),
                        curve.clone(),
                        crossfade.then(|| curve_points_for_gain(block, fade, left, handle, true)),
                    );
                });
                let paths: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Path(path) => Some(path),
                        _ => None,
                    })
                    .collect();
                assert_eq!(paths.len(), if crossfade { 2 } else { 1 });
                assert_eq!(paths.last().unwrap().points, curve);
                if crossfade {
                    for (a, b) in paths[0].points.iter().zip(&paths[1].points) {
                        assert_eq!(a.x, b.x);
                        let index = paths[1].points.iter().position(|point| point == b).unwrap();
                        let knot = visual_position(fade);
                        let t = if index <= 16 {
                            knot * index as f32 / 16.0
                        } else {
                            knot + (1.0 - knot) * (index - 16) as f32 / 16.0
                        };
                        assert!((a.y - gain_y(block, top, fade.gain(1.0 - t))).abs() < 0.0001);
                    }
                }
                let mut output = output;
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn fade_markers_have_transparent_bodies_dark_borders_and_dark_curve_strokes() {
        let ctx = egui::Context::default();
        let center = Pos2::new(100.0, 50.0);
        let midpoint = Pos2::new(50.0, 80.0);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            paint_curve(
                ui.painter(),
                vec![
                    Pos2::new(0.0, 100.0),
                    midpoint,
                    Pos2::new(center.x, center.y - HANDLE_RADIUS),
                ],
            );
            paint_guide(ui.painter(), center, 100.0);
            paint_crossover_guides(
                ui.painter(),
                Rect::from_min_max(Pos2::new(20.0, 30.0), Pos2::new(80.0, 100.0)),
            );
            paint_handle(ui.painter(), center);
            paint_midpoint(ui.painter(), midpoint);
        });
        assert!(output.shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Rect(rect) if rect.rect.center() == center
                && rect.fill == egui::Color32::TRANSPARENT && rect.stroke.color == DARK_INK
                && rect.stroke.width == 1.0)));
        assert!(output.shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Circle(circle) if circle.center == midpoint
                && circle.fill == egui::Color32::TRANSPARENT && circle.stroke.color == DARK_INK
                && circle.stroke.width == 1.0)));
        assert!(output.shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::Path(path) if path.stroke.color == egui::epaint::ColorMode::Solid(DARK_INK)
                && path.stroke.width == 1.0)));
        assert!(output.shapes.iter().any(|s| matches!(&s.shape,
            egui::Shape::LineSegment { points, stroke }
                if *points == [Pos2::new(center.x, center.y + HANDLE_RADIUS), Pos2::new(center.x, 100.0)]
                    && stroke.color == DARK_INK && stroke.width == 1.0)));
        for x in [20.5, 79.5] {
            assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::LineSegment { points, stroke }
                    if *points == [Pos2::new(x, 30.0), Pos2::new(x, 100.0)]
                        && stroke.color == DARK_INK && stroke.width == 1.0)));
        }
        output.textures_delta.clear();
    }
}
