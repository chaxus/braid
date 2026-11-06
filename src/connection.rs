//! Connection state machine — drives a [`DataPlane`] (TECH_PLAN §3.3 / §4).

use crate::profile::Profile;
use crate::runtime::{DataPlane, FailKind, PlaneError, PlaneStatus};
use crate::singbox::render_singbox_pretty;

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
    pub profile_loaded: bool,
    pub config_bytes: usize,
    pub server: Option<String>,
    pub profile: Option<Profile>,
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
            profile_loaded: false,
            config_bytes: 0,
            server: None,
            profile: None,
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

    pub fn footer_line(&self) -> String {
        if self.mock_plane {
            "TUN · mock plane (set BRAID_LIVE=1 for real sing-box)".to_owned()
        } else if self.phase == Phase::Connected {
            "TUN · process up (P3a; NE/helper still required for reliable TUN)".to_owned()
        } else {
            "TUN · live plane (P3a)".to_owned()
        }
    }

    pub fn fail_copy(&self) -> Option<String> {
        let reason = self.fail?;
        let base = match reason {
            FailReason::Mock => "Simulated failure.".to_owned(),
            FailReason::Permission => {
                "TUN needs administrator rights or a Network Extension (P3b).".to_owned()
            }
            FailReason::Path => {
                "Path to the server is down (local network looks fine).".to_owned()
            }
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
                self.phase = Phase::Connected;
                self.latency_ms = if plane.is_mock() { Some(42) } else { None };
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
    use crate::runtime::MockPlane;

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
    fn path_failure_copy() {
        let mut s = Snapshot::default();
        s.simulate_path_failure();
        assert_eq!(s.phase, Phase::Failed);
        assert!(s.fail_copy().unwrap().contains("Path to the server"));
    }
}
