# Braid

Personal **TUN-first** proxy client. UI: [Native Shell (NSH)](https://nsh.dev). Data plane (later): sing-box.

Mac first; Android / iOS later from the same app model. This repository stays clean — no production hosts or secrets.

## Status

**P2 (current):** Profile JSON + sing-box config generation + mock connect UI. Labelled `TUN · simulated (P2)`.

| Phase | Deliverable |
|---|---|
| P1 | NSH shell, connect UI, mock state |
| P2 | Profile import + sing-box config generation |
| P3 | Real macOS TUN |
| P4 | Dual-link auto + diagnostics copy |

Demo profile (fake credentials only): [`fixtures/demo.profile.json`](fixtures/demo.profile.json).

**Tech plan (source of truth):** [docs/TECH_PLAN.md](docs/TECH_PLAN.md)  
Also: [docs/SPEC.md](docs/SPEC.md) · [docs/README.md](docs/README.md)

## Requirements

- macOS 14+
- [nsh](https://nsh.dev/docs/installation) 0.4+ (`curl -fsSL https://nsh.dev/install.sh | sh`)
- Rust **nightly** (pinned in `rust-toolchain.toml`; nsh-core 0.4 needs it)

## Run

```bash
export PATH="$HOME/.nsh/bin:$PATH"
nsh run --target macos
# or:
cargo run
```

## Test

```bash
cargo test --lib
nsh test
```

## License

MIT — see [LICENSE](LICENSE).
