//! Shared built-in effect descriptors, keys, and default constructors.
use crate::*;
#[derive(Clone, Copy, PartialEq)]
pub enum EffectCategory {
    Analysis,
    Eq,
    Filters,
    Dynamics,
    Distortion,
    Imaging,
    Reverb,
}
impl EffectCategory {
    pub const ALL: [Self; 7] = [
        Self::Eq,
        Self::Filters,
        Self::Dynamics,
        Self::Distortion,
        Self::Imaging,
        Self::Reverb,
        Self::Analysis,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Analysis => "Analysis",
            Self::Eq => "EQ",
            Self::Filters => "Filters",
            Self::Dynamics => "Dynamics",
            Self::Distortion => "Distortion",
            Self::Imaging => "Imaging",
            Self::Reverb => "Reverb",
        }
    }
}

#[derive(Clone, Copy)]
pub struct EffectDescriptor {
    pub key: &'static str,
    pub type_id: &'static str,
    pub name: &'static str,
    pub category: EffectCategory,
    pub make: fn() -> EffectInstance,
}
impl EffectDescriptor {
    const fn new(
        key: &'static str,
        type_id: &'static str,
        name: &'static str,
        category: EffectCategory,
        make: fn() -> EffectInstance,
    ) -> Self {
        Self {
            key,
            type_id,
            name,
            category,
            make,
        }
    }
}

pub const BUILTIN_EFFECTS: [EffectDescriptor; 23] = [
    EffectDescriptor::new(
        "equalizer",
        EQ_TYPE_ID,
        "3-band EQ",
        EffectCategory::Eq,
        EffectInstance::equalizer,
    ),
    EffectDescriptor::new(
        "drive",
        DRIVE_TYPE_ID,
        "Drive",
        EffectCategory::Distortion,
        EffectInstance::drive,
    ),
    EffectDescriptor::new(
        "filter",
        FILTER_TYPE_ID,
        "Filter",
        EffectCategory::Filters,
        EffectInstance::filter,
    ),
    EffectDescriptor::new(
        "auto-filter",
        AUTO_FILTER_TYPE_ID,
        "Auto-filter",
        EffectCategory::Filters,
        EffectInstance::auto_filter,
    ),
    EffectDescriptor::new(
        "compressor",
        COMPRESSOR_TYPE_ID,
        "Compressor",
        EffectCategory::Dynamics,
        EffectInstance::compressor,
    ),
    EffectDescriptor::new(
        "ott",
        OTT_TYPE_ID,
        "OTT",
        EffectCategory::Dynamics,
        EffectInstance::ott,
    ),
    EffectDescriptor::new(
        "limiter",
        LIMITER_TYPE_ID,
        "Limiter",
        EffectCategory::Dynamics,
        EffectInstance::limiter,
    ),
    EffectDescriptor::new(
        "direction-mixer",
        DIRECTION_MIXER_TYPE_ID,
        "Direction Mixer",
        EffectCategory::Imaging,
        EffectInstance::direction_mixer,
    ),
    EffectDescriptor::new(
        "stereo-spread",
        STEREO_SPREAD_TYPE_ID,
        "Stereo Spread",
        EffectCategory::Imaging,
        EffectInstance::stereo_spread,
    ),
    EffectDescriptor::new(
        "bitcrusher",
        BITCRUSHER_TYPE_ID,
        "Bitcrusher",
        EffectCategory::Distortion,
        EffectInstance::bitcrusher,
    ),
    EffectDescriptor::new(
        "saturator",
        SATURATOR_TYPE_ID,
        "Saturator",
        EffectCategory::Distortion,
        EffectInstance::saturator,
    ),
    EffectDescriptor::new(
        "drive-v2",
        DRIVE_V2_TYPE_ID,
        "Drive v2",
        EffectCategory::Distortion,
        EffectInstance::drive_v2,
    ),
    EffectDescriptor::new(
        "multiband-equalizer",
        MULTIBAND_EQ_TYPE_ID,
        "Multiband EQ",
        EffectCategory::Eq,
        EffectInstance::multiband_equalizer,
    ),
    EffectDescriptor::new(
        "chorus",
        CHORUS_TYPE_ID,
        "Chorus",
        EffectCategory::Filters,
        EffectInstance::chorus,
    ),
    EffectDescriptor::new(
        "exciter",
        EXCITER_TYPE_ID,
        "Multiband Exciter",
        EffectCategory::Distortion,
        EffectInstance::exciter,
    ),
    EffectDescriptor::new(
        "harmonic-lift",
        HARMONIC_LIFT_TYPE_ID,
        "Harmonic Lift",
        EffectCategory::Distortion,
        EffectInstance::harmonic_lift,
    ),
    EffectDescriptor::new(
        "plate-reverb",
        PLATE_REVERB_TYPE_ID,
        "Plate Reverb",
        EffectCategory::Reverb,
        EffectInstance::plate_reverb,
    ),
    EffectDescriptor::new(
        "ambient-reverb",
        AMBIENT_REVERB_TYPE_ID,
        "Ambient Reverb",
        EffectCategory::Reverb,
        EffectInstance::ambient_reverb,
    ),
    EffectDescriptor::new(
        "loudness",
        LOUDNESS_TYPE_ID,
        "Loudness Meter",
        EffectCategory::Analysis,
        EffectInstance::loudness,
    ),
    EffectDescriptor::new(
        "multiband-compressor",
        MULTIBAND_COMPRESSOR_TYPE_ID,
        "Multiband Compressor",
        EffectCategory::Dynamics,
        EffectInstance::multiband_compressor,
    ),
    EffectDescriptor::new(
        "shimmer-reverb",
        SHIMMER_REVERB_TYPE_ID,
        "Shimmer Reverb",
        EffectCategory::Reverb,
        EffectInstance::shimmer_reverb,
    ),
    EffectDescriptor::new(
        "transient-shaper",
        TRANSIENT_SHAPER_TYPE_ID,
        "Transient Shaper",
        EffectCategory::Dynamics,
        EffectInstance::transient_shaper,
    ),
    EffectDescriptor::new(
        "expander",
        EXPANDER_TYPE_ID,
        "Expander",
        EffectCategory::Dynamics,
        EffectInstance::expander,
    ),
];
