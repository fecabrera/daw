//! Host construction and state retention for shared continuous modulation providers.
use super::modulation::Route;
use daw_unit_core::*;

enum Unit {
    Lfo(Box<daw_lfo::Lfo>),
    Macro(daw_macro::Macro),
}
pub(crate) struct ProviderControl {
    pub source: ModulationSourceId,
    unit: Unit,
    output: ModulationOutput,
    pub routes: Vec<Route>,
}
impl ProviderControl {
    pub fn lfo(layer: daw_core::Id, settings: &LfoInstance, context: ProcessingContext) -> Self {
        let source = ModulationSourceId {
            layer,
            unit: settings.id,
        };
        Self {
            source,
            unit: Unit::Lfo(Box::new(daw_lfo::Lfo::new(settings.settings, context))),
            output: ModulationOutput::new(source, settings.present && settings.enabled, context),
            routes: settings
                .modulations
                .iter()
                .map(|r| Route::new(*r, context.sample_rate_hz))
                .collect(),
        }
    }
    pub fn macro_control(
        layer: daw_core::Id,
        module: &MacroModuleSettings,
        control: &MacroInstance,
        context: ProcessingContext,
    ) -> Self {
        let source = ModulationSourceId {
            layer,
            unit: control.id,
        };
        Self {
            source,
            unit: Unit::Macro(daw_macro::Macro::from_settings(control, context)),
            output: ModulationOutput::new(
                source,
                module.present && module.enabled && control.present && control.enabled,
                context,
            ),
            routes: prepare_target_routes(&module.modulations, control.id, context.sample_rate_hz),
        }
    }
    pub fn reset(&mut self) {
        match &mut self.unit {
            Unit::Lfo(u) => u.reset(),
            Unit::Macro(u) => u.reset(),
        }
        self.output.reset();
        for route in &mut self.routes {
            route.reset();
        }
    }
    pub fn tick(
        &mut self,
        values: &[ModulationValue],
        index: &ModulationSourceIndex,
        context: ModulationSourceContext,
    ) -> ModulationValue {
        match &mut self.unit {
            Unit::Lfo(u) => sample_provider(
                u.as_mut(),
                &mut self.output,
                &mut self.routes,
                (&[], values),
                ModulationContext {
                    shared_sources: Some(index),
                    frame: context.frame,
                    tempo_bpm: context.tempo_bpm,
                    ..Default::default()
                },
                context,
            ),
            Unit::Macro(u) => sample_provider(
                u,
                &mut self.output,
                &mut self.routes,
                (&[], values),
                ModulationContext {
                    shared_sources: Some(index),
                    frame: context.frame,
                    tempo_bpm: context.tempo_bpm,
                    ..Default::default()
                },
                context,
            ),
        }
    }
    pub fn values(&self) -> [f32; 2] {
        match &self.unit {
            Unit::Lfo(u) => u.modulation_values(),
            Unit::Macro(u) => [u.value(), f32::NAN],
        }
    }
    pub fn retarget(&mut self, next: &mut Self) {
        match (&mut self.unit, &next.unit) {
            (Unit::Lfo(old), Unit::Lfo(new)) => old.set_settings(new.settings()),
            (Unit::Macro(old), Unit::Macro(new)) => old.set_settings(new.settings()),
            _ => return,
        }
        self.output.retarget(&next.output);
        retarget_modulation_routes(&mut self.routes, &mut next.routes);
        std::mem::swap(self, next);
    }
}

pub(crate) fn sample_provider<D: ModulationSource + ModulationDestination<Audio = ()>>(
    unit: &mut D,
    output: &mut ModulationOutput,
    routes: &mut [Route],
    values: (&[ModulationValue], &[ModulationValue]),
    inputs: ModulationContext,
    context: ModulationSourceContext,
) -> ModulationValue {
    process_modulation(unit, routes, (), values.0, values.1, inputs);
    output.sample(unit, context)
}
