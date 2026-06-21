import { motion } from "framer-motion";
import { Globe, Clock, Activity, Network, Shield, ArrowRight } from "lucide-react";
import type { ReactNode } from "react";
import type { StatusSnapshot } from "../lib/api";
import { StatusOrb } from "../components/StatusOrb";
import { ThroughputMeter } from "../components/ThroughputMeter";
import { Button } from "../components/ui/Button";
import { formatDuration, shortId } from "../lib/format";
import type { Route } from "../components/Sidebar";

function StatTile({
  Icon,
  label,
  value,
  accent,
}: {
  Icon: typeof Globe;
  label: string;
  value: ReactNode;
  accent?: boolean;
}) {
  return (
    <div className="glass rounded-2xl px-4 py-3.5">
      <div className="mb-1 flex items-center gap-2 text-[color:var(--color-faint)]">
        <Icon className="h-3.5 w-3.5" />
        <span className="text-[11px] font-medium tracking-wide uppercase">
          {label}
        </span>
      </div>
      <div
        className={`truncate text-[15px] font-semibold tabular-nums ${
          accent ? "text-accent" : "text-ink"
        }`}
      >
        {value}
      </div>
    </div>
  );
}

export function Dashboard({
  status,
  historyUp,
  historyDown,
  logs,
  busy,
  onNavigate,
  onDisconnect,
  onStopHost,
}: {
  status: StatusSnapshot;
  historyUp: number[];
  historyDown: number[];
  logs: string[];
  busy: boolean;
  onNavigate: (r: Route) => void;
  onDisconnect: () => void;
  onStopHost: () => void;
}) {
  const { state, role, stats } = status;
  const active = state === "connected" || state === "hosting";
  const engaged = role !== "idle" && state !== "idle";
  const isHost = role === "host";

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3 }}
      className="grid h-full grid-cols-[300px_1fr] gap-5"
    >
      {/* left: orb + action */}
      <div className="glass-strong flex flex-col items-center rounded-3xl p-6">
        <StatusOrb state={state} role={role} />

        <div className="mt-2 text-center">
          <div className="text-lg font-semibold tracking-tight">
            {status.networkName ?? "Not connected"}
          </div>
          <div className="mt-1 max-w-[230px] text-[13px] leading-snug text-[color:var(--color-muted)]">
            {status.message ??
              "Host a network or connect to one to get started."}
          </div>
        </div>

        <div className="mt-auto w-full pt-6">
          {engaged ? (
            <Button
              variant="danger"
              size="lg"
              className="w-full"
              disabled={busy}
              onClick={isHost ? onStopHost : onDisconnect}
            >
              {isHost ? "Stop hosting" : "Disconnect"}
            </Button>
          ) : (
            <div className="grid grid-cols-2 gap-2">
              <Button
                variant="primary"
                className="w-full"
                onClick={() => onNavigate("host")}
              >
                Host
              </Button>
              <Button
                variant="subtle"
                className="w-full"
                onClick={() => onNavigate("connect")}
              >
                Connect
              </Button>
            </div>
          )}
        </div>
      </div>

      {/* right: stats + throughput + log */}
      <div className="flex flex-col gap-4 overflow-hidden">
        <div className="grid grid-cols-3 gap-3">
          <StatTile
            Icon={Network}
            label="Virtual IP"
            value={status.virtualIp ?? "—"}
          />
          <StatTile
            Icon={Globe}
            label="Egress"
            value={status.publicIp ?? "—"}
          />
          <StatTile
            Icon={Clock}
            label="Uptime"
            value={active ? formatDuration(stats.connectedSecs) : "—"}
          />
          <StatTile
            Icon={Activity}
            label="Latency"
            value={stats.latencyMs != null ? `${stats.latencyMs} ms` : "—"}
            accent={active}
          />
          <StatTile
            Icon={Shield}
            label="Link"
            value={active ? (stats.direct ? "Direct P2P" : "Relayed") : "—"}
            accent={active && stats.direct}
          />
          <StatTile
            Icon={ArrowRight}
            label="Peer"
            value={shortId(status.peerEndpointId)}
          />
        </div>

        <ThroughputMeter
          rateUp={stats.rateUp}
          rateDown={stats.rateDown}
          historyUp={historyUp}
          historyDown={historyDown}
        />

        <div className="glass flex min-h-0 flex-1 flex-col rounded-2xl p-4">
          <div className="mb-2 text-[11px] font-medium tracking-wide text-[color:var(--color-faint)] uppercase">
            Activity
          </div>
          <div className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1 font-mono text-xs leading-relaxed text-[color:var(--color-muted)]">
            {logs.length === 0 ? (
              <div className="text-[color:var(--color-faint)]">
                No activity yet.
              </div>
            ) : (
              logs.map((l, i) => (
                <div key={i} className="flex gap-2">
                  <span className="text-accent/70">›</span>
                  <span className="truncate">{l}</span>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </motion.div>
  );
}
