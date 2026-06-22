//! Tauri commands — the bridge between the React UI and the VPN engine.

use std::sync::Arc;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;

use crate::error::{Result, VpnError};
use crate::public_vpn::PublicVpn;
use crate::state::*;
use crate::vpn::{VpnEngine, EVT_LOG};

const STORE_FILE: &str = "settings.json";
const STORE_KEY: &str = "settings";

// --- settings persistence ------------------------------------------------

pub fn load_settings(app: &AppHandle) -> Settings {
    match app.store(STORE_FILE) {
        Ok(store) => store
            .get(STORE_KEY)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

fn store_settings(app: &AppHandle, s: &Settings) -> Result<()> {
    let store = app.store(STORE_FILE).map_err(|e| VpnError::msg(e.to_string()))?;
    let value = serde_json::to_value(s).map_err(|e| VpnError::msg(e.to_string()))?;
    store.set(STORE_KEY, value);
    store.save().map_err(|e| VpnError::msg(e.to_string()))?;
    Ok(())
}

// The host passphrase proof (hash) is kept under its own key so the UI's
// settings round-trip never overwrites it, and only the hash is ever stored.
const PROOF_KEY: &str = "last_host_proof";

fn store_host_proof(app: &AppHandle, proof: &Option<String>) {
    if let Ok(store) = app.store(STORE_FILE) {
        let value = serde_json::to_value(proof).unwrap_or(serde_json::Value::Null);
        store.set(PROOF_KEY, value);
        let _ = store.save();
    }
}

fn load_host_proof(app: &AppHandle) -> Option<String> {
    app.store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(PROOF_KEY))
        .and_then(|v| serde_json::from_value::<Option<String>>(v).ok())
        .flatten()
}

// --- trusted devices -----------------------------------------------------
// Hosts we've successfully connected to are remembered (with the proof we used)
// so future connections are frictionless — no code or passphrase required.
const SAVED_HOSTS_KEY: &str = "saved_hosts";

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SavedHost {
    network_name: String,
    endpoint_id: String,
    proof: Option<String>,
}

fn load_saved_hosts(app: &AppHandle) -> Vec<SavedHost> {
    app.store(STORE_FILE)
        .ok()
        .and_then(|s| s.get(SAVED_HOSTS_KEY))
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn remember_host(app: &AppHandle, host: SavedHost) {
    if host.endpoint_id.is_empty() {
        return;
    }
    let mut hosts = load_saved_hosts(app);
    hosts.retain(|h| h.endpoint_id != host.endpoint_id);
    hosts.insert(0, host);
    hosts.truncate(16);
    if let Ok(store) = app.store(STORE_FILE) {
        if let Ok(value) = serde_json::to_value(&hosts) {
            store.set(SAVED_HOSTS_KEY, value);
            let _ = store.save();
        }
    }
}

// --- status / control ----------------------------------------------------

#[tauri::command]
pub async fn get_status(engine: State<'_, Arc<VpnEngine>>) -> Result<StatusSnapshot> {
    Ok(engine.snapshot())
}

#[tauri::command]
pub async fn list_discovered(
    app: AppHandle,
    engine: State<'_, Arc<VpnEngine>>,
) -> Result<Vec<DiscoveredHost>> {
    let mut hosts = engine.list_discovered().await;
    let live: std::collections::HashSet<String> =
        hosts.iter().map(|h| h.endpoint_id.clone()).collect();
    // Append previously-trusted hosts that aren't currently visible on the LAN.
    for saved in load_saved_hosts(&app) {
        if !live.contains(&saved.endpoint_id) {
            hosts.push(DiscoveredHost {
                network_name: saved.network_name,
                endpoint_id: saved.endpoint_id,
                source: "saved".to_string(),
                requires_passphrase: false,
                online: false,
            });
        }
    }
    Ok(hosts)
}

#[tauri::command]
pub async fn start_host(
    app: AppHandle,
    engine: State<'_, Arc<VpnEngine>>,
    public: State<'_, Arc<PublicVpn>>,
    config: HostConfig,
) -> Result<StatusSnapshot> {
    if public.is_active() {
        return Err(VpnError::msg(
            "Disconnect the public VPN before hosting a private network.",
        ));
    }
    let proof = config
        .passphrase
        .as_deref()
        .filter(|p| !p.is_empty())
        .map(crate::vpn::hash_pass);
    let snap = engine
        .start_host(config.network_name.clone(), proof.clone())
        .await?;
    let mut s = load_settings(&app);
    s.was_hosting = true;
    s.last_network_name = Some(config.network_name);
    let _ = store_settings(&app, &s);
    store_host_proof(&app, &proof);
    Ok(snap)
}

#[tauri::command]
pub async fn stop_host(app: AppHandle, engine: State<'_, Arc<VpnEngine>>) -> Result<()> {
    engine.stop_host().await?;
    let mut s = load_settings(&app);
    s.was_hosting = false;
    let _ = store_settings(&app, &s);
    Ok(())
}

#[tauri::command]
pub async fn connect(
    app: AppHandle,
    engine: State<'_, Arc<VpnEngine>>,
    public: State<'_, Arc<PublicVpn>>,
    config: ConnectConfig,
) -> Result<StatusSnapshot> {
    if public.is_active() {
        return Err(VpnError::msg(
            "Disconnect the public VPN before joining a private network.",
        ));
    }

    let mut config = config;
    // Trusted devices: with no passphrase supplied, reuse the saved proof for a
    // host we've connected to before so reconnecting needs no authentication.
    let has_secret = config.proof.as_deref().is_some_and(|p| !p.is_empty())
        || config.passphrase.as_deref().is_some_and(|p| !p.is_empty());
    if !has_secret {
        let target = config.endpoint_id.clone().or_else(|| config.ticket.clone());
        if let Some(found) = load_saved_hosts(&app).into_iter().find(|h| {
            Some(&h.endpoint_id) == target.as_ref() || h.network_name == config.network_name
        }) {
            config.proof = found.proof;
            if config.endpoint_id.is_none() && config.ticket.is_none() {
                config.endpoint_id = Some(found.endpoint_id);
            }
        }
    }

    // Compute the proof we will actually present, so we can remember it.
    let proof_used = config.proof.clone().filter(|p| !p.is_empty()).or_else(|| {
        config
            .passphrase
            .as_deref()
            .filter(|p| !p.is_empty())
            .map(crate::vpn::hash_pass)
    });

    let snap = engine.connect(config.clone()).await?;

    if let Some(endpoint_id) = snap.peer_endpoint_id.clone() {
        remember_host(
            &app,
            SavedHost {
                network_name: config.network_name.clone(),
                endpoint_id,
                proof: proof_used,
            },
        );
    }
    Ok(snap)
}

#[tauri::command]
pub async fn disconnect(engine: State<'_, Arc<VpnEngine>>) -> Result<()> {
    engine.disconnect().await
}

// --- public VPN mode -----------------------------------------------------
// A separate feature from the private P2P VPN: free public servers by country.

#[tauri::command]
pub async fn public_refresh(public: State<'_, Arc<PublicVpn>>) -> Result<usize> {
    public.refresh().await
}

#[tauri::command]
pub async fn public_servers(
    public: State<'_, Arc<PublicVpn>>,
) -> Result<Vec<PublicServer>> {
    public.servers().await
}

#[tauri::command]
pub async fn public_connect(
    engine: State<'_, Arc<VpnEngine>>,
    public: State<'_, Arc<PublicVpn>>,
    server_id: String,
) -> Result<PublicStatus> {
    if !matches!(
        engine.snapshot().state,
        ConnectionState::Idle | ConnectionState::Error
    ) {
        return Err(VpnError::msg(
            "Stop the private VPN (host or connect) before using a public server.",
        ));
    }
    public.connect(server_id).await
}

#[tauri::command]
pub async fn public_disconnect(public: State<'_, Arc<PublicVpn>>) -> Result<()> {
    public.disconnect().await
}

#[tauri::command]
pub async fn public_status(public: State<'_, Arc<PublicVpn>>) -> Result<PublicStatus> {
    Ok(public.status())
}

// --- update check --------------------------------------------------------

const UPDATE_REPO: &str = "sayantanmandal1/MyVPN";

/// Check GitHub Releases for a newer version. Best-effort and privacy-light: a
/// single GET of public release metadata (no identifiers sent). Returns the new
/// version and release page URL, or `None` when already current or unreachable.
#[tauri::command]
pub async fn check_update() -> Result<Option<UpdateInfo>> {
    let url = format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent(concat!("MyVPN/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| VpnError::msg(e.to_string()))?;
    let resp = match client.get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return Ok(None),
    };
    let body = match resp.text().await {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let tag = json.get("tag_name").and_then(|v| v.as_str()).unwrap_or_default();
    let page = json.get("html_url").and_then(|v| v.as_str()).unwrap_or_default();
    if !page.is_empty() && parse_version(tag) > parse_version(env!("CARGO_PKG_VERSION")) {
        Ok(Some(UpdateInfo {
            version: tag.trim_start_matches('v').to_string(),
            url: page.to_string(),
        }))
    } else {
        Ok(None)
    }
}

/// Parse a `v1.2.3` / `1.2.3` tag into a comparable tuple (unknown parts -> 0).
fn parse_version(s: &str) -> (u32, u32, u32) {
    let s = s.trim().trim_start_matches('v');
    let mut it = s.split(['.', '-', '+']);
    let a = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let b = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let c = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    (a, b, c)
}

/// Download and install the latest signed release via the Tauri updater, then
/// relaunch. Returns `Ok(false)` if already up to date. Errors (e.g. the updater
/// isn't configured yet, or no signed artifact is published) let the UI fall
/// back to opening the release page for a manual download.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<bool> {
    #[cfg(desktop)]
    {
        use tauri_plugin_updater::UpdaterExt;
        let updater = app
            .updater()
            .map_err(|e| VpnError::msg(e.to_string()))?;
        if let Some(update) = updater
            .check()
            .await
            .map_err(|e| VpnError::msg(e.to_string()))?
        {
            update
                .download_and_install(|_, _| {}, || {})
                .await
                .map_err(|e| VpnError::msg(e.to_string()))?;
            app.restart();
        }
        Ok(false)
    }
    #[cfg(not(desktop))]
    {
        let _ = app;
        Ok(false)
    }
}

#[tauri::command]
pub async fn generate_ticket(engine: State<'_, Arc<VpnEngine>>) -> Result<String> {
    engine.generate_ticket().await
}

// --- settings ------------------------------------------------------------

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Settings> {
    Ok(load_settings(&app))
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    engine: State<'_, Arc<VpnEngine>>,
    settings: Settings,
) -> Result<()> {
    engine.set_relay_url(settings.relay_url.clone());
    store_settings(&app, &settings)
}

// --- autostart -----------------------------------------------------------

// Autostart uses a scheduled task that runs with highest privileges at logon,
// so the elevated app starts silently (no UAC prompt) and can bring the tunnel
// up immediately. (A registry Run key cannot launch an elevated app at login.)
const TASK_NAME: &str = "MyVPN Autostart";

fn current_exe_path() -> Result<String> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
        .ok_or_else(|| VpnError::msg("cannot resolve executable path"))
}

#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool> {
    use std::process::Command;
    if enabled {
        let exe = current_exe_path()?;
        let run = format!("\"{exe}\" --minimized");
        let out = Command::new("schtasks")
            .args([
                "/Create", "/TN", TASK_NAME, "/TR", &run, "/SC", "ONLOGON", "/RL", "HIGHEST",
                "/F",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| VpnError::msg(format!("schtasks failed: {e}")))?;
        if !out.status.success() {
            return Err(VpnError::msg(format!(
                "could not enable start on boot: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
    } else {
        let _ = Command::new("schtasks")
            .args(["/Delete", "/TN", TASK_NAME, "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
    let mut s = load_settings(&app);
    s.autostart = enabled;
    let _ = store_settings(&app, &s);
    Ok(enabled)
}

#[tauri::command]
pub async fn get_autostart(_app: AppHandle) -> Result<bool> {
    use std::process::Command;
    let exists = Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    Ok(exists)
}

// --- window / lifecycle --------------------------------------------------

#[tauri::command]
pub async fn show_window(app: AppHandle) -> Result<()> {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) -> Result<()> {
    app.exit(0);
    Ok(())
}

// --- startup resume ------------------------------------------------------

/// If the user had "resume hosting" enabled and was hosting at shutdown,
/// automatically bring the host back up shortly after launch.
pub fn maybe_resume_hosting(app: &AppHandle, engine: Arc<VpnEngine>) {
    let settings = load_settings(app);
    if !(settings.resume_hosting && settings.was_hosting) {
        return;
    }
    let Some(name) = settings.last_network_name.clone() else {
        return;
    };
    let proof = load_host_proof(app);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        if engine.start_host(name.clone(), proof).await.is_ok() {
            let _ = app.emit(EVT_LOG, format!("Auto-resumed hosting \"{name}\" on startup"));
        }
    });
}
