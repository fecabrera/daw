//! Gesture preview, release, and selected-block deletion.
use super::*;
impl DawUi {
    pub(super) fn update_arrangement_drag(&mut self, ui: &egui::Ui) {
        let Some(mut drag) = self.arrangement.drag.take() else {
            return;
        };
        let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) else {
            return;
        };
        let position = drag
            .tempo
            .as_mut()
            .map_or(pointer, |tempo| tempo.direction.update(pointer));
        let timeline = self.musical_timeline();
        let raw_delta = timeline.frame_delta(f64::from(position.x - drag.origin.x));
        let start = drag.item.frame;
        let end = drag
            .item
            .length_frames
            .map_or(u64::MAX, |length| start.saturating_add(length));
        let edge = match drag.edge {
            DragEdge::Move => None,
            DragEdge::Left => Some(ClipEdge::Left),
            DragEdge::Right => Some(ClipEdge::Right),
        };
        let anchor = if drag.edge == DragEdge::Right {
            end
        } else {
            start
        };
        let bounds = if drag.item.kind == Kind::Tempo && start == 0 {
            0..=0
        } else if drag.linked {
            0..=u64::MAX
        } else {
            match drag.edge {
                DragEdge::Move => {
                    if drag.item.kind == Kind::Signature {
                        1..=end - 1
                    } else {
                        0..=u64::MAX.saturating_sub(drag.item.length_frames.unwrap_or(0))
                    }
                }
                DragEdge::Left => {
                    (if drag.item.kind == Kind::Signature {
                        1
                    } else {
                        0
                    })..=end - 1
                }
                DragEdge::Right => start + 1..=u64::MAX,
            }
        };
        let anchors = self
            .arrangement_items(drag.item.kind)
            .iter()
            .flat_map(|item| {
                [
                    item.frame,
                    item.frame.saturating_add(item.length_frames.unwrap_or(0)),
                ]
            })
            .collect::<Vec<_>>();
        drag.delta = if raw_delta == 0 {
            0
        } else {
            i128::from(timeline.snap_bounded(
                i128::from(anchor) + raw_delta,
                bounds,
                anchors,
                musical_time::snap_bypassed(ui),
            )) - i128::from(anchor)
        };
        drag.markers = None;
        drag.signatures = None;
        drag.error = None;
        if let Some(tempo) = &mut drag.tempo {
            let delta_y = position.y - tempo.previous_y;
            if delta_y != 0.0 {
                tempo.bpm = tempo
                    .value
                    .update(delta_y, ui.input(|input| input.modifiers.shift));
            }
            tempo.previous_y = position.y;
        }
        if drag.item.kind == Kind::Tempo {
            drag.frame = (i128::from(start) + drag.delta)
                .clamp(if start == 0 { 0 } else { 1 }, i128::from(u64::MAX))
                as u64;
        }
        if drag.delta != 0 {
            match drag.item.kind {
                Kind::Marker => match self.session.project.edited_sections(
                    drag.item.id,
                    &drag.selected,
                    edge,
                    drag.delta,
                    drag.linked,
                ) {
                    Ok(markers) => drag.markers = Some(markers),
                    Err(error) => drag.error = Some(error.to_string()),
                },
                Kind::Signature => match self.session.project.edited_signature_blocks(
                    drag.item.id,
                    &drag.selected,
                    edge,
                    drag.delta,
                    drag.linked,
                ) {
                    Ok(events) => drag.signatures = Some(events),
                    Err(error) => drag.error = Some(error.to_string()),
                },
                Kind::Tempo => {}
            }
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            let tempo_changed = drag
                .tempo
                .as_ref()
                .is_some_and(|tempo| tempo.bpm != tempo.initial_bpm);
            let inside = if drag.tempo.is_some() {
                position.x >= drag.lane.left() && position.x <= drag.lane.right()
            } else {
                drag.lane.contains(pointer)
            };
            if inside && (drag.delta != 0 || tempo_changed) {
                if let Some(error) = drag.error {
                    self.fail(error);
                } else if let Some(markers) = drag.markers {
                    self.edit(Edit::SetMarkers { markers });
                } else if let Some(events) = drag.signatures {
                    self.edit(Edit::SetSignatureEvents { events });
                } else if drag.item.kind == Kind::Tempo {
                    let mut events = self.session.project.tempo_events();
                    if let Some(event) = events.iter_mut().find(|e| e.id == drag.item.id) {
                        event.start_frame = drag.frame;
                        if let Some(tempo) = drag.tempo {
                            event.bpm = tempo.bpm;
                        }
                    }
                    self.edit(Edit::SetTempoEvents { events });
                }
            }
        } else {
            self.arrangement.drag = Some(drag);
        }
    }

    pub(super) fn delete_arrangement_selection(&mut self) {
        let mut edits = Vec::new();
        let markers: Vec<_> = self
            .session
            .project
            .markers
            .iter()
            .filter(|m| !self.arrangement.selection.contains(&(Kind::Marker, m.id)))
            .cloned()
            .collect();
        if markers != self.session.project.markers {
            edits.push(Edit::SetMarkers { markers });
        }
        let original = self.session.project.tempo_events();
        let events: Vec<_> = original
            .iter()
            .copied()
            .filter(|e| {
                e.start_frame == 0 || !self.arrangement.selection.contains(&(Kind::Tempo, e.id))
            })
            .collect();
        if events != original {
            edits.push(Edit::SetTempoEvents { events });
        }
        let original = self.session.project.signature_events();
        let events: Vec<_> = original
            .iter()
            .copied()
            .filter(|e| {
                e.start_frame == 0
                    || !self
                        .arrangement
                        .selection
                        .contains(&(Kind::Signature, e.id))
            })
            .collect();
        if events != original {
            edits.push(Edit::SetSignatureEvents { events });
        }
        // Remove signature changes before tempo edits reproject their anchors.
        edits.sort_by_key(|edit| u8::from(matches!(edit, Edit::SetTempoEvents { .. })));
        if !edits.is_empty() {
            self.edit(Edit::Batch(edits));
        }
        self.arrangement.selection.clear();
        self.arrangement.selected = None;
    }
}
