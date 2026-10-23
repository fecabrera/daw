//! Effect registration and the shared categorized catalog adapter.
#[derive(Clone, Copy, PartialEq)]
enum Category {
    EqAndFilters,
    Dynamics,
    Distortion,
    Imaging,
}
impl Category {
    const ALL: [Self; 4] = [
        Self::EqAndFilters,
        Self::Dynamics,
        Self::Distortion,
        Self::Imaging,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::EqAndFilters => "EQ & Filters",
            Self::Dynamics => "Dynamics",
            Self::Distortion => "Distortion",
            Self::Imaging => "Imaging",
        }
    }
}

#[cfg(test)]
pub(super) const CATEGORY_COUNT: usize = Category::ALL.len();

#[derive(Clone, Copy)]
pub(super) struct Entry {
    pub type_id: &'static str,
    pub name: &'static str,
    category: Category,
    pub make: fn() -> daw_core::EffectInstance,
}
impl Entry {
    const fn new(
        type_id: &'static str,
        name: &'static str,
        category: Category,
        make: fn() -> daw_core::EffectInstance,
    ) -> Self {
        Self {
            type_id,
            name,
            category,
            make,
        }
    }
}

pub(super) const ENTRIES: [Entry; 11] = [
    Entry::new(
        daw_core::EQ_TYPE_ID,
        "3-band EQ",
        Category::EqAndFilters,
        daw_core::EffectInstance::equalizer,
    ),
    Entry::new(
        daw_core::DRIVE_TYPE_ID,
        "Drive",
        Category::Distortion,
        daw_core::EffectInstance::drive,
    ),
    Entry::new(
        daw_core::FILTER_TYPE_ID,
        "Filter",
        Category::EqAndFilters,
        daw_core::EffectInstance::filter,
    ),
    Entry::new(
        daw_core::AUTO_FILTER_TYPE_ID,
        "Auto-filter",
        Category::EqAndFilters,
        daw_core::EffectInstance::auto_filter,
    ),
    Entry::new(
        daw_core::COMPRESSOR_TYPE_ID,
        "Compressor",
        Category::Dynamics,
        daw_core::EffectInstance::compressor,
    ),
    Entry::new(
        daw_core::OTT_TYPE_ID,
        "OTT",
        Category::Dynamics,
        daw_core::EffectInstance::ott,
    ),
    Entry::new(
        daw_core::LIMITER_TYPE_ID,
        "Limiter",
        Category::Dynamics,
        daw_core::EffectInstance::limiter,
    ),
    Entry::new(
        daw_core::DIRECTION_MIXER_TYPE_ID,
        "Direction Mixer",
        Category::Imaging,
        daw_core::EffectInstance::direction_mixer,
    ),
    Entry::new(
        daw_core::STEREO_SPREAD_TYPE_ID,
        "Stereo Spread",
        Category::Imaging,
        daw_core::EffectInstance::stereo_spread,
    ),
    Entry::new(
        daw_core::BITCRUSHER_TYPE_ID,
        "Bitcrusher",
        Category::Distortion,
        daw_core::EffectInstance::bitcrusher,
    ),
    Entry::new(
        daw_core::SATURATOR_TYPE_ID,
        "Saturator",
        Category::Distortion,
        daw_core::EffectInstance::saturator,
    ),
];

pub(super) fn catalog() -> crate::catalog::Catalog {
    crate::catalog::Catalog::new(ENTRIES.map(|entry| (entry.type_id, entry.name))).with_folders(
        Category::ALL.map(|category| {
            (
                category.name(),
                ENTRIES
                    .iter()
                    .filter(|entry| entry.category == category)
                    .map(|entry| entry.type_id)
                    .collect(),
            )
        }),
    )
}

/// Construct only on an explicit addition or valid drop, in captured display order.
pub(super) fn instantiate<'a>(
    types: &'a [&'a str],
) -> impl Iterator<Item = daw_core::EffectInstance> + 'a {
    types.iter().filter_map(|type_id| {
        ENTRIES
            .iter()
            .find(|entry| entry.type_id == *type_id)
            .map(|entry| (entry.make)())
    })
}
