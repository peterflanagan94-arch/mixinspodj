import { useState } from "react";
import { TraktorStatus, TrackSummary, TRACK_DRAG_MIME } from "../types";

type Props = {
  track: TrackSummary | null;
  traktorStatus: TraktorStatus | null;
  onPick: () => void;
  onDropTrack: (trackId: number) => void;
};

/**
 * What to say, and whether to animate, while nothing is playing yet — so
 * the gap between opening the app and a first track showing up (Traktor
 * connecting, then broadcasting a track) doesn't read as the app being
 * stuck. The bar only animates while something is genuinely in progress;
 * a real problem (the port busy) gets a plain message instead.
 */
function emptyStatus(status: TraktorStatus | null): { text: string; animate: boolean } {
  if (!status) return { text: "Starting…", animate: true };
  switch (status.state) {
    case "connected":
      return { text: "Traktor is connected — waiting for a track to load…", animate: true };
    case "port-busy":
      return { text: `Port ${status.port} is in use by another app — check Settings`, animate: false };
    case "listening":
    default:
      return { text: "Listening for Traktor…", animate: true };
  }
}

export function NowPlaying({ track, traktorStatus, onPick, onDropTrack }: Props) {
  const [dragOver, setDragOver] = useState(false);

  function handleDragOver(e: React.DragEvent) {
    if (e.dataTransfer.types.includes(TRACK_DRAG_MIME)) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      setDragOver(true);
    }
  }

  function handleDrop(e: React.DragEvent) {
    e.preventDefault();
    setDragOver(false);
    const id = e.dataTransfer.getData(TRACK_DRAG_MIME);
    if (id) onDropTrack(Number(id));
  }

  const dragProps = {
    onDragOver: handleDragOver,
    onDragLeave: () => setDragOver(false),
    onDrop: handleDrop,
  };

  if (!track) {
    const status = emptyStatus(traktorStatus);
    return (
      <button
        className={`now-playing now-playing-empty ${dragOver ? "now-playing-drag-over" : ""}`}
        onClick={onPick}
        {...dragProps}
      >
        {dragOver ? (
          "Drop to set Now Playing"
        ) : (
          <>
            <span className={status.animate ? undefined : "error"}>{status.text}</span>
            {status.animate && (
              <div className="listening-bar listening-bar-active">
                <div className="listening-bar-fill" />
              </div>
            )}
            <span className="now-playing-empty-hint">or tap to choose, or drag a track here</span>
          </>
        )}
      </button>
    );
  }

  return (
    <button
      className={`now-playing ${dragOver ? "now-playing-drag-over" : ""}`}
      onClick={onPick}
      {...dragProps}
    >
      <div className="now-playing-label">NOW PLAYING</div>
      <div className="now-playing-title">{track.title ?? "Untitled"}</div>
      <div className="now-playing-artist">{track.artist ?? "Unknown artist"}</div>
      <div className="now-playing-stats">
        <span>{track.bpm !== null ? `${Math.round(track.bpm)} BPM` : "— BPM"}</span>
        <span>{track.camelot_key ?? "—"}</span>
        <span>{track.energy !== null ? `ENERGY ${Math.round(track.energy)}` : "ENERGY —"}</span>
      </div>
    </button>
  );
}
