//! Launch and control the bundled OpenVPN client for the public VPN mode.
//!
//! We talk to OpenVPN over its localhost **management interface** so we can both
//! observe connection state (`>STATE:` notifications) and shut it down cleanly
//! (`signal SIGTERM`), which lets OpenVPN remove its own routes/DNS on exit
//! instead of leaving them behind after a hard process kill.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use anyhow::Context;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Locate the OpenVPN executable. Prefers the copy bundled inside the installed
/// app (no separate OpenVPN install is required); falls back to a system-wide
/// install if, for some reason, the bundle is missing.
pub fn find_openvpn(resource_dir: &Path) -> Option<PathBuf> {
    let mut candidates = vec![
        resource_dir.join("openvpn").join("openvpn.exe"),
        resource_dir.join("openvpn.exe"),
    ];
    // Some packaging layouts place resources next to the executable rather than
    // under the reported resource dir; check those too.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("openvpn").join("openvpn.exe"));
            candidates.push(dir.join("resources").join("openvpn").join("openvpn.exe"));
        }
    }
    // Last resort: a system-wide OpenVPN install.
    candidates.push(PathBuf::from(r"C:\Program Files\OpenVPN\bin\openvpn.exe"));
    candidates.push(PathBuf::from(r"C:\Program Files (x86)\OpenVPN\bin\openvpn.exe"));

    candidates.into_iter().find(|p| p.exists())
}

/// Reserve a free localhost TCP port for the management interface. The listener
/// is dropped immediately; OpenVPN then binds the port itself.
fn free_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").context("reserve management port")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

/// A spawned OpenVPN process plus the management port it listens on.
pub struct OpenVpnProcess {
    pub child: Child,
    pub mgmt_port: u16,
}

/// Spawn OpenVPN against a config file with a localhost management interface.
///
/// Compatibility cipher flags are added so the older configs published by free
/// servers still negotiate with a modern OpenVPN build, and `block-outside-dns`
/// is filtered out so we don't require the privileged WFP/service component.
pub fn spawn(openvpn: &Path, config: &Path, work_dir: &Path) -> anyhow::Result<OpenVpnProcess> {
    let mgmt_port = free_port()?;
    let mut cmd = Command::new(openvpn);
    cmd.arg("--config")
        .arg(config)
        .arg("--management")
        .arg("127.0.0.1")
        .arg(mgmt_port.to_string())
        .arg("--management-query-passwords")
        .arg("--data-ciphers")
        .arg("AES-256-GCM:AES-128-GCM:AES-256-CBC:AES-128-CBC:BF-CBC")
        .arg("--data-ciphers-fallback")
        .arg("AES-128-CBC")
        .arg("--connect-retry-max")
        .arg("3")
        .arg("--pull-filter")
        .arg("ignore")
        .arg("block-outside-dns")
        .current_dir(work_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let child = cmd.spawn().context("launch OpenVPN")?;
    Ok(OpenVpnProcess { child, mgmt_port })
}
