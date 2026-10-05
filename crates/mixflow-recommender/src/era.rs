/// Era compatibility: simple year-distance decay. Tracks from the same
/// few years feel like they belong together; decades apart, less so —
/// though not a hard rule, since a good DJ bridges eras deliberately
/// sometimes, hence a gentle decay rather than a cliff.
pub fn score(now_year: Option<i32>, candidate_year: Option<i32>) -> Option<(f32, &'static str)> {
    let now = now_year?;
    let candidate = candidate_year?;
    let diff = (now - candidate).unsigned_abs();

    Some(match diff {
        0..=2 => (1.0, "Same era"),
        3..=7 => (0.8, "Similar era"),
        8..=15 => (0.55, "Neighbouring era"),
        16..=30 => (0.3, "Different era"),
        _ => (0.15, "Distant era"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_year_scores_highest() {
        let (value, label) = score(Some(1978), Some(1978)).unwrap();
        assert_eq!(value, 1.0);
        assert_eq!(label, "Same era");
    }

    #[test]
    fn decades_apart_scores_low() {
        let (value, _) = score(Some(1978), Some(2022)).unwrap();
        assert!(value < 0.2, "value: {value}");
    }

    #[test]
    fn missing_year_is_unscored() {
        assert!(score(None, Some(1978)).is_none());
    }
}
