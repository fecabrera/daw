use super::*;
use daw_core::{MidiNote, MidiPattern};

fn press_split(app: &mut DawUi, ctx: &egui::Context) {
    let shortcut = ClipAction::Split.shortcut();
    frame(
        app,
        ctx,
        vec![egui::Event::Key {
            key: shortcut.logical_key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: shortcut.modifiers,
        }],
    );
}

#[test]
fn split_command_follows_the_playhead_selection_and_clip_views() {
    let (mut app, track) = fixture();
    let ctx = context();
    let clip = app.session.project.tracks[0].clips[0].id;
    app.session.project.tracks[0].clips[0].length_frames = frames(2.0);
    frame(&mut app, &ctx, vec![]);
    assert!(!app.clip_action_enabled(ClipAction::Split, &ctx));
    app.select_clip(clip, track, false);
    assert!(!app.clip_action_enabled(ClipAction::Split, &ctx));
    app.session.project.transport.playhead_frame = frames(1.0);
    assert!(app.clip_action_enabled(ClipAction::Split, &ctx));
    for mode in [ViewMode::Mixer, ViewMode::Instrument] {
        app.set_view_mode(mode, &ctx);
        assert!(!app.clip_action_enabled(ClipAction::Split, &ctx));
    }
    app.set_view_mode(ViewMode::Edit, &ctx);
    assert!(app.clip_action_enabled(ClipAction::Split, &ctx));
    app.set_view_mode(ViewMode::Timeline, &ctx);
    let outside = Id::new_v4();
    let template = app.session.project.tracks[0].clips[0].clone();
    app.session.project.tracks[0].clips.push(Clip {
        id: outside,
        name: "Later".into(),
        start_frame: frames(4.0),
        length_frames: frames(1.0),
        ..template
    });
    app.select_clip(outside, track, true);
    press_split(&mut app, &ctx);
    let clips = &app.session.project.tracks[0].clips;
    assert_eq!(clips.len(), 3);
    assert!(
        clips
            .iter()
            .any(|candidate| candidate.id == clip && candidate.length_frames == frames(1.0))
    );
    assert!(
        clips
            .iter()
            .any(|candidate| candidate.id == outside && candidate.length_frames == frames(1.0))
    );
    assert!(clips.iter().any(|candidate| {
        candidate.id != clip && candidate.id != outside && candidate.start_frame == frames(1.0)
    }));
    assert!(app.dirty);
    assert!(app.selected_clips.len() >= 2);
}

#[test]
fn split_uses_the_midi_note_setting_and_the_edit_menu() {
    let mut app = DawUi::default();
    let ctx = context();
    let track = app.session.project.add_midi_track().unwrap();
    let clip = app.session.project.add_midi_pattern(track, 0).unwrap();
    let note = Id::new_v4();
    app.session
        .project
        .edit(Edit::ReplaceMidiPattern {
            clip_id: clip,
            pattern: MidiPattern {
                length_ticks: 3840,
                notes: vec![MidiNote {
                    id: note,
                    start_tick: 960,
                    length_ticks: 960,
                    pitch: 60,
                    velocity: 100,
                }],
            },
        })
        .unwrap();
    app.session.project.tracks[0].clips[0].length_frames = app.session.project.midi_frame(3840);
    app.select_clip(clip, track, false);
    app.session.project.transport.playhead_frame = app.session.project.midi_frame(1440);
    app.settings.midi_note_split = daw_core::MidiNoteSplit::PreserveFirst;
    let menu = |app: &mut DawUi, events: Vec<egui::Event>| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0))),
                events,
                ..Default::default()
            },
            |ui| app.edit_menu(ui),
        );
        output.textures_delta.clear();
        output
    };
    let output = menu(&mut app, vec![]);
    assert!(app.clip_action_enabled(ClipAction::Split, &ctx));
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "Split at playhead" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("edit menu split command");
    menu(
        &mut app,
        vec![egui::Event::PointerMoved(point), button(point, true)],
    );
    menu(&mut app, vec![button(point, false)]);
    let (start, asset_id, right_id) = {
        let right = app.session.project.tracks[0]
            .clips
            .iter()
            .find(|candidate| candidate.id != clip)
            .unwrap();
        (right.start_frame, right.asset_id, right.id)
    };
    assert_eq!(start, app.session.project.midi_frame(1440));
    let kept = app
        .session
        .project
        .assets
        .iter()
        .find(|asset| asset.id == asset_id)
        .unwrap()
        .midi_pattern
        .as_ref()
        .unwrap()
        .notes
        .iter()
        .any(|candidate| candidate.id == note);
    assert!(!kept);
    assert!(app.clip_selected(clip) && app.clip_selected(right_id));
}
