//! Network Extension control plane (P3b-NE).
//!
//! Host writes sing-box JSON into the App Group; the Packet Tunnel appex
//! runs it. NEVPN start/stop goes through `NETunnelProviderManager` when the
//! signed appex is installed; otherwise callers fall back to braid-helper.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const APP_GROUP_ID: &str = "group.com.chaxus.braid";
pub const TUNNEL_BUNDLE_ID: &str = "com.chaxus.braid.tunnel";
pub const CONFIG_RELATIVE: &str = "Library/Application Support/Braid/ne-config.json";
pub const STATUS_RELATIVE: &str = "Library/Application Support/Braid/ne-status.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NeError {
    /// Appex missing, unsigned host, or user has not approved VPN — use helper.
    Unavailable(String),
    Io(String),
    Vpn(String),
}

impl NeError {
    pub fn message(&self) -> &str {
        match self {
            NeError::Unavailable(m) | NeError::Io(m) | NeError::Vpn(m) => m,
        }
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self, NeError::Unavailable(_))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NeStatus {
    pub state: String,
    #[serde(default)]
    pub ts: u64,
    #[serde(default)]
    pub detail: Option<String>,
}

impl NeStatus {
    pub fn is_up(&self) -> bool {
        self.state == "up"
    }
}

/// `~/Library/Group Containers/group.com.chaxus.braid`
pub fn group_container() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("Library/Group Containers").join(APP_GROUP_ID))
}

pub fn config_path() -> Option<PathBuf> {
    group_container().map(|c| c.join(CONFIG_RELATIVE))
}

pub fn status_path() -> Option<PathBuf> {
    group_container().map(|c| c.join(STATUS_RELATIVE))
}

/// Write sing-box JSON for the Packet Tunnel provider.
pub fn write_config(json: &str) -> Result<PathBuf, NeError> {
    let path = config_path().ok_or_else(|| NeError::Io("home dir unavailable".into()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| NeError::Io(e.to_string()))?;
    }
    fs::write(&path, json.as_bytes()).map_err(|e| NeError::Io(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .map_err(|e| NeError::Io(e.to_string()))?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms).map_err(|e| NeError::Io(e.to_string()))?;
    }
    Ok(path)
}

pub fn read_status() -> Option<NeStatus> {
    let path = status_path()?;
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn write_status_for_tests(dir: &Path, status: &NeStatus) -> Result<(), NeError> {
    let path = dir.join(STATUS_RELATIVE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| NeError::Io(e.to_string()))?;
    }
    let text = serde_json::to_string_pretty(status).map_err(|e| NeError::Io(e.to_string()))?;
    fs::write(path, text).map_err(|e| NeError::Io(e.to_string()))
}

pub fn now_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Start the system VPN session that loads `com.chaxus.braid.tunnel`.
pub fn start_vpn() -> Result<(), NeError> {
    #[cfg(target_os = "macos")]
    {
        macos::start_vpn()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(NeError::Unavailable(
            "Network Extension is only available on macOS".into(),
        ))
    }
}

pub fn stop_vpn() -> Result<(), NeError> {
    #[cfg(target_os = "macos")]
    {
        macos::stop_vpn()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

/// True when status file says up, or (on macOS) NEVPN connection is connected.
pub fn tunnel_seems_up() -> bool {
    if read_status().is_some_and(|s| s.is_up()) {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        return macos::connection_is_connected();
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::NeError;
    use block2::RcBlock;
    use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoopRunInMode};
    use objc2::ClassType;
    use objc2::rc::Retained;
    use objc2_foundation::{NSArray, NSError, NSString};
    use objc2_network_extension::{
        NETunnelProviderManager, NETunnelProviderProtocol, NEVPNStatus,
    };
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use super::TUNNEL_BUNDLE_ID;

    fn wait_runloop(done: &Cell<bool>, timeout: Duration) -> bool {
        let start = Instant::now();
        while !done.get() {
            if start.elapsed() > timeout {
                return false;
            }
            unsafe {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, 0);
            }
        }
        true
    }

    pub fn start_vpn() -> Result<(), NeError> {
        let manager = load_or_create_manager()?;
        unsafe {
            manager.setEnabled(true);
            manager
                .connection()
                .startVPNTunnelAndReturnError()
                .map_err(|e| NeError::Vpn(format!("startVPNTunnel: {e}")))?;
        }
        Ok(())
    }

    pub fn stop_vpn() -> Result<(), NeError> {
        let managers = load_managers().unwrap_or_default();
        for manager in managers {
            unsafe {
                manager.connection().stopVPNTunnel();
            }
        }
        Ok(())
    }

    pub fn connection_is_connected() -> bool {
        let Ok(managers) = load_managers() else {
            return false;
        };
        managers
            .iter()
            .any(|m| unsafe { m.connection().status() == NEVPNStatus::Connected })
    }

    fn load_managers() -> Result<Vec<Retained<NETunnelProviderManager>>, NeError> {
        let slot: Rc<RefCell<Option<Result<Vec<Retained<NETunnelProviderManager>>, NeError>>>> =
            Rc::new(RefCell::new(None));
        let done = Rc::new(Cell::new(false));
        let slot_c = Rc::clone(&slot);
        let done_c = Rc::clone(&done);
        let block = RcBlock::new(
            move |arr: *mut NSArray<NETunnelProviderManager>, err: *mut NSError| {
                let result = if !err.is_null() {
                    let e = unsafe { &*err };
                    Err(NeError::Unavailable(format!(
                        "loadAllFromPreferences failed: {e}"
                    )))
                } else if arr.is_null() {
                    Ok(Vec::new())
                } else {
                    let array = unsafe { &*arr };
                    let mut out = Vec::new();
                    let count = unsafe { array.count() };
                    for i in 0..count {
                        let obj = unsafe { array.objectAtIndex(i) };
                        out.push(obj);
                    }
                    Ok(out)
                };
                *slot_c.borrow_mut() = Some(result);
                done_c.set(true);
            },
        );
        unsafe {
            NETunnelProviderManager::loadAllFromPreferencesWithCompletionHandler(&block);
        }
        if !wait_runloop(&done, Duration::from_secs(8)) {
            return Err(NeError::Unavailable(
                "Network Extension preference API timed out (build macos/ with a Team and run BraidHost once)"
                    .into(),
            ));
        }
        slot.borrow_mut()
            .take()
            .unwrap_or_else(|| Err(NeError::Vpn("empty NE load callback".into())))
    }

    fn load_or_create_manager() -> Result<Retained<NETunnelProviderManager>, NeError> {
        let mut managers = load_managers()?;
        if let Some(existing) = managers.pop() {
            configure_protocol(&existing)?;
            save_manager(&existing)?;
            return Ok(existing);
        }
        let manager = unsafe { NETunnelProviderManager::new() };
        configure_protocol(&manager)?;
        save_manager(&manager)?;
        let mut again = load_managers()?;
        again.pop().ok_or_else(|| {
            NeError::Unavailable(
                "VPN configuration saved but not visible — enable Network Extension in Xcode and approve VPN"
                    .into(),
            )
        })
    }

    fn configure_protocol(manager: &NETunnelProviderManager) -> Result<(), NeError> {
        unsafe {
            let proto = NETunnelProviderProtocol::new();
            proto.setProviderBundleIdentifier(Some(&NSString::from_str(TUNNEL_BUNDLE_ID)));
            proto.setServerAddress(Some(&NSString::from_str("braid")));
            manager.setProtocolConfiguration(Some(ClassType::as_super(&*proto)));
            manager.setLocalizedDescription(Some(&NSString::from_str("Braid")));
            manager.setEnabled(true);
        }
        Ok(())
    }

    fn save_manager(manager: &NETunnelProviderManager) -> Result<(), NeError> {
        let slot: Rc<RefCell<Option<Result<(), NeError>>>> = Rc::new(RefCell::new(None));
        let done = Rc::new(Cell::new(false));
        let slot_c = Rc::clone(&slot);
        let done_c = Rc::clone(&done);
        let block = RcBlock::new(move |err: *mut NSError| {
            let result = if !err.is_null() {
                let e = unsafe { &*err };
                Err(NeError::Unavailable(format!(
                    "saveToPreferences failed: {e} (approve VPN / sign with Team)"
                )))
            } else {
                Ok(())
            };
            *slot_c.borrow_mut() = Some(result);
            done_c.set(true);
        });
        unsafe {
            manager.saveToPreferencesWithCompletionHandler(Some(&block));
        }
        if !wait_runloop(&done, Duration::from_secs(8)) {
            return Err(NeError::Unavailable(
                "saveToPreferences timed out".into(),
            ));
        }
        slot.borrow_mut()
            .take()
            .unwrap_or_else(|| Err(NeError::Vpn("empty NE save callback".into())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrip_helpers() {
        let dir = std::env::temp_dir().join(format!("braid-ne-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let status = NeStatus {
            state: "up".into(),
            ts: 1,
            detail: None,
        };
        write_status_for_tests(&dir, &status).unwrap();
        let path = dir.join(STATUS_RELATIVE);
        let got: NeStatus = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert!(got.is_up());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn config_write_uses_group_path_shape() {
        let p = config_path().expect("home");
        assert!(p.to_string_lossy().contains(APP_GROUP_ID));
        assert!(p.ends_with("ne-config.json"));
    }
}
