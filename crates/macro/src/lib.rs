//! Portable shared Macro control. Hosts own routing, identity and source publication.
use daw_unit_core::*;

#[derive(Clone)]
pub struct Macro {
    settings: MacroSettings,
    target: Option<ModulationTargetId>,
    base: ModulationRamp,
    offsets: ModulationSmoother<1>,
    fade_frames: u32,
    effective: f32,
    last_frame: Option<u64>,
}
impl Macro {
    pub fn new(settings: MacroSettings, context: ProcessingContext) -> Self {
        Self {
            settings,
            target: None,
            base: ModulationRamp::new(settings.value),
            offsets: ModulationSmoother::new(context),
            fade_frames: (context.sample_rate_hz / 200).max(1),
            effective: settings.value,
            last_frame: None,
        }
    }
    pub fn from_settings(control: &MacroInstance, context: ProcessingContext) -> Self {
        Self {
            target: Some(control.id),
            ..Self::new(control.settings, context)
        }
    }
    pub fn descriptor(&self) -> UnitDescriptor {
        UnitDescriptor {
            type_id: MACRO_TYPE_ID,
            name: "Macro",
            is_source: false,
            inputs: &[PortKind::Modulation],
            outputs: &[PortKind::Modulation],
        }
    }
    pub fn settings(&self) -> MacroSettings {
        self.settings
    }
    pub fn set_settings(&mut self, settings: MacroSettings) {
        self.base.set(settings.value, self.fade_frames);
        self.settings = settings;
    }
    pub fn reset(&mut self) {
        self.base.reset();
        self.offsets.reset();
        self.effective = self.settings.value;
        self.last_frame = None;
    }
    pub fn value(&self) -> f32 {
        self.effective
    }
}
impl ModulationSource for Macro {
    const MODULATION_OUTPUT: ModulationOutputDescriptor =
        <MacroInstance as ModulationSourceSettings>::MODULATION_OUTPUT;
    fn next_modulation(&mut self, _: ModulationSourceContext) -> f32 {
        self.effective
    }
}
impl ModulationDestination for Macro {
    type Audio = ();
    const MODULATION_PARAMETERS: &'static [ModulationParameter] =
        <MacroModuleSettings as ModulationDestinationSettings>::MODULATION_PARAMETERS;
    const MODULATION_SCOPE: ModulationScope = ModulationScope::Shared;
    fn modulation_target(&self) -> Option<ModulationTargetId> {
        self.target
    }
    fn process_with_modulation(
        &mut self,
        (): (),
        inputs: &ModulationInputs,
        context: ModulationContext,
    ) {
        if self
            .last_frame
            .is_some_and(|last| context.frame != last && context.frame != last.saturating_add(1))
        {
            self.offsets.reset();
        }
        self.last_frame = Some(context.frame);
        let [offset] = self
            .offsets
            .process(inputs.offsets([ModulationParameter::MacroValue]));
        self.effective = (f64::from(self.base.next_value()) + offset).clamp(0.0, 1.0) as f32;
    }
}

#[cfg(test)]
mod tests;
