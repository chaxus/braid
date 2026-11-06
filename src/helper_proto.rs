//! JSON-line protocol between the UI and `braid-helper` (P3b).

use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// LaunchDaemon listens here (root-owned).
pub const HELPER_SOCKET_PATH: &str = "/var/run/braid-helper.sock";

pub const HELPER_LABEL: &str = "com.chaxus.braid.helper";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum HelperRequest {
    Ping { v: u32 },
    Start {
        v: u32,
        config: String,
        binary: String,
    },
    Stop { v: u32 },
    Status { v: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HelperResponse {
    pub v: u32,
    pub ok: bool,
    pub op: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission: Option<bool>,
}

impl HelperRequest {
    pub fn ping() -> Self {
        Self::Ping { v: 1 }
    }

    pub fn start(config: impl Into<String>, binary: impl Into<String>) -> Self {
        Self::Start {
            v: 1,
            config: config.into(),
            binary: binary.into(),
        }
    }

    pub fn stop() -> Self {
        Self::Stop { v: 1 }
    }

    pub fn status() -> Self {
        Self::Status { v: 1 }
    }

    pub fn to_line(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn from_line(line: &str) -> Result<Self, String> {
        serde_json::from_str(line.trim()).map_err(|e| e.to_string())
    }
}

impl HelperResponse {
    pub fn ok_ping() -> Self {
        Self {
            v: 1,
            ok: true,
            op: "ping".into(),
            running: None,
            pid: None,
            error: None,
            permission: None,
        }
    }

    pub fn err(op: &str, message: impl Into<String>, permission: bool) -> Self {
        Self {
            v: 1,
            ok: false,
            op: op.into(),
            running: None,
            pid: None,
            error: Some(message.into()),
            permission: if permission { Some(true) } else { None },
        }
    }

    pub fn to_line(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn from_line(line: &str) -> Result<Self, String> {
        serde_json::from_str(line.trim()).map_err(|e| e.to_string())
    }
}

/// Only allow configs/binaries under known Braid directories (no `..`).
///
/// Helper runs as root, so we accept any `…/Library/Application Support/Braid/…`
/// under `/Users` or system Library, not only the caller's `dirs::data_dir()`.
pub fn path_allowed_for_helper(path: &Path) -> bool {
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return false;
    }
    let s = path.to_string_lossy();
    if s.contains("..") {
        return false;
    }
    if let Some(home_data) = dirs::data_dir().map(|d| d.join("Braid")) {
        if path.starts_with(&home_data) {
            return true;
        }
    }
    // /Users/<name>/Library/Application Support/Braid/...
    if let Some(rest) = s.strip_prefix("/Users/") {
        if let Some((_, after_user)) = rest.split_once('/') {
            let marker = "Library/Application Support/Braid";
            if after_user == marker
                || after_user.starts_with(&format!("{marker}/"))
            {
                return true;
            }
        }
    }
    let system = [
        PathBuf::from("/Library/Application Support/Braid"),
        PathBuf::from("/var/root/Library/Application Support/Braid"),
    ];
    system.iter().any(|p| path.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_start() {
        let req = HelperRequest::start(
            "/Library/Application Support/Braid/config.json",
            "/Library/Application Support/Braid/bin/sing-box",
        );
        let line = req.to_line().unwrap();
        let back = HelperRequest::from_line(&line).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn rejects_parent_dir() {
        assert!(!path_allowed_for_helper(Path::new(
            "/Library/Application Support/Braid/../etc/passwd"
        )));
    }

    #[test]
    fn allows_library_braid() {
        assert!(path_allowed_for_helper(Path::new(
            "/Library/Application Support/Braid/config.json"
        )));
    }

    #[test]
    fn allows_user_library_braid() {
        assert!(path_allowed_for_helper(Path::new(
            "/Users/demo/Library/Application Support/Braid/bin/sing-box"
        )));
    }

    #[test]
    fn response_roundtrip() {
        let r = HelperResponse::err("start", "nope", true);
        let back = HelperResponse::from_line(&r.to_line().unwrap()).unwrap();
        assert!(!back.ok);
        assert_eq!(back.permission, Some(true));
    }
}
