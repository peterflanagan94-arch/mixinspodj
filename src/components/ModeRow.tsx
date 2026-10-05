import { DIRECTION_MODES, DirectionMode } from "../types";

type Props = {
  active: DirectionMode | null;
  onSelect: (mode: DirectionMode) => void;
};

const LABELS: Record<DirectionMode, string> = {
  KEEP: "KEEP",
  BUILD: "BUILD ↑",
  COOL: "COOL ↓",
  SWITCH: "SWITCH ↪",
  RESCUE: "RESCUE",
};

/**
 * Direction controls are visual/local-state only for now — there's no
 * scoring engine yet (M5) for them to affect. Selecting one highlights it
 * but doesn't change the (also not-yet-real) NEXT list below.
 */
export function ModeRow({ active, onSelect }: Props) {
  return (
    <div className="mode-row">
      {DIRECTION_MODES.map((mode) => (
        <button
          key={mode}
          className={`mode-btn ${active === mode ? (mode === "RESCUE" ? "mode-btn-rescue" : "mode-btn-active") : ""}`}
          onClick={() => onSelect(active === mode ? "KEEP" : mode)}
        >
          {LABELS[mode]}
        </button>
      ))}
    </div>
  );
}
