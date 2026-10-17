//! Signed amount scales derived from the destination's existing knob definition.
use super::{KnobSpec, ParameterScale};

#[derive(Clone, Copy)]
pub(crate) enum AmountMapping {
    /// Signed offsets in destination units. Gain mirrors its attenuation curve.
    Offset { origin: f32, direction: f32 },
    /// Saved octaves correspond to ratios on the destination's frequency or time scale.
    Octaves,
    /// Saved envelope depth corresponds to attenuation on the destination's gain scale.
    Depth,
}
#[derive(Clone, Copy)]
pub(super) struct AmountScale {
    native_min: f32,
    native_max: f32,
    native_reset: f32,
    native_keyboard_step: f32,
    native_scale: Option<ParameterScale>,
    mapping: AmountMapping,
    limit: f32,
}
impl KnobSpec {
    pub(crate) fn modulation_amount(self, limit: f32, mapping: AmountMapping) -> Self {
        // Symmetric destinations already have the signed domain and quantization.
        if self.min == -limit && self.max == limit {
            return self.named("Modulation amount").centered(0.0).bipolar();
        }
        Self {
            name: "Modulation amount",
            min: -limit,
            max: limit,
            reset: 0.0,
            center: Some(0.0),
            bipolar: true,
            scale: None,
            amount_scale: Some(AmountScale {
                native_min: self.min,
                native_max: self.max,
                native_reset: self.reset,
                native_keyboard_step: self.keyboard_step,
                native_scale: self.scale,
                mapping,
                limit,
            }),
            ..self
        }
    }
}
impl AmountScale {
    fn native_position(self, value: f32) -> f32 {
        self.native_scale.map_or_else(
            || (value - self.native_min) / (self.native_max - self.native_min),
            |scale| (scale.position)(value),
        )
    }
    fn native_value(self, position: f32) -> f32 {
        self.native_scale.map_or_else(
            || self.native_min + position * (self.native_max - self.native_min),
            |scale| (scale.value)(position),
        )
    }
    fn to_native(self, magnitude: f32) -> f32 {
        match self.mapping {
            AmountMapping::Offset { origin, direction } => origin + direction * magnitude,
            AmountMapping::Octaves => self.native_reset * 2.0_f32.powf(magnitude),
            AmountMapping::Depth => {
                if magnitude >= 1.0 {
                    daw_core::SILENT_GAIN_DB
                } else {
                    20.0 * (1.0 - magnitude).log10()
                }
            }
        }
    }
    fn amount_from_native(self, value: f32) -> f32 {
        match self.mapping {
            AmountMapping::Offset { origin, direction } => (value - origin) / direction,
            AmountMapping::Octaves => (value / self.native_reset).log2(),
            AmountMapping::Depth => 1.0 - daw_core::linear_gain(value),
        }
    }
    pub(super) fn position(self, value: f32) -> f32 {
        if value == 0.0 {
            return 0.5;
        }
        if matches!(self.mapping, AmountMapping::Octaves) {
            let min = self.native_position(self.to_native(-self.limit));
            let max = self.native_position(self.to_native(self.limit));
            return (self.native_position(self.to_native(value)) - min) / (max - min);
        }
        let origin = self.native_position(self.to_native(0.0));
        let end = self.native_position(self.to_native(self.limit));
        0.5 + value.signum() * 0.5 * (self.native_position(self.to_native(value.abs())) - origin)
            / (end - origin)
    }
    pub(super) fn value(self, position: f32) -> f32 {
        if position == 0.5 {
            return 0.0;
        }
        if position <= 0.0 {
            return -self.limit;
        }
        if position >= 1.0 {
            return self.limit;
        }
        if matches!(self.mapping, AmountMapping::Octaves) {
            let min = self.native_position(self.to_native(-self.limit));
            let max = self.native_position(self.to_native(self.limit));
            return self.amount_from_native(self.native_value(min + position * (max - min)));
        }
        let signed = 2.0 * position - 1.0;
        let origin = self.native_position(self.to_native(0.0));
        let end = self.native_position(self.to_native(self.limit));
        signed.signum()
            * self.amount_from_native(self.native_value(origin + signed.abs() * (end - origin)))
    }
    pub(super) fn step(self, value: f32, delta: f32) -> f32 {
        let sign = if value < 0.0 { -1.0 } else { 1.0 };
        let direction = match self.mapping {
            AmountMapping::Offset { direction, .. } => sign * direction,
            AmountMapping::Octaves => 1.0,
            AmountMapping::Depth => -sign,
        };
        let native = self.to_native(if matches!(self.mapping, AmountMapping::Octaves) {
            value
        } else {
            value.abs()
        });
        let next = self.native_scale.map_or_else(
            || {
                (native / self.native_keyboard_step + direction * delta / self.native_keyboard_step)
                    .round()
                    * self.native_keyboard_step
            },
            |s| (s.step)(native, direction * delta),
        );
        let amount = self.amount_from_native(next);
        if matches!(self.mapping, AmountMapping::Octaves) {
            amount
        } else {
            sign * amount
        }
    }
    /// Preserve travel per destination-scale unit when adapting its domain to signed amounts.
    pub(super) fn drag_factor(self) -> f32 {
        let span = if matches!(self.mapping, AmountMapping::Octaves) {
            self.native_position(self.to_native(self.limit))
                - self.native_position(self.to_native(-self.limit))
        } else {
            2.0 * (self.native_position(self.to_native(self.limit))
                - self.native_position(self.to_native(0.0)))
            .abs()
        };
        1.0 / span
    }
    pub(super) fn reference_amount(self, native: f32) -> Option<f32> {
        if matches!(self.mapping, AmountMapping::Depth) && native > 0.0 {
            return None;
        }
        let amount = self.amount_from_native(native).abs();
        (amount.is_finite() && amount <= self.limit).then_some(amount)
    }
}
