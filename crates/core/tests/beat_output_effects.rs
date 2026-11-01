use InstrumentComponent::{Effect, Macros};
use daw_core::*;

#[test]
fn final_output_uses_shared_order_for_moves_copies_and_hidden_controls() {
    for explicit in [false, true] {
        for hidden in [false, true] {
            for parent_target in [false, true] {
                let mut project = Project::default();
                let parent = project.add_beat_track().unwrap();
                let output = project.beat(parent).unwrap().output.id;
                let target = EffectChainTarget::Layer {
                    track: if parent_target { parent } else { output },
                    layer: output,
                };
                // Catalog additions intentionally leave the optional layout empty.
                for effect in [
                    EffectInstance::drive(),
                    EffectInstance::chorus(),
                    EffectInstance::filter(),
                ] {
                    let mut effects = project.effect_chain(target).unwrap().0.to_vec();
                    effects.push(effect);
                    project.edit(target.edit(effects, false)).unwrap();
                }
                let ids: Vec<_> = project
                    .effect_chain(target)
                    .unwrap()
                    .0
                    .iter()
                    .map(|e| e.id)
                    .collect();
                let [a, b, c] = [ids[0], ids[1], ids[2]];
                let bus = &mut project.tracks[0].beat.as_mut().unwrap().output;
                bus.macro_module.add();
                bus.macro_module.present = !hidden;
                if explicit {
                    bus.component_order = vec![Effect(a), Macros, Effect(b), Effect(c)];
                }
                project.validate().unwrap();
                let transfer = project
                    .transfer_effects(target, target, &[c], 1, false)
                    .unwrap();
                let transfer = project
                    .position_inline_effects(transfer, target, Some(Effect(b)))
                    .unwrap();
                project.edit(transfer.edit).unwrap();
                assert_eq!(
                    project
                        .effect_chain(target)
                        .unwrap()
                        .0
                        .iter()
                        .map(|e| e.id)
                        .collect::<Vec<_>>(),
                    vec![a, c, b]
                );
                assert!(
                    project
                        .beat(parent)
                        .unwrap()
                        .output
                        .component_order
                        .contains(&Macros)
                );
                let transfer = project
                    .transfer_effects(target, target, &[a], 3, false)
                    .unwrap();
                let transfer = project
                    .position_inline_effects(transfer, target, None)
                    .unwrap();
                project.edit(transfer.edit).unwrap();
                assert_eq!(
                    project
                        .effect_chain(target)
                        .unwrap()
                        .0
                        .iter()
                        .map(|e| e.id)
                        .collect::<Vec<_>>(),
                    vec![c, b, a]
                );
                // A library copy gets a fresh identity and uses the same inline path.
                let library = EffectInstance::drive();
                let library_id = library.id;
                project.master.effects.push(library.clone());
                let transfer = project
                    .transfer_effects(EffectChainTarget::Master, target, &[library_id], 0, true)
                    .unwrap();
                let copied = transfer.selected[0];
                let transfer = project
                    .position_inline_effects(transfer, target, Some(Effect(c)))
                    .unwrap();
                project.edit(transfer.edit).unwrap();
                let effects = project.effect_chain(target).unwrap().0;
                assert_eq!(
                    effects.iter().map(|e| e.id).collect::<Vec<_>>(),
                    vec![copied, c, b, a]
                );
                assert_ne!(copied, library_id);
                assert_eq!(effects[0].unit, library.unit);
                let saved = serde_json::to_value(&project).unwrap();
                let reopened: Project = serde_json::from_value(saved).unwrap();
                reopened.validate().unwrap();
                assert_eq!(reopened, project);
            }
        }
    }
}
