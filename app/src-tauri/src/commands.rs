//! Tauri commands — the bridge between the React UI and the VPN engine.

use std::sync::Arc;

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

// --- status / control ----------------------------------------------------

#[tauri::command]
pub async fn get_status(engine: State<'_, Arc<VpnEngine>>) -> Result<StatusSnapshot> {
    Ok(engine.snapshot())
}

#[tauri::command]
pub async fn list_discovered(
    engine: State<'_, Arc<VpnEngine>>,
) -> Result<Vec<DiscoveredHost>> {
    Ok(engine.list_discovered().await)
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
    engine: State<'_, Arc<VpnEngine>>,
    public: State<'_, Arc<PublicVpn>>,
    config: ConnectConfig,
) -> Result<StatusSnapshot> {
    if public.is_active() {
        return Err(VpnError::msg(
            "Disconnect the public VPN before joining a private network.",
        ));
    }
    engine.connect(config).await
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
