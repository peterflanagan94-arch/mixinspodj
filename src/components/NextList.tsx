import { ScoredTrack, TrackSummary } from "../types";
import { SuggestionRow } from "./SuggestionRow";

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
 * Local-scoring suggestions ("Style") — BPM, key, energy, genre and era,
 * weighted by the active mode. Built to glance at while Traktor plays live:
 * no drag, no Play button. Tap a row to copy its artist and title, ready to
 * paste into Traktor's own search.
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
    return <div className="next-empty">Select a Now Playing track to see suggestions.</div>;
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
        const playedTime = scored.played_at?.slice(11, 16) ?? null;
        return (
          <SuggestionRow
            key={track.id}
            title={track.title ?? "Untitled"}
            artist={track.artist}
            bpm={track.bpm}
            camelotKey={track.camelot_key}
            score={scored.score_percent}
            unscored={unscored}
            reason={scored.components[0]?.explanation}
            playedTime={playedTime}
          />
        );
      })}
    </div>
  );
}
