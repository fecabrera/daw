//! Shared Clip menu layout, targets and guarded dispatch for native and egui menus.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipMenuAction {
    Import,
    AddMidi,
    Split,
    Separate,
    Edit,
    ToggleMuted,
    Delete,
}

impl ClipMenuAction {
    pub const ALL: [Self; 7] = [
        Self::Import,
        Self::AddMidi,
        Self::Split,
        Self::Separate,
        Self::Edit,
        Self::ToggleMuted,
        Self::Delete,
    ];
    pub const GROUPS: [&'static [Self]; 3] = [
        &[Self::Import, Self::AddMidi],
        &[Self::Split, Self::Separate],
        &[Self::Edit, Self::ToggleMuted, Self::Delete],
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Import => "Import clip…",
            Self::AddMidi => "Add MIDI pattern",
            Self::Split => "Split at playhead",
            Self::Separate => "Stem separation",
            Self::Edit => "Edit clip",
            Self::ToggleMuted => "Disable clip",
            Self::Delete => "Delete clip",
        }
    }

    pub fn shortcut(self) -> Option<egui::KeyboardShortcut> {
        match self {
            Self::Import => FileAction::Import.shortcut(),
            Self::Split => Some(ClipAction::Split.shortcut()),
            _ => None,
        }
    }
}

impl DawUi {
    fn menu_clip(&self, target: Option<Id>) -> Option<(Id, &Clip)> {
        let id = target.or(self.selected_clip)?;
        self.session.project.tracks.iter().find_map(|track| {
            track
                .clips
                .iter()
                .find(|clip| clip.id == id)
                .map(|clip| (track.id, clip))
        })
    }

    fn menu_clip_targets(&self, id: Id) -> Vec<Id> {
        if self.clip_selected(id) {
            self.selected_clip_ids()
        } else {
            vec![id]
        }
    }

    pub fn clip_menu_action_label(&self, action: ClipMenuAction) -> &'static str {
        self.clip_menu_label_for(action, None)
    }

    fn clip_menu_label_for(&self, action: ClipMenuAction, target: Option<Id>) -> &'static str {
        if let Some((_, clip)) = self.menu_clip(target) {
            if action == ClipMenuAction::ToggleMuted && clip.muted {
                return "Enable clip";
            }
            if action == ClipMenuAction::Delete && self.menu_clip_targets(clip.id).len() > 1 {
                return "Delete selected clips";
            }
        }
        action.label()
    }

    pub fn clip_menu_action_enabled(&self, action: ClipMenuAction, ctx: &egui::Context) -> bool {
        self.clip_menu_enabled_for(action, None, ctx)
    }

    fn clip_menu_enabled_for(
        &self,
        action: ClipMenuAction,
        target: Option<Id>,
        ctx: &egui::Context,
    ) -> bool {
        if !self.clip_actions_available(ctx) {
            return false;
        }
        if action == ClipMenuAction::Import {
            return self.file_action_enabled(FileAction::Import);
        }
        if !self.view_mode.has_clip_lanes() {
            return false;
        }
        if action == ClipMenuAction::AddMidi {
            return self.session.project.tracks.iter().any(|track| {
                Some(track.id) == self.selected_track && track.kind == daw_core::TrackKind::Midi
            });
        }
        let Some((_, clip)) = self.menu_clip(target) else {
            return false;
        };
        match action {
            ClipMenuAction::Split => !self
                .clips_crossed_by_playhead(&self.menu_clip_targets(clip.id))
                .is_empty(),
            ClipMenuAction::Separate => {
                self.view_mode == ViewMode::Timeline
                    && self.separation_enabled()
                    && self
                        .session
                        .project
                        .assets
                        .iter()
                        .any(|asset| asset.id == clip.asset_id && asset.midi_pattern.is_none())
            }
            ClipMenuAction::Edit | ClipMenuAction::ToggleMuted | ClipMenuAction::Delete => true,
            ClipMenuAction::Import | ClipMenuAction::AddMidi => false,
        }
    }

    pub fn perform_clip_menu_action(&mut self, action: ClipMenuAction, ctx: &egui::Context) {
        self.perform_clip_menu_action_for(action, None, ctx);
    }

    fn perform_clip_menu_action_for(
        &mut self,
        action: ClipMenuAction,
        target: Option<Id>,
        ctx: &egui::Context,
    ) {
        if !self.clip_menu_enabled_for(action, target, ctx) {
            return;
        }
        if action == ClipMenuAction::Import {
            self.perform_file_action(FileAction::Import);
            return;
        }
        if action == ClipMenuAction::AddMidi {
            if let Some(track) = self.selected_track {
                self.add_midi_pattern(track, self.session.project.transport.playhead_frame, ctx);
            }
            return;
        }
        let Some((track, clip)) = self.menu_clip(target) else {
            return;
        };
        let (id, muted) = (clip.id, clip.muted);
        match action {
            ClipMenuAction::Split => self.split_at(&self.menu_clip_targets(id)),
            ClipMenuAction::Separate => self.separate_clip(id),
            ClipMenuAction::Edit => self.open_clip_editor(id, track, ctx),
            ClipMenuAction::ToggleMuted => self.edit(Edit::SetClipMuted {
                clip_id: id,
                muted: !muted,
            }),
            ClipMenuAction::Delete => self.edit(Edit::Batch(
                self.menu_clip_targets(id)
                    .into_iter()
                    .map(Edit::DeleteClip)
                    .collect(),
            )),
            ClipMenuAction::Import | ClipMenuAction::AddMidi => {}
        }
    }

    pub(super) fn clip_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(220.0);
        self.clip_menu_items(ui, &ClipMenuAction::GROUPS, None);
    }

    pub(super) fn clip_context_menu(&mut self, ui: &mut egui::Ui, id: Id) {
        self.clip_menu_items(ui, &ClipMenuAction::GROUPS[1..], Some(id));
    }

    fn clip_menu_items(
        &mut self,
        ui: &mut egui::Ui,
        groups: &[&[ClipMenuAction]],
        target: Option<Id>,
    ) {
        for (index, group) in groups.iter().enumerate() {
            if index > 0 {
                menus::separator(ui);
            }
            for &action in *group {
                let mut button = menus::action_button(self.clip_menu_label_for(action, target));
                if let Some(shortcut) = action.shortcut() {
                    button =
                        button.shortcut_text(toolbars::menu_shortcut_text(ui.ctx(), &shortcut));
                }
                let response =
                    ui.add_enabled(self.clip_menu_enabled_for(action, target, ui.ctx()), button);
                let response = if action == ClipMenuAction::Separate {
                    let audio_in_timeline = self.view_mode == ViewMode::Timeline
                        && self.menu_clip(target).is_some_and(|(_, clip)| {
                            self.session.project.assets.iter().any(|asset| {
                                asset.id == clip.asset_id && asset.midi_pattern.is_none()
                            })
                        });
                    response.on_disabled_hover_text(if audio_in_timeline {
                        self.separation_disabled_hint()
                    } else {
                        "Select an audio clip in Timeline"
                    })
                } else {
                    response
                };
                if response.clicked() {
                    self.perform_clip_menu_action_for(action, target, ui.ctx());
                    ui.close();
                }
            }
        }
    }
}
