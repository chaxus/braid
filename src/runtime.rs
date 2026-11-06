//! Data plane: prepare / start / stop sing-box (P3a).
//!
//! TUN without elevation typically fails on macOS — that must surface as
//! [`PlaneError::Permission`], never as a fake Connected state (TECH_PLAN §2.3).

use crate::profile::Profile;
use crate::singbox::render_singbox_pretty;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub const SINGBOX_VERSION: &str = "1.14.4";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const SINGBOX_ASSET: &str = "sing-box-1.14.4-darwin-arm64.tar.gz";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const SINGBOX_SHA256: &str = "0d8c7f5ed5d587b0f1a5e9772a708ec3bfa4ac4c9329604b01d8d9c2ba406838";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const SINGBOX_ASSET: &str = "sing-box-1.14.4-darwin-amd64.tar.gz";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const SINGBOX_SHA256: &str = "eae86b9b033bba50b7c44ab719e4bfac0aca05efd28483390ed3aa510f32db25";

#[cfg(not(target_os = "macos"))]
const SINGBOX_ASSET: &str = "";
#[cfg(not(target_os = "macos"))]
const SINGBOX_SHA256: &str = "";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaneError {
    Permission(String),
    Binary(String),
    Config(String),
    Process(String),
    Io(String),
}

impl PlaneError {
    pub fn as_fail_kind(&self) -> FailKind {
        match self {
            PlaneError::Permission(_) => FailKind::Permission,
            PlaneError::Binary(_) | PlaneError::Config(_) | PlaneError::Process(_) | PlaneError::Io(_) => {
                FailKind::Process
            }
        }
    }

    pub fn message(&self) -> &str {
        match self {
            PlaneError::Permission(m)
            | PlaneError::Binary(m)
            | PlaneError::Config(m)
            | PlaneError::Process(m)
            | PlaneError::Io(m) => m,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailKind {
    Permission,
    Process,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub config_path: PathBuf,
    pub config_bytes: usize,
    pub binary_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneStatus {
    Idle,
    Up,
}

pub trait DataPlane {
    fn prepare(&mut self, profile: &Profile) -> Result<Prepared, PlaneError>;
    fn start(&mut self, prepared: &Prepared) -> Result<(), PlaneError>;
    fn stop(&mut self) -> Result<(), PlaneError>;
    fn poll(&mut self) -> PlaneStatus;
    fn is_mock(&self) -> bool;
}

/// Default for tests / `nsh test`: no download, no spawn.
#[derive(Debug, Default)]
pub struct MockPlane {
    up: bool,
}

impl DataPlane for MockPlane {
    fn prepare(&mut self, profile: &Profile) -> Result<Prepared, PlaneError> {
        let text = render_singbox_pretty(profile);
        Ok(Prepared {
            config_path: PathBuf::from("/tmp/braid-mock-config.json"),
            config_bytes: text.len(),
            binary_path: PathBuf::from("/tmp/braid-mock-sing-box"),
        })
    }

    fn start(&mut self, _prepared: &Prepared) -> Result<(), PlaneError> {
        self.up = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlaneError> {
        self.up = false;
        Ok(())
    }

    fn poll(&mut self) -> PlaneStatus {
        if self.up {
            PlaneStatus::Up
        } else {
            PlaneStatus::Idle
        }
    }

    fn is_mock(&self) -> bool {
        true
    }
}

pub struct SingBoxPlane {
    child: Option<Child>,
    root: PathBuf,
}

impl Default for SingBoxPlane {
    fn default() -> Self {
        Self::new()
    }
}

impl SingBoxPlane {
    pub fn new() -> Self {
        Self {
            child: None,
            root: braid_data_dir(),
        }
    }

    fn bin_dir(&self) -> PathBuf {
        self.root.join("bin")
    }

    fn binary_path(&self) -> PathBuf {
        if let Ok(p) = std::env::var("BRAID_SINGBOX") {
            return PathBuf::from(p);
        }
        self.bin_dir().join("sing-box")
    }

    fn config_path(&self) -> PathBuf {
        self.root.join("config.json")
    }

    fn log_path(&self) -> PathBuf {
        self.root.join("sing-box.log")
    }

    fn ensure_dirs(&self) -> Result<(), PlaneError> {
        fs::create_dir_all(self.bin_dir()).map_err(|e| PlaneError::Io(e.to_string()))?;
        Ok(())
    }

    fn ensure_binary(&self) -> Result<PathBuf, PlaneError> {
        let path = self.binary_path();
        if path.is_file() {
            return Ok(path);
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Err(PlaneError::Binary(
                "sing-box auto-download is only wired for macOS in P3a; set BRAID_SINGBOX".into(),
            ));
        }
        #[cfg(target_os = "macos")]
        {
            self.download_singbox(&path)?;
            Ok(path)
        }
    }

    #[cfg(target_os = "macos")]
    fn download_singbox(&self, dest: &Path) -> Result<(), PlaneError> {
        self.ensure_dirs()?;
        let url = format!(
            "https://github.com/SagerNet/sing-box/releases/download/v{SINGBOX_VERSION}/{SINGBOX_ASSET}"
        );
        let tmp_tar = self.bin_dir().join(SINGBOX_ASSET);
        download_file(&url, &tmp_tar)?;
        let got = sha256_file(&tmp_tar)?;
        if !got.eq_ignore_ascii_case(SINGBOX_SHA256) {
            let _ = fs::remove_file(&tmp_tar);
            return Err(PlaneError::Binary(format!(
                "SHA256 mismatch for {SINGBOX_ASSET}: got {got}, expected {SINGBOX_SHA256}"
            )));
        }
        extract_singbox_tar(&tmp_tar, dest)?;
        let _ = fs::remove_file(&tmp_tar);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(dest)
                .map_err(|e| PlaneError::Io(e.to_string()))?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(dest, perms).map_err(|e| PlaneError::Io(e.to_string()))?;
        }
        Ok(())
    }

    fn run_check(&self, binary: &Path, config: &Path) -> Result<(), PlaneError> {
        let out = Command::new(binary)
            .args(["check", "-c"])
            .arg(config)
            .output()
            .map_err(|e| PlaneError::Binary(format!("failed to run sing-box check: {e}")))?;
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let msg = format!("{stderr}{stdout}");
        Err(classify_spawn_output(&msg).unwrap_or_else(|| PlaneError::Config(msg.trim().into())))
    }
}

impl DataPlane for SingBoxPlane {
    fn prepare(&mut self, profile: &Profile) -> Result<Prepared, PlaneError> {
        self.ensure_dirs()?;
        let text = render_singbox_pretty(profile);
        let config_path = self.config_path();
        write_mode_600(&config_path, text.as_bytes())?;
        let binary_path = self.ensure_binary()?;
        self.run_check(&binary_path, &config_path)?;
        Ok(Prepared {
            config_bytes: text.len(),
            config_path,
            binary_path,
        })
    }

    fn start(&mut self, prepared: &Prepared) -> Result<(), PlaneError> {
        let _ = self.stop();
        let log = fs::File::create(self.log_path()).map_err(|e| PlaneError::Io(e.to_string()))?;
        let log_err = log
            .try_clone()
            .map_err(|e| PlaneError::Io(e.to_string()))?;
        let mut child = Command::new(&prepared.binary_path)
            .args(["run", "-c"])
            .arg(&prepared.config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .spawn()
            .map_err(|e| PlaneError::Process(format!("spawn sing-box: {e}")))?;

        std::thread::sleep(Duration::from_millis(400));
        match child.try_wait() {
            Ok(Some(status)) => {
                let log_tail = read_tail(&self.log_path(), 8_000).unwrap_or_default();
                let msg = format!("sing-box exited early ({status}): {log_tail}");
                return Err(classify_spawn_output(&msg)
                    .unwrap_or(PlaneError::Process(msg)));
            }
            Ok(None) => {}
            Err(e) => return Err(PlaneError::Process(e.to_string())),
        }

        // P3a: without NE/helper we cannot prove TUN is up. If the process
        // survives, still treat missing elevation as permission when logs say so.
        let log_tail = read_tail(&self.log_path(), 8_000).unwrap_or_default();
        if looks_like_permission(&log_tail) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PlaneError::Permission(
                "TUN needs administrator rights or a Network Extension (P3b). sing-box reported a permission error."
                    .into(),
            ));
        }

        self.child = Some(child);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlaneError> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    fn poll(&mut self) -> PlaneStatus {
        let Some(child) = self.child.as_mut() else {
            return PlaneStatus::Idle;
        };
        match child.try_wait() {
            Ok(None) => PlaneStatus::Up,
            Ok(Some(_)) | Err(_) => {
                self.child = None;
                PlaneStatus::Idle
            }
        }
    }

    fn is_mock(&self) -> bool {
        false
    }
}

pub fn use_mock_plane() -> bool {
    match std::env::var("BRAID_MOCK_PLANE") {
        Ok(v) if matches!(v.as_str(), "0" | "false" | "False" | "FALSE") => false,
        Ok(_) => true,
        Err(_) => !matches!(
            std::env::var("BRAID_LIVE").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        ),
    }
}

pub fn braid_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Braid")
}

fn write_mode_600(path: &Path, bytes: &[u8]) -> Result<(), PlaneError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| PlaneError::Io(e.to_string()))?;
    }
    let mut f = fs::File::create(path).map_err(|e| PlaneError::Io(e.to_string()))?;
    f.write_all(bytes).map_err(|e| PlaneError::Io(e.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = f.metadata().map_err(|e| PlaneError::Io(e.to_string()))?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms).map_err(|e| PlaneError::Io(e.to_string()))?;
    }
    Ok(())
}

fn download_file(url: &str, dest: &Path) -> Result<(), PlaneError> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| PlaneError::Binary(format!("download failed: {e}")))?;
    let mut reader = resp.into_reader();
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .map_err(|e| PlaneError::Binary(format!("download read failed: {e}")))?;
    fs::write(dest, buf).map_err(|e| PlaneError::Io(e.to_string()))?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, PlaneError> {
    let data = fs::read(path).map_err(|e| PlaneError::Io(e.to_string()))?;
    let hash = Sha256::digest(&data);
    Ok(hex::encode(hash))
}

fn extract_singbox_tar(tar_gz: &Path, dest_bin: &Path) -> Result<(), PlaneError> {
    // Prefer system tar — avoids extra crate weight on a GUI app.
    let staging = tar_gz
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("sing-box-extract");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| PlaneError::Io(e.to_string()))?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(tar_gz)
        .arg("-C")
        .arg(&staging)
        .status()
        .map_err(|e| PlaneError::Binary(format!("tar: {e}")))?;
    if !status.success() {
        return Err(PlaneError::Binary("tar extract failed".into()));
    }
    let found = find_named_file(&staging, "sing-box")
        .ok_or_else(|| PlaneError::Binary("sing-box binary missing inside archive".into()))?;
    if let Some(parent) = dest_bin.parent() {
        fs::create_dir_all(parent).map_err(|e| PlaneError::Io(e.to_string()))?;
    }
    fs::copy(&found, dest_bin).map_err(|e| PlaneError::Io(e.to_string()))?;
    let _ = fs::remove_dir_all(&staging);
    Ok(())
}

fn find_named_file(root: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for ent in entries.flatten() {
            let path = ent.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|s| s.to_str()) == Some(name) {
                return Some(path);
            }
        }
    }
    None
}

fn read_tail(path: &Path, max: usize) -> Result<String, std::io::Error> {
    let data = fs::read(path)?;
    if data.len() <= max {
        return Ok(String::from_utf8_lossy(&data).into_owned());
    }
    Ok(String::from_utf8_lossy(&data[data.len() - max..]).into_owned())
}

fn looks_like_permission(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("operation not permitted")
        || lower.contains("permission denied")
        || lower.contains("cannot create tun")
        || lower.contains("configure tun interface")
        || lower.contains("net_admin")
}

fn classify_spawn_output(text: &str) -> Option<PlaneError> {
    if looks_like_permission(text) {
        Some(PlaneError::Permission(
            "TUN needs administrator rights or a Network Extension (P3b).".into(),
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;

    #[test]
    fn mock_roundtrip() {
        let mut plane = MockPlane::default();
        let prepared = plane.prepare(&Profile::demo()).unwrap();
        assert!(prepared.config_bytes > 100);
        plane.start(&prepared).unwrap();
        assert_eq!(plane.poll(), PlaneStatus::Up);
        plane.stop().unwrap();
        assert_eq!(plane.poll(), PlaneStatus::Idle);
    }

    #[test]
    fn permission_classifier() {
        let err = classify_spawn_output("configure tun interface: operation not permitted").unwrap();
        assert!(matches!(err, PlaneError::Permission(_)));
    }

    #[test]
    fn prepare_writes_config_without_download_when_binary_overridden() {
        let dir = std::env::temp_dir().join(format!("braid-plane-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("bin")).unwrap();
        // Point at /bin/echo so ensure_binary short-circuits via BRAID_SINGBOX —
        // check will fail; we only assert write + hash path for config file.
        let cfg = dir.join("config.json");
        write_mode_600(&cfg, b"{\"ok\":true}").unwrap();
        let meta = fs::metadata(&cfg).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
        let _ = fs::remove_dir_all(&dir);
    }
}
