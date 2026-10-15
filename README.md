# Anchor

A personal desktop app for tracking B3 stock and crypto investment theses — fair-price valuation
models plus a crypto scoring dashboard, replacing a spreadsheet.

## Why

Tracking "is this stock cheap?" across several valuation methods, each with its own assumptions,
outgrows a spreadsheet fast — especially when you want to keep more than one calculation per asset
(different growth-rate assumptions, compared side by side) and get notified automatically when an
asset crosses into a "buy zone." This app replaces that spreadsheet with a small local database and
a UI built around exactly that workflow.

## What it does

- **7 stock valuation models**: Bazin, Graham, Gordon/DDM, DCF/FCFF, Banks (P/B–ROE–Gordon), RNAV,
  and Projected Ceiling Price — each with its own guarded inputs and pass/fail verdict.
- **Multiple saved valuations per ticker**, each with its own assumptions, browsable and comparable.
- **Crypto score dashboard**: a 9-indicator green/neutral/red score (TVL trend, net issuance,
  fees-vs-issuance, NVT ratio automated; the rest fall back to manual entry where no free data
  source exists).
- **Alerts**: register a price/indicator rule, a background check runs periodically, and a native
  OS notification fires when it triggers.
- **Stock Lookup**: search any B3 ticker for quote, fundamentals, technicals (SMA/CAGR), a
  dividend-history chart, and a free-text note — auto-fetches data on first search, cached after.
- **(in progress)** Cross-device sync via [TruthID](https://github.com/masterlxz/truthid) + IPFS —
  no server operated by this project; see [`project/PHASE.md`](project/PHASE.md), Fase 8.

## Architecture

| Component | Stack | Path |
|---|---|---|
| Desktop app | Tauri + Rust + React/TypeScript | [`desktop/`](desktop/) |
| Smart contract (sync, in progress) | Solidity (Foundry) | [`contracts/`](contracts/) |

Quotes, fundamentals, and crypto data come from a local instance of the
[Lume Finance API](https://github.com/Arven-Labs/lume) (a sibling open-source
project), embedded as a Tauri sidecar process — the Rust side talks to it over HTTP, no
Python or subprocess-per-fetch involved.

## Building from source

**Desktop app** (Tauri + React, runs in Docker to avoid host toolchain setup):
```
cd desktop
./dev.sh          # run the app
./dev.sh check    # quick compile check (tsc + cargo check), no app window
```

**Smart contracts** ([Foundry](https://book.getfoundry.sh/)):
```
cd contracts
forge build
forge test
```

**CI**: GitHub Actions runs the desktop (`cargo test`, `tsc`), contracts (`forge test`) and mobile
(`flutter analyze`/`flutter test`) checks on every push to `main` touching each folder. Pushing a
`v*` tag builds the Linux/Windows/macOS installers and the Android APK (debug-signed for now) into a
draft release. See
[`.github/workflows/`](.github/workflows/).

## Status

This is a personal project built incrementally, session by session, at a background pace — not
every phase is finished. [`project/`](project/) has the full roadmap, architecture decisions, and
session-by-session history — start at [`project/INDEX.md`](project/INDEX.md).

## Security

- API keys (e.g. a remote Finance API key) live in the OS keyring, never in the SQLite database or
  in git.
- Your local SQLite database (actual valuations/portfolio data) is gitignored and never leaves your
  machine.
- This is a personal tool, not professionally audited — treat it as early-stage software.

## License

MIT — see [`LICENSE`](LICENSE).
