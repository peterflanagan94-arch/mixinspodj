import { EVENT_CONTEXTS, EventContext } from "../types";

type Props = {
  alwaysOnTop: boolean;
  onToggleAlwaysOnTop: () => void;
  onMinimize: () => void;
  onOpenSettings: () => void;
  onOpenSearch: () => void;
  eventContext: EventContext;
  onChangeEventContext: (ctx: EventContext) => void;
};

export function TopBar({
  alwaysOnTop,
  onToggleAlwaysOnTop,
  onMinimize,
  onOpenSettings,
  onOpenSearch,
  eventContext,
  onChangeEventContext,
}: Props) {
  return (
    <div className="topbar">
      <div className="topbar-left">
        <span className="brand-mark" aria-hidden>
          M
        </span>
        <span className="live-badge">
          <span className="live-dot" />
          LIVE
        </span>
        <select
          className="event-select"
          value={eventContext}
          onChange={(e) => onChangeEventContext(e.target.value as EventContext)}
          title="Event context (not yet used by scoring — lands with the recommendation engine)"
        >
          {EVENT_CONTEXTS.map((ctx) => (
            <option key={ctx} value={ctx}>
              {ctx}
            </option>
          ))}
        </select>
      </div>
      <div className="topbar-right">
        <button className="icon-btn" title="Find a track" onClick={onOpenSearch}>
          ⌕
        </button>
        <button
          className={`icon-btn ${alwaysOnTop ? "icon-btn-active" : ""}`}
          title="Always on top"
          onClick={onToggleAlwaysOnTop}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
            <path
              d="M6 1l1 1-.5 3L9 7.5 8 8.5 6 7l-2.5 2.5L3 9l2.5-2.5-1.5-2L4 4l3-.5 1-1L6 1z"
              fill="currentColor"
            />
            <path d="M3.5 8.5L1.5 10.5" stroke="currentColor" strokeWidth="0.8" strokeLinecap="round" />
          </svg>
        </button>
        <button className="icon-btn" title="Settings" onClick={onOpenSettings}>
          ⚙
        </button>
        <button className="icon-btn" title="Minimize" onClick={onMinimize}>
          −
        </button>
      </div>
    </div>
  );
}
