impl Parameter {
    pub fn validate(self, value: f64) -> Result<f64, ParameterError> {
        let info = self.info();
        if !value.is_finite()
            || !(info.minimum..=info.maximum).contains(&value)
            || (info.steps > 0 && value.fract() != 0.0)
        {
            return Err(ParameterError);
        }
        Ok(value)
    }
    pub fn normalize(self, plain: f64) -> Result<f64, ParameterError> {
        self.validate(plain)?;
        // This converter is shared by effect products. Scope the legacy selector
        // mapping to Saturator rather than requiring its enum variant in other products.
        if MANIFEST_TYPE_ID == daw_unit_core::SATURATOR_TYPE_ID && self.info().key == "clip_mode" {
            // Keep legacy plain/state values 0/1/2. Put Hard Clip at the old
            // normalized endpoint, with Tube on the new four-choice grid.
            return Ok([0.0, 1.0 / 3.0, 1.0, 2.0 / 3.0][plain as usize]);
        }
        let info = self.info();
        Ok(match info.scale {
            Scale::Gain => gain::knob_position(plain),
            Scale::Logarithmic => (plain / info.minimum).ln() / (info.maximum / info.minimum).ln(),
            _ => (plain - info.minimum) / (info.maximum - info.minimum),
        })
    }
    pub fn denormalize(self, normalized: f64) -> Result<f64, ParameterError> {
        if !normalized.is_finite() || !(0.0..=1.0).contains(&normalized) {
            return Err(ParameterError);
        }
        // This converter is shared by effect products. Scope the legacy selector
        // mapping to Saturator rather than requiring its enum variant in other products.
        if MANIFEST_TYPE_ID == daw_unit_core::SATURATOR_TYPE_ID && self.info().key == "clip_mode" {
            // The old Soft Clip position (0.5) remains Soft Clip. Intermediate
            // automation above that boundary can now select Tube Clip.
            return Ok(if normalized < 1.0 / 6.0 { 0.0 }
                else if normalized <= 0.5 { 1.0 }
                else if normalized < 5.0 / 6.0 { 3.0 }
                else { 2.0 });
        }
        let info = self.info();
        if normalized == 0.0 {
            return Ok(info.minimum);
        }
        if normalized == 1.0 {
            return Ok(info.maximum);
        }
        let value = match info.scale {
            Scale::Gain => gain::knob_db(normalized),
            Scale::Logarithmic => info.minimum * (info.maximum / info.minimum).powf(normalized),
            Scale::Boolean | Scale::Choice => (info.minimum + normalized * (info.maximum - info.minimum)).round(),
            Scale::Linear => info.minimum + normalized * (info.maximum - info.minimum),
        };
        self.validate(value)
    }
}
