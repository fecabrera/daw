#![allow(dead_code)]
use daw_unit_core::{
    Modulation, ModulationBehavior, ModulationContext, ModulationParameter as P, ModulationRange,
    ModulationRoute, ModulationSourceId, ModulationValue, process_modulation,
};

pub fn contour() -> ModulationValue {
    ModulationValue {
        source: ModulationSourceId {
            layer: "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            unit: "00000000-0000-0000-0000-000000000002".parse().unwrap(),
        },
        value: 1.0,
        weight: 1.0,
        release_age: None,
        range: ModulationRange::Unipolar,
        behavior: ModulationBehavior::Contour,
    }
}
pub fn route(source: ModulationSourceId, parameter: P, amount: f32, rate: u32) -> ModulationRoute {
    ModulationRoute::new(
        Modulation {
            source: Some(source),
            amount,
            ..Modulation::new(parameter)
        },
        rate,
    )
}
pub fn process_contour(
    unit: &mut impl daw_unit_core::ModulationDestination<Audio = [f32; 2]>,
    routes: &mut [ModulationRoute],
    input: [f32; 2],
    local: bool,
) -> [f32; 2] {
    let values = [contour()];
    let (local, shared) = if local {
        (&values[..], &[][..])
    } else {
        (&[][..], &values[..])
    };
    process_modulation(
        unit,
        routes,
        input,
        local,
        shared,
        ModulationContext::default(),
    )
}
