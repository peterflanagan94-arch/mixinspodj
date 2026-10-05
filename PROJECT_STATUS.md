# MixFlow — Project Status

Working title; renaming later should only touch `tauri.conf.json`,
`package.json`, and the app identifier — nothing in the architecture
depends on the name.

## Where this came from

Foundation (stack, architecture, DB schema, audio analysis approach,
recommendation scoring model, risks, milestone plan) was agreed before any
code was written. See commit history / conversation for the full rationale.
Short version:

- **Shell:** Tauri v2 (Rust core + React/TS webview), not Electron — the
  analysis engine needs to be native/fast/background anyway.
- **DB:** SQLite, local-first, one file, no server.
- **Audio analysis:** custom Rust DSP pipeline (symphonia + rustfft +
  rubato), *not* Essentia — Essentia is AGPL and that's incompatible with
  closed-source commercial distribution without a paid license.
- **Recommendation engine:** deterministic, explainable, weighted scoring —
  no ML in v0.1. Lives in its own crate so it can be tested in isolation.

## Milestone status (MVP 0.1)

| # | Milestone | Status |
|---|---|---|
| M0 | Project scaffold, workspace, SQLite + migrations wired | **Done** |
| M1 | Folder picker, recursive scan, metadata extraction, dedup | Not started |
| M2 | Background BPM/key/energy analysis pipeline | Not started |
| M3 | Library browser UI | Not started |
| M4 | NOW PLAYING manual select / drag-in | Not started |
| M5 | Recommendation engine wired to UI | Not started |
| M6 | Mode buttons (Keep Dancing / Build Energy / etc.) | Not started |
| M7 | Play tracking, already-played detection | Not started |
| M8 | Local backups, crash recovery, QA pass, tag v0.1 | Not started |

Explicitly postponed past 0.1: GET MUSIC acquisition providers, PREP/event
module, Transition Finder, Crowd Mode, DJ-software integrations,
cloud/collective intelligence, payments. Stub crates exist for some of
these so the shape is right, but nothing in them is implemented.

## M0 — what's actually here

```
mixflow/
  Cargo.toml                    # workspace root
  src-tauri/                    # Tauri app crate — commands, app setup
  crates/
    mixflow-db/                 # REAL: schema, migrations, connect()
    mixflow-analysis/           # stub — target of M2
    mixflow-recommender/        # stub — target of M5
    mixflow-providers/          # stub — out of scope until post-MVP
    mixflow-session/            # stub — target of M4 / M7
  src/                          # React/TS frontend (default Tauri template)
```

`mixflow-db` has the full schema from the agreed design (tracks,
audio_features, library_folders, duplicates, gigs, gig_tracklist,
played_tracks, transitions, requests, scoring_weights, feature_flags,
acquisition_log) and 3 passing unit tests (migration validity, table
presence, insert/read round-trip).

The Tauri app wires this up for real: on startup it opens
`<app data dir>/mixflow.sqlite3`, runs migrations, and exposes a
`db_health_check` command the frontend can call to prove the connection is
live end-to-end. This is scaffolding, not a long-term API — it goes away
once M1 adds real library commands.

The frontend is still the default Tauri/React template (`npm create
tauri-app` output) — no MixFlow UI yet. That's M1+.

## How to run it

```bash
cd ~/mixflow
npm run tauri dev
```

Rust side verified independently of the GUI:

```bash
cargo build --workspace   # whole workspace compiles
cargo test --workspace    # mixflow-db: 3/3 passing
```

GUI smoke test: built a debug `.app` bundle (`npm run tauri build --
--debug`) and launched it with `open`, then screenshotted the live window —
it opens, renders the dark default template, and its controls are
interactive.

Note on `npm run tauri dev`: launching it from a detached/background shell
(no attached terminal) produced a window that Cocoa considered visible but
that never actually appeared on screen or in Mission Control — an artifact
of the detached launch, not an app bug. Running `npm run tauri dev`
normally from an interactive Terminal does not have this problem; it only
showed up because of how this session automated the check. Worth knowing
if a future automated check does the same.

## Known gaps / honesty notes

- No CI configured yet.
