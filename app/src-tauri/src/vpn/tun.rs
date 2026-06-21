//! Wintun TUN adapter wrapper for the VPN data plane.
//!
//! Requires the `wintun.dll` driver (from <https://www.wintun.net>) to be
//! present next to the executable, and Administrator privileges to create the
//! virtual adapter.

use std::sync::Arc;

use crate::error::{Result, VpnError};
use crate::vpn::net;

/// A live Wintun adapter plus its packet session.
pub struct Tun {
    // Kept alive for the lifetime of the session.
    _adapter: Arc<wintun::Adapter>,
    session: Arc<wintun::Session>,
}

impl Tun {
    /// Create the adapter, start a session, and assign its address + MTU.
    pub fn create(name: &str, ip: &str, prefix: u8, mtu: u32) -> Result<Self> {
        // Prefer the wintun.dll shipped with the app; fall back to the
        // working directory.
        let from_exe = std::env::current_exe().ok().and_then(|exe| {
            exe.parent().and_then(|dir| {
                [dir.join("wintun.dll"), dir.join("resources").join("wintun.dll")]
                    .into_iter()
                    .find(|p| p.exists())
            })
        });
        let loaded = match from_exe {
            Some(path) => unsafe { wintun::load_from_path(&path) },
            None => unsafe { wintun::load() },
        };
        let wintun = loaded
            .map_err(|e| VpnError::msg(format!("Wintun driver (wintun.dll) unavailable: {e}")))?;

        let adapter = wintun::Adapter::create(&wintun, name, "MyVPN", None)
            .map_err(|e| VpnError::msg(format!("create TUN adapter: {e}")))?;

        let session = adapter
            .start_session(wintun::MAX_RING_CAPACITY)
            .map_err(|e| VpnError::msg(format!("start TUN session: {e}")))?;

        net::set_interface_ipv4(name, ip, prefix)?;
        let _ = net::set_mtu(name, mtu);

        Ok(Self {
            _adapter: adapter,
            session: Arc::new(session),
        })
    }

    /// A cloneable handle to the packet session (read/write IP packets).
    pub fn session(&self) -> Arc<wintun::Session> {
        self.session.clone()
    }

    /// Unblock any reader and stop the session.
    pub fn shutdown(&self) {
        self.session.shutdown().ok();
    }
}
