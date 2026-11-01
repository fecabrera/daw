use daw_core::*;
use daw_project::Session;
use std::fs;

#[test]
fn missing_beat_effects_report_load_warnings() {
    let root = std::env::temp_dir().join(format!("beat-effect-warnings-{}", Id::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let mut observations = Vec::new();
    for placement in [
        "master",
        "track",
        "midi-output",
        "midi-layer",
        "beat-layer",
        "beat-subtrack-output",
        "beat-final-output",
    ] {
        let mut session = Session::default();
        let effect = EffectInstance {
            id: Id::new_v4(),
            unit: EffectSettings::Missing {
                type_id: "test.unavailable".into(),
                state: serde_json::json!({"version": 99}),
            },
        };
        if matches!(placement, "master" | "track" | "midi-output" | "midi-layer") {
            session.project.add_midi_track().unwrap();
            match placement {
                "master" => session.project.master.effects.push(effect),
                "track" => session.project.tracks[0].effects.push(effect),
                "midi-output" => session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .main
                    .effects
                    .push(effect),
                _ => session.project.tracks[0]
                    .instrument
                    .as_mut()
                    .unwrap()
                    .layers[0]
                    .effects
                    .push(effect),
            }
        } else {
            let track = session.project.add_beat_track().unwrap();
            let mut row = BeatSubtrack::new("Kick".into());
            match placement {
                "beat-layer" => row.instrument.layers[0].effects.push(effect.clone()),
                "beat-subtrack-output" => row.instrument.main.effects.push(effect.clone()),
                _ => {}
            }
            session
                .project
                .edit(Edit::Beat {
                    track_id: track,
                    edit: BeatEdit::Add {
                        subtrack: Box::new(row),
                        index: 0,
                    },
                })
                .unwrap();
            if placement == "beat-final-output" {
                session.project.tracks[0]
                    .beat
                    .as_mut()
                    .unwrap()
                    .output
                    .effects
                    .push(effect);
            }
        }
        session.project.validate().unwrap();
        let folder = root.join(placement);
        session.save(&folder).unwrap();
        let opened = Session::open_document(&folder)
            .unwrap()
            .into_editor_session();
        assert_eq!(opened.project.all_effects().filter(|effect| matches!(
            &effect.unit, EffectSettings::Missing { type_id, .. } if type_id == "test.unavailable"
        )).count(), 1, "Missing effect payload must remain retained");
        for (owner, effects) in opened.project.effect_chains() {
            assert_eq!(opened.project.effect_chain(owner).unwrap().0, effects);
        }
        let retained = opened
            .project
            .all_effects()
            .find(|effect| matches!(&effect.unit, EffectSettings::Missing { .. }))
            .unwrap();
        if let EffectSettings::Missing { state, .. } = &retained.unit {
            assert_eq!(*state, serde_json::json!({"version": 99}));
        }
        let warned = opened
            .warnings
            .iter()
            .any(|w| w.contains("test.unavailable"));
        observations.push((placement, warned));
    }
    fs::remove_dir_all(&root).unwrap();
    eprintln!("load warning present: {:?}", observations);
    assert!(
        observations.iter().all(|(_, warned)| *warned),
        "Every unavailable effect must be reported"
    );
}
