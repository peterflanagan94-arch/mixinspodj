import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { TopBar } from "./components/TopBar";
import { NowPlaying } from "./components/NowPlaying";
import { ModeRow } from "./components/ModeRow";
import { FilterRow, Filter } from "./components/FilterRow";
import { NextList } from "./components/NextList";
import { SuggestionRow } from "./components/SuggestionRow";
import { SearchPanel } from "./components/SearchPanel";
import { TopPlayedPanel } from "./components/TopPlayedPanel";
import { SetsPanel } from "./components/SetsPanel";
import { AiSuggestResults } from "./components/AiSuggestResults";
import { SettingsPanel } from "./components/SettingsPanel";
import { DirectionMode, EventContext, ScoredTrack, TraktorStatus, TraktorTrack, TrackSummary } from "./types";
import "./App.css";

function App() {
  const [dbStatus, setDbStatus] = useState<string>("connecting…");
  const [tracks, setTracks] = useState<TrackSummary[]>([]);
  const [nowPlayingId, setNowPlayingId] = useState<number | null>(null);

  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [eventContext, setEventContext] = useState<EventContext>("WEDDING");
  const [activeMode, setActiveMode] = useState<DirectionMode | null>(null);
  const [activeFilters, setActiveFilters] = useState<Filter[]>([]);

  const [searchOpen, setSearchOpen] = useState(false);
  const [topPlayedOpen, setTopPlayedOpen] = useState(false);
  const [setsOpen, setSetsOpen] = useState(false);
  const [suggestionSource, setSuggestionSource] = useState<"style" | "ai">("style");
  const [compact, setCompact] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const [recommendations, setRecommendations] = useState<ScoredTrack[]>([]);
  const [recommendationsLoading, setRecommendationsLoading] = useState(false);

  async function refreshTracks() {
    try {
      const result = await invoke<TrackSummary[]>("list_tracks");
      setTracks(result);
    } catch (e) {
      console.error("list_tracks failed", e);
    }
  }

  useEffect(() => {
    invoke<string>("db_health_check")
      .then(setDbStatus)
      .catch((e) => setDbStatus(`error: ${e}`));
    refreshTracks();
  }, []);

  async function toggleAlwaysOnTop() {
    const next = !alwaysOnTop;
    await getCurrentWindow().setAlwaysOnTop(next);
    setAlwaysOnTop(next);
  }

  const [randomPick, setRandomPick] = useState<{ scored: ScoredTrack; track: TrackSummary } | null>(null);

  async function rollRandom() {
    if (nowPlayingId === null) return;
    try {
      const pick = await invoke<ScoredTrack | null>("random_suggestion", {
        nowPlayingId,
        mode: activeMode ?? "KEEP",
      });
      const track = pick ? tracks.find((t) => t.id === pick.track_id) : undefined;
      setRandomPick(pick && track ? { scored: pick, track } : null);
    } catch (e) {
      console.error("random_suggestion failed", e);
    }
  }

  async function toggleCompact() {
    const next = !compact;
    await getCurrentWindow().setSize(next ? new LogicalSize(360, 340) : new LogicalSize(410, 760));
    setCompact(next);
  }

  async function minimize() {
    await getCurrentWindow().minimize();
  }

  const [analysingId, setAnalysingId] = useState<number | null>(null);

  async function analyseNow(trackId: number) {
    setAnalysingId(trackId);
    try {
      await invoke("analyze_track", { trackId });
      await refreshTracksRef.current();
    } catch (e) {
      console.error("analyze_track failed", e);
    } finally {
      setAnalysingId((current) => (current === trackId ? null : current));
    }
  }

  async function playTrack(trackId: number, detected = false) {
    if (trackId === nowPlayingId) return;
    const index = recommendations.findIndex((r) => r.track_id === trackId);
    try {
      await invoke("play_track", {
        trackId,
        previousTrackId: nowPlayingId,
        mode: activeMode ?? "KEEP",
        rank: index >= 0 ? index + 1 : null,
        detected,
      });
    } catch (e) {
      console.error("play_track failed", e);
    }
    setNowPlayingId(trackId);
    void analyseNow(trackId);
    // A track dragged in from the Library window might not be in this
    // window's in-memory list yet (e.g. it was just scanned there) —
    // refresh so the card doesn't show empty while nowPlayingId is set.
    if (!tracks.some((t) => t.id === trackId)) {
      await refreshTracks();
    }
  }

  const [traktorStatus, setTraktorStatus] = useState<TraktorStatus | null>(null);
  const [detected, setDetected] = useState<TraktorTrack | null>(null);
  const playRef = useRef(playTrack);
  playRef.current = playTrack;
  const refreshTracksRef = useRef(refreshTracks);
  refreshTracksRef.current = refreshTracks;

  useEffect(() => {
    const unlistenTrack = listen<TraktorTrack>("traktor-track", (event) => {
      const track = event.payload;
      if (track.track_id !== null) {
        setDetected(null);
        void playRef.current(track.track_id, true);
      } else {
        setDetected(track);
      }
    });
    const unlistenLibrary = listen("library-updated", () => {
      void refreshTracksRef.current();
    });
    const unlistenStatus = listen<TraktorStatus>("traktor-status", (event) => {
      setTraktorStatus(event.payload);
    });
    return () => {
      unlistenTrack.then((f) => f());
      unlistenStatus.then((f) => f());
      unlistenLibrary.then((f) => f());
    };
  }, []);

  useEffect(() => {
    if (nowPlayingId === null) {
      setRecommendations([]);
      return;
    }
    let cancelled = false;
    setRecommendationsLoading(true);
    invoke<ScoredTrack[]>("recommend_next", { nowPlayingId, mode: activeMode ?? "KEEP" })
      .then((result) => {
        if (!cancelled) setRecommendations(result);
      })
      .catch((e) => {
        console.error("recommend_next failed", e);
        if (!cancelled) setRecommendations([]);
      })
      .finally(() => {
        if (!cancelled) setRecommendationsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [nowPlayingId, activeMode, tracks]);

  async function openLibrary() {
    const existing = await WebviewWindow.getByLabel("library");
    if (existing) {
      await existing.setFocus();
      return;
    }
    new WebviewWindow("library", {
      url: "index.html?view=library",
      title: "MixInspo DJ — Library",
      width: 960,
      height: 640,
      minWidth: 720,
      minHeight: 420,
      dragDropEnabled: false,
    });
  }

  const nowPlaying = tracks.find((t) => t.id === nowPlayingId) ?? null;

  return (
    <main className={`app ${compact ? "app-compact" : ""}`}>
      <TopBar
        alwaysOnTop={alwaysOnTop}
        onToggleAlwaysOnTop={toggleAlwaysOnTop}
        onMinimize={minimize}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenSearch={() => setSearchOpen(true)}
        onOpenTopPlayed={() => setTopPlayedOpen(true)}
        onOpenSets={() => setSetsOpen(true)}
        compact={compact}
        onToggleCompact={toggleCompact}
        onOpenLibrary={openLibrary}
        eventContext={eventContext}
        onChangeEventContext={setEventContext}
      />

      <NowPlaying track={nowPlaying} onPick={() => setSearchOpen(true)} onDropTrack={playTrack} />
      {detected && (
        <div className="detected-line">
          Traktor: {detected.artist ?? "Unknown artist"} — {detected.title ?? "Untitled"} ·{" "}
          {detected.searching ? "looking in your folders…" : "not in your library"}
        </div>
      )}

      <div className="source-row">
        <button
          type="button"
          className={`source-btn ${suggestionSource === "style" ? "source-btn-active" : ""}`}
          onClick={() => setSuggestionSource("style")}
        >
          Style
        </button>
        <button
          type="button"
          className={`source-btn source-btn-ai ${suggestionSource === "ai" ? "source-btn-active" : ""}`}
          onClick={() => setSuggestionSource("ai")}
        >
          AI Suggest
        </button>
      </div>

      <ModeRow active={activeMode} onSelect={setActiveMode} />

      {suggestionSource === "style" && (
        <>
          <div className="next-header">
            <span className="next-header-note">
              {dbStatus.startsWith("connected") ? (recommendationsLoading ? "scoring…" : "") : dbStatus}
            </span>
            <button
              type="button"
              className="random-btn"
              disabled={nowPlayingId === null}
              onClick={() => void rollRandom()}
              title="A random track from your library that fits the one playing now"
            >
              Random
            </button>
          </div>
          {randomPick && (
            <div className="random-pick">
              <SuggestionRow
                title={randomPick.track.title ?? "Untitled"}
                artist={randomPick.track.artist}
                bpm={randomPick.track.bpm}
                camelotKey={randomPick.track.camelot_key}
                energy={randomPick.track.energy}
                score={randomPick.scored.score_percent}
                reason={randomPick.scored.components[0]?.explanation}
              />
            </div>
          )}
          <NextList
            tracks={tracks}
            nowPlayingId={nowPlayingId}
            recommendations={recommendations}
            needsAnalysis={nowPlaying !== null && (nowPlaying.bpm === null || nowPlaying.camelot_key === null)}
            analysing={analysingId === nowPlayingId}
          />
        </>
      )}

      {suggestionSource === "ai" && <AiSuggestResults nowPlayingId={nowPlayingId} />}

      <FilterRow
        active={activeFilters}
        onToggle={(f) =>
          setActiveFilters((prev) => (prev.includes(f) ? prev.filter((x) => x !== f) : [...prev, f]))
        }
      />

      <SetsPanel open={setsOpen} onClose={() => setSetsOpen(false)} />
      <TopPlayedPanel
        open={topPlayedOpen}
        onClose={() => setTopPlayedOpen(false)}
        onSelectTrack={(trackId) => void playTrack(trackId)}
      />
      <SearchPanel
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        onSelectTrack={(track) => void playTrack(track.id)}
      />
      <SettingsPanel
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onLibraryChanged={refreshTracks}
        traktorStatus={traktorStatus}
      />
    </main>
  );
}

export default App;
