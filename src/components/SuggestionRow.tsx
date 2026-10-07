import { CSSProperties, useState } from "react";

type Props = {
  title: string;
  artist: string | null;
  bpm: number | null;
  camelotKey: string | null;
  /** 1-10. Shown so BUILD/COOL read as a direction, not just a tied score. */
  energy?: number | null;
  /** 0-100, shown bold and large — the one number to glance at. */
  score: number | null;
  /** Shown as a dash for a track with no usable score (not yet analysed). */
  unscored?: boolean;
  reason?: string;
  playedTime?: string | null;
};

/**
 * A row built to glance at, not click: this app doesn't control Traktor, so
 * there's no Play button. Clicking copies "Artist - Title" to the clipboard
 * instead, ready to paste into Traktor's own search.
 */
export function SuggestionRow({ title, artist, bpm, camelotKey, energy, score, unscored, reason, playedTime }: Props) {
  const [copied, setCopied] = useState(false);

  async function copy() {
    const text = artist ? `${artist} - ${title}` : title;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {
      // Clipboard access can be denied; the row just doesn't confirm.
    }
  }

  return (
    <button className="suggestion-row" onClick={() => void copy()} title="Click to copy artist and title">
      <div className="suggestion-row-main">
        <div className="suggestion-row-title">{title}</div>
        <div className="suggestion-row-artist">
          {copied ? "Copied — paste into Traktor's search" : playedTime ? `PLAYED ${playedTime}` : artist ?? "Unknown artist"}
        </div>
        {reason && !copied && <div className="suggestion-row-reason">{reason}</div>}
      </div>
      <div className="suggestion-row-stats">
        <span className="suggestion-row-bpm">{bpm !== null ? Math.round(bpm) : "—"}</span>
        <span className="suggestion-row-key" style={keyStyle(camelotKey)}>
          {camelotKey ?? "—"}
        </span>
        {energy != null && <span className="suggestion-row-energy">E{Math.round(energy)}</span>}
      </div>
      <div className="suggestion-row-score">{unscored || score === null ? "—" : Math.round(score)}</div>
    </button>
  );
}

/**
 * A colour per Camelot number: the wheel's 12 positions spaced evenly round
 * the hue circle, so neighbouring keys (the ones that mix well) get
 * neighbouring colours. Minor keys (A) are darker than majors (B).
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
