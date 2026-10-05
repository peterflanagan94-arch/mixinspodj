# MixInspo DJ — Project Status

Product name (on screen: "MixInspo DJ"); built so a future rename only
touches `tauri.conf.json`, `package.json`, and the app identifier — nothing
in the architecture depends on the name. Renamed twice so far: "MixFlow"
(working title) → "MixMe DJ" → "MixInspo DJ", both 2026-10-05. Internal
crate names under `crates/` still say `mixflow-*`, since those are
implementation-internal, not the product's identity.

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
| M1 | Folder picker, recursive scan, metadata extraction, dedup | **Done** |
| — | Compact Live shell (design direction, cross-cutting) | **Done** |
| M2 | Background BPM/key/energy analysis pipeline | **Done** |
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
mixinspodj/
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
`<app data dir>/mixinspodj.sqlite3`, runs migrations, and exposes a
`db_health_check` command the frontend can call to prove the connection is
live end-to-end. This is scaffolding, not a long-term API — it goes away
once M1 adds real library commands.

The frontend is still the default Tauri/React template (`npm create
tauri-app` output) — no MixInspo DJ UI yet. That's M1+.

## M1 — what's actually here

Real folder scanning, end to end:

- `pick_music_folder` command opens a native macOS folder picker (`rfd`).
- `scan_library_folder(folder)` walks it recursively (`walkdir`), filters to
  known audio extensions (mp3/flac/wav/m4a/mp4/aac/ogg/aif/aiff), and for
  each file: hashes the content (`blake3`, streamed — not loaded whole into
  memory), reads tags + audio properties (`lofty`: title, artist, album,
  genre, year, duration, bitrate, sample rate, format), and upserts into
  `tracks` via `mixflow-db::library`.
- Dedup: re-scanning a path updates the existing row rather than
  duplicating it; a new path whose content hash matches an existing track
  is still inserted (it's a real second file on disk) but a row is written
  to `duplicates` linking the two, and the scan summary reports a
  `duplicates_found` count — this is the OWNED/MISSING/POSSIBLE DUPLICATES
  distinction from the spec, implemented at the hash level (exact-content
  dedup). Fuzzy dedup (same song, different file/rip) is not implemented —
  postponed, not silently approximated.
- Progress is streamed to the frontend via a `scan-progress` Tauri event
  (current/total/filename) as the scan runs; the scan itself runs as a
  synchronous command, which Tauri schedules off the main thread by
  default, so the UI stays responsive.
- `mixflow-db::library` has 3 new unit tests (fresh insert, re-scan
  updates in place, same-content-new-path is flagged as a duplicate) — 6/6
  passing in `mixflow-db` total.
- Frontend: replaced the default template with a real (if minimal) Library
  screen — folder picker button, live progress bar, and a results summary
  (files found / added / updated / duplicates / errors). Dark theme now
  applies unconditionally (DJ booth requirement — not just `prefers-color-
  scheme`), flat colors, no gradients.

**Verified against a real scan**, not just unit tests: built the debug
`.app`, drove the actual native folder picker, and pointed it at a
synthetic test library (2 distinct WAV files + 1 exact duplicate + 1
non-audio file, one of them in a subfolder to test recursion). Result:
3 files found (notes.txt correctly excluded), 3 added, 1 duplicate
correctly flagged, 0 errors — confirmed both in the UI and by querying
the actual SQLite file directly.

## Compact Live shell — what's actually here

A full redesign of the window around a narrow, always-on-top-capable
utility-panel form factor (~410px wide), inspired by (not copied from —
original terms, original layout decisions, MixInspo brand colors) a
reference product the user walked through: PulseDJ's compact DJ-companion
window, plus its public FAQ/quick-guide content for mechanics.

**Design system:** CSS custom properties for MixInspo cyan (`#27c1d3`,
active/live/selected) and MixInspo yellow (`#ffc13b`, attention/discovery),
near-black background, flat colors, no gradients — `src/App.css`.

**Real and wired:**
- Window resized to the narrow panel form factor (410×760, min 360×480 —
  `tauri.conf.json`).
- Top bar: brand mark, LIVE badge, event-context selector (Wedding/18th
  /.../Club/Bar/Corporate/Other — local state only, not yet used by any
  scoring), search icon, **real** Always-On-Top toggle (Tauri
  `setAlwaysOnTop`, new `core:window:allow-set-always-on-top` capability),
  **real** minimize.
- NOW PLAYING: compact single-row card, thin cyan left border, manually
  selectable (via the search panel) from the real scanned library — BPM/
  key/energy shown as real values once present, `—` when not (no fake
  numbers; M2 hasn't run yet so these are always `—` today).
- NEXT list: compact two-line rows (title / artist · bpm · key) for every
  other track in the library, dominant vertical space, scrollable. Drag
  handle (⋮⋮) shown static cyan — "local file available" is actually true
  for every scanned track today; it is **not** wired to real OS drag-out
  to another app yet (see Deferred below).
- FIND A TRACK (search icon): real local search (new `search_tracks`
  backend command, SQL LIKE over title/artist) against the actual scanned
  library. A match shows "✓ IN LIBRARY" and selecting it sets NOW PLAYING.
  No match shows "Not in your library" with a **working** COPY action
  (writes the typed name to the system clipboard) — this mirrors a pattern
  from the PulseDJ FAQ (click-to-copy instead of a fake action) and is
  deliberately not a disabled "GET TRACK" button, since GET MUSIC
  acquisition isn't built.
- SETTINGS (gear icon): houses the M1 library-scan flow (unchanged
  behavior, moved into a panel) plus an explicit written note that mini
  mode, menu-bar show/hide, launch-at-login, and remembering window
  position/size are not built.
- New backend commands: `list_tracks`, `search_tracks` in
  `src-tauri/src/tracks.rs`.

**Visual-only / local React state, not wired to anything real yet** (no
recommendation engine exists for them to affect — documented in-code, not
silently pretended to work):
- Quick direction controls (KEEP / BUILD↑ / COOL↓ / SWITCH↪ / RESCUE) —
  `ModeRow.tsx`.
- Bottom filter chips (LOCAL / SIMILAR / UNPLAYED / BPM ±5 / SWITCH,
  translated from PulseDJ's Draggable/Similar/Fresh/BPM/Curveball chip
  set into original terms) — `FilterRow.tsx`.

**Deliberately not attempted this pass** (each is real, nontrivial work
that deserves its own milestone, not a rushed bolt-on):
- Native drag-out of a local file from the app window to another
  application (Rekordbox/Serato/VirtualDJ) — the brief explicitly asked to
  investigate this and not fake it. Not investigated yet.
- Mini Mode, menu-bar tray icon + global show/hide, launch at login,
  remembering window position/size/current gig across restarts.
- Match %, energy bar, "already played" dimming on NEXT rows — all need
  M5 (scoring) or M7 (play tracking), which don't exist yet. Honestly
  blank today, not faked.
- OWNED ONLY toggle and genre/era/key filter wiring — not meaningful until
  a Discover (non-owned) mode exists.
- Track history back-control.

**Verified live, not just compiled:** built the debug `.app`, scanned a
6-track synthetic library (real ID3-tagged WAVs via `mutagen`) through the
real Settings flow, confirmed the NEXT list renders real titles/artists
with honest `—` BPM/key, searched and selected a track into NOW PLAYING
(correctly removed from NEXT), searched a track that doesn't exist and
confirmed the COPY button actually writes to the system clipboard (read
back via the OS clipboard, not assumed), and toggled Always-On-Top and
confirmed the icon's color state on the real rendered pixels.

## M2 — what's actually here

Real BPM/key/energy/danceability analysis, no external model, no Essentia
(AGPL — incompatible with closed-source distribution), all classic MIR
DSP implemented directly in `mixflow-analysis`:

- **Decode** (`decode.rs`): `symphonia` (mp3/flac/wav/m4a/aac/ogg/aiff via
  the `all` feature), downmixed to mono f32 PCM.
- **BPM** (`onset.rs` + `tempo.rs`): spectral-flux onset detection
  function (windowed FFT, half-wave-rectified frame-to-frame magnitude
  increase), then autocorrelation of that function across the lag range
  for 60-200 BPM to find the dominant beat period. Octave-error
  correction picks whichever of {bpm, bpm×2, bpm÷2} lands in 80-160 BPM
  with the strongest autocorrelation there, rather than trusting the raw
  peak blindly. Confidence = normalized autocorrelation strength.
- **Key** (`key.rs`): whole-track 12-bin chroma (FFT bins mapped to pitch
  class via the MIDI/frequency relationship, 80-5000Hz), correlated
  against all 24 rotated Krumhansl-Kessler major/minor profiles (standard
  published MIR research data, not copyrighted code — just numeric
  constants). Winner mapped to the standard Camelot wheel via a verified
  lookup table (relative major/minor share a number, e.g. C=8B/Am=8A).
- **Energy / danceability** (`energy.rs`): explainable, tunable composite
  — loudness (RMS) + spectral centroid (brightness) + onset density +
  tempo, documented weights, explicitly labeled as a starting heuristic
  to retune later, not a calibrated model. Danceability: BPM-in-range +
  BPM confidence.
- New `mixflow-db::library` functions: `tracks_needing_analysis` (anything
  with `analyzed_at IS NULL`) and `save_analysis` (writes `tracks` headline
  fields + `audio_features` raw components).
- New Tauri command `analyze_library`: processes pending tracks
  sequentially (not parallel — bounded CPU use over raw speed, per the
  "never peg the CPU during a live gig" principle), streaming
  `analysis-progress` events; wired into Settings next to the scan flow.

**Tested two ways**, not just one:
1. Unit tests (8, all passing) against synthetic signals with known
   ground truth: click tracks at 128/90 BPM, a C-major and an A-minor
   triad, and an energy ordering check — these pin the *algorithms*
   correct in isolation.
2. Live end-to-end verification against real files on disk: generated
   three WAV files (kick pulses at a known BPM + a sustained chord at a
   known key, real ID3 tags), scanned and analyzed them through the
   actual Settings UI in the built `.app`, and checked the result both in
   the UI and directly in SQLite. Detected **120 BPM/8B** (target 120/C
   major, exact), **128 BPM/8A** (target 128/A minor, exact), and **99
   BPM/9B** (target 100/G major — within 1 BPM, correct key) — this is
   what actually proves the full decode→DSP→persist pipeline works, since
   the unit tests never touch `symphonia` decoding or the Tauri command
   layer.

`mixflow-analysis`: 8/8 tests passing. `mixflow-db`: 8/8 (2 new, covering
the analysis repository functions).

## How to run it

```bash
cd ~/mixinspodj
npm run tauri dev
```

Rust side verified independently of the GUI:

```bash
cargo build --workspace   # whole workspace compiles
cargo test --workspace    # mixflow-db: 6/6 passing
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
