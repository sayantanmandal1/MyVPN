import { useMemo, useState } from "react";
import { motion } from "framer-motion";
import {
  Globe2,
  RefreshCw,
  ShieldAlert,
  ChevronRight,
  Gauge,
  Users,
  Power,
  Loader2,
} from "lucide-react";
import type { PublicServer, PublicStatus } from "../lib/api";
import { Button } from "../components/ui/Button";
import { TextField } from "../components/ui/TextField";
import { cn } from "../lib/utils";

function fmtDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

function Flag({ code }: { code: string }) {
  return (
    <span className="grid h-8 w-10 shrink-0 place-items-center rounded-md bg-white/5 text-[11px] font-semibold tracking-wide text-[color:var(--color-muted)]">
      {code.slice(0, 2).toUpperCase() || "??"}
    </span>
  );
}

type CountryGroup = { country: string; code: string; servers: PublicServer[] };

export function PublicScreen({
  servers,
  status,
  loading,
  busy,
  onRefresh,
  onConnect,
  onDisconnect,
}: {
  servers: PublicServer[];
  status: PublicStatus;
  loading: boolean;
  busy: boolean;
  onRefresh: () => void;
  onConnect: (serverId: string) => void;
  onDisconnect: () => void;
}) {
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<string | null>(null);

  const connecting = status.state === "connecting";
  const connected = status.state === "connected";
  const active = connecting || connected;

  const countries = useMemo<CountryGroup[]>(() => {
    const m = new Map<string, CountryGroup>();
    for (const s of servers) {
      let g = m.get(s.country);
      if (!g) {
        g = { country: s.country, code: s.countryCode, servers: [] };
        m.set(s.country, g);
      }
      g.servers.push(s);
    }
    const q = query.trim().toLowerCase();
    const list = [...m.values()].filter(
      (c) =>
        !q ||
        c.country.toLowerCase().includes(q) ||
        c.code.toLowerCase().includes(q),
    );
    list.sort((a, b) => a.country.localeCompare(b.country));
    return list;
  }, [servers, query]);

  const bestPing = (list: PublicServer[]) =>
    list.reduce<number | null>(
      (min, s) =>
        s.pingMs == null ? min : min == null ? s.pingMs : Math.min(min, s.pingMs),
      null,
    );

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="mx-auto flex h-full max-w-3xl flex-col overflow-hidden"
    >
      <header className="mb-4 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Free public servers</h1>
          <p className="mt-1 text-sm text-[color:var(--color-muted)]">
            Connect through a volunteer-run server in another country.
          </p>
        </div>
        <Button
          variant="ghost"
          onClick={onRefresh}
          disabled={loading}
          className="shrink-0"
        >
          <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
          Refresh
        </Button>
      </header>

      {/* Trust / safety notice — these are not as private as the P2P mode. */}
      <div className="mb-4 flex items-start gap-3 rounded-2xl border border-[color:var(--color-warn)]/25 bg-[color:var(--color-warn)]/5 px-4 py-3">
        <ShieldAlert className="mt-0.5 h-4 w-4 shrink-0 text-[color:var(--color-warn)]" />
        <p className="text-xs leading-relaxed text-[color:var(--color-muted)]">
          These servers are operated by <strong>volunteers</strong>, aggregated from open
          sources. Great for changing your region or casual browsing — but avoid sensitive
          logins. For full privacy, use the <strong>Private VPN</strong> with your own host.
        </p>
      </div>

      {/* Active connection banner. */}
      {active && (
        <div className="glass-strong mb-4 flex items-center gap-4 rounded-2xl p-4">
          <div
            className={cn(
              "grid h-12 w-12 shrink-0 place-items-center rounded-xl",
              connected ? "ring-accent bg-[color:var(--color-accent-dim)]" : "bg-white/5",
            )}
          >
            {connecting ? (
              <Loader2 className="h-5 w-5 animate-spin text-[color:var(--color-warn)]" />
            ) : (
              <Globe2 className="h-5 w-5 text-accent" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-sm font-semibold">
              {connected ? "Connected" : "Connecting…"}
              {status.country ? ` · ${status.country}` : ""}
            </div>
            <div className="truncate text-xs text-[color:var(--color-muted)]">
              {status.message ?? ""}
            </div>
          </div>
          {connected && (
            <div className="mr-1 text-right">
              <div className="font-mono text-sm tabular-nums">
                {fmtDuration(status.connectedSecs)}
              </div>
              <div className="text-[10px] text-[color:var(--color-faint)] uppercase">
                Uptime
              </div>
            </div>
          )}
          <Button variant="danger" onClick={onDisconnect} disabled={busy}>
            <Power className="h-4 w-4" />
            Disconnect
          </Button>
        </div>
      )}

      <TextField
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search countries…"
        spellCheck={false}
        className="mb-3"
      />

      {/* Country list. */}
      <div className="glass min-h-0 flex-1 divide-y divide-white/5 overflow-y-auto rounded-2xl">
        {countries.length === 0 ? (
          <div className="grid h-full min-h-40 place-items-center p-8 text-center">
            <div className="text-sm text-[color:var(--color-muted)]">
              {loading
                ? "Loading servers…"
                : servers.length === 0
                  ? "No servers loaded yet. Click Refresh to fetch the list."
                  : "No countries match your search."}
            </div>
          </div>
        ) : (
          countries.map((c) => {
            const isOpen = expanded === c.country;
            const ping = bestPing(c.servers);
            return (
              <div key={c.country}>
                <button
                  onClick={() => setExpanded(isOpen ? null : c.country)}
                  className="flex w-full items-center gap-3 px-4 py-3 text-left transition hover:bg-white/5"
                >
                  <Flag code={c.code} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm font-medium">{c.country}</div>
                    <div className="text-xs text-[color:var(--color-faint)]">
                      {c.servers.length} server{c.servers.length === 1 ? "" : "s"}
                      {ping != null ? ` · best ${ping} ms` : ""}
                    </div>
                  </div>
                  <ChevronRight
                    className={cn(
                      "h-4 w-4 text-[color:var(--color-faint)] transition-transform",
                      isOpen && "rotate-90",
                    )}
                  />
                </button>

                {isOpen && (
                  <div className="bg-black/20 px-2 pb-2">
                    {c.servers.map((s) => {
                      const isCurrent = status.serverId === s.id;
                      return (
                        <div
                          key={s.id}
                          className="flex items-center gap-3 rounded-xl px-3 py-2.5"
                        >
                          <div className="min-w-0 flex-1">
                            <div className="truncate font-mono text-xs text-[color:var(--color-muted)]">
                              {s.hostname || s.ip}
                            </div>
                            <div className="mt-0.5 flex flex-wrap gap-x-3 gap-y-0.5 text-[11px] text-[color:var(--color-faint)]">
                              {s.pingMs != null && <span>{s.pingMs} ms</span>}
                              {s.speedMbps != null && (
                                <span className="inline-flex items-center gap-1">
                                  <Gauge className="h-3 w-3" />
                                  {s.speedMbps} Mbps
                                </span>
                              )}
                              {s.sessions != null && (
                                <span className="inline-flex items-center gap-1">
                                  <Users className="h-3 w-3" />
                                  {s.sessions}
                                </span>
                              )}
                              <span className="opacity-60">{s.source}</span>
                            </div>
                          </div>
                          <Button
                            size="sm"
                            variant={isCurrent && active ? "danger" : "primary"}
                            disabled={busy}
                            onClick={() =>
                              isCurrent && active ? onDisconnect() : onConnect(s.id)
                            }
                          >
                            {isCurrent && active
                              ? connected
                                ? "Disconnect"
                                : "Cancel"
                              : "Connect"}
                          </Button>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </motion.div>
  );
}
