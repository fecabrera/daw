//! Exclusive frame-range operations shared by clips and conductor blocks.
/// The same exclusive-range subtraction used by clip overwrite and resize.
pub(crate) fn uncovered_ranges(
    start: u64,
    end: u64,
    cover_start: u64,
    cover_end: u64,
) -> Vec<(u64, u64)> {
    if end <= cover_start || start >= cover_end {
        return vec![(start, end)];
    }
    let mut pieces = Vec::with_capacity(2);
    if start < cover_start {
        pieces.push((start, cover_start));
    }
    if end > cover_end {
        pieces.push((cover_end, end));
    }
    pieces
}
