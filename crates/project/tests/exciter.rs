use daw_core::*;
use daw_project::Session;
use std::{fs, path::PathBuf};
struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("daw-exciter-{}", Id::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn session() -> Session {
    let mut session = Session::default();
    let p = &mut session.project;
    let track = p.add_midi_track().unwrap();
    p.tracks[0].instrument = Some(OscillatorSettings::default().into());
    let clip = p.add_midi_pattern(track, 0).unwrap();
    p.edit(Edit::ReplaceMidiPattern {
        clip_id: clip,
        pattern: MidiPattern {
            length_ticks: 960,
            notes: vec![MidiNote {
                id: Id::new_v4(),
                start_tick: 0,
                length_ticks: 960,
                pitch: 69,
                velocity: 100,
            }],
        },
    })
    .unwrap();
    let mut e = EffectInstance::exciter();
    let EffectSettings::Exciter(s) = &mut e.unit else {
        unreachable!()
    };
    s.bands[1].mode = ExciterMode::Tube;
    s.bands[1].drive_db = 12.0;
    s.bands[1].mix = 0.3;
    p.edit(Edit::SetEffects {
        track_id: track,
        effects: vec![e],
        compact: false,
    })
    .unwrap();
    session
}
fn render(s: &Session) -> Vec<[f32; 2]> {
    let mut out = vec![[0.0; 2]; 8192];
    s.plan().render(0, &mut out);
    out
}
#[test]
fn exciter_save_reopen_save_as_and_float_export_preserve_settings_and_audio() {
    let folder = Folder::new();
    let mut original = session();
    let expected = render(&original);
    assert!(expected.iter().any(|v| v[0].abs() > 0.01));
    let saved = folder.0.join("original");
    original.save(&saved).unwrap();
    let opened = Session::open(&saved).unwrap();
    assert!(opened.warnings.is_empty());
    assert_eq!(render(&opened), expected);
    let copy = folder.0.join("copy");
    let mut copied = opened.clone();
    copied.save(&copy).unwrap();
    fs::remove_dir_all(&saved).unwrap();
    let copied = Session::open(&copy).unwrap();
    assert_eq!(render(&copied), expected);
    assert_eq!(
        copied.project.tracks[0].effects,
        original.project.tracks[0].effects
    );
    let a = folder.0.join("a.wav");
    let b = folder.0.join("b.wav");
    let settings = daw_media::ExportSettings {
        wav_codec: daw_media::WavCodec::Float32,
        ..Default::default()
    };
    original.export_with_settings(&a, false, settings).unwrap();
    copied.export_with_settings(&b, false, settings).unwrap();
    assert_eq!(fs::read(a).unwrap(), fs::read(b).unwrap());
    let manifest = fs::read_to_string(copy.join("project.json")).unwrap();
    let value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    let state = &value["tracks"][0]["effects"][0]["unit"]["state"];
    assert!(state.is_object());
    assert!(state.get("solo").is_none());
    assert!(
        state["bands"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b.get("solo").is_none())
    );
}
#[test]
fn invalid_band_edits_are_atomic_and_copied_bus_effects_remap_targets() {
    let mut session = session();
    let p = &mut session.project;
    let track = p.tracks[0].id;
    let effect = p.tracks[0].effects[0].clone();
    let before = p.clone();
    let mut bad = effect.clone();
    let EffectSettings::Exciter(s) = &mut bad.unit else {
        unreachable!()
    };
    s.bands.clear();
    assert!(
        p.edit(Edit::SetEffects {
            track_id: track,
            effects: vec![bad],
            compact: true
        })
        .is_err()
    );
    assert_eq!(*p, before);
    let instrument = p.tracks[0].instrument.as_mut().unwrap();
    instrument.main.macro_module.add();
    let source = ModulationSourceId {
        layer: instrument.main.id,
        unit: instrument.main.macro_module.controls[0].id,
    };
    let layer = instrument.layers[0].id;
    let mut routed = effect.clone();
    let EffectSettings::Exciter(s) = &mut routed.unit else {
        unreachable!()
    };
    let mut route = Modulation::new(ModulationParameter::SaturatorDrive);
    route.source = Some(source);
    route.target = Some(s.bands[1].id);
    route.amount = 12.0;
    s.modulations.push(route);
    p.edit(Edit::SetEffects {
        track_id: track,
        effects: vec![routed.clone()],
        compact: false,
    })
    .unwrap();
    let transfer = p
        .transfer_effects(
            EffectChainTarget::Track(track),
            EffectChainTarget::Layer { track, layer },
            &[routed.id],
            0,
            true,
        )
        .unwrap();
    p.edit(transfer.edit).unwrap();
    p.validate().unwrap();
    let copy = &p.tracks[0].instrument.as_ref().unwrap().layers[0].effects[0];
    assert_ne!(copy.id, routed.id);
    assert!(
        copy.unit
            .band_ids()
            .all(|id| !routed.unit.band_ids().any(|b| b == id))
    );
    let target = copy.unit.modulations()[0].target.unwrap();
    assert!(copy.unit.band_ids().any(|id| id == target));
    assert_eq!(copy.unit.modulations()[0].source, Some(source));
    let captured = p.clone();
    assert!(
        p.transfer_effects(
            EffectChainTarget::Track(track),
            EffectChainTarget::Master,
            &[routed.id],
            0,
            true
        )
        .is_err()
    );
    assert_eq!(*p, captured);
}
