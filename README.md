# Braid

Personal **TUN-first** proxy client.

- **UI**: [Native Shell (NSH)](https://nsh.dev)
- **Data plane**: [sing-box](https://github.com/SagerNet/sing-box) (downloaded at runtime, version-pinned + SHA256)
- **First target**: macOS; Android / iOS later from the same app model

This repository stays clean — no production hosts or secrets.

**Tech plan (source of truth):** [docs/TECH_PLAN.md](docs/TECH_PLAN.md)  
Also: [docs/SPEC.md](docs/SPEC.md) · [docs/README.md](docs/README.md)

## Status

**macOS MVP complete:** privileged helper, saved profile, honest Connected, Path/Local diagnostics, live RTT, Refresh status, ad-hoc `nsh package`. Default UI uses a **mock plane**; live: `BRAID_LIVE=1`.

Still out of scope: Network Extension, Apple notarization, menu-bar Status Item (no NSH 0.4 API), Android/iOS.

| Phase | Deliverable |
|---|---|
| P1–P3b | Shell, profile/render, DataPlane, LaunchDaemon helper |
| P3 usable | Saved profile, helper Connected gate, TCP probe |
| P4 core | Local vs Path classify, RTT, tick, Run diagnostics |
| P3c lite | `scripts/package.sh` (ad-hoc `.app`) |

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
