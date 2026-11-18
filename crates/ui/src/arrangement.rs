//! Pinned conductor lanes. Model changes commit through the ordinary history path.
use super::*;
mod add;
mod gestures;
mod tempo;
use daw_core::{ArrangementMarker, ClipEdge, SignatureEvent, TempoEvent, TimeSignature};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Marker,
    Tempo,
    Signature,
}
#[derive(Clone)]
struct Item {
    kind: Kind,
    id: Id,
    frame: u64,
    label: String,
    length_frames: Option<u64>,
}
struct Draft {
    item: Item,
    text: String,
    numerator: u16,
    denominator: u16,
}
struct SectionRename {
    project: Id,
    id: Id,
    original: String,
    text: String,
    focus: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum DragEdge {
    Move,
    Left,
    Right,
}
struct Drag {
    item: Item,
    frame: u64,
    origin: Pos2,
    revision: u64,
    edge: DragEdge,
    selected: Vec<Id>,
    linked: bool,
    lane: Rect,
    markers: Option<Vec<ArrangementMarker>>,
    signatures: Option<Vec<SignatureEvent>>,
    error: Option<String>,
    delta: i128,
    tempo: Option<tempo::TempoGesture>,
}
pub(super) struct State {
    pub(super) ruler_bounds: Rect,
    visible: [bool; 3],
    pub(super) collapsed: bool,
    drag: Option<Drag>,
    draft: Option<Draft>,
    rename: Option<SectionRename>,
    selected: Option<Item>,
    selection: HashSet<(Kind, Id)>,
    focus: Option<egui::Id>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            ruler_bounds: Rect::NOTHING,
            visible: [true; 3],
            collapsed: !daw_core::ProjectDisplay::default().arrangement_expanded,
            drag: None,
            draft: None,
            rename: None,
            selected: None,
            selection: HashSet::new(),
            focus: None,
        }
    }
}
impl DawUi {
    fn arrangement_items(&self, kind: Kind) -> Vec<Item> {
        let p = &self.session.project;
        let clock = p.musical_clock().ok();
        let mut items: Vec<_> = match kind {
            Kind::Marker => self
                .arrangement
                .drag
                .as_ref()
                .and_then(|d| d.markers.as_ref())
                .unwrap_or(&p.markers)
                .iter()
                .map(|m| Item {
                    kind,
                    id: m.id,
                    frame: m.start_frame,
                    label: m.name.clone(),
                    length_frames: clock
                        .as_ref()
                        .and_then(|clock| clock.section_end_frame(m).ok())
                        .map(|end| end - m.start_frame),
                })
                .collect(),
            Kind::Tempo => self
                .arrangement_tempos()
                .iter()
                .map(|e| Item {
                    kind,
                    id: e.id,
                    frame: e.start_frame,
                    label: format!("{} BPM", e.bpm),
                    length_frames: None,
                })
                .collect(),
            Kind::Signature => {
                let events = self
                    .arrangement
                    .drag
                    .as_ref()
                    .and_then(|d| d.signatures.clone())
                    .unwrap_or_else(|| p.signature_events());
                events
                    .iter()
                    .enumerate()
                    .map(|(index, e)| Item {
                        kind,
                        id: e.id,
                        frame: e.start_frame,
                        label: format!("{}/{}", e.signature.numerator, e.signature.denominator),
                        length_frames: events
                            .get(index + 1)
                            .map(|next| next.start_frame - e.start_frame),
                    })
                    .collect()
            }
        };
        items.sort_by_key(|i| (i.frame, i.id));
        items
    }
    fn arrangement_draft(&mut self, item: Item, new: bool) {
        if item.kind == Kind::Tempo && new {
            self.add_tempo_point(item);
            return;
        }
        if item.kind == Kind::Marker {
            self.arrangement.drag = None;
            if new {
                let mut markers = self.session.project.markers.clone();
                let mut marker = ArrangementMarker {
                    id: item.id,
                    start_frame: item.frame,
                    name: "New section".into(),
                    length_frames: None,
                };
                match self
                    .session
                    .project
                    .musical_clock()
                    .and_then(|clock| clock.section_end_frame(&marker))
                {
                    Ok(end) => marker.length_frames = Some(end - item.frame),
                    Err(error) => {
                        self.fail(error);
                        return;
                    }
                }
                markers.push(marker);
                self.edit(Edit::SetMarkers { markers });
            } else {
                self.arrangement.rename = Some(SectionRename {
                    project: self.session.project.project_id,
                    id: item.id,
                    original: item.label.clone(),
                    text: item.label,
                    focus: true,
                });
            }
            return;
        }
        let p = &self.session.project;
        let signature = p
            .musical_clock()
            .map_or(p.time_signature, |c| c.signature_at(item.frame));
        let text = if item.kind == Kind::Tempo {
            p.musical_clock()
                .map_or(p.tempo_bpm, |c| c.tempo_at(item.frame))
                .to_string()
        } else {
            item.label.clone()
        };
        self.arrangement.draft = Some(Draft {
            item,
            text,
            numerator: signature.numerator,
            denominator: signature.denominator,
        });
    }
    fn section_name_input(&mut self, ui: &mut egui::Ui, item: &Item, bounds: Rect) -> bool {
        if !self
            .arrangement
            .rename
            .as_ref()
            .is_some_and(|r| r.id == item.id)
        {
            return false;
        }
        let mut rename = self.arrangement.rename.take().unwrap();
        let height = toolbars::CONTROL_HEIGHT.min(bounds.height());
        let left = (bounds.left() + 6.0).min(bounds.right() - 1.0);
        let input = Rect::from_min_size(
            Pos2::new(left, bounds.center().y - height * 0.5),
            Vec2::new((bounds.right() - left - 6.0).max(1.0), height),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("section-name", item.id))
                .max_rect(input),
        );
        child.set_clip_rect(bounds.intersect(ui.clip_rect()));
        let id = child.make_persistent_id(("section-name-input", item.id));
        let response = rows::name_input(&mut child, &mut rename.text, id, 12.0, &mut rename.focus);
        response.widget_info(|| {
            egui::WidgetInfo::text_edit(
                child.is_enabled(),
                &rename.original,
                &rename.text,
                "Section name",
            )
        });
        self.arrangement.focus = Some(response.id);
        if let Some(action) = inline_edit_action(&mut child, &response) {
            response.surrender_focus();
            if action == InlineEditAction::Commit && rename.text.trim() != rename.original {
                let mut markers = self.session.project.markers.clone();
                if let Some(marker) = markers.iter_mut().find(|m| m.id == rename.id) {
                    marker.name = rename.text.trim().into();
                    self.edit(Edit::SetMarkers { markers });
                }
            }
        } else {
            self.arrangement.rename = Some(rename);
        }
        true
    }
    pub(super) fn arrangement_toggle(&mut self, ui: &mut egui::Ui) {
        let (icon, label) = if self.arrangement.collapsed {
            (icons::Lucide::PanelTopOpen, "Expand arrangement lanes")
        } else {
            (icons::Lucide::PanelTopClose, "Collapse arrangement lanes")
        };
        if icons::panel_button(ui, icon, label, true, theme::palette(ui.ctx()).text).clicked() {
            self.arrangement.collapsed = !self.arrangement.collapsed;
        }
    }

    pub(super) fn arrangement_view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Arrangement lanes", |ui| {
            ui.checkbox(&mut self.arrangement.collapsed, "Collapse lanes");
            self.arrangement_visibility_menu(ui);
        });
    }

    fn arrangement_visibility_menu(&mut self, ui: &mut egui::Ui) {
        for (visible, label) in
            self.arrangement
                .visible
                .iter_mut()
                .zip(["Markers", "Tempo", "Signature"])
        {
            ui.checkbox(visible, label);
        }
    }
    pub(super) fn arrangement_has_focus(&self, ctx: &egui::Context) -> bool {
        self.arrangement
            .focus
            .is_some_and(|id| ctx.memory(|m| m.has_focus(id)))
    }
    pub(super) fn arrangement_lanes(&mut self, ui: &mut egui::Ui, content_bottom: f32) {
        if self.arrangement.drag.is_some()
            && (ui.input(|i| i.key_pressed(egui::Key::Escape) || !i.focused)
                || self.midi.busy()
                || self.job.is_some()
                || self.arrangement.draft.is_some()
                || self.arrangement.rename.is_some()
                || self
                    .arrangement
                    .drag
                    .as_ref()
                    .is_some_and(|d| d.revision != self.change_revision))
        {
            self.arrangement.drag = None;
        }
        if self.arrangement.collapsed
            || content_bottom - ui.cursor().top() < 160.0
            || !self.arrangement.visible.iter().any(|visible| *visible)
        {
            self.arrangement.drag = None;
            self.arrangement.rename = None;
            self.arrangement.focus = None;
            return;
        }

        let palette = theme::palette(ui.ctx());
        let timeline = self.musical_timeline();
        let editable = ui.is_enabled() && self.edit_environment_available() && !self.is_playing();
        if self.arrangement.drag.as_ref().is_some_and(|d| {
            !editable
                || !self.arrangement.visible[match d.item.kind {
                    Kind::Marker => 0,
                    Kind::Tempo => 1,
                    Kind::Signature => 2,
                }]
        }) {
            self.arrangement.drag = None;
        }
        self.update_arrangement_drag(ui);
        let valid: HashSet<_> = [Kind::Marker, Kind::Tempo, Kind::Signature]
            .into_iter()
            .flat_map(|kind| {
                self.arrangement_items(kind)
                    .into_iter()
                    .map(move |item| (kind, item.id))
            })
            .collect();
        self.arrangement.selection.retain(|key| valid.contains(key));
        if self.arrangement.rename.as_ref().is_some_and(|r| {
            !editable
                || !self.arrangement.visible[0]
                || r.project != self.session.project.project_id
                || !self
                    .session
                    .project
                    .markers
                    .iter()
                    .any(|m| m.id == r.id && m.name == r.original)
        }) {
            self.arrangement.rename = None;
        }
        if editable
            && self.arrangement_has_focus(ui.ctx())
            && !ui.ctx().text_edit_focused()
            && self.arrangement.draft.is_none()
            && self.arrangement.rename.is_none()
            && self.arrangement.drag.is_none()
        {
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.arrangement.selected = None;
                self.arrangement.selection.clear();
            }
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::A))
                && let Some(kind) = self.arrangement.selected.as_ref().map(|i| i.kind)
            {
                let keys = self
                    .arrangement_items(kind)
                    .into_iter()
                    .map(|i| (kind, i.id))
                    .collect::<Vec<_>>();
                unit_selection::select_all(&mut self.arrangement.selection, keys);
            }
            if clip_delete::take_delete(ui.ctx()) {
                self.delete_arrangement_selection();
            }
        }
        let rows = [
            (Kind::Marker, "Markers"),
            (Kind::Tempo, "Tempo"),
            (Kind::Signature, "Signature"),
        ];
        let mut first_row = true;
        for (index, (kind, name)) in rows.into_iter().enumerate() {
            if !self.arrangement.visible[index] {
                continue;
            }
            let (row, _) = ui.allocate_exact_size(
                Vec2::new(ui.available_width(), beat_editor::SUBTRACK_LANE_HEIGHT),
                Sense::hover(),
            );
            let header = Rect::from_min_max(
                row.min,
                Pos2::new(row.left() + panels::ControlPanel::WIDTH, row.bottom()),
            );
            let lane = Rect::from_min_max(header.right_top(), row.max);
            ui.painter().rect_filled(lane, 0.0, palette.background);
            self.paint_grid(&ui.painter().with_clip_rect(lane), lane);
            ui.painter().rect_filled(header, 0.0, palette.panel);
            if first_row {
                panels::ControlPanel::pinned_row_divider(ui, row);
                first_row = false;
            }
            ui.painter().text(
                header.left_center() + Vec2::new(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                name,
                crate::fonts::semibold(12.0),
                palette.text,
            );
            let add_name = match kind {
                Kind::Marker => "Add section at playhead",
                Kind::Tempo => "Add tempo point at playhead",
                Kind::Signature => "Add time signature at playhead",
            };
            let add_rect = Rect::from_center_size(
                header.right_center() - Vec2::new(8.0 + icons::COMPACT_BUTTON_SIZE * 0.5, 0.0),
                Vec2::splat(icons::COMPACT_BUTTON_SIZE),
            );
            let h = ui.interact(
                Rect::from_min_max(
                    header.min,
                    Pos2::new(add_rect.left() - 4.0, header.bottom()),
                ),
                ui.id().with(("arrangement-header", index)),
                Sense::click(),
            );
            h.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name)
            });
            h.context_menu(|ui| self.arrangement_visibility_menu(ui));
            let mut add_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("arrangement-add", index))
                    .max_rect(add_rect),
            );
            add_ui.set_clip_rect(header.intersect(ui.clip_rect()));
            let add = icons::compact_panel_button(
                &mut add_ui,
                icons::Lucide::Plus,
                add_name,
                editable,
                palette.text,
            );
            if add.clicked() {
                self.add_arrangement_at_playhead(kind);
                self.arrangement.focus = Some(add.id);
                add.request_focus();
            }
            let background = ui.interact(
                lane,
                ui.id().with(("arrangement-lane", index)),
                Sense::click(),
            );
            background.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, editable, format!("Add {name}"))
            });
            let insertion_frame = background
                .interact_pointer_pos()
                .or_else(|| ui.input(|i| i.pointer.hover_pos()))
                .map(|pos| {
                    timeline.snap_bounded(
                        timeline.frame_at_pixel(f64::from(pos.x - lane.left())),
                        0..=u64::MAX,
                        [],
                        ui.input(|i| i.modifiers.shift),
                    )
                })
                .unwrap_or(0);
            let new_item = || Item {
                kind,
                id: Id::new_v4(),
                frame: insertion_frame,
                length_frames: None,
                label: if kind == Kind::Marker {
                    "New section".into()
                } else {
                    String::new()
                },
            };
            if background.clicked() && !background.double_clicked() {
                self.arrangement.selection.clear();
                self.arrangement.selected = None;
            }
            if background.double_clicked() && editable {
                if kind == Kind::Tempo {
                    self.arrangement.focus = Some(background.id);
                    background.request_focus();
                }
                self.arrangement_draft(new_item(), true);
            }
            background.context_menu(|ui| {
                if ui
                    .add_enabled(editable, egui::Button::new(format!("Add {name}")))
                    .clicked()
                {
                    if kind == Kind::Tempo {
                        self.arrangement.focus = Some(background.id);
                        background.request_focus();
                    }
                    self.arrangement_draft(new_item(), true);
                    ui.close();
                }
            });
            let items = self.arrangement_items(kind);
            let tempos = self.arrangement_tempos();
            let (low, high) = self
                .arrangement
                .drag
                .as_ref()
                .and_then(|d| d.tempo.as_ref())
                .map_or_else(|| tempo::value_range(&tempos), |gesture| gesture.range);
            let node_y = |id: Id| {
                let bpm = tempos
                    .iter()
                    .find(|e| e.id == id)
                    .map_or(self.session.project.tempo_bpm, |e| e.bpm);
                let fraction = ((f64::from(bpm) - f64::from(low))
                    / (f64::from(high) - f64::from(low)).max(0.01))
                .clamp(0.0, 1.0) as f32;
                lane.bottom() - 5.0 - fraction * (lane.height() - 10.0)
            };
            // Copy node heights before interactions can edit the project.
            let heights: Vec<_> = items.iter().map(|item| node_y(item.id)).collect();
            for (i, item) in items.iter().enumerate() {
                let x = lane.left() + timeline.pixel_at_frame(item.frame) as f32;
                let right = if let Some(length) = item.length_frames {
                    lane.left() + timeline.pixel_at_frame(item.frame.saturating_add(length)) as f32
                } else {
                    items.get(i + 1).map_or(lane.right(), |next| {
                        lane.left() + timeline.pixel_at_frame(next.frame) as f32
                    })
                };
                let block = if kind == Kind::Tempo {
                    Rect::from_min_size(
                        Pos2::new(x, lane.top() + 1.0),
                        Vec2::new(86.0, lane.height() - 2.0),
                    )
                } else {
                    Rect::from_min_max(
                        Pos2::new(x, lane.top()),
                        Pos2::new(right.max(x + 2.0) - 1.0, lane.bottom() - 1.0),
                    )
                };
                if !block.intersects(lane) {
                    continue;
                }
                let painter = ui.painter().with_clip_rect(lane);
                if kind == Kind::Tempo {
                    painter.hline(
                        x.max(lane.left())..=right.min(lane.right()),
                        heights[i],
                        egui::Stroke::new(1.0, palette.secondary),
                    );
                    if let Some(next) = heights.get(i + 1) {
                        painter.vline(
                            right,
                            heights[i].min(*next)..=heights[i].max(*next),
                            egui::Stroke::new(1.0, palette.secondary),
                        );
                    }
                    painter.circle_filled(
                        Pos2::new(x, heights[i]),
                        3.0,
                        panels::unit_header_text(
                            ui.ctx(),
                            self.arrangement.selection.contains(&(kind, item.id)),
                        ),
                    );
                } else {
                    painter.rect_filled(block, 0.0, theme::arrangement_block_fill(ui.ctx()));
                }
                let text_rect = block.intersect(lane);
                let selected = self.arrangement.selection.contains(&(kind, item.id));
                if self.section_name_input(ui, item, text_rect) {
                    continue;
                }
                painter.with_clip_rect(text_rect).text(
                    Pos2::new(x + 6.0, block.center().y),
                    egui::Align2::LEFT_CENTER,
                    &item.label,
                    if kind == Kind::Tempo {
                        egui::TextStyle::Small.resolve(ui.style())
                    } else {
                        fonts::semibold(panels::HEADER_FONT_SIZE)
                    },
                    if kind == Kind::Tempo {
                        palette.text
                    } else {
                        panels::unit_header_text(ui.ctx(), selected)
                    },
                );
                let response = ui
                    .interact(
                        if kind == Kind::Tempo {
                            Rect::from_center_size(Pos2::new(x, heights[i]), Vec2::splat(12.0))
                                .intersect(lane)
                        } else {
                            text_rect
                        },
                        ui.id().with(("arrangement-item", item.id)),
                        Sense::click_and_drag(),
                    )
                    .on_hover_text(&item.label);
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        &item.label,
                    )
                });
                if kind == Kind::Tempo && response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                if response.double_clicked() && editable && kind != Kind::Tempo {
                    self.arrangement_draft(item.clone(), false);
                }
                if response.clicked() || response.secondary_clicked() {
                    if response.clicked() || !self.arrangement.selection.contains(&(kind, item.id))
                    {
                        unit_selection::select(
                            &mut self.arrangement.selection,
                            (kind, item.id),
                            response.clicked() && ui.input(|i| i.modifiers.shift),
                        );
                    }
                    self.arrangement.selected = Some(item.clone());
                    self.arrangement.focus = Some(response.id);
                    response.request_focus();
                }
                let deletable = editable && (kind == Kind::Marker || item.frame > 0);
                let edge_at = |pos: Pos2| {
                    if (kind == Kind::Marker || kind == Kind::Signature && item.frame > 0)
                        && (pos.x - block.left()).abs() < trim::EDGE_RESIZE_RADIUS
                    {
                        DragEdge::Left
                    } else if (kind == Kind::Marker
                        || kind == Kind::Signature && item.length_frames.is_some())
                        && (pos.x - right).abs() < trim::EDGE_RESIZE_RADIUS
                    {
                        DragEdge::Right
                    } else {
                        DragEdge::Move
                    }
                };
                let press_edge = edge_at(
                    ui.input(|i| i.pointer.press_origin())
                        .unwrap_or(block.center()),
                );
                let movable = editable
                    && (kind == Kind::Marker
                        || kind == Kind::Tempo
                        || item.frame > 0
                        || kind == Kind::Signature && press_edge == DragEdge::Right);
                if editable
                    && response.hovered()
                    && response
                        .hover_pos()
                        .is_some_and(|p| edge_at(p) != DragEdge::Move)
                {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
                if response.drag_started() && movable {
                    self.arrangement.selected = Some(item.clone());
                    self.arrangement.focus = Some(response.id);
                    response.request_focus();
                    if !self.arrangement.selection.contains(&(kind, item.id)) {
                        unit_selection::select(
                            &mut self.arrangement.selection,
                            (kind, item.id),
                            false,
                        );
                    }
                    let press = ui
                        .input(|i| i.pointer.press_origin())
                        .unwrap_or(block.center());
                    let boundary = if press_edge == DragEdge::Left {
                        x
                    } else {
                        right
                    };
                    let linked = press_edge != DragEdge::Move
                        && (press.x - boundary).abs() <= trim::SHARED_BOUNDARY_RADIUS
                        && items.iter().any(|other| {
                            other.id != item.id
                                && match press_edge {
                                    DragEdge::Left => {
                                        other.frame.saturating_add(other.length_frames.unwrap_or(0))
                                            == item.frame
                                    }
                                    DragEdge::Right => {
                                        item.frame.saturating_add(item.length_frames.unwrap_or(0))
                                            == other.frame
                                    }
                                    DragEdge::Move => false,
                                }
                        });
                    let target = item;
                    self.arrangement.drag = Some(Drag {
                        item: target.clone(),
                        frame: target.frame,
                        origin: ui.input(|i| i.pointer.press_origin()).unwrap_or(block.min),
                        revision: self.change_revision,
                        edge: press_edge,
                        selected: self
                            .arrangement
                            .selection
                            .iter()
                            .filter_map(|(k, id)| (*k == kind).then_some(*id))
                            .collect(),
                        linked,
                        lane,
                        markers: None,
                        signatures: None,
                        error: None,
                        delta: 0,
                        tempo: if kind == Kind::Tempo {
                            tempos.iter().find(|e| e.id == item.id).map(|event| {
                                tempo::TempoGesture {
                                    initial_bpm: event.bpm,
                                    value: musical_time::TempoValueDrag::new(event.bpm),
                                    direction: tempo::TempoDirection::new(press, item.frame > 0),
                                    bpm: event.bpm,
                                    previous_y: press.y,
                                    range: tempo::value_range(&tempos),
                                }
                            })
                        } else {
                            None
                        },
                    });
                    if kind == Kind::Tempo {
                        self.update_arrangement_drag(ui);
                    }
                }
                response.context_menu(|ui| {
                    if kind == Kind::Marker && ui.button("Seek here").clicked() {
                        self.seek(item.frame);
                        ui.close();
                    }
                    for coincident in items
                        .iter()
                        .filter(|other| other.frame == item.frame && other.id != item.id)
                    {
                        if ui
                            .add_enabled(
                                editable,
                                egui::Button::new(format!("Edit {}…", coincident.label)),
                            )
                            .clicked()
                        {
                            self.arrangement_draft(coincident.clone(), false);
                            ui.close();
                        }
                    }
                    if ui
                        .add_enabled(editable, egui::Button::new("Edit…"))
                        .clicked()
                    {
                        self.arrangement_draft(item.clone(), false);
                        ui.close();
                    }
                    if ui
                        .add_enabled(deletable, egui::Button::new("Delete"))
                        .clicked()
                    {
                        self.delete_arrangement_selection();
                        ui.close();
                    }
                });
            }
            ui.painter().hline(
                row.x_range(),
                row.bottom() - 0.5,
                egui::Stroke::new(1.0, palette.border),
            );
            ui.painter().vline(
                header.right(),
                row.y_range(),
                egui::Stroke::new(1.0, palette.border),
            );
        }
        if self.arrangement.rename.as_ref().is_some_and(|r| !r.focus)
            && !self.arrangement_has_focus(ui.ctx())
        {
            self.arrangement.rename = None;
        }
    }
    pub(super) fn arrangement_dialog(&mut self, ctx: &egui::Context) {
        if self.view_mode != ViewMode::Timeline {
            self.arrangement.selection.clear();
            self.arrangement.selected = None;
            self.arrangement.drag = None;
        }
        if self.view_mode != ViewMode::Timeline
            || !self.edit_environment_available()
            || self.is_playing()
            || !ctx.input(|i| i.focused)
        {
            self.arrangement.rename = None;
        }
        let Some(mut draft) = self.arrangement.draft.take() else {
            return;
        };
        if self.midi.busy() || self.job.is_some() || self.is_playing() {
            return;
        }
        let action = dialogs::Dialog::new(
            "Arrangement event",
            "",
            &[("Apply", true), ("Cancel", false)],
        )
        .footer_separator(true)
        .show_with_content(ctx, |ui| match draft.item.kind {
            Kind::Marker => {}
            Kind::Tempo => {
                ui.label("Tempo (quarter-note BPM)");
                ui.text_edit_singleline(&mut draft.text);
            }
            Kind::Signature => {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut draft.numerator).range(1..=u16::MAX));
                    ui.label("/");
                    ui.add(egui::DragValue::new(&mut draft.denominator).range(1..=u16::MAX));
                });
            }
        });
        match action {
            Some(true) => {
                let p = &self.session.project;
                let id = draft.item.id;
                let start_frame = draft.item.frame;
                match draft.item.kind {
                    Kind::Marker => {}
                    Kind::Tempo => match draft.text.parse::<f32>() {
                        Ok(bpm) => {
                            let mut events = p.tempo_events();
                            events.retain(|e| e.id != id);
                            events.push(TempoEvent {
                                id,
                                start_frame,
                                bpm,
                            });
                            self.edit(Edit::SetTempoEvents { events });
                        }
                        Err(_) => self.fail("Tempo must be a positive finite BPM value"),
                    },
                    Kind::Signature => {
                        let mut events = p.signature_events();
                        let signature = TimeSignature {
                            numerator: draft.numerator,
                            denominator: draft.denominator,
                        };
                        events.retain(|e| e.id != id);
                        events.push(SignatureEvent {
                            id,
                            start_frame,
                            signature,
                        });
                        self.edit(Edit::SetSignatureEvents { events });
                    }
                }
            }
            Some(false) => {}
            None => self.arrangement.draft = Some(draft),
        }
    }
}
