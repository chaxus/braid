## Plan: Braid P1 — NSH macOS shell + connection state UI

### Assumptions

| # | Assumption | Evidence | Status |
|---|---|---|---|
| 1 | TUN is mandatory for product success | user statement | verified |
| 2 | NSH is the UI framework (`chaxus/nsh` / longbridge NSH) | user statement | verified |
| 3 | sing-box is the intended data plane | prior design agreement | verified |
| 4 | P1 ships mock connect only; real TUN is P3 | spec `docs/SPEC.md` | verified |
| 5 | Repo stays private; no host secrets | user statement after wall audit | verified |

### Change list

- Create `docs/SPEC.md`, `docs/SCOPE_LOCK.md`, `docs/superpowers/plans/2026-11-06-braid-p1.md`
- Create NSH app under repo root (`nsh.json`, Rust and/or TSX entry)
- Implement connection store + menu-panel UI
- Update `README.md` with run instructions

### Scope Lock

- ✅ May touch: `/Users/ranzhouhang/Desktop/braid/**` (this repo only)
- 🚫 Untouched: `/Users/ranzhouhang/Desktop/wall/**`, any live credentials, AWS, server hosts
- ⛔ Compatibility red lines: do not commit real node secrets; do not claim TUN works until P3

### Acceptance + verification

- App runs on macOS via `nsh run --target macos` — manual
- State transitions Idle ↔ Connecting ↔ Connected / Failed — manual + unit test if cheap
- `git status` clean of secrets — review staged files

### Simplest alternative

- Full TUN in P1 → rejected (Network Extension + signing is a multi-week track)
- System-proxy-only MVP → rejected (user requires TUN; proxy-only cannot replace Clash Verge)
- Adopted: NSH shell + mock state machine first
