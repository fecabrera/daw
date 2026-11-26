use super::*;

pub(super) fn check<T>(
    rate: u32,
    owner: &str,
    mut effect: EffectInstance,
    roundtrip: bool,
    configure: impl FnOnce(&mut EffectSettings, ModulationSourceId) -> T,
    verify: impl FnOnce(T, &[f32]),
) {
    let mut p = plan(Waveform::Saw, rate);
    let instrument = p.project.tracks[0].instrument.as_mut().unwrap();
    instrument.main.macro_module.add();
    instrument.main.macro_module.controls[0].settings.value = 1.0;
    let source = ModulationSourceId {
        layer: instrument.main.id,
        unit: instrument.main.macro_module.controls[0].id,
    };
    let target = daw_engine::ModulationTarget::Effect(effect.id);
    let expected_controls = configure(&mut effect.unit, source);
    match owner {
        "layer" => instrument.layers[0].effects.push(effect),
        "main" => instrument.main.effects.push(effect),
        _ => p.project.tracks[0].effects.push(effect),
    }
    p.project.validate().unwrap();
    let reopened = if roundtrip {
        let reopened: Project =
            serde_json::from_value(serde_json::to_value(&p.project).unwrap()).unwrap();
        if owner == "main" {
            let original = p.project.tracks[0].instrument.as_ref().unwrap();
            let track = &reopened.tracks[0];
            assert_eq!(track.effects, original.main.effects);
            assert!(track.instrument.as_ref().unwrap().main.effects.is_empty());
        } else {
            assert_eq!(reopened, p.project);
        }
        reopened.validate().unwrap();
        Some(reopened)
    } else {
        None
    };
    let mut expected = vec![[0.0; 2]; (rate / 10) as usize];
    Renderer::new(p.clone()).render_offline(&mut expected);
    if let Some(project) = reopened {
        p.project = project;
    }
    let mut live = Renderer::new(p);
    live.play();
    ALLOCATIONS.with(|v| v.set(0));
    TRACK_ALLOCATIONS.with(|v| v.set(true));
    for (frame, sample) in expected.iter().enumerate() {
        let actual = live.next_sample();
        if frame > rate as usize / 200 {
            assert!((0..2).all(|ch| (actual[ch] - sample[ch]).abs() < 2e-6));
        }
    }
    let values = live.modulation_snapshot(target).unwrap();
    TRACK_ALLOCATIONS.with(|v| v.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    verify(expected_controls, &values);
}
