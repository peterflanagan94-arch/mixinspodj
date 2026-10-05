import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TopBar } from "./components/TopBar";
import { NowPlaying } from "./components/NowPlaying";
import { ModeRow } from "./components/ModeRow";
import { FilterRow, Filter } from "./components/FilterRow";
import { NextList } from "./components/NextList";
import { SearchPanel } from "./components/SearchPanel";
import { SettingsPanel } from "./components/SettingsPanel";
import { DirectionMode, EventContext, TrackSummary } from "./types";
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

  const nowPlaying = tracks.find((t) => t.id === nowPlayingId) ?? null;
  const nextTracks = tracks.filter((t) => t.id !== nowPlayingId);

  return (
    <main className="app">
      <TopBar
        alwaysOnTop={alwaysOnTop}
        onToggleAlwaysOnTop={toggleAlwaysOnTop}
        onMinimize={minimize}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenSearch={() => setSearchOpen(true)}
        eventContext={eventContext}
        onChangeEventContext={setEventContext}
      />

      <NowPlaying track={nowPlaying} onPick={() => setSearchOpen(true)} />

      <ModeRow active={activeMode} onSelect={setActiveMode} />

      <div className="next-header">
        <span>NEXT</span>
        <span className="next-header-note">{dbStatus.startsWith("connected") ? "" : dbStatus}</span>
      </div>

      <NextList tracks={nextTracks} />

      <FilterRow
        active={activeFilters}
        onToggle={(f) =>
          setActiveFilters((prev) => (prev.includes(f) ? prev.filter((x) => x !== f) : [...prev, f]))
        }
      />

      <SearchPanel
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        onSelectTrack={(track) => setNowPlayingId(track.id)}
      />
      <SettingsPanel
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
        onScanComplete={refreshTracks}
      />
    </main>
  );
}

export default App;
