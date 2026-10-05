import { useState } from "react";
import { TrackSummary, TRACK_DRAG_MIME } from "../types";

type Props = {
  track: TrackSummary | null;
  onPick: () => void;
  onDropTrack: (trackId: number) => void;
};

export function NowPlaying({ track, onPick, onDropTrack }: Props) {
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
    return (
      <button
        className={`now-playing now-playing-empty ${dragOver ? "now-playing-drag-over" : ""}`}
        onClick={onPick}
        {...dragProps}
      >
        {dragOver ? "Drop to set Now Playing" : "No track selected — tap to choose, or drag a track here"}
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
