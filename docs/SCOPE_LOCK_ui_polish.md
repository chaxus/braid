## Plan: Braid main-panel UI polish

> Authority: [TECH_PLAN.md](TECH_PLAN.md) §6, [SPEC.md](SPEC.md) UX. Connect-first panel inspired by WARP/Surge; not Clash Verge.

### Scope Lock

- ✅ `src/lib.rs` layout, `theme.json` / `nsh.json`, `src/main.rs` size, `App.test.tsx`, docs
- 🚫 Network Extension, Status Item, YAML/rules UI, DataPlane semantics
- ⛔ Keep one primary Connect; Simulate only on mock plane

### Acceptance

- First viewport reads as Braid + Connect
- `nsh test` green
- Live UI has no Simulate control
