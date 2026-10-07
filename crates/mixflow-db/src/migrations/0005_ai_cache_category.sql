-- AI Suggestions now has categories (Similar BPM, Similar Genre, Popular
-- Mixes), each a different question to the AI, so each needs its own cached
-- answer per song. The cache is disposable — nothing is lost by starting it
-- fresh rather than migrating old rows in place.
DROP TABLE ai_suggestion_cache;

CREATE TABLE ai_suggestion_cache (
    id            INTEGER PRIMARY KEY,
    title_key     TEXT NOT NULL,
    artist_key    TEXT NOT NULL,
    category_key  TEXT NOT NULL,
    response_json TEXT NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(title_key, artist_key, category_key)
);
