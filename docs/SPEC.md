# Braid — Product Spec (v0.1)

Personal TUN-first proxy client. UI in Native Shell (NSH); data plane is sing-box.
Mac first; Android / iOS later from the same app model.

> **Implementation authority:** [TECH_PLAN.md](TECH_PLAN.md). If this summary drifts from the tech plan, fix one of them deliberately — do not “just code past it”.

## Goals

1. Replace Clash Verge for daily use: **TUN must work**; system proxy alone is not enough.
2. Feel advanced: one primary connect control, clear status, progressive disclosure.
3. Stay a **clean** repo: no production IPs, passwords, AWS keys, or host inventory.

## Non-goals (v0.1)

- Shipping iOS / Android stores
- Multi-subscription marketplace / rule-set CDN
- Replacing server-side wall installers (that stays private)

## Architecture

```
┌─────────────────────────────┐
│  NSH app (menu bar + panel) │  connect UX, import profile, diagnostics copy
└──────────────┬──────────────┘
               │ local IPC / process control
┌──────────────▼──────────────┐
│  braid-core (Rust)          │  state machine, profile model, health probes
└──────────────┬──────────────┘
               │ spawn / configure
┌──────────────▼──────────────┐
│  sing-box + macOS TUN       │  Network Extension or privileged helper + utun
└─────────────────────────────┘
```

## UX (locked)

- **Primary surface**: macOS menu-bar popover (not a dashboard window).
- **Primary action**: Connect / Disconnect.
- **Summary line**: node label · latency · egress IP (copyable).
- **Link policy**: Auto (prefer stealth, fail over to speed) — map later to REALITY / Hysteria2 without requiring those names on the main surface.
- **Failure copy**: distinguish “path to server dead (LAN ok)” vs “server unhealthy” vs “permission missing”.
- **Visual**: system-native, one accent, no neon / purple-gradient chrome.

## Profile model (user-facing)

A **Profile** is one egress endpoint set the user imports (JSON), not raw Clash YAML tables.

Minimum fields (v0.1 internal):

- `name`, `server` (host/IP), `port`
- Reality credentials (uuid, public key, short id, sni/dest)
- Hysteria2 credentials (password, obfs password, sni, optional pinSHA256)
- Optional `cidr_direct` for TUN bypass of the server address

Import sources later: paste JSON, file, or deep link. v0.1 can use a fixture profile for UI.

## Connection state machine

`Idle → RequestingPermission → Connecting → Connected ↔ Degraded → Disconnecting → Idle`  
`* → Failed` with a reason code.

## Phases

| Phase | Deliverable |
|---|---|
| **P0** | Spec, plan, repo hygiene |
| **P1** | NSH macOS shell: menu-bar style panel, state machine UI, mock connect |
| **P2** | Profile import + sing-box config generation (no TUN yet) |
| **P3** | macOS TUN via Network Extension / helper; real traffic |
| **P4** | Dual-link auto, diagnostics one-liners, IP sync hook (optional) |

## Acceptance (P1)

- `nsh run --target macos` launches the app
- User can toggle mock Connect / Disconnect and see status + summary update
- No secrets in git; MIT LICENSE present

## Acceptance (P2)

- Demo profile JSON loads (`fixtures/demo.profile.json`, documentation IPs only)
- `render_singbox` emits TUN inbound, stealth + speed outbounds, `auto` urltest, server CIDR direct
- UI: Load demo profile → config ready → Connect (still simulated)
- `cargo test --lib` and `nsh test` green

## Out of scope through P2

Real TUN, sing-box binary bundling, wall server coupling, geosite databases.
