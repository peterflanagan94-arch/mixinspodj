import { TrackSummary, TRACK_DRAG_MIME } from "../types";

type Props = {
  tracks: TrackSummary[];
};

/**
 * Compact recommendation rows. Match % and an energy/popularity bar are
 * intentionally omitted rather than faked — there's no scoring engine yet
 * (M5), so this currently renders the library (minus the now-playing
 * track) as plain rows, honestly, with "—" wherever a field hasn't been
 * analyzed (M2 fills BPM/key/energy in once a track's been through
 * Analyze Library; it's not that the pipeline doesn't exist).
 *
 * Each row is a real HTML5 drag source (M4's "drag-in" — the handle was
 * purely decorative before this) — drag it onto NOW PLAYING to switch to
 * it. Works across the Library window too, since native DnD is owned by
 * the OS, not React.
 */
export function NextList({ tracks }: Props) {
  if (tracks.length === 0) {
    return <div className="next-empty">No other tracks in your library yet.</div>;
  }

  return (
    <div className="next-list">
      {tracks.map((track) => (
        <div
          className="track-row"
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
