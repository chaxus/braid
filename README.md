# Braid

A personal cross-platform proxy client with **TUN** as the primary path.

- **UI**: [Native Shell (NSH)](https://nsh.dev)
- **Data plane**: sing-box (planned)
- **First target**: macOS menu-bar app; Android / iOS later from the same UI model

Braid is intentionally a **clean, separate** project from server-side deployment tooling. No production credentials or host inventory belong in this repository.

## Status

Scaffold only. Product direction:

1. One primary action: connect / disconnect (TUN)
2. Dual-link aware UX (stealth vs speed) without exposing kernel config by default
3. Clear failure copy (path vs server), not a settings dump

## License

MIT — see [LICENSE](LICENSE).
