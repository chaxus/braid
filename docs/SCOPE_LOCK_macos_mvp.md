## Plan: Braid macOS personal MVP complete

> Authority: [TECH_PLAN.md](TECH_PLAN.md). Closes the macOS personal-client vertical after P3 usable.

### In scope (done)

- P4 core: Path vs Local classification, live RTT, Connected tick / Refresh status, Run diagnostics
- P3c lite: `scripts/package.sh` (`nsh package --target macos`, ad-hoc; no notarization)
- Docs mark **macOS MVP complete**

### Out of scope

- P5 Android / iOS
- Network Extension
- Apple notarization / Developer ID
- NSStatusItem (NSH 0.4 has no tray API; compact window remains)

### Acceptance

- `cargo test --lib` / `nsh test` green
- One feature per commit for this slice
- No real profiles or secrets in git
