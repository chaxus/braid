## Plan: Braid P3b — Privileged Helper (LaunchDaemon)

> Authority: [TECH_PLAN.md](TECH_PLAN.md). **P3b 主路径锁定为 Privileged Helper**，不是 Network Extension（NE 推迟，避免签名/公证阻塞个人可用）。

### Decision

| 选项 | 结论 |
|---|---|
| Network Extension | 推迟（开发者证书、Packet Tunnel、公证成本高） |
| **LaunchDaemon + Unix socket helper** | **采用** — 一次 `sudo` 安装，之后 UI 无提权即可让 root 跑 sing-box TUN |

### Assumptions

| # | Assumption | Status |
|---|---|---|
| 1 | Root-owned LaunchDaemon can create TUN | verified (industry practice) |
| 2 | Socket only accepts paths under Braid data dirs | verified by design |
| 3 | `nsh test` never requires helper | verified |

### Scope Lock

- ✅ `src/helper_proto.rs`, `src/bin/braid_helper.rs`, `src/runtime.rs` helper client, `scripts/*helper*`, packaging plist, docs/UI
- 🚫 Network Extension target, notarization, App Store
- ⛔ Do not report Connected if helper start fails; Permission must mention helper install

### Acceptance

- Protocol unit tests (encode/decode, path allowlist)
- Helper binary builds (`cargo build --bin braid-helper`)
- Without helper: live plane still Permission-honest (P3a)
- With helper installed (manual): Connect via socket starts sing-box as root
- Mock / `nsh test` unchanged green
