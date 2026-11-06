//! Connection state machine (P1: UI + mock transitions; real TUN arrives in P3).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Connecting,
    Connected,
    Disconnecting,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Permission / Server / Mock used as the model grows past P1.
pub enum FailReason {
    /// Reserved for intentional mock failures in development.
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
    /// Shown in the UI so P1 is never mistaken for a live tunnel.
    pub simulated: bool,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            fail: None,
            node: "Demo node".to_owned(),
            latency_ms: None,
            egress_ip: None,
            simulated: true,
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
        matches!(self.phase, Phase::Idle | Phase::Connected | Phase::Failed)
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

    pub fn fail_copy(&self) -> Option<&'static str> {
        match self.fail? {
            FailReason::Mock => Some("Simulated failure."),
            FailReason::Permission => Some("Network extension permission is missing."),
            FailReason::Path => Some("Path to the server is down (local network looks fine)."),
            FailReason::Server => Some("Server looks unhealthy."),
        }
    }

    /// Primary button: connect or disconnect (mock, synchronous for P1).
    pub fn toggle_primary(&mut self) {
        match self.phase {
            Phase::Idle | Phase::Failed => {
                self.phase = Phase::Connecting;
                self.fail = None;
                // P1: no async runtime wired yet — complete the mock handshake immediately.
                self.phase = Phase::Connected;
                self.latency_ms = Some(42);
                self.egress_ip = Some("203.0.113.10".to_owned());
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
    fn connect_then_disconnect() {
        let mut s = Snapshot::default();
        assert_eq!(s.phase, Phase::Idle);
        s.toggle_primary();
        assert_eq!(s.phase, Phase::Connected);
        assert_eq!(s.latency_ms, Some(42));
        assert_eq!(s.egress_ip.as_deref(), Some("203.0.113.10"));
        s.toggle_primary();
        assert_eq!(s.phase, Phase::Idle);
        assert!(s.latency_ms.is_none());
    }

    #[test]
    fn path_failure_copy() {
        let mut s = Snapshot::default();
        s.simulate_path_failure();
        assert_eq!(s.phase, Phase::Failed);
        assert!(s.fail_copy().unwrap().contains("Path to the server"));
    }
}
