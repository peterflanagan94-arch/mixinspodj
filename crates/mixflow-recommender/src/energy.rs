use crate::Mode;

/// Energy compatibility — the component whose *target* genuinely changes
/// per mode, not just its weight. Energy is on a 1-10 scale.
pub fn score(now_energy: Option<f32>, candidate_energy: Option<f32>, mode: Mode) -> Option<(f32, &'static str)> {
    let candidate = candidate_energy?;

    // Rescue cares about the candidate's own energy, not the delta from
    // what's playing — "proven crowd favourite," not "a clever transition."
    if mode == Mode::Rescue {
        return Some(if candidate >= 7.0 {
            (1.0, "High-energy crowd favourite")
        } else if candidate >= 5.0 {
            (0.6, "Moderate energy")
        } else {
            (0.25, "Low energy — risky for a rescue")
        });
    }

    let now = now_energy?;
    let delta = candidate - now;

    // KEEP, BUILD and COOL's bands don't touch — there's a small gap between
    // each pair (0.75 to 1.0, and -1.0 to -0.75) where nothing scores full
    // marks. Without that gap a mild energy bump (delta +0.6, say) scored
    // 1.0 under both KEEP and BUILD, so picking a mode barely changed the
    // list. SWITCH stays wide — it only needs to rule out a jarring change,
    // not pick a direction.
    let (target_low, target_high, on_target_label): (f32, f32, &'static str) = match mode {
        Mode::Keep => (-0.75, 0.75, "Holds the energy"),
        Mode::Build => (1.0, 3.5, "Energy increase"),
        Mode::Cool => (-3.5, -1.0, "Energy decrease"),
        Mode::Switch => (-2.0, 2.0, "Energy stays workable"),
        Mode::Rescue => unreachable!("handled above"),
    };

    if delta >= target_low && delta <= target_high {
        return Some((1.0, on_target_label));
    }

    // Outside the target band: decay with distance from the nearer edge,
    // steeply enough that the wrong mode's best matches are visibly lower,
    // not just a rounding error away from 100%.
    let distance = if delta < target_low {
        target_low - delta
    } else {
        delta - target_high
    };
    let value = (1.0 - distance / 3.0).max(0.0);
    Some((value, "Energy direction doesn't fit the mode"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_prefers_similar_energy() {
        let (value, _) = score(Some(6.0), Some(6.3), Mode::Keep).unwrap();
        assert_eq!(value, 1.0);
    }

    #[test]
    fn keep_penalizes_a_big_jump() {
        let (value, _) = score(Some(4.0), Some(9.0), Mode::Keep).unwrap();
        assert!(value < 0.5, "value: {value}");
    }

    #[test]
    fn build_rewards_a_moderate_increase() {
        let (value, label) = score(Some(5.0), Some(6.5), Mode::Build).unwrap();
        assert_eq!(value, 1.0);
        assert_eq!(label, "Energy increase");
    }

    #[test]
    fn build_penalizes_a_decrease() {
        let (value, _) = score(Some(7.0), Some(4.0), Mode::Build).unwrap();
        assert!(value < 0.5, "value: {value}");
    }

    #[test]
    fn cool_rewards_a_decrease() {
        let (value, label) = score(Some(8.0), Some(6.5), Mode::Cool).unwrap();
        assert_eq!(value, 1.0);
        assert_eq!(label, "Energy decrease");
    }

    #[test]
    fn rescue_rewards_high_absolute_energy_regardless_of_current() {
        let (value, _) = score(Some(2.0), Some(8.5), Mode::Rescue).unwrap();
        assert_eq!(value, 1.0);
    }

    #[test]
    fn rescue_penalizes_low_energy_even_if_close_to_current() {
        let (value, _) = score(Some(3.0), Some(3.2), Mode::Rescue).unwrap();
        assert!(value < 0.5, "value: {value}");
    }

    #[test]
    fn keep_and_build_no_longer_both_give_full_marks_to_the_same_track() {
        // A delta that used to sit inside both bands (0.5 to 1.0) now falls
        // short of full marks in either — the deliberate gap between modes.
        let (keep, _) = score(Some(6.0), Some(6.9), Mode::Keep).unwrap();
        let (build, _) = score(Some(6.0), Some(6.9), Mode::Build).unwrap();
        assert!(keep < 1.0, "keep: {keep}");
        assert!(build < 1.0, "build: {build}");
    }

    #[test]
    fn missing_energy_is_unscored() {
        assert!(score(None, Some(5.0), Mode::Keep).is_none());
        assert!(score(Some(5.0), None, Mode::Keep).is_none());
    }
}
