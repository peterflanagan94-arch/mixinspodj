import { TraktorStatus } from "../types";

type Props = {
  traktorStatus: TraktorStatus | null;
};

const BROADCASTING_HINT =
  "Waiting for Traktor. In Traktor: Preferences → Broadcasting → Server Settings — " +
  "Address 127.0.0.1, Port 8765, Mount path /mixinspo, Password mixinspo. " +
  "Then turn Broadcasting on (the antenna icon).";

function statusInfo(status: TraktorStatus | null): { className: string; hint: string } {
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
 * Bottom bar: just the Traktor connection status. Hovering it reminds the
 * DJ to turn Broadcasting on when it's not yet connected.
 */
export function StatusBar({ traktorStatus }: Props) {
  const status = statusInfo(traktorStatus);
  return (
    <div className="filter-row">
      <div className="traktor-status" title={status.hint}>
        <span className={`traktor-dot ${status.className}`} />
        TRAKTOR
      </div>
    </div>
  );
}
