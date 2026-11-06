//! Lightweight path probes (P3 usable / P4 start).

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

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
}
