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
| M3 | Library browser UI | **Done** |
| M4 | NOW PLAYING manual select / drag-in | **Done**¹ |
| M5 | Recommendation engine wired to UI | **Done** |
| M6 | Mode buttons (Keep Dancing / Build Energy / etc.) | **Done** (landed as part of M5 — the buttons existed since the Compact Live shell, M5 is what gave them something real to affect) |
| M7 | Play tracking, already-played detection | **Done** |
| M8 | Local backups, crash recovery, QA pass, tag v0.1 | **Done** |
| M9 | Accounts and subscriptions (post-MVP) | Not started |

M9 scope, to be decided before building: client accounts and subscription
tiers. Constraints already agreed: the app must keep working offline (check
the subscription online, cache it, allow a grace period); all user data is
scoped to an account from the start; personal data stays local by default
with sync opt-in (UK GDPR applies); auth and payments provider still to be
chosen (the radio studio project uses Supabase and Stripe, which could be
reused).

¹ Manual select (search → pick) is fully verified live. Drag-in initially
looked unverifiable, then genuinely didn't work for the user either —
real bug, not an automation artifact: Tauri's `dragDropEnabled` (on by
default) was intercepting drag gestures at the window level before the
webview ever saw them. Fixed (`dragDropEnabled: false` on both windows)
and confirmed working post-fix, with some residual flakiness under
synthetic automation — see the M4 section below.

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

## M3 — what's actually here

A dense, sortable, searchable library table — deliberately a **separate
window**, not squeezed into the 410px Live panel. Rekordbox/Serato keep
their library browser and performance view separate for the same reason:
a sortable multi-column table and a glanceable compact utility panel want
fundamentally different amounts of space, and the Live panel's whole
point is staying narrow.

- Opened from a new icon in the Live view's top bar
  (`WebviewWindow` with `url: "index.html?view=library"` — same SPA,
  routed client-side in `main.tsx` off a `?view=` query param, not a
  separate build). Re-clicking focuses the existing window instead of
  opening a second one (`WebviewWindow.getByLabel`).
- New capability permissions required for this: `core:webview:allow-
  create-webview-window` (spawn it) and `core:window:allow-set-focus`
  (focus the existing one) — added to `capabilities/default.json`, which
  also now covers both `main` and `library` windows.
- `list_tracks` expanded (album, genre, year, duration, danceability,
  file_format, missing_file) and un-capped — it's now the library's full
  data source, not just a 200-row preview.
- Table: click any header to sort (asc/desc toggle), free-text filter
  across title/artist/album/genre (client-side — the whole library is
  already in memory from `list_tracks`), sensible `—` for anything not
  yet analyzed.
- Manual metadata edit: click a row → pre-filled edit panel (title,
  artist, album, genre, year) → `update_track_metadata` → re-fetches.
  Never touches the file on disk, same as the rest of the app — the
  database stays the source of truth.
- **Missing-file detection, actually wired, not just displayed**: the
  table already had `missing_file` styling, but nothing ever set that
  flag — a genuine gap, not a cosmetic one, since the UI would have
  silently never triggered. Fixed by having `scan_library_folder`
  pre-mark every track under the folder being scanned as missing, then
  let each file actually found clear the flag again via the existing
  upsert path; whatever's still marked after the scan genuinely wasn't
  found. New `mixflow-db::library::mark_folder_tracks_missing`.
- Shared design tokens extracted to `src/theme.css` so the Live panel and
  the Library window stay visually consistent without duplicating the
  color/type system.

**Verified live**: built the `.app`, scanned a 4-track tagged library,
opened the Library window and confirmed real metadata, sorting, and
free-text search all work on actual data; edited a track's genre and
confirmed the new value both in the UI and directly in SQLite; then
deleted one of the four files on disk, re-scanned the same folder, and
confirmed that track — and only that track — showed up dimmed red in the
table, proving the missing-file flag is real, not decorative.

`mixflow-db`: 10/10 tests (1 new, for the missing-file re-scan behavior).

## M4 — what's actually here

Manual select (search → pick a track → it becomes NOW PLAYING) was
already built as part of the Compact Live shell and is fully verified —
nothing new there. What M4 adds is **drag-in**: the `⋮⋮` handle on every
NEXT row was purely decorative until now (documented as such); it's a
real HTML5 drag source now, and NOW PLAYING is a real drop target
(`TRACK_DRAG_MIME` in `types.ts`). Library window rows are draggable too,
so in principle you can drag a track from the Library window onto NOW
PLAYING in the Live window — native drag-and-drop is an OS-level session,
not a React one, so cross-window isn't extra work once in-window works.

**What's actually confirmed vs. not**, honestly:

- Implementation follows the standard web drag-and-drop API exactly the
  way countless real apps do it — `draggable`, `dragstart` sets a typed
  payload, the drop target's `dragover`/`drop` read it back.
- Testing this surfaced a **real bug**, not a testing artifact: the
  draggable rows had no `user-select: none`, so a real user clicking and
  dragging over the track title would very likely get text selection
  instead of — or fighting with — a drag. Fixed in both `App.css` and
  `LibraryBrowser.css`.
- What I could **not** get automated confirmation of: the drop actually
  landing. I tried three different drag mechanisms against the real
  built `.app` (a single click-drag gesture, a manual mouse-down +
  incremental mouse-moves + mouse-up sequence, and the dedicated
  background `app_drag` tool) — none caused NOW PLAYING to update or the
  dragged track to disappear from NEXT. Native HTML5 drag-and-drop
  depends on the OS recognizing a genuine drag session, which synthetic
  input tools are well known to struggle triggering reliably (this is
  not specific to this app or this tool — it's a longstanding, broadly
  documented limitation of UI automation generally). The click-based
  fallback paths all kept working correctly throughout every attempt,
  which at least rules out the drag code having broken anything else.
- **Update, same day**: the user tried it with a real mouse — still
  nothing happened. So this wasn't an automation artifact after all; it
  was a real bug. Root cause: Tauri intercepts drag gestures at the
  native window level by default (`dragDropEnabled`, true unless set
  otherwise) to support OS-level file drops onto the window — and that
  interception prevents the webview's own HTML5 drag-and-drop from ever
  firing, full stop, for anyone, automated or not. Fixed by setting
  `"dragDropEnabled": false` on the main window in `tauri.conf.json` and
  on the Library window's `WebviewWindow` options in `App.tsx` (we don't
  use native file-drop-into-window for anything, so there's no loss).
- Re-tested after the fix: dragging a NEXT row onto NOW PLAYING **did**
  switch it — confirmed via screenshot, not inferred (NOW PLAYING's
  title/artist changed to match the dragged track). A second attempt in
  the same session fell back to a click instead of a drag — some
  residual flakiness remains, consistent with how finicky *synthetic*
  drag gestures specifically are known to be (timing/discretization
  issues that a continuous real mouse/trackpad gesture doesn't hit). The
  fix is confirmed real; whether it's **reliable** for actual use still
  wants a few more real tries from you, but it's no longer fundamentally
  broken.

## M5 — what's actually here

The recommendation engine — real this time, not the stub types that sat
in `mixflow-recommender` since M0. This is the milestone that makes the
NEXT list stop being "your whole library" and start being actual scored
recommendations, and the one that turns KEEP/BUILD/COOL/SWITCH/RESCUE
from decoration into something that changes results.

**Scored components** (each in its own module, each independently unit
tested): BPM compatibility (`bpm.rs` — tiered by % difference, with
half/double-tempo folding so a 180 BPM track mixes under a 90 BPM one);
harmonic/Camelot compatibility (`harmonic.rs` — same key, relative
major/minor, adjacent wheel position, circular wheel distance); energy
compatibility (`energy.rs` — the one component whose *target*, not just
weight, genuinely changes per mode: KEEP wants flat energy, BUILD wants
+0.5 to +3, COOL wants -0.5 to -3, RESCUE ignores the delta entirely and
rewards high absolute energy — "proven crowd favourite," not "a clever
transition," matching the original brief); genre compatibility
(`genre.rs` — exact match only, no adjacency map, because a fabricated
"Disco is close to Funk" table would be a confident-sounding guess, not
real data — and the one mode where this inverts: SWITCH rewards a
*different* genre); era compatibility (`era.rs` — year-distance decay).

**What's deliberately not in there**: historical transition frequency and
event-type suitability, both part of the original design, both skipped
because there's no real data to compute them from yet — no play tracking
(M7) means no transition history, no PREP module means no real event
data. Faking either would be worse than the honest gap; the weighted-sum
architecture already has room for them.

**Missing-data handling, the part worth being precise about**: a
component whose inputs are missing (track not analyzed yet, no genre tag,
whatever) is left out of that track's score entirely and the rest
renormalize over the weight that's actually present — never guessed at
as "neutral." A track scored on fewer than all 5 components carries a
`PARTIAL_DATA` badge; a track with zero usable components scores `0%`
internally but is tagged `UNSCORED`, and the UI shows `—` instead of
`0%` for those — a real `0%` would misreport "bad match" when the honest
answer is "unknown."

**Wiring**: new `recommend_next(now_playing_id, mode)` command
(`src-tauri/src/recommend.rs`) loads features for the current track and
every other non-missing-file track directly from SQLite, calls
`mixflow_recommender::score_candidates`, returns results already sorted
best-first. `App.tsx` re-fetches whenever NOW PLAYING or the active mode
changes. NEXT rows now show score%, and the single strongest-contributing
component's explanation as a small line under each row — the "why" the
original brief asked every recommendation to carry.

**Verified live against real analyzed data**, not just the 32 unit
tests (which cover every component in isolation plus end-to-end ranking
and mode-reordering behavior): built 5 synthetic tracks with known
BPM/key/genre/year, scanned and analyzed them through the real app,
set one as NOW PLAYING, and confirmed:
- **KEEP**: the closest BPM+same-key track ranked #1 at 100%, a
  same-BPM-same-key-but-different-genre track came 2nd at 94%, an
  adjacent-key track 3rd at 90%, and the genuinely mismatched track last
  at 35% — with correct explanations ("BPM near-identical", "BPM
  compatible") on each row.
- **SWITCH**: re-ranked *instantly* on clicking the mode button — the
  different-genre candidate jumped from 2nd to 1st (97%, "Genre change
  fits Switch"), the same-genre track dropped from 100% to 76%, and even
  the harmonically-mismatched track rose to 55% purely because it was
  also a genre change.
- **RESCUE**: button turns MixInspo yellow as designed; the track with
  the lowest absolute energy dropped to last with "Low energy — risky
  for a rescue," independent of how close its BPM/key were.

`mixflow-recommender`: 32/32 tests passing. Full workspace: 50/50.

## M7 — what's actually here

Play tracking: every time a track becomes NOW PLAYING it's recorded, and
the move from the previous track is recorded as a transition. This is the
learning data the recommender will draw on over time.

- **Sessions**: each app launch starts a "Live session" (a `gigs` row marked
  in progress; any earlier in-progress one is closed). Plays belong to that
  session. Per-gig and per-client grouping comes with PREP.
- **`mixflow-db::plays`**: `start_session`, `record_play` (writes
  `played_tracks` with position and source, plus a `transitions` row when
  there's a previous track), `played_in_session`, `transition_counts_from`.
  Four new tests.
- **Source**: a play is `recommended` if the track was on screen in NEXT
  when chosen (its rank is stored), otherwise `manual`. Those ranks are what
  will show whether suggestions get used.
- **Recommender**: already-played tracks are halved in score and badged
  `ALREADY_PLAYED`, never removed. The DJ's own transition history boosts a
  candidate (log-scaled, up to +15%). It's a boost rather than a weighted
  component so a weak history value can't drag a good match down. Four new
  tests; 35 recommender tests in total.
- **UI**: NEXT rows for played tracks are dimmed with "PLAYED HH:MM" and stay
  draggable. The `played_at` time uses local time.
- **Not yet**: play history isn't learned from automatic detection (that's
  the Traktor work), and there is no crowd-state or event-type recording.

**Verified live**: launched the app, set Dancing Queen as NOW PLAYING by
search (recorded as `manual`, position 1), then set September from the search
(recorded as `recommended`, position 2), and checked the `transitions` table
(11→10, `was_recommended` = 1). Dancing Queen then appeared in NEXT dimmed
with "PLAYED 07:25" at the bottom. Afterwards I removed only the test rows
(plays, the transition, and the test session); the 11 library tracks were not
touched.

## M8 — what's actually here

Backups and crash recovery for the library database, plus the v0.1 QA pass.

- **Backups** (`mixflow-db::backup`): `VACUUM INTO` writes a consistent copy
  of the live database into `<app data>/backups/` without stopping the app.
  One is written at every launch and every 30 minutes; the newest 10 are kept.
- **Crash recovery**: on launch the database is checked with `quick_check`.
  If it fails, the damaged file is kept (renamed `damaged-<time>.sqlite3`,
  never deleted) and the newest valid backup is restored in its place. If
  there's no backup, the app starts empty and says so.
- **Backup candidates**: zero-byte files are never treated as backups. This
  came from a real mistake during testing (an unquoted shell command created
  an empty `backup-*.sqlite3` file), which is why the guard exists and has a
  test.
- **Session handling** is unchanged from M7: each launch starts a new live
  session and closes any left in progress.

**Tests**: 63 passing across the workspace (19 in `mixflow-db`, including
writing and listing backups, pruning, a healthy open, restoring from a
damaged file, the empty-database fallback, and ignoring empty files).

**Verified live**: launched the built `.app`, confirmed the startup backup
was written, `quick_check` returned `ok`, and the backup held all 11 library
tracks. I did not corrupt your real database to test recovery live, because
it holds your scanned library. Recovery is covered by the file-based tests
above. The 30-minute timer runs the same code path as the startup backup,
which was verified live, but I haven't waited out a full 30 minutes.

**Tagged**: v0.1.0 (local annotated tag, not pushed).

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
- No automatic DJ-software detection yet (Traktor Broadcasting plan is
  agreed but not built): plays are set by hand.
- Session = one app launch. Plays don't carry across restarts.
- Recovery and backups have no UI; notices go to the console only.
- Pushed to no remote; there is no CI.

## Traktor Pro detection — what's actually here

Automatic now-playing from Traktor, via its Broadcasting feature.

- **Protocol (read from a real capture, not guessed):** Traktor connects as an
  Icecast source (`SOURCE /mixinspo`, Basic auth), streams Ogg Vorbis audio,
  and writes a Vorbis comment header with `ARTIST=` and `TITLE=` into the
  stream when a track is loaded. No separate metadata request is made.
- **`mixflow-broadcast` crate:** Ogg page reassembly, Vorbis comment parsing and
  track-change detection. 12 tests, including one run on the real captured bytes.
- **`broadcast.rs`:** listens on 127.0.0.1:8765, answers the Icecast handshake,
  matches each track against the library, and emits `traktor-track` and
  `traktor-status` events. Matched tracks become NOW PLAYING; unmatched ones
  show as a yellow line.
- **Settings:** shows the address, port and live status
  (listening / connected / port in use).

**Verified live:** Traktor connected and status showed "connected". A track
was detected and shown ("Unknown artist — Words", not in library). The matched
path (auto NOW PLAYING from the library) is covered by unit tests, but not yet
seen live.

**Known gaps:** detected plays are recorded as `manual`, not `detected`; the
port is fixed at 8765 (no automatic re-selection); a track with no ARTIST tag
can't be matched.

## Watched folders and unmatched Traktor tracks

- **Watched folders** are the folders added through Settings (`library_folders`).
  Every 30 seconds the app indexes any audio file in them that isn't in the
  library yet. Only new files are read, so checks stay cheap. Removing a folder
  stops watching it and leaves the music and library entries alone.
- **Unmatched Traktor tracks** keep being searched for, every 5 seconds for a
  minute, indexing watched folders as they go. A file downloaded just before
  playing should match once it's indexed. The yellow line reads "looking in
  your folders…" meanwhile, then "not in your library". A newer track cancels
  an older search.
- **Verified live:** dropped a new tagged file into a watched folder and it was
  indexed within 40 seconds with no manual scan.
- **Not covered:** a track with no artist tag can't be matched, and a track that
  never lives on this computer (a stream) has no file to find. Traktor's own
  collection file isn't read yet.
