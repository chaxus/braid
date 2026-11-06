# Braid 技术规划（权威版）

> **地位**：本文件是实现与评审的准绳。若与临时代码、口头约定或阶段性 Scope Lock 冲突，以本文件为准；修改本文件需显式修订「变更记录」。  
> **产品一句话**：用 NSH 做跨端壳、用 sing-box 做数据面的 **TUN 优先**个人代理客户端；先替代 Clash Verge（macOS），再扩 Android / iOS。  
> **仓库约束**：与 wall（服务端运维仓）严格分离；本仓永不提交生产节点凭据、AWS Key、真实主机清单。

相关短文：

| 文档 | 用途 |
|---|---|
| [SPEC.md](SPEC.md) | 产品目标与阶段验收摘要 |
| [SCOPE_LOCK*.md](SCOPE_LOCK.md) | 单次迭代的可改文件边界（执行用） |
| 本文件 | **架构、接口、阶段切片、红线、验收**（防偏移） |

---

## 1. 背景与要解决的问题

### 1.1 为什么做

- Clash Verge 能用但不想继续依赖其产品形态（配置面过重、心智是「内核面板」）。
- 日常需要 **TUN 整机接管**；仅系统代理无法覆盖不认代理的 App / UDP，**不能算替代成功**。
- 服务端（wall）已有 REALITY + Hysteria2 双链路；客户端应对齐该模型，但用更干净的配置面。

### 1.2 明确不做什么（红线）

| 不做 | 原因 |
|---|---|
| 在本仓实现协议栈 | 成本与正确性都不可接受；数据面固定为 sing-box |
| 把 wall 的安装脚本/密钥迁入本仓 | 公开/协作边界与泄密风险 |
| 以「系统代理 MVP」宣称可替代 Clash | 与已锁定需求冲突 |
| P3 一次做完 Network Extension + 公证 + 上架 | 爆炸半径过大；必须切片 |
| 默认加载 GeoIP/GeoSite 大库 | 与「轻量个人客户端」及低配机器经验冲突；规则先做最小集 |
| UI 做成第二个 Clash（YAML/规则编辑器当主路径） | 与 UX 锁定冲突 |

### 1.3 成功标准（产品级）

用户在 macOS 上可以：

1. 导入一份 Braid Profile（JSON）；
2. 一点 Connect，**流量经 TUN 出去**（不只是进程起来）；
3. 断线时能看懂是「没权限 / 到服务器路径挂了 / 服务端问题」；
4. Disconnect 后系统路由与代理状态不留残缺。

未同时满足 2 与 4，不得在 README/UI 去掉「simulated / 需要权限」类诚实标注。

---

## 2. 总体架构

### 2.1 分层

```
┌──────────────────────────────────────────────┐
│  Presentation（NSH）                         │
│  菜单栏/紧凑面板 · 状态文案 · 导入入口         │
└─────────────────────┬────────────────────────┘
                      │ 仅调用 braid-core API
┌─────────────────────▼────────────────────────┐
│  braid-core（Rust，与 UI 同 crate 或日后拆箱） │
│  Profile · 状态机 · sing-box 配置渲染         │
│  DataPlane trait · 诊断探针（P4）             │
└─────────────────────┬────────────────────────┘
                      │ 写配置 / 启停 / 健康
┌─────────────────────▼────────────────────────┐
│  Data plane                                  │
│  sing-box 二进制 + TUN（utun / NE PacketTun）│
└──────────────────────────────────────────────┘
```

**硬规则**：UI 不得直接拼 sing-box JSON、不得直接 `Command::new("sing-box")`。全部经 core 的 `DataPlane` / `Profile` / `Snapshot` API，便于测试与换实现。

### 2.2 组件职责

| 组件 | 职责 | 非职责 |
|---|---|---|
| NSH App | 展示 Snapshot、触发 connect/disconnect/import | 不知道二进制 URL、不知道 utun |
| `profile` | 校验与解析用户 Profile JSON | 不生成路由策略细节以外的副作用 |
| `singbox`（渲染器） | Profile → sing-box JSON（纯函数） | 不落盘、不启进程 |
| `runtime` / DataPlane | 落盘、校验二进制、启停、读日志、分类错误 | 不画 UI |
| 诊断（P4） | 国内直连 / 到 server TCP / 经隧道 HTTPS | 不替代状态机 |

### 2.3 进程与权限模型（macOS）

业界事实（必须写进规划，避免实现时幻想「普通 App 直接 TUN」）：

- 创建 utun、改默认路由，通常需要 **root** 或 **Network Extension（Packet Tunnel）**。
- Clash Verge 等产品用 **Privileged Helper / 常驻服务** 跑核心；UI 无特权。
- Apple 长期正确形态是 **System Extension + NE**，需签名与用户批准「VPN 配置」。

因此 macOS TUN 分两级实现，**禁止跳级对外宣称完成**：

| 级别 | 代号 | 做法 | 用户体验 | 何时算「真 TUN 可用」 |
|---|---|---|---|---|
| A | **P3a** | App 管理配置与二进制；尝试拉起 sing-box；无权限则 **Failed(Permission)**，文案诚实 | 开发者可用 `sudo`/helper 试验 | 仅当进程存活 **且** 路由/接口证据表明 TUN 生效 |
| B | **P3b** | **Privileged Helper（LaunchDaemon + socket）**；NE 推迟 | 接近 Clash Verge | helper 下 TUN 证据齐全即算门槛 |
| C | **P3c** | 公证、稳定安装器、菜单栏 Status Item | 可分发 | 可选 |

**偏移警告**：只把 sing-box 进程拉起来、TUN 实际失败却显示 Connected → **视为缺陷**，不是可接受的中间态。

### 2.4 跨端预留

| 平台 | UI | TUN 载体 | 备注 |
|---|---|---|---|
| macOS | NSH | NE / helper + sing-box | 当前主路径 |
| Android | NSH | `VpnService` + sing-box | 无「先系统代理」捷径 |
| iOS | NSH | Network Extension；上架限制多 | 晚于 Android |
| Windows/Linux | NSH | Wintun / TUN | 非 v1 优先 |

Profile JSON 与 sing-box 渲染逻辑跨端共用；**平台差异只允许出现在 DataPlane 实现与打包**。

---

## 3. 数据与配置契约

### 3.1 Profile（用户可见、可导入）

路径约定（运行时，不进 git）：

- 目录：`~/Library/Application Support/Braid/`（macOS；其他平台用对应 data dir）
- 当前档案：`profile.json`
- 生成配置：`config.json`（仅由渲染器输出，用户一般不手改）
- 日志：`sing-box.log` / `braid.log`
- 二进制：`bin/sing-box`（版本钉扎）

Profile schema（字段名稳定；增字段必须向后兼容）：

```json
{
  "name": "string",
  "server": "host or ip",
  "port": 443,
  "reality": {
    "uuid": "...",
    "public_key": "...",
    "short_id": "...",
    "server_name": "SNI / dest"
  },
  "hysteria2": {
    "password": "...",
    "obfs_password": "...",
    "server_name": "...",
    "pin_sha256": "optional AA:BB:... OpenSSL fingerprint"
  },
  "cidr_direct": ["x.x.x.x/32"]
}
```

规则：

- **不是** Clash YAML；不做「任意 mihomo 配置全量导入」为 v1 目标。
- 仓库内只允许 `fixtures/demo.profile.json` 一类 **RFC 5737 文档地址** + 假凭据。
- 从 wall 导出的转换器（若需要）放在 **wall 私有仓或本地脚本**，或本仓提供「粘贴转换」但不提交真实输出。

### 3.2 sing-box 渲染契约（已实现于 P2，变更需测）

生成配置必须包含：

| 元素 | 要求 |
|---|---|
| inbound `tun` | 存在；`auto_route` / 合理地址段；tag 稳定 |
| outbound `stealth` | VLESS + REALITY + vision + utls chrome |
| outbound `speed` | Hysteria2 + salamander obfs；TLS insecure + 可选 pin |
| outbound `auto` | urltest（或等价），成员含 stealth + speed |
| outbound `direct` | 必须有 |
| route | `ip_is_private → direct`；`cidr_direct → direct`；`final → auto` |
| DNS | 经隧道的远程解析 + local；避免解析环 |

**禁止默认塞入**：巨型 rule-set、需要额外下载的 geosite 数据库（P4+ 再议，且需单独开关）。

Link policy「Auto」语义（UX）：优先可用链路、兼顾延迟；实现上可用 urltest。对外文案用「Auto」，高级页才显示协议名。

### 3.3 状态机（权威）

```
Idle
  └─(connect, profile ok)→ RequestingPermission? → Connecting → Connected
                                                      └─ fail → Failed(reason)
Connected ↔ Degraded（半通/单边链路差，P4）
Connected|Failed|Idle ← Disconnecting ← (disconnect)
任意 → Failed(Permission|Path|Server|Binary|Config|Process)
```

| FailReason | 用户文案方向 | 典型证据 |
|---|---|---|
| Permission | 需要网络扩展/管理员权限才能启用 TUN | NE 拒绝；operation not permitted；helper 未装 |
| Path | 到服务器的网络中断（本机网络看起来正常） | 物理网卡到 server:443 超时；国内直连仍通 |
| Server | 服务器侧异常 | 自连/鉴权失败且路径可达 |
| Binary | 核心缺失或校验失败 | 下载失败、hash 不符 |
| Config | 配置无法通过 `sing-box check` | check 非 0 |
| Process | 进程异常退出 | 启动后立刻挂掉且不像权限 |

UI 主路径只暴露 Permission / Path / Server（可合并 Binary/Config/Process 到 Server 或单独「无法启动核心」）；**内部枚举要细**，避免排障时信息丢失。

Connected 的充要条件（P3a 起）：

1. DataPlane 报告进程在跑；**并且**
2. 至少一种「TUN 生效」证据：存在约定 tun 接口 / 默认路由指向 tun / 探测流量走隧道（P3a 可先做 1+ 接口或路由检查；缺证据只能显示 Connecting/Failed，不能稳 Connected）。

若暂时只能做到「进程在跑、TUN 因权限失败」→ **必须 Failed(Permission)**，禁止 Connected。

---

## 4. DataPlane 接口（实现不得偏离）

```text
trait DataPlane {
  fn prepare(&self, profile: &Profile) -> Result<Prepared, PlaneError>
      // 渲染、落盘 config.json、ensure binary、sing-box check

  fn start(&mut self, prepared: &Prepared) -> Result<(), PlaneError>
      // 启进程或激活 NE；返回前完成「存活 + TUN 证据」或错误

  fn stop(&mut self) -> Result<(), PlaneError>
      // 杀进程 / 断开 NE；尽力清理路由；可重入

  fn poll(&mut self) -> PlaneStatus
      // Idle | Starting | Up | Degraded | Stopping | Down { reason }
}
```

测试用 `MockPlane`：不下载、不启真实进程；`nsh test` / CI **默认 MockPlane**（环境变量如 `BRAID_MOCK_PLANE=1` 或 compile cfg）。

真机手动：`BRAID_MOCK_PLANE=0` + 已安装二进制；权限不足时断言走 Permission。

---

## 5. 二进制与供应链

| 项 | 决定 |
|---|---|
| 上游 | SagerNet/sing-box 官方 release |
| 版本 | **钉扎**（例如 1.14.x）；升级单独 PR，附 changelog |
| 校验 | SHA256（按 darwin-arm64 / darwin-amd64 分行）；校验失败不得运行 |
| 获取 | 首次 Connect/`braid doctor` 下载到 Application Support；允许 `BRAID_SINGBOX` 覆盖路径 |
| 镜像 | 可选后续；默认 GitHub release |

与 wall 一致原则：**先校验再替换正在使用的二进制**。

---

## 6. UX 与信息架构（防做成 Clash）

### 6.1 主表面（菜单栏级面板）

固定元素：

1. 状态点 + Disconnected/Connecting/Connected/Failed  
2. 摘要：`节点 · 延迟 · 出口IP`  
3. 主按钮：Connect / Disconnect（唯一 primary）  
4. Link policy：Auto（只读或简单切换，细节进高级）  
5. 诚实页脚：未达「真 TUN」前必须标明阶段（如 `TUN · needs permission` / `TUN · simulated`）

次要（可折叠/二级）：Load demo、加载 `profile.json`、Simulate failure（仅 debug）、打开日志目录。

### 6.2 禁止出现在主路径

- 完整 JSON/YAML 编辑器  
- 规则集市场、订阅广场  
- 多内核切换（mihomo/xray/sing-box 乱选）—— **只支持 sing-box**  
- 调试端口、混合代理开关堆砌（系统代理若做，仅作高级排障，默认关）

### 6.3 视觉

系统原生、单一强调色、语义色仅表示状态；无霓虹/紫渐变模板脸。详见既有 UX 讨论结论，不在实现期重新「发挥」。

---

## 7. 阶段切片（必须按序；不得合并宣传）

### 已完成

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | 仓库、许可证、无密钥 | 完成（Apache-2.0 + Public） |
| P1 | NSH 面板 + 状态机 UI（mock） | 完成 |
| P2 | Profile + sing-box 渲染 + demo 导入 | 完成 |
| P3a | DataPlane / 启停 / 诚实 Permission | 完成（默认 mock；`BRAID_LIVE=1` 真平面） |
| P3b | Privileged Helper（LaunchDaemon） | **主路径已落地**；NE 推迟 |
| P3 usable | 本机 Profile + Connected 门槛 + TCP Path | **已完成**（见 [SCOPE_LOCK_P3_usable.md](SCOPE_LOCK_P3_usable.md)） |
| P4 core + P3c lite | 诊断/延迟/掉线 + `nsh package` | **macOS MVP 收口完成**（见 [SCOPE_LOCK_macos_mvp.md](SCOPE_LOCK_macos_mvp.md)） |

### P3a — 进程面与诚实失败（已完成）

**目标**：具备「像真客户端一样」的启停与配置落盘；TUN 无权限时 **明确失败**，不为了演示假 Connected。

交付：`DataPlane` + `SingBoxPlane` + `MockPlane`、钉扎下载、`prepare`/`start`/`stop`/`poll`、UI 接真平面（测试仍 mock）。

### P3b — Privileged Helper（当前主路径；NE 推迟）

**目标**：一次 `sudo` 安装 helper 后，普通用户账号下 Connect 可拉起 root 侧 sing-box TUN。

**已锁定主路径**：LaunchDaemon + Unix socket（`/var/run/braid-helper.sock`），见 [SCOPE_LOCK_P3b.md](SCOPE_LOCK_P3b.md)。Network Extension / 公证 / App Store **不在本切片**。

交付：

- `helper_proto` JSON 行协议 + 路径白名单  
- `braid-helper` 二进制（root 启停 sing-box）  
- `scripts/install-helper.sh` / `uninstall-helper.sh` + LaunchDaemon plist  
- `SingBoxPlane`：socket 可达则走 helper，否则回退本地 spawn 并诚实 Permission  
- UI / footer 提示安装 helper  

验收：

- Protocol / allowlist 单测；`cargo build --bin braid-helper`  
- `nsh test`（mock）全绿  
- 未装 helper：Permission 文案含 install 路径  
- 已装 helper（手工）：Connect 经 socket 由 root 跑 sing-box  

后续（P3c / 另开切片）：首次引导 UI、签名公证、NE 作为长期替代。

### P3 usable — 真机可用桥接（已完成）

**目标**：在 helper 已有的前提下，能加载真实 Profile，且 live Connected 诚实（helper + Path TCP）。

交付：

- `…/Braid/profile.json` 或 `BRAID_PROFILE` 加载；UI「Load saved profile」  
- live：无 helper → Failed(Permission)；有 helper 后 TCP 失败 → Failed(Path)  
- `src/probe.rs` 最小探测  

不包含：菜单栏、公证、完整诊断 UI（P3c / P4）。

### P3c — 分发与菜单栏

| 项 | 状态 |
|---|---|
| `nsh package` ad-hoc `.app`（`scripts/package.sh`） | **已完成（lite）** |
| 菜单栏 NSStatusItem | **未做**（NSH 0.4 无 API；紧凑窗保留） |
| Apple 公证 / Developer ID / 自动更新 | **未做**（另开） |

### P4 — 体验与双链路智能

| 项 | 状态 |
|---|---|
| 诊断一句人话（本地 OK + VPS TCP 失败 ⇒ Path；LocalDown） | **已完成** |
| live 延迟（TCP RTT） | **已完成** |
| Connected 掉线检测（Refresh status / tick） | **已完成** |
| Degraded 态、双链路智能、wall sync_ip、巨库规则 | **未做**（可选后续） |

### P5 — Android / iOS

- 同一 Profile 与渲染器  
- 平台 DataPlane  
- 商店合规单独评估  

---

## 8. 测试策略

| 层 | 内容 | 是否联网 | 是否真 TUN |
|---|---|---|---|
| 单元 | Profile 校验、渲染快照、错误分类、状态机 | 否 | 否 |
| NSH | 面板文案与按钮流（MockPlane） | 否 | 否 |
| 可选集成 | 已缓存二进制时 `sing-box check` | 否 | 否 |
| 手工 | §9 清单 | 是 | 是 |

**禁止**：默认 CI 下载 sing-box 或依赖管理员权限。

回归锁：渲染出的 JSON 关键 tag（`stealth`/`speed`/`auto`/`tun-in`/`direct`）变更必须改测试。

---

## 9. 手工验收清单（真 TUN）

环境：真实 Profile（勿提交）、已授权环境（helper/NE/sudo 开发钩子）。

1. Connect 后 `ifconfig`/`route` 可见 tun/utun 或等价；访问境外 URL 成功。  
2. 断开后路由恢复；Safari 在未连时行为正常。  
3. 拔掉到 VPS 的路径（或故意错误 IP）→ Failed(Path) 或等价，而非转圈到死。  
4. 拒绝 VPN/Helper 权限 → Failed(Permission)。  
5. 杀 sing-box 进程 → UI 进入 Failed/Disconnected，不得假 Connected。  
6. 服务器 IP 的 `/32` DIRECT 仍在，避免 TUN 自环。

---

## 10. 安全与隐私

- Profile 含密钥：文件权限 `0600`；不进日志原文。  
- 崩溃日志脱敏（UUID/密码/pin）。  
- 下载二进制校验 hash。  
- 不引入分析 SDK（v1）。  
- 许可证：**Apache-2.0**（见 `LICENSE` / `NOTICE`）。公开仓仅清洁树；演示夹具不得变真凭据。

---

## 11. 与 wall 的关系

| | wall | braid |
|---|---|---|
| 定位 | 服务端部署与运维 | 客户端 |
| 可见性 | 保持私有 | 私有起步；公开另审 |
| 协作 | `client_config` / 导出 Profile 的桥可在 wall | 只消费 Profile JSON |
| 禁止 | — | 复制 server.conf、aws key、真实 IP 进 git |

「一键同步 IP」若做：只改本地 Profile 的 `server` 字段，凭据来源仍是用户本机已有文件。

---

## 12. 决策记录（防扯皮）

| ID | 决策 | 不采纳的替代 | 重评条件 |
|---|---|---|---|
| D1 | 数据面只用 sing-box | 多内核 / 自研 | sing-box 长期无法满足双链路 |
| D2 | TUN 强制；系统代理非主路径 | 代理-only MVP | 无 |
| D3 | 用户配置用 Profile JSON | 主路径 Clash YAML | 大量用户强依赖 YAML 且不愿转换 |
| D4 | P3 分 a（进程/诚实失败）与 b（NE/helper） | 一次做完 NE | P3a 已证明启停稳定且人手够 |
| D5 | 主 UI 极简 | Clash 式控制台 | 目标用户变为硬核规则编辑者 |
| D6 | 与 wall 分仓 | monorepo 含服务端密钥史 | 无 |

---

## 13. 当前实现对照（防文档空转）

| 规划项 | 代码位置（若有） | 状态 |
|---|---|---|
| Profile | `src/profile.rs` | 已有 |
| sing-box 渲染 | `src/singbox.rs` | 已有 |
| 状态机 UI | `src/connection.rs`, `src/lib.rs` | 已有（仍为 mock 连接） |
| DataPlane / 真启停 | `src/runtime.rs` | **P3a 已实现**（默认 mock；`BRAID_LIVE=1` 真平面） |
| Privileged Helper | `src/helper_proto.rs`, `src/bin/braid_helper.rs`, `scripts/*helper*` | **P3b 已实现**（NE 推迟） |
| 本机 Profile / Connected 门槛 / Path TCP | `src/profile.rs`, `src/connection.rs`, `src/probe.rs` | **P3 usable 已实现** |
| 诊断 / 延迟 / tick | `src/probe.rs`, `src/connection.rs`, UI | **P4 核心已实现** |
| ad-hoc 打包 | `scripts/package.sh` | **P3c lite 已实现** |
| NE / 公证 / Status Item / P5 | — | **未做** |

实现新功能前：更新本表与对应阶段验收；若改 D1–D6，先改 §12 再写代码。

---

## 14. 变更记录

| 日期 | 变更 |
|---|---|
| 2026-11-06 | 初版：架构、契约、P3a/b 切片、红线、验收与决策表 |
| 2026-11-06 | 许可证改为 Apache-2.0；仓库公开；P3a DataPlane 落地 |
| 2026-11-06 | P3b：锁定 LaunchDaemon helper（非 NE）；协议/二进制/安装脚本/plane 优先 socket |
| 2026-11-06 | P3 usable：本机 Profile、live Connected 需 helper、TCP Path 探测 |
| 2026-11-06 | macOS MVP 收口：P4 诊断/RTT/tick + P3c lite package；排除 NE/公证/Status Item/P5 |

---

## 15. 下一步（实现入口）

**macOS 个人客户端 MVP 代码侧已收口。** 余下：

1. 本机 §9 手工 TUN（真实 Profile + helper）  
2. 另开切片：NE、公证、NSStatusItem（待 NSH API）、P5、Degraded/双链路  

未装 helper 时，对外描述须保留「需安装 helper」类限定语。
