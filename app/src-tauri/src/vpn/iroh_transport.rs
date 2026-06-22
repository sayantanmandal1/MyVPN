//! Real peer-to-peer transport built on [`iroh`](https://www.iroh.computer/).
//!
//! An [`IrohNode`] wraps an iroh QUIC endpoint. The host binds an endpoint with
//! the MyVPN ALPN and accepts connections; the client dials the host by its
//! `EndpointId` (which doubles as the pairing code). Connectivity uses NAT
//! hole-punching with relay fallback, and every connection is end-to-end
//! encrypted with TLS 1.3.
//!
//! In this phase the connection carries a small JSON handshake to prove the
//! tunnel works end to end. Phase 3 reuses the same connection to carry IP
//! packets between the two Wintun adapters.

use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Context;
use bytes::Bytes;
use iroh::endpoint::{presets, Connection, RecvStream, SendStream};
use iroh::{Endpoint, EndpointId, RelayMode, RelayUrl, SecretKey};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// Application-Layer Protocol Negotiation identifier for MyVPN connections.
pub const ALPN: &[u8] = b"myvpn/0";

const MAX_FRAME: usize = 64 * 1024;

/// The single active client connection for a host, shared with the persistent
/// TUN reader so return traffic is routed to it. Holds an mpsc sender (Send +
/// Sync) rather than the connection itself. (One client at a time, since every
/// client uses the same tunnel IP.)
pub type ClientSlot = Arc<Mutex<Option<(u64, tokio::sync::mpsc::UnboundedSender<Bytes>)>>>;

static HOST_GEN: AtomicU64 = AtomicU64::new(1);

/// A bound iroh endpoint that can host or dial MyVPN peers.
#[derive(Clone)]
pub struct IrohNode {
    endpoint: Endpoint,
}

impl IrohNode {
    /// Bind an endpoint that accepts incoming MyVPN connections (host role).
    ///
    /// `relay_url` optionally pins the endpoint to a self-hosted relay; `None`
    /// keeps iroh's default multi-region n0 relays.
    pub async fn bind_host(
        secret: SecretKey,
        relay_url: Option<String>,
    ) -> anyhow::Result<Self> {
        let mut builder = Endpoint::builder(presets::N0)
            .secret_key(secret)
            .alpns(vec![ALPN.to_vec()]);
        if let Some(mode) = custom_relay_mode(relay_url.as_deref()) {
            builder = builder.relay_mode(mode);
        }
        let endpoint = builder.bind().await.context("bind host endpoint")?;
        Ok(Self { endpoint })
    }

    /// Bind an endpoint used to dial a host (client role).
    pub async fn bind_client(
        secret: SecretKey,
        relay_url: Option<String>,
    ) -> anyhow::Result<Self> {
        let mut builder = Endpoint::builder(presets::N0).secret_key(secret);
        if let Some(mode) = custom_relay_mode(relay_url.as_deref()) {
            builder = builder.relay_mode(mode);
        }
        let endpoint = builder.bind().await.context("bind client endpoint")?;
        Ok(Self { endpoint })
    }

    /// This endpoint's id, which is also the pairing code shared out of band.
    pub fn id_string(&self) -> String {
        self.endpoint.id().to_string()
    }

    /// Best-effort wait until the endpoint has registered with a relay, so its
    /// address is published to discovery and it becomes dialable.
    pub async fn wait_online(&self) {
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.endpoint.online(),
        )
        .await;
    }

    /// Dial a peer by its endpoint id / pairing code.
    pub async fn connect(&self, peer: &str) -> anyhow::Result<Connection> {
        let id = EndpointId::from_str(peer.trim())
            .map_err(|e| anyhow::anyhow!("invalid pairing code: {e}"))?;
        self.endpoint
            .connect(id, ALPN)
            .await
            .context("open connection to host")
    }

    /// Accept the next incoming connection, if any.
    pub async fn accept(&self) -> Option<Connection> {
        let incoming = self.endpoint.accept().await?;
        match incoming.await {
            Ok(conn) => Some(conn),
            Err(err) => {
                tracing::warn!("incoming connection failed: {err}");
                None
            }
        }
    }

    /// Gracefully close the endpoint and all connections.
    pub async fn close(&self) {
        self.endpoint.close().await;
    }
}

/// Build a custom relay configuration from a user-provided relay URL. Returns
/// `None` to keep iroh's default multi-region n0 relays. An invalid URL is
/// ignored (and logged) so a bad setting can never brick connectivity.
fn custom_relay_mode(relay_url: Option<&str>) -> Option<RelayMode> {
    let url = relay_url?.trim();
    if url.is_empty() {
        return None;
    }
    match url.parse::<RelayUrl>() {
        Ok(parsed) => Some(RelayMode::custom([parsed])),
        Err(e) => {
            tracing::warn!("ignoring invalid relay URL {url:?}: {e}");
            None
        }
    }
}

// --- pairing handshake ---------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
struct Hello {
    network_name: String,
    version: String,
    #[serde(default)]
    pass_proof: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HelloAck {
    ok: bool,
    host_name: String,
    message: String,
}

/// Derive a passphrase proof. Sent over the already end-to-end-encrypted QUIC
/// channel and compared in constant time by the host.
pub fn hash_pass(passphrase: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(format!("myvpn-auth:{passphrase}").as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Derive a deterministic iroh identity from a network name and its passphrase
/// proof. A passphrase-protected host binds this identity, so a remote client
/// that knows the name + passphrase can compute the same endpoint id (see
/// [`derived_endpoint_id`]) and dial it through iroh's global discovery — no
/// pairing code required.
///
/// Security: anyone who knows the name + passphrase can derive this identity —
/// that *is* the intended shared secret, and the handshake still verifies the
/// passphrase proof. Choose a strong passphrase, or use a pairing code (a
/// random, non-derivable identity) when you don't want name-based discovery.
pub fn derive_secret(name: &str, proof: &str) -> SecretKey {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"myvpn-net-identity:v1");
    hasher.update(name.trim().to_lowercase().as_bytes());
    hasher.update([0u8]);
    hasher.update(proof.as_bytes());
    let digest = hasher.finalize();
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&digest);
    SecretKey::from_bytes(&seed)
}

/// The endpoint id (pairing code / dial target) of the identity derived from a
/// network name and passphrase proof.
pub fn derived_endpoint_id(name: &str, proof: &str) -> String {
    derive_secret(name, proof).public().to_string()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Client side: open a stream, send a hello, await the host's acknowledgement.
/// Returns a human-readable welcome message on success.
pub async fn client_handshake(
    conn: &Connection,
    network_name: &str,
    pass_proof: Option<String>,
) -> anyhow::Result<String> {
    let (mut send, mut recv) = conn.open_bi().await.context("open stream")?;
    let hello = Hello {
        network_name: network_name.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        pass_proof,
    };
    write_frame(&mut send, &serde_json::to_vec(&hello)?).await?;
    let raw = read_frame(&mut recv).await?;
    let ack: HelloAck = serde_json::from_slice(&raw).context("decode handshake ack")?;
    if !ack.ok {
        anyhow::bail!(if ack.message.is_empty() {
            "host rejected the connection".to_string()
        } else {
            ack.message
        });
    }
    Ok(ack.message)
}

/// Host side: accept the client's stream, read its hello, acknowledge, then
/// forward IP packets between the client and the host's TUN/NAT gateway until
/// the connection closes.
pub async fn host_serve(
    conn: Connection,
    host_name: String,
    expected_proof: Option<String>,
    session: Option<Arc<wintun::Session>>,
    slot: ClientSlot,
    down: Arc<AtomicU64>,
    rtt: Arc<AtomicU64>,
) -> anyhow::Result<()> {
    let (mut send, mut recv) = conn.accept_bi().await.context("accept stream")?;
    let raw = read_frame(&mut recv).await?;
    let hello: Hello = serde_json::from_slice(&raw).context("decode hello")?;

    // Authenticate the peer against the network passphrase, if one is set.
    if let Some(expected) = expected_proof.as_deref() {
        let ok = hello
            .pass_proof
            .as_deref()
            .map(|p| constant_time_eq(p.as_bytes(), expected.as_bytes()))
            .unwrap_or(false);
        if !ok {
            let nack = HelloAck {
                ok: false,
                host_name: host_name.clone(),
                message: "Incorrect passphrase".to_string(),
            };
            let _ = write_frame(&mut send, &serde_json::to_vec(&nack)?).await;
            anyhow::bail!("peer failed passphrase authentication");
        }
    }

    // Single tunnel IP => one active client. A newly authenticated peer takes
    // over the slot: this is almost always the same device reconnecting after a
    // network blip, before the old connection's idle timeout fires — so eviction
    // (rather than rejection) makes reconnects fast instead of stalling for tens
    // of seconds. The previous connection's sender then drains and exits.
    let my_gen = HOST_GEN.fetch_add(1, Ordering::Relaxed);
    let mut pump: Option<(tokio::sync::mpsc::UnboundedReceiver<Bytes>, Arc<wintun::Session>)> =
        None;
    if let Some(s) = session.as_ref() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Bytes>();
        *slot.lock() = Some((my_gen, tx));
        pump = Some((rx, s.clone()));
    }

    tracing::info!("peer joined \"{}\" (client v{})", host_name, hello.version);
    let ack = HelloAck {
        ok: true,
        host_name: host_name.clone(),
        message: format!("Connected to {host_name}"),
    };
    write_frame(&mut send, &serde_json::to_vec(&ack)?).await?;

    if let Some((rx, s)) = pump {
        let sender = spawn_datagram_sender(conn.clone(), rx);
        let writer = spawn_tun_writer(conn.clone(), s, down);
        let sampler = spawn_rtt_sampler(conn.clone(), rtt);
        let _ = conn.closed().await;
        sender.abort();
        writer.abort();
        sampler.abort();
        // Release the slot if we still own it.
        let mut g = slot.lock();
        if g.as_ref().map(|(gen, _)| *gen) == Some(my_gen) {
            *g = None;
        }
    } else {
        let _ = conn.closed().await;
    }
    Ok(())
}

/// A single long-lived reader that pulls packets from a TUN session and
/// forwards them to the active peer via the slot's channel. Exits when the
/// session is shut down. Used by both the host gateway and the client tunnel.
pub fn spawn_session_reader(session: Arc<wintun::Session>, slot: ClientSlot, up: Arc<AtomicU64>) {
    std::thread::spawn(move || {
        while let Ok(packet) = session.receive_blocking() {
            let target = slot.lock().as_ref().map(|(_, tx)| tx.clone());
            if let Some(tx) = target {
                let data = packet.bytes();
                up.fetch_add(data.len() as u64, Ordering::Relaxed);
                let _ = tx.send(Bytes::copy_from_slice(data));
            }
        }
    });
}

/// Write IP packets received from a peer into a TUN session.
pub fn spawn_tun_writer(
    conn: Connection,
    session: Arc<wintun::Session>,
    down: Arc<AtomicU64>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        while let Ok(data) = conn.read_datagram().await {
            // A tunnelled IP packet never exceeds the adapter MTU; ignore
            // anything empty or larger than a Wintun packet can hold
            // (defensive: avoids a truncating cast + copy_from_slice panic
            // that would tear down the data plane).
            if data.is_empty() || data.len() > u16::MAX as usize {
                continue;
            }
            down.fetch_add(data.len() as u64, Ordering::Relaxed);
            if let Ok(mut packet) = session.allocate_send_packet(data.len() as u16) {
                packet.bytes_mut().copy_from_slice(&data);
                session.send_packet(packet);
            }
        }
    })
}

/// Drain a channel of IP packets into a peer connection as QUIC datagrams.
pub fn spawn_datagram_sender(
    conn: Connection,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<Bytes>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        while let Some(pkt) = rx.recv().await {
            if conn.send_datagram(pkt).is_err() {
                break;
            }
        }
    })
}

/// The current best (lowest) path round-trip time for this connection, in
/// milliseconds, or `None` if no path has a measured RTT yet.
pub fn current_rtt_ms(conn: &Connection) -> Option<u64> {
    let mut best: Option<std::time::Duration> = None;
    for path in conn.paths().iter() {
        let r = path.rtt();
        if r > std::time::Duration::ZERO {
            best = Some(best.map_or(r, |b| b.min(r)));
        }
    }
    best.map(|d| d.as_millis() as u64)
}

/// Sample the connection RTT once per second into a shared counter so the stats
/// ticker can surface live latency. Stores 0 when no path RTT is known yet.
/// Exits when the connection closes.
pub fn spawn_rtt_sampler(
    conn: Connection,
    rtt_ms: Arc<AtomicU64>,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        loop {
            if conn.close_reason().is_some() {
                break;
            }
            rtt_ms.store(current_rtt_ms(&conn).unwrap_or(0), Ordering::Relaxed);
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    })
}

/// The maximum QUIC datagram payload for this connection, if negotiated. Used
/// to clamp the tunnel MTU so IP packets never exceed what a datagram can carry.
pub fn max_datagram(conn: &Connection) -> Option<usize> {
    conn.max_datagram_size()
}

/// The remote socket addresses iroh is currently using for this connection
/// (direct IP paths and resolved relay hosts). Used to pin carrier traffic to
/// the physical gateway so the tunnel doesn't route its own packets (NAT
/// loop-avoidance).
pub fn remote_socket_addrs(conn: &Connection) -> Vec<SocketAddr> {
    use std::net::ToSocketAddrs;
    let mut out = Vec::new();
    for path in conn.paths().iter() {
        match path.remote_addr() {
            iroh::TransportAddr::Ip(addr) => out.push(*addr),
            iroh::TransportAddr::Relay(url) => {
                let host = url.host_str().unwrap_or_default().to_string();
                let port = url.port().unwrap_or(443);
                if let Ok(resolved) = (host.as_str(), port).to_socket_addrs() {
                    out.extend(resolved);
                }
            }
            _ => {}
        }
    }
    out
}

// --- length-prefixed framing --------------------------------------------

async fn write_frame(send: &mut SendStream, data: &[u8]) -> anyhow::Result<()> {
    let len = (data.len() as u32).to_be_bytes();
    send.write_all(&len).await?;
    send.write_all(data).await?;
    Ok(())
}

async fn read_frame(recv: &mut RecvStream) -> anyhow::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    recv.read_exact(&mut len).await?;
    let n = u32::from_be_bytes(len) as usize;
    anyhow::ensure!(n <= MAX_FRAME, "frame too large ({n} bytes)");
    let mut buf = vec![0u8; n];
    recv.read_exact(&mut buf).await?;
    Ok(buf)
}

// --- identity persistence ------------------------------------------------

/// Magic prefix marking a DPAPI-encrypted identity blob (vs. a legacy 32-byte
/// plaintext key written by older builds).
const IDENTITY_MAGIC: &[u8; 4] = b"MVK1";

/// Load a persisted endpoint secret key, or generate and persist a new one so
/// this device keeps a stable identity (and pairing code) across restarts.
///
/// The key is stored encrypted at rest with Windows DPAPI (tied to the current
/// user account), so it cannot be read by other users or copied off-device.
/// Legacy plaintext keys are transparently upgraded to an encrypted blob.
pub fn load_or_create_secret(path: &Path) -> anyhow::Result<SecretKey> {
    if let Ok(bytes) = std::fs::read(path) {
        if let Some(secret) = decode_secret(&bytes) {
            // Upgrade a legacy plaintext key to an encrypted blob in place.
            if bytes.len() == 32 {
                let _ = persist_secret(path, &secret);
            }
            return Ok(secret);
        }
    }
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| anyhow::anyhow!("secure RNG failed: {e}"))?;
    let secret = SecretKey::from_bytes(&seed);
    persist_secret(path, &secret)?;
    Ok(secret)
}

/// Decode the on-disk identity, accepting either a DPAPI blob or a legacy
/// 32-byte plaintext key.
fn decode_secret(bytes: &[u8]) -> Option<SecretKey> {
    if bytes.len() > IDENTITY_MAGIC.len() && &bytes[..IDENTITY_MAGIC.len()] == IDENTITY_MAGIC {
        let plain = dpapi_unprotect(&bytes[IDENTITY_MAGIC.len()..])?;
        let arr: [u8; 32] = plain.as_slice().try_into().ok()?;
        return Some(SecretKey::from_bytes(&arr));
    }
    let arr: [u8; 32] = bytes.try_into().ok()?;
    Some(SecretKey::from_bytes(&arr))
}

/// Persist the secret key, encrypted with DPAPI when available. Falls back to a
/// plaintext write only if the platform cannot encrypt (the file is still
/// created in a per-user directory).
fn persist_secret(path: &Path, secret: &SecretKey) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let raw = secret.to_bytes();
    let payload = match dpapi_protect(&raw) {
        Some(blob) => {
            let mut out = Vec::with_capacity(IDENTITY_MAGIC.len() + blob.len());
            out.extend_from_slice(IDENTITY_MAGIC);
            out.extend_from_slice(&blob);
            out
        }
        None => raw.to_vec(),
    };
    std::fs::write(path, payload).context("persist identity key")?;
    Ok(())
}

/// Encrypt bytes with the current user's DPAPI master key. Returns `None` if the
/// OS call fails (the caller then stores plaintext as a best-effort fallback).
#[cfg(windows)]
fn dpapi_protect(data: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows_sys::Win32::Foundation::LocalFree;

    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        let ok = CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 || output.pbData.is_null() {
            return None;
        }
        let blob = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(output.pbData as *mut core::ffi::c_void);
        Some(blob)
    }
}

/// Decrypt a DPAPI blob produced by [`dpapi_protect`].
#[cfg(windows)]
fn dpapi_unprotect(data: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows_sys::Win32::Foundation::LocalFree;

    let input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    unsafe {
        let ok = CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        );
        if ok == 0 || output.pbData.is_null() {
            return None;
        }
        let plain = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(output.pbData as *mut core::ffi::c_void);
        Some(plain)
    }
}

#[cfg(not(windows))]
fn dpapi_protect(_data: &[u8]) -> Option<Vec<u8>> {
    None
}

#[cfg(not(windows))]
fn dpapi_unprotect(_data: &[u8]) -> Option<Vec<u8>> {
    None
}
