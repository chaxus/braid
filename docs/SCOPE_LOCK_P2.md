## Plan: Braid P2 — Profile import + sing-box config generation

### Assumptions

| # | Assumption | Evidence | Status |
|---|---|---|---|
| 1 | Profile is JSON, not Clash YAML | docs/SPEC.md | verified |
| 2 | No real TUN / no spawning sing-box in P2 | SPEC phases | verified |
| 3 | Demo fixture uses documentation IPs only | RFC 5737 `203.0.113.0/24` | verified |
| 4 | Auto policy = Reality first, Hysteria2 fallback | prior UX lock | verified |

### Change list

- Add `src/profile.rs`, `src/singbox.rs`
- Add `fixtures/demo.profile.json`
- Wire profile into UI summary + load-demo action
- Tests for parse + generated JSON shape
- Update SPEC acceptance for P2

### Scope Lock

- ✅ `braid` repo: profile / singbox / UI / fixtures / docs / tests
- 🚫 wall repo; real credentials; bundling sing-box binary; Network Extension
- ⛔ Do not mark UI as live TUN; keep `simulated` until P3

### Acceptance + verification

- `Profile::from_json` loads fixture — `cargo test`
- `render_singbox` emits tun inbound, reality + hy2 outbounds, server IP direct, auto group — `cargo test`
- UI shows profile name after Load demo — `nsh test`
- `nsh test` + `cargo test --lib` green

### Simplest alternative

- Full Clash YAML import → rejected (product model is Profile JSON)
- Geosite/GeoIP in generated config → rejected for P2 (memory / complexity; add in P4)
