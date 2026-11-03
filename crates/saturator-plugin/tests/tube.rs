use daw_saturator_plugin::{Parameter as P, PluginState};
use daw_unit_core::{SaturatorClipMode as Mode, SaturatorSettings};
#[test]
fn tube_state_text_and_host_mappings_preserve_legacy_choices_and_identities() {
    for (plain, mode, normalized) in [
        (0.0, Mode::NoClip, 0.0),
        (1.0, Mode::SoftClip, 0.5),
        (2.0, Mode::HardClip, 1.0),
    ] {
        let mut old = PluginState::default();
        old.set(P::ClipMode, plain).unwrap();
        let bytes = old.encode();
        assert_eq!(
            PluginState::decode(&bytes).unwrap().to_settings().clip_mode,
            mode
        );
        assert_eq!(P::ClipMode.denormalize(normalized), Ok(plain));
    }
    let settings = SaturatorSettings {
        clip_mode: Mode::TubeClip,
        ..Default::default()
    };
    let tube = PluginState::from_settings(&settings).unwrap();
    assert_eq!(tube.get(P::ClipMode), 3.0);
    assert_eq!(P::ClipMode as u32, 104);
    assert_eq!(P::ClipMode.normalize(3.0), Ok(2.0 / 3.0));
    assert_eq!(P::ClipMode.denormalize(2.0 / 3.0), Ok(3.0));
    assert_eq!(P::ClipMode.parse("Tube Clip"), Ok(3.0));
    assert_eq!(P::ClipMode.format(3.0).unwrap(), "Tube Clip");
    assert_eq!(
        PluginState::decode(&tube.encode()).unwrap().to_settings(),
        settings
    );
    assert_eq!(P::ClipMode.info().steps, 3);
    assert!(P::ClipMode.validate(4.0).is_err());
}
