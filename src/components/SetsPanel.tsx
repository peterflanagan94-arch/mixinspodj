import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SetPlay, SetSummary, TraktorPlay, TraktorSet } from "../types";

type Props = {
  open: boolean;
  onClose: () => void;
};

const SOURCE_LABEL: Record<string, string> = {
  recommended: "suggested",
  manual: "own pick",
  detected: "Traktor",
};

/** "history_2026y10m06d_08h19m42s.nml" becomes "6 Oct 2026, 08:19". */
function sessionLabel(fileName: string): string {
  const m = fileName.match(/history_(\d{4})y(\d{2})m(\d{2})d_(\d{2})h(\d{2})m/);
  if (!m) return fileName.replace(/\.nml$/, "");
  const date = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  const day = date.toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });
  return `${day}, ${m[4]}:${m[5]}`;
}

function clockTime(seconds: number): string {
  const hours = Math.floor(seconds / 3600) % 24;
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

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
  const [traktorSets, setTraktorSets] = useState<TraktorSet[]>([]);
  const [openTraktorSet, setOpenTraktorSet] = useState<TraktorSet | null>(null);
  const [traktorPlays, setTraktorPlays] = useState<TraktorPlay[]>([]);
  const [exportNote, setExportNote] = useState<string | null>(null);

  async function exportPdf(kind: "app" | "traktor", id: number) {
    setExportNote(null);
    try {
      const saved = await invoke<string | null>("export_set_pdf", { kind, id });
      if (saved) setExportNote(`Saved to ${saved}`);
    } catch (e) {
      setExportNote(String(e));
    }
  }

  useEffect(() => {
    if (!open) {
      setOpenSet(null);
      setOpenTraktorSet(null);
      return;
    }
    setError(null);
    invoke<SetSummary[]>("list_sets")
      .then(setSets)
      .catch((e) => setError(String(e)));
    invoke<TraktorSet[]>("list_traktor_sets")
      .then(setTraktorSets)
      .catch((e) => setError(String(e)));
  }, [open]);

  useEffect(() => {
    if (!openTraktorSet) return;
    invoke<TraktorPlay[]>("traktor_set_report", { setId: openTraktorSet.set_id })
      .then(setTraktorPlays)
      .catch((e) => setError(String(e)));
  }, [openTraktorSet]);

  useEffect(() => {
    if (!openSet) return;
    invoke<SetPlay[]>("set_report", { sessionId: openSet.session_id })
      .then(setPlays)
      .catch((e) => setError(String(e)));
  }, [openSet]);

  if (!open) return null;

  if (openTraktorSet) {
    const totalSeconds = traktorPlays.reduce((sum, p) => sum + p.played_seconds, 0);
    const matched = traktorPlays.filter((p) => p.track_id !== null).length;
    return (
      <div className="panel-overlay" onClick={onClose}>
        <div className="panel" onClick={(e) => e.stopPropagation()}>
          <div className="panel-header">
            <button className="icon-btn" onClick={() => setOpenTraktorSet(null)}>
              ←
            </button>
            <span>TRAKTOR SESSION</span>
            <button className="icon-btn" onClick={onClose}>
              ✕
            </button>
          </div>
          <div className="panel-results">
            {error && <p className="error">{error}</p>}
            <button
              type="button"
              className="export-btn"
              onClick={() => void exportPdf("traktor", openTraktorSet.set_id)}
            >
              Export PDF
            </button>
            {exportNote && <p className="hint">{exportNote}</p>}
            <p className="hint">
              {traktorPlays.length} tracks · {formatMinutes(totalSeconds)} played ·{" "}
              {matched} matched to your library
            </p>
            {traktorPlays.map((p, i) => (
              <div key={`${p.start_seconds}-${i}`} className="search-result-row">
                <div>
                  <div className="search-result-title">{p.title}</div>
                  <div className="search-result-artist">
                    {p.artist ?? (p.track_id === null ? "not in your library" : "Unknown artist")}
                  </div>
                </div>
                <div className="search-result-meta">
                  {clockTime(p.start_seconds)} · {formatMinutes(p.played_seconds)}
                </div>
              </div>
            ))}
            <p className="hint">Playing times are exact, from Traktor's own history.</p>
          </div>
        </div>
      </div>
    );
  }

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

          {!openSet && (
            <>
              {traktorSets.length > 0 && <p className="hint">FROM TRAKTOR · exact times</p>}
              {traktorSets.map((set) => (
                <button
                  key={set.set_id}
                  className="search-result-row"
                  onClick={() => setOpenTraktorSet(set)}
                >
                  <div>
                    <div className="search-result-title">{sessionLabel(set.file_name)}</div>
                    <div className="search-result-artist">{set.plays} tracks</div>
                  </div>
                </button>
              ))}
              {sets.length > 0 && <p className="hint">FROM THIS APP</p>}
            </>
          )}

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
              <button
                type="button"
                className="export-btn"
                onClick={() => void exportPdf("app", openSet.session_id)}
              >
                Export PDF
              </button>
              {exportNote && <p className="hint">{exportNote}</p>}
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
