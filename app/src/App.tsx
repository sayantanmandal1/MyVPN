import { useCallback, useEffect, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { Download, X } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { getVersion } from "@tauri-apps/api/app";
import { Sidebar, type Route } from "./components/Sidebar";
import { Dashboard } from "./screens/Dashboard";
import { HostScreen } from "./screens/HostScreen";
import { ConnectScreen } from "./screens/ConnectScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { PublicScreen } from "./screens/PublicScreen";
import { Button } from "./components/ui/Button";
import {
  api,
  events,
  type DiscoveredHost,
  type PublicServer,
  type PublicStatus,
  type Settings,
  type StatusSnapshot,
  type UpdateInfo,
} from "./lib/api";

const HISTORY_LEN = 32;
const zeros = () => Array<number>(HISTORY_LEN).fill(0);

const IDLE_STATUS: StatusSnapshot = {
  state: "idle",
  role: "idle",
  networkName: null,
  endpointId: null,
  peerEndpointId: null,
  virtualIp: null,
  publicIp: null,
  message: null,
  stats: {
    bytesUp: 0,
    bytesDown: 0,
    rateUp: 0,
    rateDown: 0,
    latencyMs: null,
    direct: false,
    connectedSecs: 0,
  },
};

const DEFAULT_SETTINGS: Settings = {
  autostart: false,
  resumeHosting: true,
  minimizeToTray: true,
  vpnSubnet: "10.66.0.0/24",
  relayUrl: null,
  lastNetworkName: null,
  wasHosting: false,
};

const IDLE_PUBLIC: PublicStatus = {
  state: "idle",
  serverId: null,
  country: null,
  countryCode: null,
  message: null,
  connectedSecs: 0,
};

function App() {
  const [route, setRoute] = useState<Route>("dashboard");
  const [status, setStatus] = useState<StatusSnapshot>(IDLE_STATUS);
  const [historyUp, setHistoryUp] = useState<number[]>(zeros());
  const [historyDown, setHistoryDown] = useState<number[]>(zeros());
  const [logs, setLogs] = useState<string[]>([]);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [discovered, setDiscovered] = useState<DiscoveredHost[]>([]);
  const [busy, setBusy] = useState(false);
  const [publicStatus, setPublicStatus] = useState<PublicStatus>(IDLE_PUBLIC);
  const [publicServers, setPublicServers] = useState<PublicServer[]>([]);
  const [publicLoading, setPublicLoading] = useState(false);
  const [publicError, setPublicError] = useState<string | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updating, setUpdating] = useState(false);
  const [appVersion, setAppVersion] = useState("");

  const pushLog = useCallback((msg: string) => {
    const line = `${new Date().toLocaleTimeString()}  ${msg}`;
    setLogs((prev) => [...prev.slice(-39), line]);
  }, []);

  // initial load + event subscriptions
  const mounted = useRef(false);
  const publicTried = useRef(false);
  useEffect(() => {
    if (mounted.current) return;
    mounted.current = true;

    api.getStatus().then(setStatus).catch(() => {});
    api.getSettings().then(setSettings).catch(() => {});
    api.getAutostart().then(setAutostartEnabled).catch(() => {});
    api.listDiscovered().then(setDiscovered).catch(() => {});
    api.publicStatus().then(setPublicStatus).catch(() => {});
    api.checkUpdate().then(setUpdate).catch(() => {});
    getVersion().then(setAppVersion).catch(() => {});

    const unsubs = [
      events.onStatus((s) => {
        setStatus(s);
        if (s.state === "idle") {
          setHistoryUp(zeros());
          setHistoryDown(zeros());
        }
      }),
      events.onStats((s) => {
        setStatus((prev) => ({ ...prev, stats: s }));
        setHistoryUp((h) => [...h.slice(1), s.rateUp]);
        setHistoryDown((h) => [...h.slice(1), s.rateDown]);
      }),
      events.onLog((msg) => pushLog(msg)),
      events.onPublicStatus((s) => setPublicStatus(s)),
    ];

    return () => {
      unsubs.forEach((p) => p.then((fn) => fn()).catch(() => {}));
    };
  }, [pushLog]);

  // refresh discovered hosts when opening the connect screen
  useEffect(() => {
    if (route === "connect") {
      api.listDiscovered().then(setDiscovered).catch(() => {});
    }
  }, [route]);

  // lazy-load the public server list the first time that tab is opened
  useEffect(() => {
    if (route === "public" && !publicTried.current) {
      publicTried.current = true;
      setPublicLoading(true);
      api
        .publicServers()
        .then((s) => {
          setPublicServers(s);
          setPublicError(null);
        })
        .catch((e) => setPublicError(String(e)))
        .finally(() => setPublicLoading(false));
    }
  }, [route]);

  // tick the public uptime while connected
  useEffect(() => {
    if (publicStatus.state !== "connected") return;
    const t = setInterval(() => {
      api.publicStatus().then(setPublicStatus).catch(() => {});
    }, 1000);
    return () => clearInterval(t);
  }, [publicStatus.state]);

  const run = useCallback(
    async (fn: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await fn();
      } catch (e) {
        pushLog(`Error: ${String(e)}`);
      } finally {
        setBusy(false);
      }
    },
    [pushLog],
  );

  const handleStartHost = (cfg: {
    networkName: string;
    passphrase?: string | null;
  }) =>
    run(async () => {
      const s = await api.startHost({ ...cfg, fullTunnel: true });
      setStatus(s);
    });

  const handleStopHost = () => run(() => api.stopHost());

  const handleConnect = (cfg: {
    networkName: string;
    passphrase?: string | null;
    ticket?: string | null;
    endpointId?: string | null;
  }) =>
    run(async () => {
      setRoute("dashboard");
      const s = await api.connect(cfg);
      setStatus(s);
    });

  const handleDisconnect = () => run(() => api.disconnect());

  const handlePublicRefresh = () =>
    run(async () => {
      setPublicLoading(true);
      try {
        await api.publicRefresh();
        setPublicServers(await api.publicServers());
        setPublicError(null);
      } catch (e) {
        setPublicError(String(e));
      } finally {
        setPublicLoading(false);
      }
    });

  const handlePublicConnect = (serverId: string) =>
    run(async () => {
      const s = await api.publicConnect(serverId);
      setPublicStatus(s);
    });

  const handlePublicDisconnect = () => run(() => api.publicDisconnect());

  const handleRefresh = () =>
    run(async () => setDiscovered(await api.listDiscovered()));

  const handleToggleAutostart = (v: boolean) =>
    run(async () => {
      const enabled = await api.setAutostart(v);
      setAutostartEnabled(enabled);
      setSettings((prev) => ({ ...prev, autostart: enabled }));
      pushLog(enabled ? "Start on boot enabled" : "Start on boot disabled");
    });

  const handleChangeSettings = (patch: Partial<Settings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...patch };
      api.saveSettings(next).catch(() => {});
      return next;
    });
  };

  const handleQuit = () => api.quitApp();

  // Manual update check (from Settings). Returns the update so the caller can
  // show inline feedback; also surfaces the top banner when one is found.
  const handleCheckUpdate = useCallback(async (): Promise<UpdateInfo | null> => {
    const found = await api.checkUpdate().catch(() => null);
    setUpdate(found);
    return found;
  }, []);

  return (
    <div className="flex h-screen gap-4 p-4">
      <Sidebar
        route={route}
        onNavigate={setRoute}
        state={status.state}
        role={status.role}
      />
      <main className="flex min-w-0 flex-1 flex-col overflow-hidden">
        {update && (
          <div className="mb-3 flex items-center gap-3 rounded-2xl border border-accent/25 bg-[color:var(--color-accent-dim)] px-4 py-2.5">
            <Download className="h-4 w-4 shrink-0 text-accent" />
            <div className="min-w-0 flex-1 text-sm">
              <span className="font-medium">MyVPN {update.version}</span>
              <span className="text-[color:var(--color-muted)]"> is available.</span>
            </div>
            <Button
              size="sm"
              variant="primary"
              disabled={updating}
              onClick={async () => {
                setUpdating(true);
                try {
                  // Installs the signed update and relaunches when configured.
                  await api.installUpdate();
                } catch {
                  // fall through to a manual download
                }
                // Reached only if the app didn't relaunch (updater not ready).
                openUrl(update.url).catch(() => {});
                setUpdating(false);
              }}
            >
              {updating ? "Updating\u2026" : "Update"}
            </Button>
            <button
              aria-label="Dismiss update notice"
              onClick={() => setUpdate(null)}
              className="rounded-lg p-1 text-[color:var(--color-muted)] transition hover:bg-white/10 hover:text-ink"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        )}
        <div className="min-h-0 flex-1">
          <AnimatePresence mode="wait">
            {route === "dashboard" && (
              <Dashboard
                key="dashboard"
                status={status}
                historyUp={historyUp}
                historyDown={historyDown}
                logs={logs}
                busy={busy}
                onNavigate={setRoute}
                onDisconnect={handleDisconnect}
                onStopHost={handleStopHost}
              />
            )}
            {route === "host" && (
              <HostScreen
                key="host"
                status={status}
                busy={busy}
                onStartHost={handleStartHost}
                onStopHost={handleStopHost}
              />
            )}
            {route === "connect" && (
              <ConnectScreen
                key="connect"
                status={status}
                discovered={discovered}
                busy={busy}
                onConnect={handleConnect}
                onRefresh={handleRefresh}
                onDisconnect={handleDisconnect}
              />
            )}
            {route === "public" && (
              <PublicScreen
                key="public"
                servers={publicServers}
                status={publicStatus}
                loading={publicLoading}
                error={publicError}
                busy={busy}
                onRefresh={handlePublicRefresh}
                onConnect={handlePublicConnect}
                onDisconnect={handlePublicDisconnect}
              />
            )}
            {route === "settings" && (
              <SettingsScreen
                key="settings"
                settings={settings}
                autostartEnabled={autostartEnabled}
                appVersion={appVersion}
                onToggleAutostart={handleToggleAutostart}
                onChange={handleChangeSettings}
                onCheckUpdate={handleCheckUpdate}
                onQuit={handleQuit}
              />
            )}
          </AnimatePresence>
        </div>
      </main>
    </div>
  );
}

export default App;

