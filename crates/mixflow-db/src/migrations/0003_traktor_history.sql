-- Traktor's own history: one row per history file (one Traktor session), and
-- one row per track play in it, with the time it actually played.
CREATE TABLE traktor_history_sets (
    id          INTEGER PRIMARY KEY,
    file_name   TEXT NOT NULL UNIQUE,
    file_mtime  INTEGER NOT NULL
);

CREATE TABLE traktor_history_plays (
    id               INTEGER PRIMARY KEY,
    set_id           INTEGER NOT NULL REFERENCES traktor_history_sets(id) ON DELETE CASCADE,
    start_code       INTEGER NOT NULL,
    start_seconds    INTEGER NOT NULL,
    played_seconds   REAL NOT NULL,
    deck             INTEGER NOT NULL,
    location         TEXT NOT NULL,
    track_id         INTEGER REFERENCES tracks(id) ON DELETE SET NULL
);

CREATE INDEX traktor_history_plays_by_set ON traktor_history_plays (set_id, start_code, start_seconds);
