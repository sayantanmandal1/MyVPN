import { useState } from "react";
import { motion } from "framer-motion";
import type { ReactNode } from "react";
import {
  Power,
  RefreshCcw,
  PanelTopClose,
  Info,
  LogOut,
  Server,
  DownloadCloud,
} from "lucide-react";
import type { Settings, UpdateInfo } from "../lib/api";
import { Toggle } from "../components/ui/Toggle";
import { Button } from "../components/ui/Button";
import { TextField } from "../components/ui/TextField";

function Row({
  Icon,
  title,
  desc,
  control,
}: {
  Icon: typeof Power;
  title: string;
  desc: string;
  control: ReactNode;
}) {
  return (
    <div className="flex items-center gap-4 px-4 py-3.5">
      <span className="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-white/5">
        <Icon className="h-4 w-4 text-accent" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium">{title}</div>
        <div className="text-xs text-[color:var(--color-muted)]">{desc}</div>
      </div>
      <div className="shrink-0">{control}</div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div>
      <div className="mb-2 px-1 text-xs font-medium tracking-wide text-[color:var(--color-faint)] uppercase">
        {title}
      </div>
      <div className="glass divide-y divide-white/5 rounded-2xl">{children}</div>
    </div>
  );
}

export function SettingsScreen({
  settings,
  autostartEnabled,
  appVersion,
  onToggleAutostart,
  onChange,
  onCheckUpdate,
  onQuit,
}: {
  settings: Settings;
  autostartEnabled: boolean;
  appVersion: string;
  onToggleAutostart: (v: boolean) => void;
  onChange: (patch: Partial<Settings>) => void;
  onCheckUpdate: () => Promise<UpdateInfo | null>;
  onQuit: () => void;
}) {
  const [checking, setChecking] = useState(false);
  const [checkMsg, setCheckMsg] = useState<string | null>(null);
  const ver = appVersion || "\u2026";

  const runCheck = async () => {
    setChecking(true);
    setCheckMsg(null);
    const found = await onCheckUpdate();
    setCheckMsg(
      found
        ? `Update available: v${found.version}`
        : "You\u2019re on the latest version.",
    );
    setChecking(false);
  };
  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="mx-auto flex h-full max-w-2xl flex-col overflow-y-auto pr-1"
    >
      <header className="mb-5">
        <h1 className="text-2xl font-semibold tracking-tight">Settings</h1>
        <p className="mt-1 text-sm text-[color:var(--color-muted)]">
          Control startup behaviour, networking, and the app.
        </p>
      </header>

      <div className="space-y-5">
        <Section title="Startup & uptime">
          <Row
            Icon={Power}
            title="Start on system boot"
            desc="Launch MyVPN automatically every time Windows starts."
            control={
              <Toggle checked={autostartEnabled} onChange={onToggleAutostart} />
            }
          />
          <Row
            Icon={RefreshCcw}
            title="Resume hosting after restart"
            desc="If you were hosting, automatically re-host on launch for near-zero downtime."
            control={
              <Toggle
                checked={settings.resumeHosting}
                onChange={(v) => onChange({ resumeHosting: v })}
              />
            }
          />
          <Row
            Icon={PanelTopClose}
            title="Minimize to tray on close"
            desc="Keep running in the background instead of quitting."
            control={
              <Toggle
                checked={settings.minimizeToTray}
                onChange={(v) => onChange({ minimizeToTray: v })}
              />
            }
          />
        </Section>

        <Section title="Connection relay">
          <div className="space-y-3 px-4 py-4">
            <div className="flex items-start gap-4">
              <span className="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-white/5">
                <Server className="h-4 w-4 text-accent" />
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium">Self-hosted relay</div>
                <div className="text-xs text-[color:var(--color-muted)]">
                  By default MyVPN uses automatic public relays in several regions
                  to help your devices find each other and punch through NATs.
                  Point it at your own relay for full sovereignty.
                </div>
              </div>
            </div>
            <TextField
              value={settings.relayUrl ?? ""}
              onChange={(e) => onChange({ relayUrl: e.target.value || null })}
              placeholder="https://relay.example.com"
              spellCheck={false}
              autoCapitalize="off"
              autoCorrect="off"
              hint="Leave blank to use the default relays. The relay only assists the connection — your traffic always exits through the host device, never the relay."
            />
          </div>
        </Section>

        <Section title="About">
          <Row
            Icon={Info}
            title="MyVPN"
            desc={`Version ${ver} \u00b7 Serverless peer-to-peer VPN`}
            control={
              <span className="rounded-lg bg-[color:var(--color-accent-dim)] px-2.5 py-1 text-xs font-medium text-accent">
                v{ver}
              </span>
            }
          />
          <Row
            Icon={DownloadCloud}
            title="Check for updates"
            desc={checkMsg ?? "See if a newer version is available."}
            control={
              <Button variant="subtle" onClick={runCheck} disabled={checking}>
                {checking ? "Checking\u2026" : "Check"}
              </Button>
            }
          />
          <Row
            Icon={LogOut}
            title="Quit MyVPN"
            desc="Fully exit the app and stop all tunnels."
            control={
              <Button variant="danger" onClick={onQuit}>
                Quit
              </Button>
            }
          />
        </Section>
      </div>
    </motion.div>
  );
}
