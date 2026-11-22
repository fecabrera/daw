//! Categorized desktop adapter for the shared built-in effect catalog.
pub(super) use daw_core::BUILTIN_EFFECTS as ENTRIES;
use daw_core::EffectCategory as Category;

#[cfg(test)]
pub(super) const CATEGORY_COUNT: usize = Category::ALL.len();

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
