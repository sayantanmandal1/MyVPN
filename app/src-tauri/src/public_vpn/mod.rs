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

#[derive(Default)]
struct Inner {
    servers: Vec<PublicServer>,
    /// Decoded OpenVPN config per server id (kept server-side only).
    configs: HashMap<String, String>,
    status: PublicStatus,
    child: Option<openvpn::ProcHandle>,
    mgmt_tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
    monitor: Option<Task>,
    connected_at: Option<Instant>,
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
        let fetched = sources::fetch_all().await.map_err(to_err)?;

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

        // Reflect intent right away. The app already runs elevated (manifest),
        // so OpenVPN starts without a prompt; show progress while it launches.
        {
            let mut g = self.inner.lock();
            g.connected_at = None;
            g.status = PublicStatus {
                state: PublicState::Connecting,
                server_id: Some(server.id.clone()),
                country: Some(server.country.clone()),
                country_code: Some(server.country_code.clone()),
                message: Some("Connecting…".to_string()),
                connected_secs: 0,
            };
        }
        emit(&self.app, &self.inner);

        // Launching can briefly block (only if a UAC fallback is needed), so run
        // it off the async runtime to avoid stalling other tasks.
        let exe2 = exe.clone();
        let cfg2 = cfg_path.clone();
        let dir2 = self.data_dir.clone();
        let proc = match tokio::task::spawn_blocking(move || openvpn::spawn(&exe2, &cfg2, &dir2))
            .await
        {
            Ok(Ok(proc)) => proc,
            Ok(Err(e)) => {
                let msg = e.to_string();
                set_state(&self.app, &self.inner, PublicState::Error, Some(&msg));
                return Err(to_err(msg));
            }
            Err(e) => {
                set_state(
                    &self.app,
                    &self.inner,
                    PublicState::Error,
                    Some("Failed to launch OpenVPN."),
                );
                return Err(to_err(e));
            }
        };
        let mgmt_port = proc.mgmt_port;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        {
            let mut g = self.inner.lock();
            g.child = Some(proc.proc);
            g.mgmt_tx = Some(tx);
            g.status.message = Some(format!(
                "Connecting to {} · {}",
                server.country, server.hostname
            ));
        }
        emit(&self.app, &self.inner);

        let monitor = spawn_monitor(
            mgmt_port,
            rx,
            self.app.clone(),
            self.inner.clone(),
            self.data_dir.clone(),
        );
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

        // Ask OpenVPN to tear down its own routes/DNS and exit cleanly. This is
        // a management-socket command interpreted by OpenVPN itself, so it works
        // even though the process runs elevated.
        if let Some(tx) = tx {
            let _ = tx.send("signal SIGTERM\n".to_string());
        }
        if let Some(mut handle) = child {
            if !handle.has_exited() {
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
            if !handle.has_exited() {
                handle.kill();
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
    data_dir: PathBuf,
) -> Task {
    tauri::async_runtime::spawn(async move {
        // OpenVPN needs a moment to bind the port; retry for a while to also
        // tolerate the user pausing on the elevation prompt.
        let mut stream = None;
        for _ in 0..60 {
            match TcpStream::connect(("127.0.0.1", port)).await {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(250)).await,
            }
        }
        let Some(stream) = stream else {
            let msg = match openvpn::log_reason(&data_dir) {
                Some(r) => format!("Couldn't start OpenVPN — {r}"),
                None => "Couldn't attach to OpenVPN.".to_string(),
            };
            set_state(&app, &inner, PublicState::Error, Some(&msg));
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
                        Ok(Some(l)) => {
                            // Some free configs declare `auth-user-pass`; answer
                            // the management password query so they still connect
                            // instead of stalling.
                            if let Some(reply) = mgmt_password_reply(&l) {
                                let _ = write_half.write_all(reply.as_bytes()).await;
                            }
                            handle_mgmt_line(&l, &app, &inner);
                        }
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

        // The management socket closed. What we surface depends on where we
        // were: a user-initiated disconnect already set Idle (stay silent); a
        // drop while Connecting means we never tunneled; a drop while Connected
        // means the link was lost. OpenVPN's own log explains why.
        let state_now = inner.lock().status.state;
        match state_now {
            PublicState::Connecting => {
                let msg = match openvpn::log_reason(&data_dir) {
                    Some(r) => format!("Couldn't establish the connection — {r}"),
                    None => "Couldn't establish the connection. The server may be \
                             offline, or administrator approval was declined."
                        .to_string(),
                };
                set_state(&app, &inner, PublicState::Error, Some(&msg));
            }
            PublicState::Connected => {
                let msg = match openvpn::log_reason(&data_dir) {
                    Some(r) => format!("Connection lost — {r}"),
                    None => "Connection lost.".to_string(),
                };
                set_state(&app, &inner, PublicState::Error, Some(&msg));
            }
            _ => {}
        }
    })
}

/// Reply to OpenVPN's management password query. Free servers (e.g. VPN Gate)
/// that declare `auth-user-pass` accept throwaway credentials, so supplying them
/// lets those configs connect instead of stalling on the prompt.
fn mgmt_password_reply(line: &str) -> Option<String> {
    if line.starts_with(">PASSWORD:Need 'Auth'") {
        Some("username \"Auth\" \"vpn\"\npassword \"Auth\" \"vpn\"\n".to_string())
    } else {
        None
    }
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
