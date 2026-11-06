## Plan: Braid P3a — sing-box process plane (not Network Extension yet)

> Semantics and red lines: **[TECH_PLAN.md](TECH_PLAN.md)** §2.3 · §4 · §7 P3a. This file only lists the iteration boundary.

### Assumptions

| # | Assumption | Evidence | Status |
|---|---|---|---|
| 1 | Real TUN on macOS needs elevated rights or a Network Extension | sing-box / Clash Verge practice | verified |
| 2 | P3a ships process lifecycle + clear Permission failures; NE is P3b | product judgment | verified |
| 3 | Headless `nsh test` must not download or spawn sing-box | CI / test harness | verified |
| 4 | Binary is version-pinned and SHA256-verified | wall project convention | verified |

### Change list

- `src/runtime.rs` — paths, ensure binary, write config, start/stop, classify errors
- Wire Connect/Disconnect through runner (mock plane when `BRAID_MOCK_PLANE=1` or tests)
- Update fail copy for permission / binary / spawn
- Docs: how to run with privileges; P3b NE deferred

### Scope Lock

- ✅ braid runtime + connection + UI footer/live flag + docs/tests
- 🚫 Network Extension target, notarized helper, App Store
- 🚫 wall repo / real production profiles in git
- ⛔ Do not claim “Connected” if the process exited; Permission must be explicit

### Acceptance

- Unit tests: mock plane connect/disconnect; error classification
- `sing-box check` against generated demo config when binary available (optional)
- Manual: with profile + binary, Connect writes config and attempts spawn
- `nsh test` still green without network download

### Simplest alternative

- Full Network Extension in this PR → rejected (signing + weeks)
- Always mock → rejected (user asked for real TUN path)
- Adopted: real process manager + honest Permission failures; NE later
