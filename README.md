# Braid

Personal **TUN-first** proxy client.

- **UI**: [Native Shell (NSH)](https://nsh.dev)
- **Data plane**: [sing-box](https://github.com/SagerNet/sing-box) (downloaded at runtime, version-pinned + SHA256)
- **First target**: macOS; Android / iOS later from the same app model

This repository stays clean — no production hosts or secrets.

**Tech plan (source of truth):** [docs/TECH_PLAN.md](docs/TECH_PLAN.md)  
Also: [docs/SPEC.md](docs/SPEC.md) · [docs/README.md](docs/README.md)

## Status

**P3a (current):** DataPlane + sing-box prepare/start/stop. Default UI uses a **mock plane** so tests stay offline. Real process plane: `BRAID_LIVE=1`. Reliable TUN for end users still needs **P3b** (Network Extension / privileged helper) — see the tech plan.

| Phase | Deliverable |
|---|---|
| P1 | NSH shell, connect UI, mock state |
| P2 | Profile import + sing-box config generation |
| P3a | sing-box process lifecycle + honest Permission failures |
| P3b | macOS NE / helper (real TUN for normal users) |
| P4 | Diagnostics copy, dual-link polish |

Demo profile (fake credentials only): [`fixtures/demo.profile.json`](fixtures/demo.profile.json).

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

# Attempt real sing-box (downloads pinned binary on first prepare; TUN often needs elevation)
BRAID_LIVE=1 nsh run --target macos
```

Optional: `BRAID_SINGBOX=/path/to/sing-box` to skip download.

## Test

```bash
cargo test --lib
# default mock plane — do not set BRAID_LIVE
nsh test
```

## License

Apache License 2.0 — see [LICENSE](LICENSE) and [NOTICE](NOTICE).

Runtime sing-box binaries are upstream artifacts with their own license; this repo does not vendor them.
