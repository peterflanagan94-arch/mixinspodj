-- Core library

CREATE TABLE library_folders (
    id              INTEGER PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,
    last_scanned_at TEXT
);

CREATE TABLE tracks (
    id              INTEGER PRIMARY KEY,
    file_path       TEXT NOT NULL UNIQUE,
    file_hash       TEXT,
    title           TEXT,
    artist          TEXT,
    album           TEXT,
    genre           TEXT,
    year            INTEGER,
    duration_ms     INTEGER,
    bitrate         INTEGER,
    sample_rate     INTEGER,
    file_format     TEXT,
    artwork_path    TEXT,
    bpm             REAL,
    bpm_confidence  REAL,
    key             TEXT,
    camelot_key     TEXT,
    energy          REAL,
    danceability    REAL,
    analyzed_at     TEXT,
    added_at        TEXT NOT NULL DEFAULT (datetime('now')),
    missing_file    INTEGER NOT NULL DEFAULT 0,
    needs_review    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_tracks_bpm ON tracks (bpm);
CREATE INDEX idx_tracks_camelot_key ON tracks (camelot_key);
CREATE INDEX idx_tracks_artist_title ON tracks (artist, title);

CREATE TABLE audio_features (
    track_id            INTEGER PRIMARY KEY REFERENCES tracks (id) ON DELETE CASCADE,
    loudness_rms        REAL,
    spectral_centroid   REAL,
    spectral_flux       REAL,
    onset_density       REAL,
    raw_features_json   TEXT
);

CREATE TABLE duplicates (
    id               INTEGER PRIMARY KEY,
    track_id_a       INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    track_id_b       INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    similarity_score REAL NOT NULL,
    status           TEXT NOT NULL DEFAULT 'possible' -- possible | confirmed | dismissed
);

-- Gigs / prep

CREATE TABLE gigs (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    date       TEXT,
    venue      TEXT,
    client_name TEXT,
    event_type TEXT,
    notes      TEXT,
    status     TEXT NOT NULL DEFAULT 'upcoming' -- upcoming | in_progress | completed
);

CREATE TABLE gig_tracklist (
    id         INTEGER PRIMARY KEY,
    gig_id     INTEGER NOT NULL REFERENCES gigs (id) ON DELETE CASCADE,
    track_id   INTEGER REFERENCES tracks (id) ON DELETE SET NULL,
    raw_title  TEXT,
    raw_artist TEXT,
    category   TEXT NOT NULL, -- MUST_PLAY | PLAY_IF_POSSIBLE | GUEST_REQUEST | BRIDE_REQUEST
                               -- | GROOM_REQUEST | FIRST_DANCE | PARENT_DANCE | DO_NOT_PLAY
    position   INTEGER
);

-- Live assistant / session history

CREATE TABLE played_tracks (
    id               INTEGER PRIMARY KEY,
    gig_id           INTEGER REFERENCES gigs (id) ON DELETE SET NULL,
    track_id         INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    played_at        TEXT NOT NULL DEFAULT (datetime('now')),
    position_in_set  INTEGER,
    source           TEXT NOT NULL, -- manual | recommended | requested
    mode             TEXT,
    crowd_state      TEXT
);

CREATE TABLE transitions (
    id                       INTEGER PRIMARY KEY,
    from_track_id            INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    to_track_id              INTEGER NOT NULL REFERENCES tracks (id) ON DELETE CASCADE,
    gig_id                   INTEGER REFERENCES gigs (id) ON DELETE SET NULL,
    played_at                TEXT NOT NULL DEFAULT (datetime('now')),
    event_type               TEXT,
    was_recommended          INTEGER NOT NULL DEFAULT 0,
    recommendation_rank      INTEGER,
    was_selected_recommendation INTEGER NOT NULL DEFAULT 0,
    crowd_state              TEXT,
    mode                     TEXT
);

CREATE INDEX idx_transitions_from ON transitions (from_track_id);
CREATE INDEX idx_transitions_to ON transitions (to_track_id);

CREATE TABLE requests (
    id               INTEGER PRIMARY KEY,
    gig_id           INTEGER NOT NULL REFERENCES gigs (id) ON DELETE CASCADE,
    requested_text   TEXT NOT NULL,
    matched_track_id INTEGER REFERENCES tracks (id) ON DELETE SET NULL,
    status           TEXT NOT NULL DEFAULT 'later', -- play_soon | later | maybe | declined | dont_repeat
    requested_at     TEXT NOT NULL DEFAULT (datetime('now')),
    fulfilled_at     TEXT
);

-- Config

CREATE TABLE scoring_weights (
    profile_name TEXT PRIMARY KEY,
    weights_json TEXT NOT NULL
);

CREATE TABLE feature_flags (
    key           TEXT PRIMARY KEY,
    enabled       INTEGER NOT NULL DEFAULT 0,
    tier_required TEXT
);

CREATE TABLE acquisition_log (
    id          INTEGER PRIMARY KEY,
    track_id    INTEGER REFERENCES tracks (id) ON DELETE SET NULL,
    source      TEXT NOT NULL,
    method      TEXT NOT NULL,
    acquired_at TEXT NOT NULL DEFAULT (datetime('now'))
);
