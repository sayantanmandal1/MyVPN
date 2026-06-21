//! Local-network host discovery via UDP broadcast beacons.
//!
//! While hosting, the app broadcasts a small beacon on the LAN every couple of
//! seconds. Every instance also listens, so hosts on the same Wi‑Fi appear in
//! the Connect list automatically and can be reached by network name — with no
//! server and no manual pairing code.
//!
//! Cross-internet discovery still uses the pairing code (the endpoint id, which
//! iroh publishes to global discovery); typing a name + passphrase across the
//! internet is the one piece reserved for a future DHT-backed resolver.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::net::UdpSocket;

use crate::state::DiscoveredHost;

pub const BEACON_PORT: u16 = 50266;
const MAGIC: &str = "MYVPN1";
const TTL: Duration = Duration::from_secs(8);

/// Shared, independently-locked table of hosts seen on the LAN.
pub type HostTable = Arc<Mutex<HashMap<String, (DiscoveredHost, Instant)>>>;

#[derive(Serialize, Deserialize)]
struct Beacon {
    magic: String,
    name: String,
    endpoint_id: String,
    requires_passphrase: bool,
}

/// Broadcast this host's presence on the LAN until the task is aborted.
pub fn spawn_beacon(
    name: String,
    endpoint_id: String,
    requires_passphrase: bool,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let sock = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let _ = sock.set_broadcast(true);
        let payload = serde_json::to_vec(&Beacon {
            magic: MAGIC.to_string(),
            name,
            endpoint_id,
            requires_passphrase,
        })
        .unwrap_or_default();
        let target = SocketAddr::from((Ipv4Addr::BROADCAST, BEACON_PORT));
        loop {
            let _ = sock.send_to(&payload, target).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
}

/// Listen for LAN beacons for the lifetime of the app, keeping `hosts` fresh
/// and emitting the updated list to the UI.
pub fn spawn_listener(app: AppHandle, hosts: HostTable) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let sock = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, BEACON_PORT)).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("LAN discovery unavailable: {e}");
                return;
            }
        };
        let mut buf = vec![0u8; 1500];
        loop {
            let mut changed = false;
            if let Ok(Ok((n, _src))) =
                tokio::time::timeout(Duration::from_secs(2), sock.recv_from(&mut buf)).await
            {
                if let Ok(b) = serde_json::from_slice::<Beacon>(&buf[..n]) {
                    if b.magic == MAGIC && !b.endpoint_id.is_empty() {
                        let host = DiscoveredHost {
                            network_name: b.name,
                            endpoint_id: b.endpoint_id.clone(),
                            source: "lan".to_string(),
                            requires_passphrase: b.requires_passphrase,
                            online: true,
                        };
                        hosts.lock().insert(b.endpoint_id, (host, Instant::now()));
                        changed = true;
                    }
                }
            }
            // Expire hosts we haven't heard from recently.
            {
                let mut g = hosts.lock();
                let before = g.len();
                g.retain(|_, (_, seen)| seen.elapsed() < TTL);
                changed |= g.len() != before;
            }
            if changed {
                let _ = app.emit(super::EVT_DISCOVERED, snapshot(&hosts));
            }
        }
    })
}

/// The current set of fresh LAN hosts.
pub fn snapshot(hosts: &HostTable) -> Vec<DiscoveredHost> {
    hosts
        .lock()
        .values()
        .filter(|(_, seen)| seen.elapsed() < TTL)
        .map(|(h, _)| h.clone())
        .collect()
}

/// Resolve a network name to an endpoint id using LAN discovery (case-insensitive).
pub fn find_by_name(hosts: &HostTable, name: &str) -> Option<String> {
    let want = name.trim().to_lowercase();
    hosts
        .lock()
        .values()
        .filter(|(_, seen)| seen.elapsed() < TTL)
        .find(|(h, _)| h.network_name.to_lowercase() == want)
        .map(|(h, _)| h.endpoint_id.clone())
}
