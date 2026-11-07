//! Delete routing shared by Timeline and Editor, with note and text precedence.
use super::*;

/// Consume pressed Delete/Backspace events, including repeats. A held key must
/// not fall through from deleting notes/units to deleting their containing clip.
pub(super) fn take_delete(ctx: &egui::Context) -> bool {
    ctx.input_mut(|input| {
        let mut pressed = false;
        input.events.retain(|event| {
            if let egui::Event::Key {
                key: egui::Key::Delete | egui::Key::Backspace,
                pressed: true,
                repeat,
                ..
            } = event
            {
                pressed |= !repeat;
                false
            } else {
                true
            }
        });
        pressed
    })
}

impl DawUi {
    pub(super) fn delete_clip_shortcut(&mut self, ctx: &egui::Context, enabled: bool) {
        if !enabled
            || !self.clip_actions_available(ctx)
            || ctx.input(|input| input.pointer.any_down())
        {
            return;
        }
        if self.view_mode == ViewMode::Edit
            && !self.midi_editor.delete_clips
            && self.portion.is_none()
        {
            return;
        }
        if !take_delete(ctx) {
            return;
        }
        if self.portion.is_some() {
            self.delete_portion();
        } else {
            let clips = self.selected_clip_snapshots();
            if !clips.is_empty() {
                self.edit(Edit::Batch(
                    clips
                        .into_iter()
                        .map(|entry| Edit::DeleteClip(entry.clip.id))
                        .collect(),
                ));
            }
        }
    }
}
