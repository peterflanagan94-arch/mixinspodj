use crate::Mode;

/// Genres that are reliably not a continuous-set dancefloor genre, whichever
/// mode is active. This is a narrower, more defensible claim than a full
/// genre-adjacency map (e.g. "Disco is close to Funk" would need curated
/// data to be honest): it doesn't say which genres mix well, only that
/// these generally don't belong in a DJ set at all.
const OFF_DANCEFLOOR: &[&str] = &[
    "country",
    "classical",
    "folk",
    "gospel",
    "soundtrack",
    "comedy",
    "children's",
    "easy listening",
    "opera",
    "musical",
    "spoken word",
];

/// Genre compatibility. Exact-match comparison, deliberately — see
/// `OFF_DANCEFLOOR` above for why a full adjacency map isn't built.
///
/// SWITCH inverts the preference: it's the one mode that intentionally
/// wants a genre change, so a different genre scores *higher*. Even SWITCH
/// still avoids an off-dancefloor genre — it wants a different dance genre,
/// not a format change.
///
/// Checked one-sided first: a candidate's off-dancefloor genre is a reason
/// to avoid it even when the now-playing track has no genre tag of its own
/// (common for untagged loops and one-shots) — that case would otherwise
/// skip genre scoring entirely and let anything through.
pub fn score(now_genre: Option<&str>, candidate_genre: Option<&str>, mode: Mode) -> Option<(f32, &'static str)> {
    if let Some(candidate) = candidate_genre.and_then(normalize) {
        if OFF_DANCEFLOOR.contains(&candidate.as_str()) {
            return Some((0.1, "Not a dancefloor genre"));
        }
    }

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

    #[test]
    fn an_off_dancefloor_candidate_is_avoided_even_with_no_now_playing_genre() {
        let (value, label) = score(None, Some("Country"), Mode::Keep).unwrap();
        assert_eq!(value, 0.1);
        assert_eq!(label, "Not a dancefloor genre");
    }

    #[test]
    fn switch_still_avoids_an_off_dancefloor_genre() {
        let (value, _) = score(Some("Disco"), Some("Classical"), Mode::Switch).unwrap();
        assert_eq!(value, 0.1);
    }

    #[test]
    fn a_dance_genre_with_no_now_playing_genre_is_still_unscored() {
        assert!(score(None, Some("House"), Mode::Keep).is_none());
    }
}
