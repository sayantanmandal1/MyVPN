//! The **public VPN** mode: connect to free, volunteer-run public VPN servers
//! (aggregated from open sources, see [`sources`]) by country, over OpenVPN.
//!
//! This is deliberately a *separate* subsystem from the private peer-to-peer
//! [`crate::vpn`] engine — different transport, different UI, mutually exclusive
//! at runtime (both want the default route). The two never share state.

mod openvpn;
mod sources;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::error::{Result, VpnError};
use crate::state::{PublicServer, PublicState, PublicStatus};

/// Event channel the UI subscribes to for live public-VPN status.
pub const EVT_PUBLIC_STATUS: &str = "public://status";

type Task = tauri::async_runtime::JoinHandle<()>;

fn to_err<E: std::fmt::Display>(e: E) -> VpnError {
    VpnError::msg(e.to_string())
}

struct Inner {
    servers: Vec<PublicServer>,
    /// Decoded OpenVPN config per server id (kept server-side only).
    configs: HashMap<String, String>,
    status: PublicStatus,
    child: Option<std::process::Child>,
    mgmt_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    monitor: Option<Task>,
    connected_at: Option<Instant>,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            configs: HashMap::new(),
            status: PublicStatus::default(),
            child: None,
            mgmt_tx: None,
            monitor: None,
            connected_at: None,
        }
    }
}

/// The public VPN manager, shared via Tauri managed state.
pub struct PublicVpn {
    app: AppHandle,
    data_dir: PathBuf,
    inner: Arc<Mutex<Inner>>,
}

impl PublicVpn {
    pub fn new(app: AppHandle, data_dir: PathBuf) -> Self {
        Self {
            app,
            data_dir,
            inner: Arc::new(Mutex::new(Inner::default())),
        }
    }

    fn resource_dir(&self) -> PathBuf {
        self.app
            .path()
            .resource_dir()
            .unwrap_or_else(|_| self.data_dir.clone())
    }

    /// Refresh the cached server list from all sources. Returns the count.
    pub async fn refresh(&self) -> Result<usize> {
        let fetched = sources::fetch_all().await;
        if fetched.is_empty() {
            return Err(VpnError::msg(
                "Could not load any public servers (check your internet connection).",
            ));
        }

        let mut servers = Vec::with_capacity(fetched.len());
        let mut configs = HashMap::new();
        for f in fetched {
            configs.insert(f.server.id.clone(), f.ovpn);
            servers.push(f.server);
        }
        // Best servers first: higher score, then lower latency.
        servers.sort_by(|a, b| {
            b.score
                .unwrap_or(0)
                .cmp(&a.score.unwrap_or(0))
                .then(a.ping_ms.unwrap_or(u32::MAX).cmp(&b.ping_ms.unwrap_or(u32::MAX)))
        });

        let n = servers.len();
        let mut g = self.inner.lock();
        g.servers = servers;
        g.configs = configs;
        Ok(n)
    }

    /// The cached server list, refreshing first if it is empty.
    pub async fn servers(&self) -> Result<Vec<PublicServer>> {
        {
            let g = self.inner.lock();
            if !g.servers.is_empty() {
                return Ok(g.servers.clone());
            }
        }
        self.refresh().await?;
        Ok(self.inner.lock().servers.clone())
    }

    /// Current public-VPN status (with live duration).
    pub fn status(&self) -> PublicStatus {
        snapshot(&self.inner)
    }

    /// Whether a public connection is in progress or established.
    pub fn is_active(&self) -> bool {
        matches!(
            self.inner.lock().status.state,
            PublicState::Connecting | PublicState::Connected
        )
    }

    /// Connect to a public server by id, launching the bundled OpenVPN client.
    pub async fn connect(&self, server_id: String) -> Result<PublicStatus> {
        let (server, ovpn) = {
            let g = self.inner.lock();
            let server = g.servers.iter().find(|s| s.id == server_id).cloned();
            let ovpn = g.configs.get(&server_id).cloned();
            (server, ovpn)
        };
        let server = server.ok_or_else(|| VpnError::msg("Unknown server (refresh the list)."))?;
        let ovpn = ovpn.ok_or_else(|| VpnError::msg("Server config unavailable (refresh)."))?;

        // Tear down any prior public session first.
        self.disconnect().await?;

        let exe = openvpn::find_openvpn(&self.resource_dir()).ok_or_else(|| {
            VpnError::msg(
                "OpenVPN client not found. Install OpenVPN, or use the bundled \
                 build — see docs/PUBLIC_VPN.md.",
            )
        })?;

        std::fs::create_dir_all(&self.data_dir).ok();
        let cfg_path = self.data_dir.join("public.ovpn");
        std::fs::write(&cfg_path, ovpn).map_err(to_err)?;

        let proc = openvpn::spawn(&exe, &cfg_path, &self.data_dir).map_err(to_err)?;
        let mgmt_port = proc.mgmt_port;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        {
            let mut g = self.inner.lock();
            g.child = Some(proc.child);
            g.connected_at = None;
            g.mgmt_tx = Some(tx);
            g.status = PublicStatus {
                state: PublicState::Connecting,
                server_id: Some(server.id.clone()),
                country: Some(server.country.clone()),
                country_code: Some(server.country_code.clone()),
                message: Some(format!(
                    "Connecting to {} · {}",
                    server.country, server.hostname
                )),
                connected_secs: 0,
            };
        }
        emit(&self.app, &self.inner);

        let monitor = spawn_monitor(mgmt_port, rx, self.app.clone(), self.inner.clone());
        self.inner.lock().monitor = Some(monitor);

        Ok(self.status())
    }

    /// Disconnect the public VPN, asking OpenVPN to exit cleanly first.
    pub async fn disconnect(&self) -> Result<()> {
        let (tx, monitor, child) = {
            let mut g = self.inner.lock();
            // Mark idle up front so the monitor treats the imminent exit as
            // expected (no spurious "connection lost") and the UI updates now.
            g.status = PublicStatus::default();
            g.connected_at = None;
            (g.mgmt_tx.take(), g.monitor.take(), g.child.take())
        };
        emit(&self.app, &self.inner);

        // Ask OpenVPN to tear down its own routes/DNS and exit.
        if let Some(tx) = tx {
            let _ = tx.send("signal SIGTERM\n".to_string());
        }
        if child.is_some() {
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
        if let Some(mut child) = child {
            match child.try_wait() {
                Ok(Some(_)) => {}
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
        if let Some(m) = monitor {
            m.abort();
        }
        Ok(())
    }
}

fn snapshot(inner: &Mutex<Inner>) -> PublicStatus {
    let g = inner.lock();
    let mut s = g.status.clone();
    s.connected_secs = g.connected_at.map(|t| t.elapsed().as_secs()).unwrap_or(0);
    s
}

fn emit(app: &AppHandle, inner: &Arc<Mutex<Inner>>) {
    let _ = app.emit(EVT_PUBLIC_STATUS, snapshot(inner));
}

/// Connect to OpenVPN's management interface, stream `>STATE:` notifications
/// into the status, and forward control commands (e.g. SIGTERM) to it.
fn spawn_monitor(
    port: u16,
    mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
    app: AppHandle,
    inner: Arc<Mutex<Inner>>,
) -> Task {
    tauri::async_runtime::spawn(async move {
        // OpenVPN needs a moment to bind the port; retry briefly.
        let mut stream = None;
        for _ in 0..40 {
            match TcpStream::connect(("127.0.0.1", port)).await {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
            }
        }
        let Some(stream) = stream else {
            set_state(&app, &inner, PublicState::Error, Some("Could not attach to OpenVPN."));
            return;
        };

        let (read_half, mut write_half) = stream.into_split();
        let mut lines = BufReader::new(read_half).lines();
        // Stream real-time state notifications.
        let _ = write_half.write_all(b"state on\n").await;

        loop {
            tokio::select! {
                line = lines.next_line() => {
                    match line {
                        Ok(Some(l)) => handle_mgmt_line(&l, &app, &inner),
                        _ => break, // EOF: OpenVPN exited
                    }
                }
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(c) => { let _ = write_half.write_all(c.as_bytes()).await; }
                        None => break,
                    }
                }
            }
        }

        // The socket closed: if we were still up, this was an unexpected drop.
        let unexpected = {
            let g = inner.lock();
            matches!(g.status.state, PublicState::Connecting | PublicState::Connected)
        };
        if unexpected {
            set_state(&app, &inner, PublicState::Error, Some("Connection lost."));
        }
    })
}

fn handle_mgmt_line(line: &str, app: &AppHandle, inner: &Arc<Mutex<Inner>>) {
    let Some(rest) = line.strip_prefix(">STATE:") else {
        return;
    };
    // Format: <time>,<STATE>,<description>,<localip>,<remoteip>,...
    let fields: Vec<&str> = rest.split(',').collect();
    let Some(state) = fields.get(1).copied() else {
        return;
    };

    {
        let mut g = inner.lock();
        // Ignore notifications once we've already torn down.
        if matches!(g.status.state, PublicState::Idle) {
            return;
        }
        match state {
            "CONNECTED" => {
                g.status.state = PublicState::Connected;
                g.connected_at = Some(Instant::now());
                let country = g.status.country.clone().unwrap_or_default();
                g.status.message = Some(format!("Connected · {country}"));
            }
            "RECONNECTING" => {
                g.status.state = PublicState::Connecting;
                g.status.message = Some("Reconnecting…".to_string());
            }
            "EXITING" => {
                g.status.state = PublicState::Error;
                g.status.message = Some("Disconnected.".to_string());
            }
            "WAIT" | "AUTH" | "GET_CONFIG" | "ASSIGN_IP" | "ADD_ROUTES" | "TCP_CONNECT"
            | "RESOLVE" => {
                g.status.state = PublicState::Connecting;
            }
            _ => {}
        }
    }
    emit(app, inner);
}

fn set_state(app: &AppHandle, inner: &Arc<Mutex<Inner>>, state: PublicState, msg: Option<&str>) {
    {
        let mut g = inner.lock();
        g.status.state = state;
        if let Some(m) = msg {
            g.status.message = Some(m.to_string());
        }
        if state != PublicState::Connected {
            g.connected_at = None;
        }
    }
    emit(app, inner);
}
