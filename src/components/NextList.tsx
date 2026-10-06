import { CSSProperties } from "react";
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
 * Suggestion rows, built to be read at a glance: title and artist large, BPM
 * and key in fixed columns, and the score as a bar. Keys are coloured by their
 * Camelot number, so a harmonic match stands out without reading the code. The
 * reason for the score sits in the row's tooltip rather than on screen.
 *
 * A PARTIAL_DATA score is drawn faded with a "~" because some of its parts
 * couldn't be computed. UNSCORED shows "—": a true zero would read as a bad match.
 *
 * Each row is a real HTML5 drag source — drag it onto NOW PLAYING to switch to it.
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
        const percent = Math.round(scored.score_percent);
        const topReason = scored.components[0]?.explanation;
        const playedTime = scored.played_at?.slice(11, 16) ?? null;

        return (
          <div
            className={`track-row ${playedTime ? "track-row-played" : ""}`}
            key={track.id}
            title={topReason}
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
              <div className="track-row-title">{track.title ?? "Untitled"}</div>
              <div className="track-row-artist">{track.artist ?? "Unknown artist"}</div>
              {playedTime && <div className="track-row-played-tag">PLAYED {playedTime}</div>}
            </div>
            <div className="track-row-stats">
              <span className="track-row-bpm">{track.bpm !== null ? Math.round(track.bpm) : "—"}</span>
              <span className="track-row-key" style={keyStyle(track.camelot_key)}>
                {track.camelot_key ?? "—"}
              </span>
            </div>
            <div className="track-row-score-col">
              <div className="score-bar" aria-hidden>
                <div
                  className={`score-fill ${partial ? "score-fill-partial" : ""}`}
                  style={{ width: unscored ? "0%" : `${percent}%` }}
                />
              </div>
              <span className="track-row-score">
                {unscored ? "—" : `${percent}%`}
                {partial && <span className="track-row-partial">~</span>}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/**
 * A colour per Camelot number: the wheel's 12 positions spaced evenly round the
 * hue circle, so neighbouring keys (the ones that mix well) get neighbouring
 * colours. Minor keys (A) are darker than majors (B).
 */
function keyStyle(camelot: string | null): CSSProperties {
  if (!camelot) return {};
  const match = camelot.match(/^(\d{1,2})([AB])$/);
  if (!match) return {};
  const number = Number(match[1]);
  const hue = ((number - 1) * 30) % 360;
  const lightness = match[2] === "A" ? 30 : 42;
  return { background: `hsl(${hue} 55% ${lightness}%)` };
}
