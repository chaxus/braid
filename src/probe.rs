//! Lightweight path probes (P4 diagnostics).

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Mainland public DNS — used as a "local network looks fine" signal.
pub const LOCAL_PROBE_HOST: &str = "223.5.5.5";
pub const LOCAL_PROBE_PORT: u16 = 53;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathClass {
    Ok,
    /// Local OK, remote unreachable → path to VPS.
    Path,
    /// Local probe failed → do not blame the VPS.
    LocalDown,
}

/// Pure classifier (unit-testable without the network).
pub fn classify_with(local_ok: bool, remote_ok: bool) -> PathClass {
    match (local_ok, remote_ok) {
        (_, true) => PathClass::Ok,
        (true, false) => PathClass::Path,
        (false, false) => PathClass::LocalDown,
    }
}

/// Try a TCP connect to `host:port` within `timeout`.
pub fn tcp_reachable(host: &str, port: u16, timeout: Duration) -> bool {
    let addr = format!("{host}:{port}");
    let Ok(mut addrs) = addr.to_socket_addrs() else {
        return false;
    };
    let Some(sock) = addrs.next() else {
        return false;
    };
    TcpStream::connect_timeout(&sock, timeout).is_ok()
}

/// Probe a well-known domestic endpoint.
pub fn local_reachable(timeout: Duration) -> bool {
    tcp_reachable(LOCAL_PROBE_HOST, LOCAL_PROBE_PORT, timeout)
}

/// Classify connectivity to the profile server.
pub fn classify_path(server: &str, port: u16, timeout: Duration) -> PathClass {
    let remote = tcp_reachable(server, port, timeout);
    let local = if remote {
        true
    } else {
        local_reachable(timeout)
    };
    classify_with(local, remote)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn classify_matrix() {
        assert_eq!(classify_with(true, true), PathClass::Ok);
        assert_eq!(classify_with(false, true), PathClass::Ok);
        assert_eq!(classify_with(true, false), PathClass::Path);
        assert_eq!(classify_with(false, false), PathClass::LocalDown);
    }

    #[test]
    fn closed_port_unreachable() {
        assert!(!tcp_reachable(
            "127.0.0.1",
            1,
            Duration::from_millis(200)
        ));
    }

    #[test]
    fn open_port_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let _ = listener.accept();
        });
        assert!(tcp_reachable(
            "127.0.0.1",
            port,
            Duration::from_millis(500)
        ));
    }

    #[test]
    fn classify_path_local_closed_is_path_or_local() {
        // 127.0.0.1:1 is closed; class depends on whether 223.5.5.5:53 answers.
        let class = classify_path("127.0.0.1", 1, Duration::from_millis(300));
        assert!(matches!(class, PathClass::Path | PathClass::LocalDown));
    }
}
