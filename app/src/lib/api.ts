import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// ---- Types (mirror of the Rust `state` module) ----

export type ConnectionState =
  | "idle"
  | "hosting"
  | "discovering"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "error";

export type VpnRole = "idle" | "host" | "client";

export interface Stats {
  bytesUp: number;
  bytesDown: number;
  rateUp: number;
  rateDown: number;
  latencyMs: number | null;
  direct: boolean;
  connectedSecs: number;
}

export interface StatusSnapshot {
  state: ConnectionState;
  role: VpnRole;
  networkName: string | null;
  endpointId: string | null;
  peerEndpointId: string | null;
  virtualIp: string | null;
  publicIp: string | null;
  message: string | null;
  stats: Stats;
}

export interface DiscoveredHost {
  networkName: string;
  endpointId: string;
  source: "lan" | "dht" | "saved" | string;
  requiresPassphrase: boolean;
  online: boolean;
}

export interface Settings {
  autostart: boolean;
  resumeHosting: boolean;
  minimizeToTray: boolean;
  vpnSubnet: string;
  relayUrl: string | null;
  lastNetworkName: string | null;
  wasHosting: boolean;
}

export interface HostConfig {
  networkName: string;
  passphrase?: string | null;
  fullTunnel?: boolean;
}

export interface ConnectConfig {
  networkName: string;
  passphrase?: string | null;
  ticket?: string | null;
  endpointId?: string | null;
}

// ---- Public VPN mode (separate from the private P2P VPN) ----

export type PublicState = "idle" | "connecting" | "connected" | "error";

export interface PublicServer {
  id: string;
  source: string;
  country: string;
  countryCode: string;
  hostname: string;
  ip: string;
  pingMs: number | null;
  speedMbps: number | null;
  sessions: number | null;
  score: number | null;
}

export interface PublicStatus {
  state: PublicState;
  serverId: string | null;
  country: string | null;
  countryCode: string | null;
  message: string | null;
  connectedSecs: number;
}

export interface UpdateInfo {
  version: string;
  url: string;
}

// ---- Command wrappers ----

export const api = {
  getStatus: () => invoke<StatusSnapshot>("get_status"),
  listDiscovered: () => invoke<DiscoveredHost[]>("list_discovered"),
  startHost: (config: HostConfig) =>
    invoke<StatusSnapshot>("start_host", { config }),
  stopHost: () => invoke<void>("stop_host"),
  connect: (config: ConnectConfig) =>
    invoke<StatusSnapshot>("connect", { config }),
  disconnect: () => invoke<void>("disconnect"),
  generateTicket: () => invoke<string>("generate_ticket"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { settings }),
  setAutostart: (enabled: boolean) =>
    invoke<boolean>("set_autostart", { enabled }),
  getAutostart: () => invoke<boolean>("get_autostart"),
  showWindow: () => invoke<void>("show_window"),
  quitApp: () => invoke<void>("quit_app"),
  publicRefresh: () => invoke<number>("public_refresh"),
  publicServers: () => invoke<PublicServer[]>("public_servers"),
  publicConnect: (serverId: string) =>
    invoke<PublicStatus>("public_connect", { serverId }),
  publicDisconnect: () => invoke<void>("public_disconnect"),
  publicStatus: () => invoke<PublicStatus>("public_status"),
  checkUpdate: () => invoke<UpdateInfo | null>("check_update"),
  installUpdate: () => invoke<boolean>("install_update"),
};

// ---- Event subscriptions ----

export const events = {
  onStatus: (cb: (s: StatusSnapshot) => void): Promise<UnlistenFn> =>
    listen<StatusSnapshot>("vpn://status", (e) => cb(e.payload)),
  onStats: (cb: (s: Stats) => void): Promise<UnlistenFn> =>
    listen<Stats>("vpn://stats", (e) => cb(e.payload)),
  onLog: (cb: (msg: string) => void): Promise<UnlistenFn> =>
    listen<string>("vpn://log", (e) => cb(e.payload)),
  onPublicStatus: (cb: (s: PublicStatus) => void): Promise<UnlistenFn> =>
    listen<PublicStatus>("public://status", (e) => cb(e.payload)),
};

export const isActive = (s: ConnectionState) =>
  s === "connected" || s === "hosting" || s === "reconnecting";
