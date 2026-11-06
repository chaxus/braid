//! Connection state machine (mock until P3 wires real TUN).

use crate::profile::Profile;
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
#[allow(dead_code)] // Permission / Server / Mock used as the model grows.
pub enum FailReason {
    Mock,
    Permission,
    Path,
    Server,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub phase: Phase,
    pub fail: Option<FailReason>,
    pub node: String,
    pub latency_ms: Option<u32>,
    pub egress_ip: Option<String>,
    /// True until P3 enables a live tunnel.
    pub simulated: bool,
    pub profile_loaded: bool,
    pub config_bytes: usize,
    pub server: Option<String>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            fail: None,
            node: "No profile".to_owned(),
            latency_ms: None,
            egress_ip: None,
            simulated: true,
            profile_loaded: false,
            config_bytes: 0,
            server: None,
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

    pub fn fail_copy(&self) -> Option<&'static str> {
        match self.fail? {
            FailReason::Mock => Some("Simulated failure."),
            FailReason::Permission => Some("Network extension permission is missing."),
            FailReason::Path => Some("Path to the server is down (local network looks fine)."),
            FailReason::Server => Some("Server looks unhealthy."),
        }
    }

    pub fn apply_profile(&mut self, profile: &Profile) {
        let config = render_singbox_pretty(profile);
        self.node = profile.name.clone();
        self.server = Some(profile.server.clone());
        self.profile_loaded = true;
        self.config_bytes = config.len();
        self.fail = None;
        if self.phase == Phase::Failed {
            self.phase = Phase::Idle;
        }
    }

    pub fn load_demo_profile(&mut self) {
        self.apply_profile(&Profile::demo());
    }

    pub fn toggle_primary(&mut self) {
        if !self.profile_loaded {
            return;
        }
        match self.phase {
            Phase::Idle | Phase::Failed => {
                self.phase = Phase::Connecting;
                self.fail = None;
                // Still mock until P3 — pretend we connected via the profile server.
                self.phase = Phase::Connected;
                self.latency_ms = Some(42);
                self.egress_ip = self.server.clone();
            }
            Phase::Connected => {
                self.phase = Phase::Disconnecting;
                self.phase = Phase::Idle;
                self.latency_ms = None;
                self.egress_ip = None;
                self.fail = None;
            }
            Phase::Connecting | Phase::Disconnecting => {}
        }
    }

    pub fn simulate_path_failure(&mut self) {
        self.phase = Phase::Failed;
        self.fail = Some(FailReason::Path);
        self.latency_ms = None;
        self.egress_ip = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_requires_profile() {
        let mut s = Snapshot::default();
        assert!(!s.primary_enabled());
        s.toggle_primary();
        assert_eq!(s.phase, Phase::Idle);
        s.load_demo_profile();
        assert!(s.primary_enabled());
        assert!(s.config_bytes > 100);
        s.toggle_primary();
        assert_eq!(s.phase, Phase::Connected);
        assert_eq!(s.egress_ip.as_deref(), Some("203.0.113.10"));
        s.toggle_primary();
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
