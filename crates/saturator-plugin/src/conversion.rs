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
