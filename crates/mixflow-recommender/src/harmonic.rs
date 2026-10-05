/// Camelot wheel harmonic compatibility. Parses codes like "8B" / "11A"
/// and scores by wheel distance + relative major/minor, the same
/// compatibility rules DJs use manually.
pub fn score(now_key: Option<&str>, candidate_key: Option<&str>) -> Option<(f32, &'static str)> {
    let now = parse_camelot(now_key?)?;
    let candidate = parse_camelot(candidate_key?)?;

    if now == candidate {
        return Some((1.0, "Same key"));
    }
    if now.0 == candidate.0 && now.1 != candidate.1 {
        return Some((0.9, "Relative major/minor"));
    }

    let dist = circular_distance(now.0, candidate.0, 12);
    if dist == 1 && now.1 == candidate.1 {
        return Some((0.85, "Harmonically adjacent"));
    }
    if dist == 2 && now.1 == candidate.1 {
        return Some((0.55, "Energy-shift compatible"));
    }

    let decay = (1.0 - dist as f32 / 6.0).max(0.0);
    Some((decay * 0.35, "Distant key"))
}

/// `(wheel_number 1-12, is_major)`.
type Camelot = (u8, bool);

fn parse_camelot(code: &str) -> Option<Camelot> {
    let code = code.trim();
    let (number_part, letter) = code.split_at(code.len().checked_sub(1)?);
    let number: u8 = number_part.parse().ok()?;
    if !(1..=12).contains(&number) {
        return None;
    }
    match letter.to_uppercase().as_str() {
        "B" => Some((number, true)),
        "A" => Some((number, false)),
        _ => None,
    }
}

fn circular_distance(a: u8, b: u8, modulus: u8) -> u8 {
    let diff = (a as i16 - b as i16).unsigned_abs() as u8;
    diff.min(modulus - diff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_code_is_a_perfect_match() {
        let (value, label) = score(Some("8B"), Some("8B")).unwrap();
        assert_eq!(value, 1.0);
        assert_eq!(label, "Same key");
    }

    #[test]
    fn adjacent_wheel_position_scores_well() {
        let (value, label) = score(Some("8B"), Some("9B")).unwrap();
        assert_eq!(value, 0.85);
        assert_eq!(label, "Harmonically adjacent");
    }

    #[test]
    fn relative_major_minor_scores_well() {
        let (value, label) = score(Some("8B"), Some("8A")).unwrap();
        assert_eq!(value, 0.9);
        assert_eq!(label, "Relative major/minor");
    }

    #[test]
    fn opposite_side_of_the_wheel_scores_low() {
        let (value, _) = score(Some("8B"), Some("2B")).unwrap();
        assert!(value < 0.2, "value: {value}");
    }

    #[test]
    fn wheel_distance_wraps_around() {
        // 1 and 12 are adjacent on the wheel (it's circular).
        let (value, label) = score(Some("1B"), Some("12B")).unwrap();
        assert_eq!(value, 0.85);
        assert_eq!(label, "Harmonically adjacent");
    }

    #[test]
    fn missing_or_unparsable_key_is_unscored() {
        assert!(score(None, Some("8B")).is_none());
        assert!(score(Some("8B"), Some("not-a-key")).is_none());
    }
}
