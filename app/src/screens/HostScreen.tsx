import { useState } from "react";
import { motion } from "framer-motion";
import { QRCodeSVG } from "qrcode.react";
import {
  RadioTower,
  KeyRound,
  Copy,
  Check,
  Globe2,
  Wifi,
  ShieldCheck,
} from "lucide-react";
import type { StatusSnapshot } from "../lib/api";
import { Button } from "../components/ui/Button";
import { TextField } from "../components/ui/TextField";

function CopyRow({ label, value }: { label: string; value: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div className="glass-inset flex items-center justify-between gap-3 rounded-xl px-3 py-2.5">
      <div className="min-w-0">
        <div className="text-[10px] tracking-wide text-[color:var(--color-faint)] uppercase">
          {label}
        </div>
        <div className="truncate font-mono text-xs text-ink">{value}</div>
      </div>
      <button
        onClick={() => {
          navigator.clipboard.writeText(value);
          setCopied(true);
          setTimeout(() => setCopied(false), 1400);
        }}
        className="shrink-0 rounded-lg p-1.5 text-[color:var(--color-muted)] transition hover:bg-white/10 hover:text-ink"
      >
        {copied ? (
          <Check className="h-4 w-4 text-accent" />
        ) : (
          <Copy className="h-4 w-4" />
        )}
      </button>
    </div>
  );
}

export function HostScreen({
  status,
  busy,
  onStartHost,
  onStopHost,
}: {
  status: StatusSnapshot;
  busy: boolean;
  onStartHost: (cfg: {
    networkName: string;
    passphrase?: string | null;
  }) => void;
  onStopHost: () => void;
}) {
  const hosting = status.state === "hosting" && status.role === "host";
  const [name, setName] = useState("");
  const [passphrase, setPassphrase] = useState("");

  const pairingCode = status.endpointId ?? "";

  if (hosting) {
    return (
      <motion.div
        initial={{ opacity: 0, y: 8 }}
        animate={{ opacity: 1, y: 0 }}
        className="mx-auto flex h-full max-w-3xl flex-col"
      >
        <header className="mb-5">
          <h1 className="text-2xl font-semibold tracking-tight">
            Hosting{" "}
            <span className="text-accent">{status.networkName}</span>
          </h1>
          <p className="mt-1 text-sm text-[color:var(--color-muted)]">
            Your network is live. Connect from another device using the code
            below, or — on the same Wi‑Fi — it appears automatically.
          </p>
        </header>

        <div className="grid grid-cols-[260px_1fr] gap-5">
          <div className="glass-strong flex flex-col items-center justify-center rounded-3xl p-6">
            <div className="rounded-2xl bg-white p-3">
              {pairingCode ? (
                <QRCodeSVG value={pairingCode} size={184} level="M" />
              ) : (
                <div className="grid h-[184px] w-[184px] place-items-center text-xs text-black/50">
                  Generating…
                </div>
              )}
            </div>
            <div className="mt-4 text-center text-xs text-[color:var(--color-muted)]">
              Paste this code on the other device's Connect screen
            </div>
          </div>

          <div className="flex flex-col gap-3">
            <CopyRow label="Pairing code" value={pairingCode || "…"} />
            <div className="glass rounded-2xl p-4">
              <div className="flex items-center gap-2 text-sm font-medium">
                <ShieldCheck className="h-4 w-4 text-accent" />
                Secure by default
              </div>
              <p className="mt-1.5 text-xs leading-relaxed text-[color:var(--color-muted)]">
                Traffic is end-to-end encrypted with TLS 1.3. Only devices that
                present this code (or your network passphrase) can connect.
              </p>
            </div>
            <Button
              variant="danger"
              size="lg"
              className="mt-auto"
              disabled={busy}
              onClick={onStopHost}
            >
              Stop hosting
            </Button>
          </div>
        </div>
      </motion.div>
    );
  }

  const canStart = name.trim().length >= 2 && !busy;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="mx-auto flex h-full max-w-xl flex-col justify-center"
    >
      <div className="glass-strong rounded-3xl p-7">
        <div className="ring-accent mb-5 grid h-12 w-12 place-items-center rounded-2xl bg-[color:var(--color-accent-dim)]">
          <RadioTower className="h-6 w-6 text-accent" />
        </div>
        <h1 className="text-2xl font-semibold tracking-tight">
          Host a network
        </h1>
        <p className="mt-1 text-sm text-[color:var(--color-muted)]">
          Turn this device into a VPN. Other devices route their traffic
          securely through your connection.
        </p>

        <div className="mt-6 space-y-4">
          <TextField
            label="Network name"
            placeholder="e.g. Home, Office, Cabin"
            value={name}
            maxLength={40}
            onChange={(e) => setName(e.target.value)}
          />
          <TextField
            label="Passphrase (optional)"
            type="password"
            placeholder="Required to find this network over the internet"
            value={passphrase}
            onChange={(e) => setPassphrase(e.target.value)}
            hint={
              <span className="flex items-center gap-1.5">
                <KeyRound className="h-3 w-3" />
                Devices type the same name + passphrase to discover you
                anywhere.
              </span>
            }
          />
        </div>

        <div className="mt-5 grid grid-cols-2 gap-3">
          <div className="glass rounded-xl px-3 py-2.5">
            <div className="flex items-center gap-2 text-xs font-medium">
              <Wifi className="h-3.5 w-3.5 text-accent" /> Same Wi‑Fi
            </div>
            <div className="mt-0.5 text-[11px] text-[color:var(--color-faint)]">
              Auto-discovered instantly
            </div>
          </div>
          <div className="glass rounded-xl px-3 py-2.5">
            <div className="flex items-center gap-2 text-xs font-medium">
              <Globe2 className="h-3.5 w-3.5 text-accent" /> Across internet
            </div>
            <div className="mt-0.5 text-[11px] text-[color:var(--color-faint)]">
              Name + passphrase, no server
            </div>
          </div>
        </div>

        <Button
          variant="primary"
          size="lg"
          className="mt-6 w-full"
          disabled={!canStart}
          onClick={() =>
            onStartHost({
              networkName: name.trim(),
              passphrase: passphrase.trim() || null,
            })
          }
        >
          {busy ? "Starting…" : "Start hosting"}
        </Button>
      </div>
    </motion.div>
  );
}
