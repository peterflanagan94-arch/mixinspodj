import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { TopBar } from "./components/TopBar";
import { NowPlaying } from "./components/NowPlaying";
import { ModeRow } from "./components/ModeRow";
import { FilterRow, Filter } from "./components/FilterRow";
import { NextList } from "./components/NextList";
import { SearchPanel } from "./components/SearchPanel";
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
    <main className="app">
      <TopBar
        alwaysOnTop={alwaysOnTop}
        onToggleAlwaysOnTop={toggleAlwaysOnTop}
        onMinimize={minimize}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenSearch={() => setSearchOpen(true)}
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

      <ModeRow active={activeMode} onSelect={setActiveMode} />

      <div className="next-header">
        <span>NEXT</span>
        <span className="next-header-note">
          {dbStatus.startsWith("connected") ? (recommendationsLoading ? "scoring…" : "") : dbStatus}
        </span>
      </div>

      <NextList
        tracks={tracks}
        nowPlayingId={nowPlayingId}
        recommendations={recommendations}
        needsAnalysis={nowPlaying !== null && (nowPlaying.bpm === null || nowPlaying.camelot_key === null)}
        analysing={analysingId === nowPlayingId}
      />

      <FilterRow
        active={activeFilters}
        onToggle={(f) =>
          setActiveFilters((prev) => (prev.includes(f) ? prev.filter((x) => x !== f) : [...prev, f]))
        }
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
