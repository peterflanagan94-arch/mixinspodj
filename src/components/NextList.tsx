import { TrackSummary } from "../types";

type Props = {
  tracks: TrackSummary[];
};

/**
 * Compact recommendation rows. Match %, and energy/popularity bar are
 * intentionally omitted rather than faked — there's no scoring engine yet
 * (M5) and no BPM/key/energy analysis yet (M2), so this currently renders
 * the library (minus the now-playing track) as plain rows, honestly, with
 * "—" wherever a field hasn't been computed.
 */
export function NextList({ tracks }: Props) {
  if (tracks.length === 0) {
    return <div className="next-empty">No other tracks in your library yet.</div>;
  }

  return (
    <div className="next-list">
      {tracks.map((track) => (
        <div className="track-row" key={track.id}>
          <span className="drag-handle drag-handle-local" title="Local file available">
            ⋮⋮
          </span>
          <div className="track-row-main">
            <div className="track-row-title-line">
              <span className="track-row-title">{track.title ?? "Untitled"}</span>
            </div>
            <div className="track-row-meta">
              <span>{track.artist ?? "Unknown artist"}</span>
              <span>{track.bpm !== null ? Math.round(track.bpm) : "—"}</span>
              <span>{track.camelot_key ?? "—"}</span>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
