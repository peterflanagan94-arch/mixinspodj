-- AI Suggestions results, kept so the same song is never asked about twice
-- on this machine. Keyed by the song itself (title + artist), not a library
-- row, so duplicate copies of the same track share one cached result.
CREATE TABLE ai_suggestion_cache (
    id            INTEGER PRIMARY KEY,
    title_key     TEXT NOT NULL,
    artist_key    TEXT NOT NULL,
    response_json TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(title_key, artist_key)
);
