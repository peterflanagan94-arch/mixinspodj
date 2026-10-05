const FILTERS = ["LOCAL", "SIMILAR", "UNPLAYED", "BPM ±5", "SWITCH"] as const;
type Filter = (typeof FILTERS)[number];

type Props = {
  active: Filter[];
  onToggle: (filter: Filter) => void;
};

/**
 * Bottom filter chips, inspired by PulseDJ's chip row (translated to
 * original terms: LOCAL/SIMILAR/UNPLAYED/BPM/SWITCH). Visual/local-state
 * only for now, same as ModeRow — there's no recommendation engine yet
 * (M5) for these to actually filter.
 */
export function FilterRow({ active, onToggle }: Props) {
  return (
    <div className="filter-row">
      {FILTERS.map((filter) => (
        <button
          key={filter}
          className={`filter-chip ${active.includes(filter) ? "filter-chip-active" : ""}`}
          onClick={() => onToggle(filter)}
        >
          {filter}
        </button>
      ))}
    </div>
  );
}

export type { Filter };
