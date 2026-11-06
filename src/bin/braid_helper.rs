//! Privileged helper daemon: runs sing-box as root for TUN (P3b).
//!
//! Installed via `scripts/install-helper.sh` as LaunchDaemon `com.chaxus.braid.helper`.

use braid::helper_proto::{
    path_allowed_for_helper, HelperRequest, HelperResponse, HELPER_SOCKET_PATH,
};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

struct State {
    child: Option<Child>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(e) = run_server() {
        eprintln!("braid-helper fatal: {e}");
        std::process::exit(1);
    }
}

fn run_server() -> Result<(), String> {
    let _ = fs::remove_file(HELPER_SOCKET_PATH);
    let listener = UnixListener::bind(HELPER_SOCKET_PATH).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(HELPER_SOCKET_PATH)
            .map_err(|e| e.to_string())?
            .permissions();
        // Local users may connect; path allowlist is the safety boundary.
        perms.set_mode(0o666);
        fs::set_permissions(HELPER_SOCKET_PATH, perms).map_err(|e| e.to_string())?;
    }
    log::info!("braid-helper listening on {HELPER_SOCKET_PATH}");
    let state = Mutex::new(State { child: None });
    for conn in listener.incoming() {
        match conn {
            Ok(stream) => {
                if let Err(e) = handle_client(stream, &state) {
                    log::warn!("client error: {e}");
                }
            }
            Err(e) => log::warn!("accept: {e}"),
        }
    }
    Ok(())
}

fn handle_client(stream: UnixStream, state: &Mutex<State>) -> Result<(), String> {
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut writer = stream;
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    if line.trim().is_empty() {
        return Ok(());
    }
    let req = HelperRequest::from_line(&line)?;
    let resp = dispatch(req, state);
    let out = resp.to_line()? + "\n";
    writer.write_all(out.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}

fn dispatch(req: HelperRequest, state: &Mutex<State>) -> HelperResponse {
    match req {
        HelperRequest::Ping { .. } => HelperResponse::ok_ping(),
        HelperRequest::Status { .. } => {
            let mut st = state.lock().unwrap();
            reap(&mut st);
            let (running, pid) = match st.child.as_mut() {
                Some(c) => (true, c.id()),
                None => (false, 0),
            };
            HelperResponse {
                v: 1,
                ok: true,
                op: "status".into(),
                running: Some(running),
                pid: if running { Some(pid) } else { None },
                error: None,
                permission: None,
            }
        }
        HelperRequest::Stop { .. } => {
            let mut st = state.lock().unwrap();
            stop_child(&mut st);
            HelperResponse {
                v: 1,
                ok: true,
                op: "stop".into(),
                running: Some(false),
                pid: None,
                error: None,
                permission: None,
            }
        }
        HelperRequest::Start { config, binary, .. } => {
            let config_path = Path::new(&config);
            let binary_path = Path::new(&binary);
            if !path_allowed_for_helper(config_path) || !path_allowed_for_helper(binary_path) {
                return HelperResponse::err("start", "config/binary path not allowed", false);
            }
            if !binary_path.is_file() {
                return HelperResponse::err("start", "sing-box binary not found", false);
            }
            if !config_path.is_file() {
                return HelperResponse::err("start", "config not found", false);
            }
            let mut st = state.lock().unwrap();
            stop_child(&mut st);
            let log_path = config_path
                .parent()
                .unwrap_or_else(|| Path::new("/tmp"))
                .join("sing-box.helper.log");
            let log = match fs::File::create(&log_path) {
                Ok(f) => f,
                Err(e) => return HelperResponse::err("start", e.to_string(), false),
            };
            let log_err = match log.try_clone() {
                Ok(f) => f,
                Err(e) => return HelperResponse::err("start", e.to_string(), false),
            };
            let child = match Command::new(binary_path)
                .args(["run", "-c"])
                .arg(config_path)
                .stdin(Stdio::null())
                .stdout(Stdio::from(log))
                .stderr(Stdio::from(log_err))
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    let permission = e.kind() == std::io::ErrorKind::PermissionDenied;
                    return HelperResponse::err("start", e.to_string(), permission);
                }
            };
            let pid = child.id();
            st.child = Some(child);
            std::thread::sleep(std::time::Duration::from_millis(500));
            reap(&mut st);
            if st.child.is_none() {
                let tail = fs::read_to_string(&log_path).unwrap_or_default();
                let permission = tail.to_ascii_lowercase().contains("operation not permitted")
                    || tail.to_ascii_lowercase().contains("permission denied");
                return HelperResponse::err(
                    "start",
                    format!("sing-box exited early: {tail}"),
                    permission,
                );
            }
            HelperResponse {
                v: 1,
                ok: true,
                op: "start".into(),
                running: Some(true),
                pid: Some(pid),
                error: None,
                permission: None,
            }
        }
    }
}

fn stop_child(st: &mut State) {
    if let Some(mut child) = st.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn reap(st: &mut State) {
    if let Some(child) = st.child.as_mut() {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => {
                st.child = None;
            }
            Ok(None) => {}
        }
    }
}
