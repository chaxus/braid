## Plan: Braid P3b-NE — macOS Packet Tunnel

> Authority: [TECH_PLAN.md](TECH_PLAN.md). Adds Network Extension alongside the existing LaunchDaemon helper (helper remains fallback).

### Decision

| 选项 | 结论 |
|---|---|
| Replace helper with NE only | 否 — helper 保留回退 |
| Packet Tunnel + App Group + sing-box in appex | **采用** |
| Notarization / App Store | 本切片不做 |

### Constants

| 项 | 值 |
|---|---|
| App Group | `group.com.chaxus.braid` |
| Tunnel bundle id | `com.chaxus.braid.tunnel` |
| Host app id | `com.chaxus.braid` |
| Shared config | `Library/Application Support/Braid/ne-config.json` under the group container |

### Scope Lock

- ✅ `macos/BraidTunnel/*`, `macos/project.yml`, `scripts/open-ne-xcode.sh`, `src/ne_plane.rs`, `src/runtime.rs` / connection / UI copy, docs
- 🚫 Delete helper; iOS NE; notarization; commit Development Team secrets
- ⛔ Without signed appex + user VPN approval, must not report Connected via NE

### Acceptance

- `nsh test` / mock unchanged green
- No Team / no appex → helper or Permission (honest)
- Signed + approved → Connect prefers NE (`via_ne`)
