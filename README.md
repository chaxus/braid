# Braid

Personal **TUN-first** proxy client.

- **UI**: [Native Shell (NSH)](https://nsh.dev)
- **Data plane**: [sing-box](https://github.com/SagerNet/sing-box) (downloaded at runtime, version-pinned + SHA256)
- **First target**: macOS; Android / iOS later from the same app model

This repository stays clean — no production hosts or secrets.

**Tech plan (source of truth):** [docs/TECH_PLAN.md](docs/TECH_PLAN.md)  
Also: [docs/SPEC.md](docs/SPEC.md) · [docs/README.md](docs/README.md)

## Status

**macOS MVP + NE skeleton:** privileged helper, Packet Tunnel scaffold, saved profile, honest Connected (`via_ne` or `via_helper`), Path/Local diagnostics, live RTT, Refresh, ad-hoc `nsh package`. Main panel is **connect-first**. Default UI uses a **mock plane**; live: `BRAID_LIVE=1`. Connect prefers Network Extension when the signed appex is installed; otherwise falls back to helper.

Still out of scope: Apple notarization, App Store, menu-bar Status Item (no NSH 0.4 API), Android/iOS, replacing the helper.

| Phase | Deliverable |
|---|---|
| P1–P3b | Shell, profile/render, DataPlane, LaunchDaemon helper |
| P3 usable | Saved profile, helper Connected gate, TCP probe |
| P4 core | Local vs Path classify, RTT, tick, Run diagnostics |
| P3c lite | `scripts/package.sh` (ad-hoc `.app`) |
| P3b-NE | Packet Tunnel + App Group + Rust NEVPN (helper fallback) |

Demo profile (fake credentials only): [`fixtures/demo.profile.json`](fixtures/demo.profile.json).

**Real profile (do not commit):** copy JSON to `~/Library/Application Support/Braid/profile.json`, or set `BRAID_PROFILE=/path/to.json`, then use **Load saved profile** in the UI.

## Requirements

- macOS 14+
- [nsh](https://nsh.dev/docs/installation) 0.4+ (`curl -fsSL https://nsh.dev/install.sh | sh`)
- Rust **nightly** (pinned in `rust-toolchain.toml`)

## Run

```bash
export PATH="$HOME/.nsh/bin:$PATH"
cd braid

# UI with mock plane (default — no download)
nsh run --target macos

# Attempt real sing-box (downloads pinned binary on first prepare)
BRAID_LIVE=1 nsh run --target macos

# One-time: install privileged helper so TUN works without per-connect sudo
./scripts/install-helper.sh

# Put your profile in place (never commit real credentials)
mkdir -p "$HOME/Library/Application Support/Braid"
cp /path/to/your.profile.json "$HOME/Library/Application Support/Braid/profile.json"
```

Optional: `BRAID_SINGBOX=/path/to/sing-box` to skip download. Uninstall helper: `./scripts/uninstall-helper.sh`.

## Network Extension (optional, preferred TUN path)

Requires **Xcode** and an **Apple Developer Team** (paid personal account is enough). Without a signed appex, Connect falls back to the helper — unsigned builds cannot claim NE Connected.

```bash
brew install xcodegen   # once
cp macos/Config/Local.xcconfig.example macos/Config/Local.xcconfig
# edit Local.xcconfig: DEVELOPMENT_TEAM = YourTeamID
# or: export DEVELOPMENT_TEAM=YourTeamID

./scripts/open-ne-xcode.sh
```

In Xcode:

1. Select **BraidHost** → Signing & Capabilities → your Team (same for **BraidTunnel**).
2. Confirm App Groups: `group.com.chaxus.braid`.
3. Confirm Network Extensions → Packet Tunnel.
4. Build & run **BraidHost** once so the system installs the appex.
5. Approve the VPN configuration when macOS prompts.
6. Run the NSH UI with `BRAID_LIVE=1 nsh run --target macos` — Connect prefers NE; footer shows `via NE`.

Do **not** commit `macos/Config/Local.xcconfig` or any certificates. Notarization is out of scope for this slice.

## Package

```bash
./scripts/package.sh
# → .nsh/out … ad-hoc signed .app (not notarized)
```

## Test

```bash
cargo test --lib
# default mock plane — do not set BRAID_LIVE
nsh test
```

## License

Apache License 2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE).

Runtime sing-box binaries are upstream artifacts with their own license; this repo does not vendor them.
