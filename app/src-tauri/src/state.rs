//! Serializable state shared between the Rust backend and the React UI.
//!
//! All structs use `camelCase` so they map cleanly onto TypeScript.

use serde::{Deserialize, Serialize};

/// Which role this device is currently playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VpnRole {
    Idle,
    Host,
    Client,
}

/// The high-level connection state, surfaced to the UI as a state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    Idle,
    Hosting,
    Discovering,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

/// Live throughput / link statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub bytes_up: u64,
    pub bytes_down: u64,
    /// Instantaneous rate in bytes/second.
    pub rate_up: u64,
    pub rate_down: u64,
    pub latency_ms: Option<u32>,
    /// `true` when the peer link is a direct hole-punched connection,
    /// `false` when traffic is relayed.
    pub direct: bool,
    pub connected_secs: u64,
}

/// A full snapshot of the engine's status. Emitted on every transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub state: ConnectionState,
    pub role: VpnRole,
    pub network_name: Option<String>,
    pub endpoint_id: Option<String>,
    pub peer_endpoint_id: Option<String>,
    pub virtual_ip: Option<String>,
    pub public_ip: Option<String>,
    pub message: Option<String>,
    pub stats: Stats,
}

impl Default for StatusSnapshot {
    fn default() -> Self {
        Self {
            state: ConnectionState::Idle,
            role: VpnRole::Idle,
            network_name: None,
            endpoint_id: None,
            peer_endpoint_id: None,
            virtual_ip: None,
            public_ip: None,
            message: None,
            stats: Stats::default(),
        }
    }
}

/// A host that has been discovered on the LAN, via the DHT, or saved locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredHost {
    pub network_name: String,
    pub endpoint_id: String,
    /// "lan" | "dht" | "saved"
    pub source: String,
    pub requires_passphrase: bool,
    pub online: bool,
}

/// Configuration provided by the UI when starting to host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostConfig {
    pub network_name: String,
    pub passphrase: Option<String>,
    #[serde(default = "default_true")]
    pub full_tunnel: bool,
}

/// Configuration provided by the UI when connecting to a host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectConfig {
    pub network_name: String,
    pub passphrase: Option<String>,
    /// An out-of-band pairing ticket (base32 blob), if connecting by code.
    pub ticket: Option<String>,
    /// A raw endpoint id, if connecting directly.
    pub endpoint_id: Option<String>,
}

/// Persisted user settings (stored via the Tauri store plugin).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub autostart: bool,
    pub resume_hosting: bool,
    pub minimize_to_tray: bool,
    pub vpn_subnet: String,
    pub relay_url: Option<String>,
    pub last_network_name: Option<String>,
    pub was_hosting: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            autostart: false,
            resume_hosting: true,
            minimize_to_tray: true,
            vpn_subnet: "10.66.0.0/24".to_string(),
            relay_url: None,
            last_network_name: None,
            was_hosting: false,
        }
    }
}

fn default_true() -> bool {
    true
}

// --- public VPN mode -----------------------------------------------------
// A separate feature from the private peer-to-peer VPN: connect to free public
// VPN servers (volunteer-run, aggregated from open sources) by country.

/// A single public VPN server entry, surfaced to the UI. The OpenVPN config
/// itself is kept server-side (keyed by `id`) and never sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicServer {
    /// Stable unique id (source + host), used to start a connection.
    pub id: String,
    /// Which open source this entry came from (e.g. "vpngate").
    pub source: String,
    /// Full country name, e.g. "Japan".
    pub country: String,
    /// ISO-3166 alpha-2 code, e.g. "JP".
    pub country_code: String,
    pub hostname: String,
    pub ip: String,
    /// Round-trip latency in milliseconds, if known.
    pub ping_ms: Option<u32>,
    /// Throughput estimate in megabits per second, if known.
    pub speed_mbps: Option<f64>,
    /// Active sessions on the server (a load indicator), if known.
    pub sessions: Option<u32>,
    /// A relative quality score (higher is better), if known.
    pub score: Option<u64>,
}

/// The state of the public VPN connection (independent of the P2P engine).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PublicState {
    Idle,
    Connecting,
    Connected,
    Error,
}

/// Live status of the public VPN connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicStatus {
    pub state: PublicState,
    pub server_id: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub message: Option<String>,
    pub connected_secs: u64,
}

impl Default for PublicStatus {
    fn default() -> Self {
        Self {
            state: PublicState::Idle,
            server_id: None,
            country: None,
            country_code: None,
            message: None,
            connected_secs: 0,
        }
    }
}
