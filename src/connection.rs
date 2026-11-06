//! Connection state machine — drives a [`DataPlane`] (TECH_PLAN §3.3 / §4).

use crate::probe::{classify_path, classify_with, local_reachable, tcp_reachable, tcp_rtt_ms, PathClass};
use crate::profile::Profile;
use crate::runtime::{DataPlane, FailKind, PlaneError, PlaneStatus};
use crate::singbox::render_singbox_pretty;
use std::time::Duration;

/// How long live Connect waits for a TCP path to the server.
const PATH_PROBE_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Mock / Server reserved for richer failure mapping.
pub enum FailReason {
    Mock,
    Permission,
    Path,
    /// Local network probe failed (do not blame the VPS).
    Local,
    Server,
    Process,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub phase: Phase,
    pub fail: Option<FailReason>,
    pub fail_detail: Option<String>,
    pub node: String,
    pub latency_ms: Option<u32>,
    pub egress_ip: Option<String>,
    /// True when the active DataPlane is mock (tests / default without BRAID_LIVE).
    pub mock_plane: bool,
    /// True when braid-helper answered ping at UI start (live plane only).
    pub helper_online: bool,
    pub profile_loaded: bool,
    pub config_bytes: usize,
    pub server: Option<String>,
    pub profile: Option<Profile>,
    /// Latest human-readable diagnostics line (not necessarily a failure).
    pub diag_line: Option<String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            fail: None,
            fail_detail: None,
            node: "No profile".to_owned(),
            latency_ms: None,
            egress_ip: None,
            mock_plane: true,
            helper_online: false,
            profile_loaded: false,
            config_bytes: 0,
            server: None,
            profile: None,
            diag_line: None,
        }
    }
}

impl Snapshot {
    pub fn status_title(&self) -> &'static str {
        match self.phase {
            Phase::Idle => "Disconnected",
            Phase::Connecting => "Connecting…",
            Phase::Connected => "Connected",
            Phase::Disconnecting => "Disconnecting…",
            Phase::Failed => "Failed",
        }
    }

    pub fn primary_label(&self) -> &'static str {
        match self.phase {
            Phase::Idle | Phase::Failed => "Connect",
            Phase::Connecting => "Connecting…",
            Phase::Connected => "Disconnect",
            Phase::Disconnecting => "Disconnecting…",
        }
    }

    pub fn primary_enabled(&self) -> bool {
        self.profile_loaded
            && matches!(self.phase, Phase::Idle | Phase::Connected | Phase::Failed)
    }

    pub fn summary_line(&self) -> String {
        let latency = self
            .latency_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_else(|| "—".to_owned());
        let ip = self
            .egress_ip
            .clone()
            .unwrap_or_else(|| "—".to_owned());
        format!("{} · {} · {}", self.node, latency, ip)
    }

    pub fn config_line(&self) -> String {
        if self.profile_loaded {
            format!("sing-box config ready · {} B", self.config_bytes)
        } else {
            "Load a profile to generate sing-box config".to_owned()
        }
    }

    pub fn set_helper_online(&mut self, online: bool) {
        self.helper_online = online;
    }

    pub fn footer_line(&self) -> String {
        if self.mock_plane {
            "TUN · mock plane (set BRAID_LIVE=1 for real sing-box)".to_owned()
        } else if self.helper_online {
            if self.phase == Phase::Connected {
                "TUN · via braid-helper".to_owned()
            } else {
                "TUN · braid-helper online".to_owned()
            }
        } else if self.phase == Phase::Connected {
            "TUN · live (no helper — TUN may lack privileges)".to_owned()
        } else {
            "TUN · live · helper: sudo ./scripts/install-helper.sh".to_owned()
        }
    }

    pub fn fail_copy(&self) -> Option<String> {
        let reason = self.fail?;
        let base = match reason {
            FailReason::Mock => "Simulated failure.".to_owned(),
            FailReason::Permission => {
                "Install the Braid helper: sudo ./scripts/install-helper.sh".to_owned()
            }
            FailReason::Path => {
                "Path to the server is down (local network looks fine).".to_owned()
            }
            FailReason::Local => "Local network looks down.".to_owned(),
            FailReason::Server => "Server looks unhealthy.".to_owned(),
            FailReason::Process => "Could not start or keep the core process running.".to_owned(),
        };
        match &self.fail_detail {
            Some(d) if !d.is_empty() && reason == FailReason::Process => {
                Some(format!("{base} {d}"))
            }
            Some(d) if reason == FailReason::Permission && !d.is_empty() => {
                Some(format!("{base} {d}"))
            }
            _ => Some(base),
        }
    }

    pub fn apply_profile(&mut self, profile: Profile) {
        let config = render_singbox_pretty(&profile);
        self.node = profile.name.clone();
        self.server = Some(profile.server.clone());
        self.profile_loaded = true;
        self.config_bytes = config.len();
        self.profile = Some(profile);
        self.fail = None;
        self.fail_detail = None;
        if self.phase == Phase::Failed {
            self.phase = Phase::Idle;
        }
    }

    pub fn load_demo_profile(&mut self) {
        self.apply_profile(Profile::demo());
    }

    /// Load `BRAID_PROFILE` or `…/Braid/profile.json`.
    pub fn load_saved_profile(&mut self) {
        match Profile::load_default() {
            Ok(profile) => self.apply_profile(profile),
            Err(e) => self.fail_profile_load(e.to_string()),
        }
    }

    pub fn load_profile_from_path(&mut self, path: &std::path::Path) {
        match Profile::load_from_path(path) {
            Ok(profile) => self.apply_profile(profile),
            Err(e) => self.fail_profile_load(e.to_string()),
        }
    }

    fn fail_profile_load(&mut self, detail: String) {
        self.phase = Phase::Failed;
        self.fail = Some(FailReason::Process);
        self.fail_detail = Some(detail);
        self.latency_ms = None;
        self.egress_ip = None;
    }

    pub fn set_mock_plane(&mut self, mock: bool) {
        self.mock_plane = mock;
    }

    pub fn connect_with(&mut self, plane: &mut dyn DataPlane) {
        if !self.profile_loaded {
            return;
        }
        let Some(profile) = self.profile.clone() else {
            return;
        };
        self.phase = Phase::Connecting;
        self.fail = None;
        self.fail_detail = None;
        self.mock_plane = plane.is_mock();

        match plane.prepare(&profile).and_then(|p| {
            self.config_bytes = p.config_bytes;
            plane.start(&p)?;
            Ok(())
        }) {
            Ok(()) => {
                if plane.poll() != PlaneStatus::Up && !plane.is_mock() {
                    self.phase = Phase::Failed;
                    self.fail = Some(FailReason::Process);
                    self.fail_detail = Some("core exited immediately after start".into());
                    let _ = plane.stop();
                    return;
                }
                // Live plane: TUN needs the privileged helper — do not sit in Connected.
                if !plane.is_mock() && !plane.via_helper() {
                    let _ = plane.stop();
                    self.apply_plane_error(PlaneError::Permission(
                        "sing-box started without braid-helper; TUN is not trusted.".into(),
                    ));
                    return;
                }
                // Live plane: classify local vs path before claiming Connected.
                if !plane.is_mock() {
                    match classify_path(&profile.server, profile.port, PATH_PROBE_TIMEOUT) {
                        PathClass::Ok => {}
                        PathClass::Path => {
                            let _ = plane.stop();
                            self.phase = Phase::Failed;
                            self.fail = Some(FailReason::Path);
                            self.fail_detail = Some(format!(
                                "TCP {}:{} timed out or refused",
                                profile.server, profile.port
                            ));
                            self.latency_ms = None;
                            self.egress_ip = None;
                            return;
                        }
                        PathClass::LocalDown => {
                            let _ = plane.stop();
                            self.phase = Phase::Failed;
                            self.fail = Some(FailReason::Local);
                            self.fail_detail = Some(
                                "local probe (223.5.5.5:53) failed".into(),
                            );
                            self.latency_ms = None;
                            self.egress_ip = None;
                            return;
                        }
                    }
                }
                self.phase = Phase::Connected;
                self.latency_ms = if plane.is_mock() {
                    Some(42)
                } else {
                    tcp_rtt_ms(&profile.server, profile.port, PATH_PROBE_TIMEOUT)
                };
                self.egress_ip = self.server.clone();
            }
            Err(err) => self.apply_plane_error(err),
        }
    }

    pub fn disconnect_with(&mut self, plane: &mut dyn DataPlane) {
        self.phase = Phase::Disconnecting;
        let _ = plane.stop();
        self.phase = Phase::Idle;
        self.latency_ms = None;
        self.egress_ip = None;
        self.fail = None;
        self.fail_detail = None;
    }

    pub fn toggle_primary(&mut self, plane: &mut dyn DataPlane) {
        if !self.primary_enabled() {
            return;
        }
        match self.phase {
            Phase::Idle | Phase::Failed => self.connect_with(plane),
            Phase::Connected => self.disconnect_with(plane),
            Phase::Connecting | Phase::Disconnecting => {}
        }
    }

    /// Refresh helper ping and detect a dead core while Connected.
    pub fn tick(&mut self, plane: &mut dyn DataPlane) {
        if !self.mock_plane {
            self.helper_online = crate::runtime::SingBoxPlane::helper_reachable();
        }
        if self.phase == Phase::Connected && plane.poll() != PlaneStatus::Up {
            self.phase = Phase::Failed;
            self.fail = Some(FailReason::Process);
            self.fail_detail = Some("core exited while connected".into());
            self.latency_ms = None;
            self.egress_ip = None;
            let _ = plane.stop();
        }
    }

    /// One-line path diagnostics (mock stays offline-safe).
    pub fn run_diagnostics(&mut self) {
        if self.mock_plane {
            self.diag_line = Some("Local OK · path OK (mock)".into());
            return;
        }
        let Some(profile) = self.profile.as_ref() else {
            self.diag_line = Some("Load a profile before running diagnostics.".into());
            return;
        };
        let local = local_reachable(PATH_PROBE_TIMEOUT);
        let remote = tcp_reachable(&profile.server, profile.port, PATH_PROBE_TIMEOUT);
        let class = classify_with(local, remote);
        let rtt = if remote {
            tcp_rtt_ms(&profile.server, profile.port, PATH_PROBE_TIMEOUT)
                .map(|ms| format!("{ms} ms"))
                .unwrap_or_else(|| "—".into())
        } else {
            "—".into()
        };
        self.diag_line = Some(match class {
            PathClass::Ok => format!("Local OK · path OK · RTT {rtt}"),
            PathClass::Path => format!(
                "Local OK · path down · {}:{}",
                profile.server, profile.port
            ),
            PathClass::LocalDown => "Local network down · path not checked".into(),
        });
    }

    pub fn diag_copy(&self) -> String {
        self.diag_line.clone().unwrap_or_default()
    }

    pub fn simulate_path_failure(&mut self) {
        self.phase = Phase::Failed;
        self.fail = Some(FailReason::Path);
        self.fail_detail = None;
        self.latency_ms = None;
        self.egress_ip = None;
    }

    fn apply_plane_error(&mut self, err: PlaneError) {
        self.phase = Phase::Failed;
        self.latency_ms = None;
        self.egress_ip = None;
        self.fail_detail = Some(err.message().to_owned());
        self.fail = Some(match err.as_fail_kind() {
            FailKind::Permission => FailReason::Permission,
            FailKind::Process => FailReason::Process,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{MockPlane, Prepared};
    use std::path::PathBuf;

    /// Live-looking plane that starts without helper (Connected must be refused).
    struct LiveNoHelperPlane {
        up: bool,
    }

    impl DataPlane for LiveNoHelperPlane {
        fn prepare(&mut self, profile: &Profile) -> Result<Prepared, PlaneError> {
            let text = render_singbox_pretty(profile);
            Ok(Prepared {
                config_path: PathBuf::from("/tmp/braid-test-config.json"),
                config_bytes: text.len(),
                binary_path: PathBuf::from("/tmp/braid-test-sing-box"),
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
            false
        }

        fn via_helper(&self) -> bool {
            false
        }
    }

    #[test]
    fn connect_requires_profile() {
        let mut s = Snapshot::default();
        let mut plane = MockPlane::default();
        assert!(!s.primary_enabled());
        s.toggle_primary(&mut plane);
        assert_eq!(s.phase, Phase::Idle);
        s.load_demo_profile();
        assert!(s.primary_enabled());
        s.toggle_primary(&mut plane);
        assert_eq!(s.phase, Phase::Connected);
        assert_eq!(s.egress_ip.as_deref(), Some("203.0.113.10"));
        s.toggle_primary(&mut plane);
        assert_eq!(s.phase, Phase::Idle);
    }

    #[test]
    fn tick_detects_dead_core() {
        let mut s = Snapshot::default();
        s.load_demo_profile();
        let mut plane = MockPlane::default();
        s.connect_with(&mut plane);
        assert_eq!(s.phase, Phase::Connected);
        plane.stop().unwrap();
        s.tick(&mut plane);
        assert_eq!(s.phase, Phase::Failed);
        assert_eq!(s.fail, Some(FailReason::Process));
        assert!(s.fail_detail.as_deref().unwrap().contains("exited"));
    }

    #[test]
    fn mock_diagnostics_line() {
        let mut s = Snapshot::default();
        s.run_diagnostics();
        assert_eq!(s.diag_copy(), "Local OK · path OK (mock)");
    }

    #[test]
    fn live_without_helper_is_permission() {
        let mut s = Snapshot::default();
        s.load_demo_profile();
        let mut plane = LiveNoHelperPlane { up: false };
        s.connect_with(&mut plane);
        assert_eq!(s.phase, Phase::Failed);
        assert_eq!(s.fail, Some(FailReason::Permission));
        assert!(!plane.up);
        assert!(
            s.fail_copy()
                .unwrap()
                .contains("install-helper")
                || s.fail_detail
                    .as_deref()
                    .unwrap_or("")
                    .contains("braid-helper")
        );
    }

    struct LiveHelperPlane {
        up: bool,
    }

    impl DataPlane for LiveHelperPlane {
        fn prepare(&mut self, profile: &Profile) -> Result<Prepared, PlaneError> {
            let text = render_singbox_pretty(profile);
            Ok(Prepared {
                config_path: PathBuf::from("/tmp/braid-test-config.json"),
                config_bytes: text.len(),
                binary_path: PathBuf::from("/tmp/braid-test-sing-box"),
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
            false
        }

        fn via_helper(&self) -> bool {
            true
        }
    }

    #[test]
    fn live_unreachable_server_is_path_or_local() {
        let mut s = Snapshot::default();
        s.load_demo_profile();
        // Force a closed local port so the remote probe fails without WAN to the VPS.
        if let Some(p) = s.profile.as_mut() {
            p.server = "127.0.0.1".into();
            p.port = 1;
        }
        s.server = Some("127.0.0.1".into());
        let mut plane = LiveHelperPlane { up: false };
        s.connect_with(&mut plane);
        assert_eq!(s.phase, Phase::Failed);
        // Path if domestic DNS answers; Local if the whole machine is offline.
        assert!(matches!(
            s.fail,
            Some(FailReason::Path) | Some(FailReason::Local)
        ));
        assert!(!plane.up);
    }

    #[test]
    fn path_failure_copy() {
        let mut s = Snapshot::default();
        s.simulate_path_failure();
        assert_eq!(s.phase, Phase::Failed);
        assert!(s.fail_copy().unwrap().contains("Path to the server"));
    }

    #[test]
    fn local_failure_copy() {
        let mut s = Snapshot::default();
        s.phase = Phase::Failed;
        s.fail = Some(FailReason::Local);
        assert!(s.fail_copy().unwrap().contains("Local network"));
    }

    #[test]
    fn load_saved_profile_missing_fails() {
        let missing = std::env::temp_dir().join(format!(
            "braid-snap-missing-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&missing);
        let mut s = Snapshot::default();
        s.load_profile_from_path(&missing);
        assert_eq!(s.phase, Phase::Failed);
        assert!(!s.profile_loaded);
        assert!(s.fail_copy().unwrap().contains("profile I/O"));
    }
}
