/// How much the DJ's own history with this exact transition should boost a
/// candidate, as a 0..1 strength. Log-scaled so the first play counts for
/// something, and repeated plays count for more but flatten out.
///
/// This is deliberately a *boost* applied after the weighted average, not a
/// weighted component inside it: a low history value must never drag down a
/// good match, only lift one the DJ has actually used before.
pub fn strength(transition_count: u32) -> f32 {
    if transition_count == 0 {
        return 0.0;
    }
    ((1.0 + transition_count as f32).log2() / 3.0).min(1.0)
}
