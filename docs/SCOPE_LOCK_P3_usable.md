## Plan: Braid P3 usable — profile file, Connected gate, Path probe

> Authority: [TECH_PLAN.md](TECH_PLAN.md). Bridge after P3b so live Connect is usable without claiming P3c distribution or full P4 diagnostics.

### Decision

| 选项 | 结论 |
|---|---|
| 文件选择器导入 Profile | 不做 — 固定路径 + `BRAID_PROFILE` |
| 菜单栏 / 公证 | 推迟 P3c |
| Connected 门槛 | live 必须 `via_helper`；否则 Permission |
| Path | Connect 后短超时 TCP 到 `server:port` |

### Scope Lock

- ✅ `src/profile.rs` load path、`src/connection.rs` gate/probe、`src/probe.rs`、UI Load saved、docs
- 🚫 Network Extension、pkg/公证、Status Item、完整诊断矩阵
- ⛔ 不在仓内提交真实 Profile；mock 下 Connected 行为不变

### Acceptance

- `cargo test --lib` / `nsh test` 绿
- 缺文件 → Failed + profile I/O 文案
- live 无 helper → Permission；有 helper 但 TCP 不通 → Path
