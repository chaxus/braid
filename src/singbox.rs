//! Render a sing-box client JSON config from a Braid [`Profile`].
//!
//! P2 generates config only — it does not spawn sing-box or enable TUN.

use crate::profile::Profile;
use base64::Engine;
use serde_json::{json, Value};

/// Build a sing-box 1.10+ style client config:
/// - TUN inbound (for P3; unused until a binary is run)
/// - Reality + Hysteria2 outbounds
/// - `auto` urltest preferring low latency (stealth + speed both probed)
/// - private + server CIDR → direct
pub fn render_singbox(profile: &Profile) -> Value {
    let mut route_rules = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        json!({ "ip_is_private": true, "outbound": "direct" }),
    ];
    for cidr in profile.direct_cidrs() {
        route_rules.push(json!({
            "ip_cidr": [cidr],
            "outbound": "direct"
        }));
    }

    let mut hy2_tls = json!({
        "enabled": true,
        "server_name": profile.hysteria2.server_name,
        "insecure": true,
        "alpn": ["h3"]
    });
    if let Some(pin) = profile.hysteria2.pin_sha256.as_deref() {
        if let Some(b64) = pin_sha256_to_base64(pin) {
            hy2_tls["certificate_public_key_sha256"] = json!([b64]);
        }
    }

    json!({
        "log": { "level": "info", "timestamp": true },
        "dns": {
            "servers": [
                { "tag": "remote", "address": "1.1.1.1", "detour": "auto" },
                { "tag": "local", "address": "local" }
            ],
            "final": "remote",
            "strategy": "ipv4_only"
        },
        "inbounds": [
            {
                "type": "tun",
                "tag": "tun-in",
                "interface_name": "braid-tun",
                "address": ["172.19.0.1/30"],
                "auto_route": true,
                "strict_route": true,
                "stack": "system"
            }
        ],
        "outbounds": [
            {
                "type": "vless",
                "tag": "stealth",
                "server": profile.server,
                "server_port": profile.port,
                "uuid": profile.reality.uuid,
                "flow": "xtls-rprx-vision",
                "tls": {
                    "enabled": true,
                    "server_name": profile.reality.server_name,
                    "utls": { "enabled": true, "fingerprint": "chrome" },
                    "reality": {
                        "enabled": true,
                        "public_key": profile.reality.public_key,
                        "short_id": profile.reality.short_id
                    }
                }
            },
            {
                "type": "hysteria2",
                "tag": "speed",
                "server": profile.server,
                "server_port": profile.port,
                "password": profile.hysteria2.password,
                "obfs": {
                    "type": "salamander",
                    "password": profile.hysteria2.obfs_password
                },
                "tls": hy2_tls
            },
            {
                "type": "urltest",
                "tag": "auto",
                "outbounds": ["stealth", "speed"],
                "url": "https://www.gstatic.com/generate_204",
                "interval": "30s",
                "tolerance": 50
            },
            { "type": "direct", "tag": "direct" },
            { "type": "block", "tag": "block" }
        ],
        "route": {
            "rules": route_rules,
            "final": "auto",
            "auto_detect_interface": true
        }
    })
}

pub fn render_singbox_pretty(profile: &Profile) -> String {
    serde_json::to_string_pretty(&render_singbox(profile))
        .expect("sing-box Value always serializes")
}

/// OpenSSL-style `AA:BB:…` SHA256 fingerprint → base64 for sing-box TLS pin.
pub fn pin_sha256_to_base64(pin: &str) -> Option<String> {
    let hex: String = pin.chars().filter(|c| *c != ':').collect();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = Vec::with_capacity(32);
    let chars: Vec<char> = hex.chars().collect();
    for chunk in chars.chunks(2) {
        let s: String = chunk.iter().collect();
        bytes.push(u8::from_str_radix(&s, 16).ok()?);
    }
    Some(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;

    #[test]
    fn renders_dual_outbounds_and_tun() {
        let p = Profile::demo();
        let cfg = render_singbox(&p);
        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert_eq!(inbounds[0]["type"], "tun");

        let tags: Vec<&str> = cfg["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|o| o["tag"].as_str())
            .collect();
        assert!(tags.contains(&"stealth"));
        assert!(tags.contains(&"speed"));
        assert!(tags.contains(&"auto"));
        assert!(tags.contains(&"direct"));

        assert_eq!(cfg["route"]["final"], "auto");
        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert!(rules.iter().any(|r| r.get("ip_cidr").is_some()));
    }

    #[test]
    fn pin_converts() {
        let pin = "DE:AD:BE:EF:DE:AD:BE:EF:DE:AD:BE:EF:DE:AD:BE:EF:DE:AD:BE:EF:DE:AD:BE:EF:DE:AD:BE:EF:DE:AD:BE:EF";
        let b64 = pin_sha256_to_base64(pin).unwrap();
        assert!(!b64.is_empty());
        let cfg = render_singbox(&Profile::demo());
        let hy2 = cfg["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["tag"] == "speed")
            .unwrap();
        assert_eq!(
            hy2["tls"]["certificate_public_key_sha256"][0].as_str().unwrap(),
            b64
        );
    }

    #[test]
    fn pretty_is_valid_json() {
        let text = render_singbox_pretty(&Profile::demo());
        let _: Value = serde_json::from_str(&text).unwrap();
    }
}
