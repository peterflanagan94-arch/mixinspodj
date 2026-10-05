/// BPM compatibility: tiered by percentage difference, with half/double
/// tempo folding (a 180 BPM track mixes fine under a 90 BPM one at half
/// tempo). Returns `None` when either BPM is unknown — an unscored
/// component is renormalized away, not guessed at.
pub fn score(now_bpm: Option<f32>, candidate_bpm: Option<f32>) -> Option<(f32, &'static str)> {
    let (now, candidate) = (now_bpm?, candidate_bpm?);
    if now <= 0.0 || candidate <= 0.0 {
        return None;
    }

    let direct_diff = (now - candidate).abs() / now;
    let folded_diff = [candidate * 2.0, candidate / 2.0]
        .iter()
        .map(|folded| (now - folded).abs() / now)
        .fold(f32::MAX, f32::min);

    let (diff, folded) = if folded_diff < direct_diff {
        (folded_diff, true)
    } else {
        (direct_diff, false)
    };

    let tier = if diff <= 0.02 {
        (1.0, "BPM near-identical")
    } else if diff <= 0.06 {
        (0.85, "BPM compatible")
    } else if diff <= 0.10 {
        (0.6, "BPM workable with pitch adjust")
    } else {
        (((1.0 - diff / 0.35).max(0.0)) * 0.5, "BPM far apart")
    };

    if folded && tier.0 > 0.0 {
        Some((tier.0 * 0.85, "Half/double-time compatible"))
    } else {
        Some(tier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_bpm_scores_highest() {
        let (value, _) = score(Some(126.0), Some(126.0)).unwrap();
        assert_eq!(value, 1.0);
    }

    #[test]
    fn small_difference_still_scores_well() {
        let (value, label) = score(Some(126.0), Some(129.0)).unwrap();
        assert!(value >= 0.8, "value: {value}");
        assert_eq!(label, "BPM compatible");
    }

    #[test]
    fn far_apart_bpm_scores_low() {
        let (value, _) = score(Some(90.0), Some(140.0)).unwrap();
        assert!(value < 0.3, "value: {value}");
    }

    #[test]
    fn double_tempo_is_recognized_as_compatible() {
        let (value, label) = score(Some(90.0), Some(179.0)).unwrap();
        assert!(value > 0.6, "value: {value}");
        assert_eq!(label, "Half/double-time compatible");
    }

    #[test]
    fn missing_bpm_is_unscored() {
        assert!(score(None, Some(120.0)).is_none());
        assert!(score(Some(120.0), None).is_none());
    }
}
