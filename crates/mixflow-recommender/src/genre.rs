use crate::Mode;

/// Genre compatibility. A simple exact-match comparison, deliberately —
/// a genre-adjacency map (e.g. "Disco is close to Funk") would need real
/// curated data to be honest rather than arbitrary, so it's not built
/// yet; exact-match-or-not is a smaller, truthful claim.
///
/// SWITCH inverts the preference: it's the one mode that intentionally
/// wants a genre change, so a different genre scores *higher*.
pub fn score(now_genre: Option<&str>, candidate_genre: Option<&str>, mode: Mode) -> Option<(f32, &'static str)> {
    let now = normalize(now_genre?)?;
    let candidate = normalize(candidate_genre?)?;
    let same = now == candidate;

    if mode == Mode::Switch {
        return Some(if same {
            (0.3, "Same genre — Switch wants a change")
        } else {
            (0.9, "Genre change fits Switch")
        });
    }

    Some(if same {
        (1.0, "Genre match")
    } else {
        (0.4, "Different genre")
    })
}

fn normalize(genre: &str) -> Option<String> {
    let trimmed = genre.trim().to_lowercase();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_genre_scores_highest_outside_switch() {
        let (value, _) = score(Some("Disco"), Some("disco"), Mode::Keep).unwrap();
        assert_eq!(value, 1.0);
    }

    #[test]
    fn different_genre_scores_lower_outside_switch() {
        let (value, _) = score(Some("Disco"), Some("Rock"), Mode::Keep).unwrap();
        assert_eq!(value, 0.4);
    }

    #[test]
    fn switch_mode_rewards_a_different_genre() {
        let (value, label) = score(Some("Disco"), Some("Rock"), Mode::Switch).unwrap();
        assert_eq!(value, 0.9);
        assert_eq!(label, "Genre change fits Switch");
    }

    #[test]
    fn switch_mode_penalizes_the_same_genre() {
        let (value, _) = score(Some("Disco"), Some("Disco"), Mode::Switch).unwrap();
        assert_eq!(value, 0.3);
    }

    #[test]
    fn missing_genre_is_unscored() {
        assert!(score(None, Some("Disco"), Mode::Keep).is_none());
        assert!(score(Some(""), Some("Disco"), Mode::Keep).is_none());
    }
}
