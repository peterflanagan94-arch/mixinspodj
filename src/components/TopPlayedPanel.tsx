import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { TopPlayed } from "../types";

type Props = {
  open: boolean;
  onClose: () => void;
  onSelectTrack: (trackId: number) => void;
};

/** The tracks this DJ has played most, across every session. Read fresh each time it opens. */
export function TopPlayedPanel({ open, onClose, onSelectTrack }: Props) {
  const [tracks, setTracks] = useState<TopPlayed[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    invoke<TopPlayed[]>("top_played_tracks", { limit: 20 })
      .then(setTracks)
      .catch((e) => setError(String(e)));
  }, [open]);

  if (!open) return null;

  return (
    <div className="panel-overlay" onClick={onClose}>
      <div className="panel" onClick={(e) => e.stopPropagation()}>
        <div className="panel-header">
          <span>YOUR MOST PLAYED</span>
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="panel-results">
          {error && <p className="error">{error}</p>}
          {!error && tracks.length === 0 && (
            <p className="hint">Nothing played yet. Tracks you play will show up here.</p>
          )}
          {tracks.map((track) => (
            <button
              key={track.track_id}
              className="search-result-row"
              onClick={() => {
                onSelectTrack(track.track_id);
                onClose();
              }}
            >
              <div>
                <div className="search-result-title">{track.title ?? "Untitled"}</div>
                <div className="search-result-artist">{track.artist ?? "Unknown artist"}</div>
              </div>
              <div className="search-result-meta">
                {track.plays}× · {track.bpm !== null ? Math.round(track.bpm) : "—"} ·{" "}
                {track.camelot_key ?? "—"}
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
