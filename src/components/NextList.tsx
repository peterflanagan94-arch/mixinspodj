import { ScoredTrack, TrackSummary, TRACK_DRAG_MIME } from "../types";

type Props = {
  tracks: TrackSummary[];
  nowPlayingId: number | null;
  /** Already sorted best-match-first by the backend. */
  recommendations: ScoredTrack[];
  /** True when the now-playing track has no BPM or key yet. */
  needsAnalysis: boolean;
  /** True while the now-playing track is being analysed in the background. */
  analysing: boolean;
};

/**
 * Compact recommendation rows, scored by mixflow-recommender (M5) —
 * BPM, harmonic/key, energy, genre, and era compatibility, weighted by
 * the active mode. A PARTIAL_DATA badge means some of those couldn't be
 * computed (the track hasn't been analyzed, say) — shown as "~on the
 * score rather than hidden, so the DJ knows it's not full-confidence.
 * UNSCORED (no usable data at all) shows "—" instead of "0%": a true
 * zero would wrongly read as "bad match" rather than "unknown."
 *
 * Each row is a real HTML5 drag source (M4) — drag it onto NOW PLAYING
 * to switch to it.
 */
export function NextList({ tracks, nowPlayingId, recommendations, needsAnalysis, analysing }: Props) {
  if (nowPlayingId !== null && needsAnalysis) {
    return (
      <div className="next-empty">
        {analysing
          ? "Analysing this track…"
          : "This track isn't analysed yet, so there are no suggestions. Settings → Analyze Library."}
      </div>
    );
  }
  if (nowPlayingId === null) {
    return <div className="next-empty">Select a Now Playing track to see recommendations.</div>;
  }

  const byId = new Map(tracks.map((t) => [t.id, t]));
  const rows = recommendations
    .map((scored) => ({ scored, track: byId.get(scored.track_id) }))
    .filter((row): row is { scored: ScoredTrack; track: TrackSummary } => row.track !== undefined);

  if (rows.length === 0) {
    return <div className="next-empty">No other tracks in your library yet.</div>;
  }

  return (
    <div className="next-list">
      {rows.map(({ scored, track }) => {
        const unscored = scored.badges.includes("UNSCORED");
        const partial = scored.badges.includes("PARTIAL_DATA");
        const topReason = scored.components[0]?.explanation;
        const playedTime = scored.played_at?.slice(11, 16) ?? null;

        return (
          <div
            className={`track-row ${playedTime ? "track-row-played" : ""}`}
            key={track.id}
            draggable
            onDragStart={(e) => {
              e.dataTransfer.setData(TRACK_DRAG_MIME, String(track.id));
              e.dataTransfer.effectAllowed = "copy";
            }}
          >
            <span className="drag-handle drag-handle-local" title="Drag to Now Playing">
              ⋮⋮
            </span>
            <div className="track-row-main">
              <div className="track-row-title-line">
                <span className="track-row-title">{track.title ?? "Untitled"}</span>
                <span className="track-row-score">
                  {unscored ? "—" : `${Math.round(scored.score_percent)}%`}
                  {partial && <span className="track-row-partial">~</span>}
                </span>
              </div>
              <div className="track-row-meta">
                <span>{track.artist ?? "Unknown artist"}</span>
                <span>{track.bpm !== null ? Math.round(track.bpm) : "—"}</span>
                <span>{track.camelot_key ?? "—"}</span>
              </div>
              {playedTime ? (
                <div className="track-row-played-tag">PLAYED {playedTime}</div>
              ) : (
                topReason && <div className="track-row-reason">{topReason}</div>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
