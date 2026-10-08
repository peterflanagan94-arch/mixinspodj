-- MixInspo DJ — anonymous play log. One row per play, from any DJ who
-- hasn't turned play sharing off in Settings. No column identifies who
-- played it or when they played it relative to anything else.
CREATE TABLE plays (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    title        TEXT NOT NULL,
    artist       TEXT,
    bpm          REAL,
    camelot_key  TEXT,
    genre        TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX plays_by_title_artist ON plays (title, artist);
