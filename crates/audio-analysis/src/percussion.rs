/// Admit repeated short attacks by checking the rise and decay around the
/// caller's physical onset landmarks. Each envelope bin is 10 ms.
pub(crate) fn has_short_attacks(
    bins: usize,
    level: impl Fn(usize) -> f64,
    positions: impl Iterator<Item = usize>,
) -> bool {
    let mut attacks = 0;
    let mut short = 0;
    for bin in positions.filter(|&bin| bin >= 7 && bin + 12 <= bins) {
        let before = (bin - 7..bin - 2).map(&level).sum::<f64>() / 5.0;
        let crest = (bin - 2..bin + 4).map(&level).fold(0.0, f64::max);
        let after = (bin + 7..bin + 12).map(&level).sum::<f64>() / 5.0;
        attacks += 1;
        short += usize::from(crest > before * 1.45 && crest > after * 1.25);
    }
    short >= 4 && short * 5 >= attacks * 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_repeated_short_attacks() {
        let positions = [10, 30, 50, 70, 90];
        let check =
            |signal: &[f64]| has_short_attacks(signal.len(), |i| signal[i], positions.into_iter());
        let mut signal = [0.1; 110];
        assert!(!check(&signal));
        for i in positions {
            signal[i] = 0.6;
        }
        assert!(check(&signal));
        signal[70] = 0.1;
        signal[90] = 0.1;
        assert!(!check(&signal));
    }
}
