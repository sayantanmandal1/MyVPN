//! Error types shared across the MyVPN backend.

use serde::Serialize;

/// The unified error type returned by Tauri commands and the VPN engine.
#[derive(Debug, thiserror::Error)]
pub enum VpnError {
    #[error("{0}")]
    Message(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl VpnError {
    pub fn msg(s: impl Into<String>) -> Self {
        VpnError::Message(s.into())
    }
}

// Tauri requires command errors to be serializable so they can cross to the UI.
impl Serialize for VpnError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, VpnError>;
