use daw_compressor::Compressor;
use daw_unit_core::{CompressorSettings, DynamicsDirection, ProcessingContext};

/// Direct reference for the shared host and plugin contract tests.
pub struct Direct(Compressor);

impl Direct {
    pub fn new(context: ProcessingContext, settings: &CompressorSettings) -> Self {
        Self(Compressor::with_direction(
            context,
            settings,
            DynamicsDirection::Expander,
        ))
    }
}

impl std::ops::Deref for Direct {
    type Target = Compressor;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Direct {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
