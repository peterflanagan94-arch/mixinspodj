import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  DEFAULT_ARC_FOR_EVENT,
  EventContext,
  SET_ARC_LABELS,
  SET_ARCS,
  SetArc,
  SetBuilderResult,
  SpotifyImportSummary,
} from "../types";

type Props = {
  open: boolean;
  onClose: () => void;
  eventContext: EventContext;
};

function formatMinutes(ms: number): string {
  const totalSeconds = Math.round(ms / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes}:${String(seconds).padStart(2, "0")}`;
}

const TRANSITION_LABEL: Record<string, string> = {
  OPEN: "Opens the set",
  BUILD: "Building energy",
  COOL: "Cooling down",
  KEEP: "Holding steady",
  PLAYLIST: "From your playlist",
};

/**
 * Drafts a full set from the local library toward a target length and
 * energy shape — a starting point for the DJ to review before a gig, not
 * something that touches Now Playing or gets played automatically.
 */
export function SetBuilderPanel({ open, onClose, eventContext }: Props) {
  const [durationMinutes, setDurationMinutes] = useState(120);
  const [arc, setArc] = useState<SetArc>(DEFAULT_ARC_FOR_EVENT[eventContext]);
  const [genre, setGenre] = useState("");
  const [building, setBuilding] = useState(false);
  const [result, setResult] = useState<SetBuilderResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [spotifyUrl, setSpotifyUrl] = useState("");
  const [importing, setImporting] = useState(false);
  const [importSummary, setImportSummary] = useState<SpotifyImportSummary | null>(null);
  const [importError, setImportError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setArc(DEFAULT_ARC_FOR_EVENT[eventContext]);
      setResult(null);
      setError(null);
      setSpotifyUrl("");
      setImportSummary(null);
      setImportError(null);
    }
    // Only re-seed the arc when the panel opens, not on every event-context
    // change — the DJ may have deliberately picked a different arc already.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  async function importPlaylist() {
    if (!spotifyUrl.trim()) return;
    setImporting(true);
    setImportError(null);
    try {
      const summary = await invoke<SpotifyImportSummary>("import_spotify_playlist", { url: spotifyUrl.trim() });
      setImportSummary(summary);
    } catch (e) {
      setImportError(String(e));
      setImportSummary(null);
    } finally {
      setImporting(false);
    }
  }

  async function build() {
    setBuilding(true);
    setError(null);
    try {
      const set = await invoke<SetBuilderResult>("build_set", {
        durationMinutes,
        arc,
        genre: genre.trim() || null,
        seedTrackIds: importSummary ? importSummary.matched.map((m) => m.track_id) : [],
      });
      setResult(set);
    } catch (e) {
      setError(String(e));
      setResult(null);
    } finally {
      setBuilding(false);
    }
  }

  if (!open) return null;

  return (
    <div className="panel-overlay" onClick={onClose}>
      <div className="panel" onClick={(e) => e.stopPropagation()}>
        <div className="panel-header">
          <span>SET BUILDER</span>
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="panel-results">
          <p className="hint">
            Drafts an ordered set from your library toward a target length and energy shape, using
            the same matching as My Local — review it, then build your own crates from it.
          </p>

          <div className="set-builder-form">
            <label className="set-builder-field">
              <span>Length (minutes)</span>
              <input
                type="number"
                className="panel-search-input"
                min={15}
                max={360}
                step={15}
                value={durationMinutes}
                onChange={(e) => setDurationMinutes(Math.max(15, Number(e.target.value) || 15))}
              />
            </label>

            <label className="set-builder-field">
              <span>Energy shape</span>
              <select
                className="panel-search-input"
                value={arc}
                onChange={(e) => setArc(e.target.value as SetArc)}
              >
                {SET_ARCS.map((a) => (
                  <option key={a} value={a}>
                    {SET_ARC_LABELS[a]}
                  </option>
                ))}
              </select>
            </label>

            <label className="set-builder-field">
              <span>Genre contains (optional)</span>
              <input
                type="text"
                className="panel-search-input"
                placeholder="e.g. Dance, House, 80s"
                value={genre}
                onChange={(e) => setGenre(e.target.value)}
              />
            </label>

            <label className="set-builder-field">
              <span>Start from a Spotify playlist (optional)</span>
              <input
                type="text"
                className="panel-search-input"
                placeholder="Paste a Spotify playlist link…"
                value={spotifyUrl}
                onChange={(e) => setSpotifyUrl(e.target.value)}
              />
            </label>
          </div>

          {spotifyUrl.trim() && !importSummary && (
            <button type="button" disabled={importing} onClick={() => void importPlaylist()} style={{ marginBottom: 10 }}>
              {importing ? "Finding tracks…" : "Find tracks in my library"}
            </button>
          )}

          {importError && <p className="error">{importError}</p>}

          {importSummary && (
            <div className="hint" style={{ marginBottom: 10 }}>
              <strong style={{ color: "var(--text-primary)" }}>{importSummary.playlist_name}</strong> — found{" "}
              {importSummary.matched.length} of {importSummary.matched.length + importSummary.unmatched.length} tracks
              in your library. The set below will open with these, in a mixable order, then fill in the rest.{" "}
              <button
                type="button"
                onClick={() => {
                  setImportSummary(null);
                  setSpotifyUrl("");
                }}
              >
                Clear
              </button>
              {importSummary.unmatched.length > 0 && (
                <details style={{ marginTop: 6 }}>
                  <summary>Not in your library ({importSummary.unmatched.length})</summary>
                  {importSummary.unmatched.map((t, i) => (
                    <div key={i}>
                      {t.title} — {t.artist}
                    </div>
                  ))}
                </details>
              )}
            </div>
          )}

          <button type="button" className="export-btn" disabled={building} onClick={() => void build()}>
            {building ? "Building…" : "Build Set"}
          </button>

          {error && <p className="error">{error}</p>}

          {result && result.tracks.length === 0 && !error && (
            <p className="hint">
              Nothing came back — try a shorter length, a wider genre filter, or make sure tracks
              have been analyzed (BPM, key and energy are all required to sequence a set).
            </p>
          )}

          {result && result.tracks.length > 0 && (
            <>
              <p className="hint">
                {result.tracks.length} tracks · {formatMinutes(result.total_duration_ms)} (target{" "}
                {formatMinutes(result.target_duration_ms)})
              </p>
              {result.tracks.map((t, i) => (
                <div key={`${t.track_id}-${i}`} className="search-result-row">
                  <div>
                    <div className="search-result-title">
                      {i + 1}. {t.title ?? "Untitled"}
                    </div>
                    <div className="search-result-artist">
                      {t.artist ?? "Unknown artist"} ·{" "}
                      {TRANSITION_LABEL[t.transition] ?? t.transition}
                    </div>
                  </div>
                  <div className="search-result-meta">
                    {t.duration_ms !== null ? formatMinutes(t.duration_ms) : "—"} ·{" "}
                    {t.bpm !== null ? Math.round(t.bpm) : "—"} · {t.camelot_key ?? "—"} · energy{" "}
                    {t.energy !== null ? t.energy.toFixed(1) : "—"}
                  </div>
                </div>
              ))}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
