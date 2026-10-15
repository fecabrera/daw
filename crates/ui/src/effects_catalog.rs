type Entry = (&'static str, &'static str, fn() -> daw_core::EffectInstance);
pub(super) const ENTRIES: [Entry; 6] = [
    (
        daw_core::EQ_TYPE_ID,
        "3-band EQ",
        daw_core::EffectInstance::equalizer,
    ),
    (
        daw_core::DRIVE_TYPE_ID,
        "Drive",
        daw_core::EffectInstance::drive,
    ),
    (
        daw_core::AUTO_FILTER_TYPE_ID,
        "Auto-filter",
        daw_core::EffectInstance::auto_filter,
    ),
    (
        daw_core::COMPRESSOR_TYPE_ID,
        "Compressor",
        daw_core::EffectInstance::compressor,
    ),
    (daw_core::OTT_TYPE_ID, "OTT", daw_core::EffectInstance::ott),
    (
        daw_core::LIMITER_TYPE_ID,
        "Limiter",
        daw_core::EffectInstance::limiter,
    ),
];
