import { TraktorStatus } from "../types";

type Props = {
  traktorStatus: TraktorStatus | null;
};

const BROADCASTING_HINT =
  "Waiting for Traktor. In Traktor: Preferences → Broadcasting → Server Settings — " +
  "Address 127.0.0.1, Port 8765, Mount path /mixinspo, Password mixinspo. " +
  "Then turn Broadcasting on (the antenna icon).";

const VIRTUALDJ_HINT =
  "No live detection yet. VirtualDJ's Network Control Plugin needs a Pro license to build " +
  "and test against, which isn't available right now. Settings → VirtualDJ (experimental) " +
  "imports BPM and key from your VirtualDJ database file instead.";

function traktorStatusInfo(status: TraktorStatus | null): { className: string; hint: string } {
  if (!status) return { className: "traktor-dot-starting", hint: "Starting…" };
  switch (status.state) {
    case "connected":
      return { className: "traktor-dot-connected", hint: "Traktor is connected and broadcasting." };
    case "port-busy":
      return {
        className: "traktor-dot-error",
        hint: `Port ${status.port} is in use by another app. Quit it and restart MixInspo DJ.`,
      };
    case "listening":
    default:
      return { className: "traktor-dot-waiting", hint: BROADCASTING_HINT };
  }
}

/**
 * Bottom bar: DJ software connection status. Traktor's is live — it
 * reflects the real Broadcasting connection, and hovering it reminds the DJ
 * to turn Broadcasting on when it's not yet connected. VirtualDJ's dot is
 * deliberately always the same dim, unlit state: there's no live detection
 * for it (see Settings), so it would be dishonest to animate this one as if
 * it were trying to connect to something.
 */
export function StatusBar({ traktorStatus }: Props) {
  const traktor = traktorStatusInfo(traktorStatus);
  return (
    <div className="filter-row">
      <div className="traktor-status" title={traktor.hint}>
        <span className={`traktor-dot ${traktor.className}`} />
        TRAKTOR
      </div>
      <div className="traktor-status" title={VIRTUALDJ_HINT}>
        <span className="traktor-dot traktor-dot-unavailable" />
        VIRTUALDJ
      </div>
    </div>
  );
}
