import { useState } from "react";
import { motion } from "framer-motion";
import {
  PlugZap,
  RefreshCw,
  Wifi,
  Globe2,
  Bookmark,
  Lock,
  ChevronRight,
} from "lucide-react";
import type { DiscoveredHost, StatusSnapshot } from "../lib/api";
import { Button } from "../components/ui/Button";
import { TextField } from "../components/ui/TextField";
import { cn } from "../lib/utils";

function sourceMeta(source: string) {
  switch (source) {
    case "lan":
      return { Icon: Wifi, label: "Same Wi‑Fi" };
    case "dht":
      return { Icon: Globe2, label: "Internet" };
    default:
      return { Icon: Bookmark, label: "Saved" };
  }
}

export function ConnectScreen({
  status,
  discovered,
  busy,
  onConnect,
  onRefresh,
  onDisconnect,
}: {
  status: StatusSnapshot;
  discovered: DiscoveredHost[];
  busy: boolean;
  onConnect: (cfg: {
    networkName: string;
    passphrase?: string | null;
    ticket?: string | null;
    endpointId?: string | null;
  }) => void;
  onRefresh: () => void;
  onDisconnect: () => void;
}) {
  const [name, setName] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [code, setCode] = useState("");

  const clientActive =
    status.role === "client" &&
    (status.state === "connecting" || status.state === "connected");

  if (clientActive) {
    const connected = status.state === "connected";
    return (
      <motion.div
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        className="mx-auto flex h-full max-w-lg flex-col justify-center"
      >
        <div className="glass-strong rounded-3xl p-8 text-center">
          <div
            className={cn(
              "mx-auto grid h-16 w-16 place-items-center rounded-2xl",
              connected
                ? "ring-accent bg-[color:var(--color-accent-dim)]"
                : "bg-white/5",
            )}
          >
            <PlugZap
              className={cn(
                "h-7 w-7",
                connected ? "text-accent" : "animate-pulse text-[color:var(--color-warn)]",
              )}
            />
          </div>
          <h1 className="mt-5 text-2xl font-semibold tracking-tight">
            {connected ? "Connected" : "Connecting…"}
          </h1>
          <p className="mt-1 text-sm text-[color:var(--color-muted)]">
            {status.message ?? status.networkName}
          </p>
          {connected && (
            <Button
              variant="danger"
              size="lg"
              className="mx-auto mt-6"
              disabled={busy}
              onClick={onDisconnect}
            >
              Disconnect
            </Button>
          )}
        </div>
      </motion.div>
    );
  }

  const canConnectByName = name.trim().length >= 2 && !busy;
  const canConnectByCode = code.trim().length >= 4 && !busy;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="mx-auto flex h-full max-w-3xl flex-col"
    >
      <header className="mb-5 flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">
            Available networks
          </h1>
          <p className="mt-1 text-sm text-[color:var(--color-muted)]">
            Pick a network nearby, or connect manually with a name or code.
          </p>
        </div>
        <Button variant="ghost" onClick={onRefresh} disabled={busy}>
          <RefreshCw className={cn("h-4 w-4", busy && "animate-spin")} />
          Refresh
        </Button>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[1fr_320px] gap-5">
        {/* discovered list */}
        <div className="glass min-h-0 overflow-y-auto rounded-2xl p-2">
          {discovered.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center px-6 text-center">
              <div className="grid h-12 w-12 place-items-center rounded-xl bg-white/5">
                <Wifi className="h-5 w-5 text-[color:var(--color-faint)]" />
              </div>
              <div className="mt-3 text-sm font-medium">No networks found</div>
              <div className="mt-1 text-xs text-[color:var(--color-muted)]">
                Make sure another device is hosting, or connect manually using a
                name + passphrase or a pairing code.
              </div>
            </div>
          ) : (
            <ul className="space-y-1">
              {discovered.map((h) => {
                const { Icon, label } = sourceMeta(h.source);
                return (
                  <li key={h.endpointId}>
                    <button
                      disabled={busy}
                      onClick={() =>
                        onConnect({
                          networkName: h.networkName,
                          endpointId: h.endpointId,
                        })
                      }
                      className="group flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left transition hover:bg-white/5 disabled:opacity-50"
                    >
                      <span className="grid h-9 w-9 place-items-center rounded-lg bg-white/5">
                        <Icon className="h-4 w-4 text-accent" />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm font-medium">
                          {h.networkName}
                        </span>
                        <span className="flex items-center gap-1.5 text-[11px] text-[color:var(--color-faint)]">
                          {label}
                          {h.requiresPassphrase && (
                            <Lock className="h-2.5 w-2.5" />
                          )}
                        </span>
                      </span>
                      <ChevronRight className="h-4 w-4 text-[color:var(--color-faint)] transition group-hover:translate-x-0.5 group-hover:text-ink" />
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {/* manual connect */}
        <div className="flex flex-col gap-3">
          <div className="glass rounded-2xl p-4">
            <div className="mb-3 text-xs font-medium tracking-wide text-[color:var(--color-faint)] uppercase">
              Connect by name
            </div>
            <div className="space-y-2.5">
              <TextField
                placeholder="Network name"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
              <TextField
                type="password"
                placeholder="Passphrase"
                value={passphrase}
                onChange={(e) => setPassphrase(e.target.value)}
              />
              <Button
                variant="primary"
                className="w-full"
                disabled={!canConnectByName}
                onClick={() =>
                  onConnect({
                    networkName: name.trim(),
                    passphrase: passphrase.trim() || null,
                  })
                }
              >
                Connect
              </Button>
            </div>
          </div>

          <div className="glass rounded-2xl p-4">
            <div className="mb-3 text-xs font-medium tracking-wide text-[color:var(--color-faint)] uppercase">
              Connect by code
            </div>
            <div className="space-y-2.5">
              <TextField
                placeholder="Paste pairing code"
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />
              <Button
                variant="subtle"
                className="w-full"
                disabled={!canConnectByCode}
                onClick={() =>
                  onConnect({
                    networkName: "Paired network",
                    ticket: code.trim(),
                  })
                }
              >
                Connect with code
              </Button>
            </div>
          </div>
        </div>
      </div>
    </motion.div>
  );
}
