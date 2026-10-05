import { TrackSummary } from "../types";

type Props = {
  track: TrackSummary | null;
  onPick: () => void;
};

export function NowPlaying({ track, onPick }: Props) {
  if (!track) {
    return (
      <button className="now-playing now-playing-empty" onClick={onPick}>
        No track selected — tap to choose
      </button>
    );
  }

  return (
    <button className="now-playing" onClick={onPick}>
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
