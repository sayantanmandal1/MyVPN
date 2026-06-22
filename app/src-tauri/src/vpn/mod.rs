//! The VPN engine: owns runtime status, drives the connection state machine,
//! and emits live events to the UI.
//!
//! The networking internals (iroh transport, Wintun data plane, host NAT
//! gateway, and discovery) are introduced in later phases behind the methods
//! below. This module keeps the public surface stable so those pieces can be
//! slotted in without touching the command layer or the UI.

mod discovery;
mod iroh_transport;
mod net;
mod tun;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use iroh::endpoint::Connection;
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use crate::error::{Result, VpnError};
use crate::state::*;
use iroh_transport::IrohNode;
pub use iroh_transport::hash_pass;

/// Event channels consumed by the frontend.
pub const EVT_STATUS: &str = "vpn://status";
pub const EVT_STATS: &str = "vpn://stats";
pub const EVT_DISCOVERED: &str = "vpn://discovered";
pub const EVT_LOG: &str = "vpn://log";

// Tunnel addressing.
const VPN_IF: &str = "MyVPN";
const HOST_IP: &str = "10.66.0.1";
const CLIENT_IP: &str = "10.66.0.2";
const SUBNET: &str = "10.66.0.0/24";
const NAT_NAME: &str = "MyVPN";
const TUN_PREFIX: u8 = 24;
const TUN_MTU: u32 = 1280;
const TUN_DNS: &str = "1.1.1.1";

type Task = tauri::async_runtime::JoinHandle<()>;

#[derive(Default)]
struct Inner {
    snapshot: StatusSnapshot,
    ticker: Option<Task>,
    net_tasks: Vec<Task>,
    node: Option<IrohNode>,
    tun: Option<tun::Tun>,
    bypass_ips: Vec<String>,
    nat_name: Option<String>,
    /// Prior smart-multihomed-DNS policy value to restore on teardown; the outer
    /// `Some` means client DNS-leak hardening is currently active.
    dns_guard: Option<Option<u32>>,
    up: Arc<AtomicU64>,
    down: Arc<AtomicU64>,
    rtt_ms: Arc<AtomicU64>,
    started_at: Option<Instant>,
}

/// The central engine. Cloneable handle is shared via Tauri managed state.
pub struct VpnEngine {
    app: AppHandle,
    key_dir: PathBuf,
    inner: Arc<Mutex<Inner>>,
    lan_hosts: discovery::HostTable,
    current_client: iroh_transport::ClientSlot,
    /// Optional self-hosted relay URL; `None` uses iroh's default relays.
    relay: Arc<Mutex<Option<String>>>,
}

impl VpnEngine {
    pub fn new(app: AppHandle, key_dir: PathBuf) -> Self {
        let lan_hosts: discovery::HostTable =
            Arc::new(Mutex::new(std::collections::HashMap::new()));
        // Listen for LAN host beacons for the lifetime of the app (detached).
        drop(discovery::spawn_listener(app.clone(), lan_hosts.clone()));
        Self {
            app,
            key_dir,
            inner: Arc::new(Mutex::new(Inner::default())),
            lan_hosts,
            current_client: Arc::new(Mutex::new(None)),
            relay: Arc::new(Mutex::new(None)),
        }
    }

    /// Set the self-hosted relay URL used for future host/connect sessions.
    /// `None` or an empty/whitespace value restores iroh's default relays.
    pub fn set_relay_url(&self, url: Option<String>) {
        let normalized = url
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty());
        *self.relay.lock() = normalized;
    }

    fn identity_path(&self) -> PathBuf {
        self.key_dir.join("identity.key")
    }

    /// Current status snapshot.
    pub fn snapshot(&self) -> StatusSnapshot {
        self.inner.lock().snapshot.clone()
    }

    /// Currently discovered hosts.
    pub async fn list_discovered(&self) -> Vec<DiscoveredHost> {
        discovery::snapshot(&self.lan_hosts)
    }

    /// Begin hosting a network. `expected_proof` is the passphrase proof hash
    /// (or None for an open network).
    pub async fn start_host(
        &self,
        network_name: String,
        expected_proof: Option<String>,
    ) -> Result<StatusSnapshot> {
        self.teardown().await;

        // A passphrase-protected network binds a deterministic identity derived
        // from (name, proof), so remote devices can find it by name + passphrase
        // through iroh's global discovery (no pairing code needed). An open
        // network keeps this device's stable, persisted identity.
        let secret = match expected_proof.as_deref() {
            Some(proof) => iroh_transport::derive_secret(&network_name, proof),
            None => {
                iroh_transport::load_or_create_secret(&self.identity_path()).map_err(to_err)?
            }
        };
        let relay = self.relay.lock().clone();
        let node = IrohNode::bind_host(secret, relay).await.map_err(to_err)?;
        let endpoint_id = node.id_string();

        // Bring up the host gateway adapter + NAT (best-effort; needs admin).
        let host_session = self.setup_host_gateway();
        let data_plane = host_session.is_some();
        let down = self.inner.lock().down.clone();
        let rtt = self.inner.lock().rtt_ms.clone();

        {
            let mut g = self.inner.lock();
            g.node = Some(node.clone());
            g.started_at = Some(Instant::now());
            g.snapshot = StatusSnapshot {
                state: ConnectionState::Hosting,
                role: VpnRole::Host,
                network_name: Some(network_name.clone()),
                endpoint_id: Some(endpoint_id),
                virtual_ip: Some(HOST_IP.to_string()),
                message: Some(if data_plane {
                    "Hosting — waiting for peers to connect".to_string()
                } else {
                    "Hosting (control-plane only — the VPN driver is unavailable; reinstall MyVPN)"
                        .to_string()
                }),
                ..Default::default()
            };
        }

        self.log(format!("Now hosting network \"{network_name}\""));
        let _ = self.app.emit(EVT_STATUS, self.snapshot());

        // Announce on the LAN so same-network devices find us by name.
        if let Some(id) = self.inner.lock().snapshot.endpoint_id.clone() {
            let beacon =
                discovery::spawn_beacon(network_name.clone(), id, expected_proof.is_some());
            self.inner.lock().net_tasks.push(beacon);
        }
        // Accept loop: publish to discovery, then serve incoming peers.
        let app = self.app.clone();
        let inner = self.inner.clone();
        let host_name = network_name.clone();
        let current_client = self.current_client.clone();
        let online_msg: &str = if data_plane {
            "Hosting — online and reachable"
        } else {
            "Hosting (control-plane only — the VPN driver is unavailable; reinstall MyVPN)"
        };
        let accept = tauri::async_runtime::spawn(async move {
            node.wait_online().await;
            let _ = app.emit(EVT_LOG, "Discovery published — reachable over the internet");
            {
                let mut g = inner.lock();
                if matches!(g.snapshot.state, ConnectionState::Hosting) {
                    g.snapshot.message = Some(online_msg.to_string());
                }
            }
            let snap = inner.lock().snapshot.clone();
            let _ = app.emit(EVT_STATUS, snap);

            while let Some(conn) = node.accept().await {
                let app2 = app.clone();
                let inner2 = inner.clone();
                let hn = host_name.clone();
                let session = host_session.clone();
                let proof = expected_proof.clone();
                let slot = current_client.clone();
                let down = down.clone();
                let rtt = rtt.clone();
                {
                    let mut g = inner.lock();
                    g.snapshot.message = Some("Peer connected".to_string());
                }
                let snap = inner.lock().snapshot.clone();
                let _ = app.emit(EVT_STATUS, snap);
                let _ = app.emit(EVT_LOG, "A peer connected to your network");
                tauri::async_runtime::spawn(async move {
                    if let Err(e) =
                        iroh_transport::host_serve(conn, hn, proof, session, slot, down, rtt)
                            .await
                    {
                        tracing::warn!("peer connection ended: {e}");
                    }
                    {
                        let mut g = inner2.lock();
                        if matches!(g.snapshot.state, ConnectionState::Hosting) {
                            g.snapshot.message = Some(online_msg.to_string());
                        }
                    }
                    let _ = app2.emit(EVT_LOG, "A peer disconnected");
                });
            }
        });
        self.inner.lock().net_tasks.push(accept);

        self.start_ticker();
        Ok(self.snapshot())
    }

    /// Create the host's TUN adapter and NAT so client traffic egresses
    /// through this machine. Returns the session if the data plane came up.
    fn setup_host_gateway(&self) -> Option<Arc<wintun::Session>> {
        match tun::Tun::create(VPN_IF, HOST_IP, TUN_PREFIX, TUN_MTU) {
            Ok(t) => {
                if let Err(e) = net::enable_forwarding(VPN_IF) {
                    self.log(format!("Enable forwarding failed: {e}"));
                }
                if let Err(e) = net::create_nat(NAT_NAME, SUBNET) {
                    self.log(format!("NAT setup failed: {e}"));
                }
                let session = t.session();
                let up = self.inner.lock().up.clone();
                iroh_transport::spawn_session_reader(
                    session.clone(),
                    self.current_client.clone(),
                    up,
                );
                let mut g = self.inner.lock();
                g.nat_name = Some(NAT_NAME.to_string());
                g.tun = Some(t);
                Some(session)
            }
            Err(e) => {
                self.log(format!(
                    "Tunnel adapter unavailable ({e}); hosting control-plane only."
                ));
                None
            }
        }
    }

    /// Create the client TUN, install full-tunnel routes, set DNS, and start a
    /// task that keeps carrier-bypass routes up to date (loop-avoidance).
    fn setup_client_tunnel(&self, conn: &Connection) -> bool {
        let tun = match tun::Tun::create(VPN_IF, CLIENT_IP, TUN_PREFIX, TUN_MTU) {
            Ok(t) => t,
            Err(e) => {
                self.log(format!(
                    "Tunnel adapter unavailable ({e}); connected control-plane only."
                ));
                return false;
            }
        };

        // Clamp the tunnel MTU so no IP packet exceeds the QUIC datagram size.
        if let Some(max) = iroh_transport::max_datagram(conn) {
            let mtu = (max as u32).min(TUN_MTU);
            if mtu < TUN_MTU {
                let _ = net::set_mtu(VPN_IF, mtu);
            }
        }

        if let Some((gw, if_index)) = net::default_gateway() {
            // Pin the current carrier addresses to the physical gateway first,
            // so installing the default route doesn't break iroh's own link.
            let mut bypass = Vec::new();
            for addr in iroh_transport::remote_socket_addrs(conn) {
                let ip = addr.ip().to_string();
                if net::add_bypass_route(&ip, &gw, if_index).is_ok() {
                    bypass.push(ip);
                }
            }
            if let Err(e) = net::add_full_tunnel_routes(VPN_IF, HOST_IP) {
                self.log(format!("Route setup failed: {e}"));
            }
            net::add_ipv6_killswitch(VPN_IF);
            // Force every DNS lookup through the tunnel resolver (no leaks to the
            // local network), remembering the prior policy to restore on teardown.
            let prior = net::harden_client_dns(VPN_IF, TUN_DNS);
            {
                let mut g = self.inner.lock();
                g.bypass_ips = bypass;
                g.dns_guard = Some(prior);
            }
        } else {
            self.log("Could not determine the default gateway; full tunnel disabled.");
        }

        self.inner.lock().tun = Some(tun);
        true
    }

    /// Stop hosting and tear down the gateway.
    pub async fn stop_host(&self) -> Result<()> {
        self.teardown().await;
        self.commit(StatusSnapshot {
            message: Some("Stopped hosting".to_string()),
            ..Default::default()
        });
        self.log("Stopped hosting");
        Ok(())
    }

    /// Connect to a host as a client (full-tunnel).
    pub async fn connect(&self, cfg: ConnectConfig) -> Result<StatusSnapshot> {
        match self.try_connect(cfg).await {
            Ok(snap) => Ok(snap),
            Err(e) => {
                // Never leave the UI stuck on "Connecting…": tear down any partial
                // setup and surface a clear error state.
                self.teardown().await;
                self.commit(StatusSnapshot {
                    state: ConnectionState::Error,
                    message: Some(e.to_string()),
                    ..Default::default()
                });
                self.log(format!("Connection failed: {e}"));
                Err(e)
            }
        }
    }

    async fn try_connect(&self, cfg: ConnectConfig) -> Result<StatusSnapshot> {
        self.teardown().await;
        let name = cfg.network_name.clone();

        self.commit(StatusSnapshot {
            state: ConnectionState::Connecting,
            role: VpnRole::Client,
            network_name: Some(name.clone()),
            message: Some("Establishing encrypted tunnel…".to_string()),
            ..Default::default()
        });
        self.log(format!("Connecting to \"{name}\""));

        // The passphrase proof authenticates us and (for name-based discovery)
        // derives the host's deterministic identity.
        let proof = cfg
            .proof
            .clone()
            .filter(|p| !p.is_empty())
            .or_else(|| {
                cfg.passphrase
                    .as_deref()
                    .filter(|p| !p.is_empty())
                    .map(iroh_transport::hash_pass)
            });

        // Resolve the dial target: an explicit pairing code wins; otherwise try
        // same-LAN discovery by name; otherwise, with a passphrase, derive the
        // host's deterministic identity so it can be found over the internet.
        let target = match resolve_target(&cfg) {
            Ok(t) => t,
            Err(e) => {
                if let Some(id) = discovery::find_by_name(&self.lan_hosts, &name) {
                    id
                } else if let Some(p) = proof.as_deref() {
                    iroh_transport::derived_endpoint_id(&name, p)
                } else {
                    return Err(e);
                }
            }
        };

        let secret =
            iroh_transport::load_or_create_secret(&self.identity_path()).map_err(to_err)?;
        let relay = self.relay.lock().clone();
        let node = IrohNode::bind_client(secret, relay).await.map_err(to_err)?;
        let conn = node.connect(&target).await.map_err(to_err)?;
        let message = iroh_transport::client_handshake(&conn, &name, proof.clone())
            .await
            .map_err(to_err)?;

        // Bring up the client TUN, route all traffic through it, and pin the
        // carrier (iroh) traffic to the physical gateway to avoid a loop.
        let tunnel_up = self.setup_client_tunnel(&conn);

        let down = self.inner.lock().down.clone();

        {
            let mut g = self.inner.lock();
            g.node = Some(node.clone());
            g.started_at = Some(Instant::now());
            g.snapshot.state = ConnectionState::Connected;
            g.snapshot.peer_endpoint_id = Some(target.clone());
            g.snapshot.virtual_ip = Some(CLIENT_IP.to_string());
            g.snapshot.public_ip = Some("routed via host".to_string());
            g.snapshot.message = Some(if tunnel_up {
                message
            } else {
                "Connected (control-plane only — the VPN driver is unavailable; reinstall MyVPN)"
                    .to_string()
            });
        }

        // One persistent reader pulls packets from the client TUN and forwards
        // them to whichever connection is currently active (swapped on reconnect).
        let session_opt = self.inner.lock().tun.as_ref().map(|t| t.session());
        if let Some(session) = session_opt.clone() {
            let up = self.inner.lock().up.clone();
            iroh_transport::spawn_session_reader(session, self.current_client.clone(), up);
        }

        let snap = self.snapshot();
        let _ = self.app.emit(EVT_STATUS, snap.clone());
        self.log("Tunnel established");

        // Supervise the connection: pump packets, and on an unexpected drop keep
        // the kill-switch engaged (routes stay) while transparently reconnecting.
        let supervisor = spawn_client_supervisor(
            conn,
            node,
            target,
            name,
            proof,
            session_opt,
            self.current_client.clone(),
            down,
            self.inner.clone(),
            self.app.clone(),
        );
        self.inner.lock().net_tasks.push(supervisor);

        self.start_ticker();
        Ok(snap)
    }

    /// Disconnect from the current host.
    pub async fn disconnect(&self) -> Result<()> {
        self.teardown().await;
        self.commit(StatusSnapshot {
            message: Some("Disconnected".to_string()),
            ..Default::default()
        });
        self.log("Disconnected");
        Ok(())
    }

    /// Produce an out-of-band pairing ticket for the currently hosted network.
    pub async fn generate_ticket(&self) -> Result<String> {
        let g = self.inner.lock();
        g.snapshot
            .endpoint_id
            .clone()
            .ok_or_else(|| VpnError::msg("Not currently hosting"))
    }

    // --- internal helpers -------------------------------------------------

    fn commit(&self, snap: StatusSnapshot) {
        {
            self.inner.lock().snapshot = snap.clone();
        }
        let _ = self.app.emit(EVT_STATUS, snap);
    }

    fn log(&self, msg: impl Into<String>) {
        let msg = msg.into();
        tracing::info!("{msg}");
        let _ = self.app.emit(EVT_LOG, msg);
    }

    fn stop_ticker(&self) {
        if let Some(h) = self.inner.lock().ticker.take() {
            h.abort();
        }
    }

    /// Stop the stats ticker, abort all networking tasks, revert routing/NAT,
    /// tear down the TUN adapter, and close the node.
    async fn teardown(&self) {
        self.stop_ticker();
        let (node, tasks, tun, bypass, nat, dns_guard) = {
            let mut g = self.inner.lock();
            g.up.store(0, Ordering::Relaxed);
            g.down.store(0, Ordering::Relaxed);
            g.rtt_ms.store(0, Ordering::Relaxed);
            (
                g.node.take(),
                std::mem::take(&mut g.net_tasks),
                g.tun.take(),
                std::mem::take(&mut g.bypass_ips),
                g.nat_name.take(),
                g.dns_guard.take(),
            )
        };
        for t in tasks {
            t.abort();
        }
        // Stop routing return traffic to a now-defunct client.
        *self.current_client.lock() = None;
        // Revert networking changes (each is a no-op if never applied).
        net::remove_full_tunnel_routes(VPN_IF);
        net::remove_ipv6_killswitch(VPN_IF);
        match dns_guard {
            Some(prior) => net::restore_client_dns(VPN_IF, prior),
            None => net::reset_dns(VPN_IF),
        }
        for ip in bypass {
            net::remove_bypass_route(&ip);
        }
        if let Some(name) = nat {
            net::remove_nat(&name);
        }
        if let Some(tun) = tun {
            tun.shutdown();
        }
        if let Some(node) = node {
            node.close().await;
        }
    }

    /// Emit live throughput stats once per second, computed from the real
    /// number of bytes pumped across the tunnel.
    fn start_ticker(&self) {
        let inner = self.inner.clone();
        let app = self.app.clone();
        let (up, down, rtt_ms) = {
            let g = inner.lock();
            (g.up.clone(), g.down.clone(), g.rtt_ms.clone())
        };
        let handle = tauri::async_runtime::spawn(async move {
            let mut last_up = up.load(Ordering::Relaxed);
            let mut last_down = down.load(Ordering::Relaxed);
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let cur_up = up.load(Ordering::Relaxed);
                let cur_down = down.load(Ordering::Relaxed);
                let rate_up = cur_up.saturating_sub(last_up);
                let rate_down = cur_down.saturating_sub(last_down);
                last_up = cur_up;
                last_down = cur_down;

                let stats = {
                    let mut g = inner.lock();
                    let active = matches!(
                        g.snapshot.state,
                        ConnectionState::Connected | ConnectionState::Hosting
                    );
                    g.snapshot.stats.connected_secs =
                        g.started_at.map(|t| t.elapsed().as_secs()).unwrap_or(0);
                    g.snapshot.stats.bytes_up = cur_up;
                    g.snapshot.stats.bytes_down = cur_down;
                    g.snapshot.stats.rate_up = rate_up;
                    g.snapshot.stats.rate_down = rate_down;
                    g.snapshot.stats.direct = active;
                    let rtt = rtt_ms.load(Ordering::Relaxed);
                    g.snapshot.stats.latency_ms =
                        if active && rtt > 0 { Some(rtt as u32) } else { None };
                    g.snapshot.stats.clone()
                };
                if app.emit(EVT_STATS, stats).is_err() {
                    break;
                }
            }
        });
        self.inner.lock().ticker = Some(handle);
    }
}

/// Periodically pin iroh's current carrier addresses to the physical gateway,
/// so the full-tunnel default route never swallows the tunnel's own traffic.
/// Follows network changes (Wi-Fi <-> Ethernet, new DHCP lease) by re-pinning
/// known carrier addresses to the new gateway. The (relatively expensive)
/// gateway lookup only runs when there is a new address to pin or periodically,
/// so a steady connection costs almost nothing.
fn spawn_bypass_refresher(conn: Connection, inner: Arc<Mutex<Inner>>) -> Task {
    tauri::async_runtime::spawn(async move {
        let mut last_gw: Option<(String, u32)> = None;
        let mut tick: u32 = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(3)).await;
            if conn.close_reason().is_some() {
                break;
            }
            tick = tick.wrapping_add(1);
            let known: Vec<String> = inner.lock().bypass_ips.clone();
            let current: Vec<String> = iroh_transport::remote_socket_addrs(&conn)
                .iter()
                .map(|a| a.ip().to_string())
                .collect();
            let has_new = current.iter().any(|ip| !known.contains(ip));
            // Only shell out for the gateway when there's a new address to pin or
            // periodically (~30s) to catch a silent network change.
            if !has_new && tick < 10 {
                continue;
            }
            tick = 0;
            let Some((gw, idx)) = net::default_gateway() else {
                continue;
            };
            // Network changed: re-pin every known carrier address to the new
            // gateway so the tunnel's own QUIC traffic still bypasses the tunnel.
            if last_gw.as_ref() != Some(&(gw.clone(), idx)) {
                for ip in &known {
                    net::remove_bypass_route(ip);
                    let _ = net::add_bypass_route(ip, &gw, idx);
                }
                last_gw = Some((gw.clone(), idx));
            }
            let mut added = Vec::new();
            for ip in current {
                if !known.contains(&ip) && net::add_bypass_route(&ip, &gw, idx).is_ok() {
                    added.push(ip);
                }
            }
            if !added.is_empty() {
                inner.lock().bypass_ips.extend(added);
            }
        }
    })
}

/// Supervise a client connection for its whole lifetime: pump packets, and on an
/// unexpected drop keep the kill-switch engaged (the full-tunnel routes stay
/// installed so traffic is blocked, never leaked) while transparently dialing the
/// host again with capped exponential backoff. The single persistent TUN reader
/// is swapped onto each new connection via the shared slot, so reconnecting never
/// leaks reader threads.
#[allow(clippy::too_many_arguments)]
fn spawn_client_supervisor(
    mut conn: Connection,
    node: IrohNode,
    target: String,
    name: String,
    proof: Option<String>,
    session: Option<Arc<wintun::Session>>,
    slot: iroh_transport::ClientSlot,
    down: Arc<AtomicU64>,
    inner: Arc<Mutex<Inner>>,
    app: AppHandle,
) -> Task {
    tauri::async_runtime::spawn(async move {
        let rtt_ms = inner.lock().rtt_ms.clone();
        loop {
            // Attach the data plane for the current connection. The persistent
            // reader forwards TUN packets to this connection via the slot.
            let (sender, writer, refresher) = match session.clone() {
                Some(s) => {
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
                    *slot.lock() = Some((0, tx));
                    let refresher = Some(spawn_bypass_refresher(conn.clone(), inner.clone()));
                    (
                        Some(iroh_transport::spawn_datagram_sender(conn.clone(), rx)),
                        Some(iroh_transport::spawn_tun_writer(
                            conn.clone(),
                            s,
                            down.clone(),
                        )),
                        refresher,
                    )
                }
                None => (None, None, None),
            };
            let sampler = iroh_transport::spawn_rtt_sampler(conn.clone(), rtt_ms.clone());

            // Block until this connection ends.
            let _ = conn.closed().await;
            if let Some(t) = sender {
                t.abort();
            }
            if let Some(t) = writer {
                t.abort();
            }
            if let Some(t) = refresher {
                t.abort();
            }
            sampler.abort();

            // If the engine has moved on (the user disconnected — teardown also
            // aborts this task), stop supervising.
            {
                let g = inner.lock();
                if !matches!(
                    g.snapshot.state,
                    ConnectionState::Connected | ConnectionState::Reconnecting
                ) {
                    break;
                }
            }

            // Engage reconnect; the kill-switch stays in place during the gap.
            {
                let mut g = inner.lock();
                g.snapshot.state = ConnectionState::Reconnecting;
                g.snapshot.stats.direct = false;
                g.snapshot.message = Some("Connection lost — reconnecting…".to_string());
            }
            rtt_ms.store(0, Ordering::Relaxed);
            let snap = inner.lock().snapshot.clone();
            let _ = app.emit(EVT_STATUS, snap);
            let _ = app.emit(EVT_LOG, "Connection lost — reconnecting…");

            // Re-dial the host with capped exponential backoff until it answers.
            // The existing carrier-bypass routes keep the relay reachable while
            // the default route still points at the (down) tunnel.
            let mut delay = Duration::from_secs(1);
            let mut last_gw: Option<(String, u32)> = None;
            loop {
                tokio::time::sleep(delay).await;
                // Follow network changes during the outage: keep carrier bypass
                // routes pointed at the current gateway so the re-dial can reach
                // the relay even if the user switched networks.
                if let Some((gw, idx)) = net::default_gateway() {
                    if last_gw.as_ref() != Some(&(gw.clone(), idx)) {
                        let known: Vec<String> = inner.lock().bypass_ips.clone();
                        for ip in &known {
                            net::remove_bypass_route(ip);
                            let _ = net::add_bypass_route(ip, &gw, idx);
                        }
                        last_gw = Some((gw, idx));
                    }
                }
                if let Ok(c) = node.connect(&target).await {
                    if iroh_transport::client_handshake(&c, &name, proof.clone())
                        .await
                        .is_ok()
                    {
                        conn = c;
                        break;
                    }
                }
                delay = (delay * 2).min(Duration::from_secs(15));
            }

            {
                let mut g = inner.lock();
                g.snapshot.state = ConnectionState::Connected;
                g.snapshot.message = Some("Reconnected".to_string());
            }
            let snap = inner.lock().snapshot.clone();
            let _ = app.emit(EVT_STATUS, snap);
            let _ = app.emit(EVT_LOG, "Reconnected");
        }
    })
}

/// Resolve a connect request into a target pairing code / endpoint id.
fn resolve_target(cfg: &ConnectConfig) -> Result<String> {
    if let Some(id) = cfg.endpoint_id.as_ref() {
        if !id.trim().is_empty() {
            return Ok(id.trim().to_string());
        }
    }
    if let Some(ticket) = cfg.ticket.as_ref() {
        let t = ticket.trim();
        // Accept either a bare endpoint id or a "myvpn:<name>:<id>" form.
        let id = t
            .strip_prefix("myvpn:")
            .map_or(t, |rest| rest.rsplit(':').next().unwrap_or(rest));
        if !id.is_empty() {
            return Ok(id.to_string());
        }
    }
    Err(VpnError::msg(
        "Enter the host's pairing code, or the network name together with its \
         passphrase, to connect.",
    ))
}

fn to_err<E: std::fmt::Display>(e: E) -> VpnError {
    VpnError::msg(e.to_string())
}
