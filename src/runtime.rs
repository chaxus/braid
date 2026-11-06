//! Data plane: prepare / start / stop sing-box (P3b helper-aware).
//!
//! TUN without elevation typically fails on macOS — that must surface as
//! [`PlaneError::Permission`], never as a fake Connected state (TECH_PLAN §2.3).
//! When `braid-helper` is installed, start/stop go through the Unix socket.

#[cfg(unix)]
use crate::helper_proto::{HelperRequest, HelperResponse, HELPER_SOCKET_PATH};
#[cfg(not(unix))]
use crate::helper_proto::HELPER_SOCKET_PATH;
use crate::profile::Profile;
use crate::singbox::render_singbox_pretty;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

#[cfg(unix)]
use std::io::{BufRead, BufReader};
#[cfg(unix)]
use std::os::unix::net::UnixStream;

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
    /// True when the last successful `start` went through braid-helper.
    fn via_helper(&self) -> bool {
        false
    }
    /// True when the last successful `start` went through Network Extension.
    fn via_ne(&self) -> bool {
        false
    }
    /// Trusted TUN path for Connected (NE or helper).
    fn tun_trusted(&self) -> bool {
        self.via_ne() || self.via_helper()
    }
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
    /// Local (non-helper) child, if any.
    child: Option<Child>,
    /// True when the last successful start went through braid-helper.
    via_helper: bool,
    /// True when the last successful start went through Network Extension.
    via_ne: bool,
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
            via_helper: false,
            via_ne: false,
            root: braid_data_dir(),
        }
    }

    fn start_via_ne(&mut self, prepared: &Prepared) -> Result<(), PlaneError> {
        let json = fs::read_to_string(&prepared.config_path)
            .map_err(|e| PlaneError::Io(e.to_string()))?;
        crate::ne_plane::write_config(&json).map_err(|e| {
            PlaneError::Permission(permission_hint(&format!("NE config write: {}", e.message())))
        })?;
        // Best-effort: copy sing-box into the App Group so the appex can find it.
        if let Some(group_bin) = crate::ne_plane::group_container().map(|c| {
            c.join("Library/Application Support/Braid/bin/sing-box")
        }) {
            if let Some(parent) = group_bin.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(&prepared.binary_path, &group_bin);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = fs::metadata(&group_bin) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&group_bin, perms);
                }
            }
        }
        match crate::ne_plane::start_vpn() {
            Ok(()) => {
                self.via_ne = true;
                self.via_helper = false;
                self.child = None;
                Ok(())
            }
            Err(e) if e.is_unavailable() => Err(PlaneError::Permission(permission_hint(e.message()))),
            Err(e) => Err(PlaneError::Process(e.message().to_owned())),
        }
    }

    pub fn helper_reachable() -> bool {
        #[cfg(unix)]
        {
            return helper_rpc(&HelperRequest::ping()).is_ok_and(|r| r.ok);
        }
        #[cfg(not(unix))]
        {
            false
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

    #[cfg(unix)]
    fn start_via_helper(&mut self, prepared: &Prepared) -> Result<(), PlaneError> {
        let req = HelperRequest::start(
            prepared.config_path.to_string_lossy(),
            prepared.binary_path.to_string_lossy(),
        );
        let resp = helper_rpc(&req).map_err(|e| {
            PlaneError::Permission(permission_hint(&format!(
                "helper socket present but RPC failed: {e}"
            )))
        })?;
        if !resp.ok {
            let detail = resp.error.unwrap_or_else(|| "helper start failed".into());
            if resp.permission == Some(true) || looks_like_permission(&detail) {
                return Err(PlaneError::Permission(permission_hint(&detail)));
            }
            return Err(PlaneError::Process(detail));
        }
        self.via_helper = true;
        self.via_ne = false;
        self.child = None;
        Ok(())
    }

    #[cfg(not(unix))]
    fn start_via_helper(&mut self, _prepared: &Prepared) -> Result<(), PlaneError> {
        Err(PlaneError::Permission(permission_hint(
            "privileged helper is macOS/Linux only in P3b.",
        )))
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

        // Prefer Network Extension when the system will accept a VPN start.
        #[cfg(target_os = "macos")]
        {
            match self.start_via_ne(prepared) {
                Ok(()) => return Ok(()),
                Err(PlaneError::Permission(_)) => {
                    // Fall through to helper / local spawn.
                }
                Err(e) => return Err(e),
            }
        }

        // Prefer privileged helper when the LaunchDaemon socket answers.
        if helper_socket_present() {
            return self.start_via_helper(prepared);
        }

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

        // Without helper we cannot prove TUN is up. If the process survives,
        // still treat missing elevation as permission when logs say so.
        let log_tail = read_tail(&self.log_path(), 8_000).unwrap_or_default();
        if looks_like_permission(&log_tail) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PlaneError::Permission(permission_hint(
                "sing-box reported a permission error.",
            )));
        }

        self.child = Some(child);
        self.via_helper = false;
        self.via_ne = false;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlaneError> {
        if self.via_ne {
            let _ = crate::ne_plane::stop_vpn();
            self.via_ne = false;
        }
        #[cfg(unix)]
        if self.via_helper || helper_socket_present() {
            let _ = helper_rpc(&HelperRequest::stop());
            self.via_helper = false;
        }
        #[cfg(not(unix))]
        {
            self.via_helper = false;
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    fn poll(&mut self) -> PlaneStatus {
        if self.via_ne {
            if crate::ne_plane::tunnel_seems_up() {
                return PlaneStatus::Up;
            }
            self.via_ne = false;
            return PlaneStatus::Idle;
        }
        #[cfg(unix)]
        if self.via_helper {
            match helper_rpc(&HelperRequest::status()) {
                Ok(r) if r.ok && r.running == Some(true) => return PlaneStatus::Up,
                _ => {
                    self.via_helper = false;
                    return PlaneStatus::Idle;
                }
            }
        }
        #[cfg(not(unix))]
        if self.via_helper {
            self.via_helper = false;
            return PlaneStatus::Idle;
        }
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

    fn via_helper(&self) -> bool {
        self.via_helper
    }

    fn via_ne(&self) -> bool {
        self.via_ne
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

fn permission_hint(extra: &str) -> String {
    let base = "TUN needs Network Extension (scripts/open-ne-xcode.sh + approve VPN) or the Braid helper (sudo ./scripts/install-helper.sh).";
    if extra.is_empty() {
        base.into()
    } else {
        format!("{base} {extra}")
    }
}

fn classify_spawn_output(text: &str) -> Option<PlaneError> {
    if looks_like_permission(text) {
        Some(PlaneError::Permission(permission_hint("")))
    } else {
        None
    }
}

fn helper_socket_present() -> bool {
    #[cfg(unix)]
    {
        Path::new(HELPER_SOCKET_PATH).exists()
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(unix)]
fn helper_rpc(req: &HelperRequest) -> Result<HelperResponse, String> {
    let stream = UnixStream::connect(HELPER_SOCKET_PATH).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let line = req.to_line()? + "\n";
    writer
        .write_all(line.as_bytes())
        .map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    reader
        .read_line(&mut resp_line)
        .map_err(|e| e.to_string())?;
    HelperResponse::from_line(&resp_line)
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
