import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SetPlay, SetSummary } from "../types";

type Props = {
  open: boolean;
  onClose: () => void;
};

const SOURCE_LABEL: Record<string, string> = {
  recommended: "suggested",
  manual: "own pick",
  detected: "Traktor",
};

function formatMinutes(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  return `${minutes}:${String(rest).padStart(2, "0")}`;
}

/**
 * Your sets: each app session, and for one of them the tracks in order, how long
 * each played, and how the picks split between suggestions and your own choices.
 */
export function SetsPanel({ open, onClose }: Props) {
  const [sets, setSets] = useState<SetSummary[]>([]);
  const [openSet, setOpenSet] = useState<SetSummary | null>(null);
  const [plays, setPlays] = useState<SetPlay[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) {
      setOpenSet(null);
      return;
    }
    setError(null);
    invoke<SetSummary[]>("list_sets")
      .then(setSets)
      .catch((e) => setError(String(e)));
  }, [open]);

  useEffect(() => {
    if (!openSet) return;
    invoke<SetPlay[]>("set_report", { sessionId: openSet.session_id })
      .then(setPlays)
      .catch((e) => setError(String(e)));
  }, [openSet]);

  if (!open) return null;

  const played = plays.reduce((sum, p) => sum + (p.est_seconds ?? 0), 0);
  const suggested = plays.filter((p) => p.source === "recommended").length;
  const counts = new Map<number, { plays: number; title: string }>();
  for (const p of plays) {
    const entry = counts.get(p.track_id) ?? { plays: 0, title: p.title ?? "Untitled" };
    entry.plays += 1;
    counts.set(p.track_id, entry);
  }
  const mostPlayed = [...counts.values()].sort((a, b) => b.plays - a.plays).slice(0, 5);

  return (
    <div className="panel-overlay" onClick={onClose}>
      <div className="panel" onClick={(e) => e.stopPropagation()}>
        <div className="panel-header">
          {openSet ? (
            <button className="icon-btn" onClick={() => setOpenSet(null)}>
              ←
            </button>
          ) : null}
          <span>{openSet ? `SET · ${openSet.started_at}` : "YOUR SETS"}</span>
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        </div>
        <div className="panel-results">
          {error && <p className="error">{error}</p>}

          {!openSet &&
            (sets.length === 0 ? (
              <p className="hint">No sets yet. Play a track and it will appear here.</p>
            ) : (
              sets.map((set) => (
                <button key={set.session_id} className="search-result-row" onClick={() => setOpenSet(set)}>
                  <div>
                    <div className="search-result-title">{set.started_at}</div>
                    <div className="search-result-artist">{set.plays} tracks</div>
                  </div>
                </button>
              ))
            ))}

          {openSet && (
            <>
              <p className="hint">
                {plays.length} tracks · about {formatMinutes(played)} played ·{" "}
                {plays.length > 0 ? Math.round((suggested / plays.length) * 100) : 0}% from suggestions
              </p>
              {mostPlayed.length > 0 && mostPlayed[0].plays > 1 && (
                <p className="hint">
                  Most played: {mostPlayed.map((m) => `${m.title} (${m.plays}×)`).join(", ")}
                </p>
              )}
              {plays.map((p) => (
                <div key={`${p.position}-${p.track_id}`} className="search-result-row">
                  <div>
                    <div className="search-result-title">
                      {p.position}. {p.title ?? "Untitled"}
                    </div>
                    <div className="search-result-artist">{p.artist ?? "Unknown artist"}</div>
                  </div>
                  <div className="search-result-meta">
                    {p.est_seconds !== null ? formatMinutes(p.est_seconds) : "—"} ·{" "}
                    {p.bpm !== null ? Math.round(p.bpm) : "—"} · {p.camelot_key ?? "—"} ·{" "}
                    {SOURCE_LABEL[p.source] ?? p.source}
                  </div>
                </div>
              ))}
              <p className="hint">
                Playing time is estimated from when each track was followed by the next, so it can
                run over if you let a track ring out. The last track in a set has no estimate.
              </p>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
