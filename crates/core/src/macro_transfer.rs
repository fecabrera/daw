//! Transfer one Macro module, merging its strips without changing destination controls.
use crate::*;
fn module(i: &mut InstrumentSettings, owner: Id) -> &mut MacroModuleSettings {
    if owner == i.main.id {
        &mut i.main.macro_module
    } else {
        &mut i
            .layers
            .iter_mut()
            .find(|l| l.id == owner)
            .unwrap()
            .macro_module
    }
}
pub(super) fn transfer(
    i: &mut InstrumentSettings,
    from: Id,
    to: Id,
    copy: bool,
) -> Vec<(ModulationSourceId, ModulationSourceId)> {
    let mut incoming = if copy {
        module(i, from).clone()
    } else {
        std::mem::take(module(i, from))
    };
    let destination_ids: std::collections::HashSet<_> = super::providers(i, to).collect();
    let destination = module(i, to);
    if destination.controls.is_empty() {
        destination.enabled = incoming.enabled;
        destination.collapsed = incoming.collapsed;
        destination.modulation_open = incoming.modulation_open;
    }
    let mut remaps = Vec::new();
    for control in &mut incoming.controls {
        let old = control.id;
        if copy || destination_ids.contains(&old) {
            control.id = Id::new_v4();
        }
        for route in &mut incoming.modulations {
            if route.target == Some(old) {
                route.target = Some(control.id);
            }
        }
        remaps.push((
            ModulationSourceId {
                layer: from,
                unit: old,
            },
            ModulationSourceId {
                layer: to,
                unit: control.id,
            },
        ));
    }
    for route in &mut incoming.modulations {
        if copy || destination.modulations.iter().any(|r| r.id == route.id) {
            route.id = Id::new_v4();
        }
    }
    destination.present = true;
    destination.controls.extend(incoming.controls);
    destination.modulations.extend(incoming.modulations);
    remaps
}
