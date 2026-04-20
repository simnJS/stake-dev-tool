# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0] — 2026-04-20

First public release.

### Added

- **Local Game Server** (`crates/lgs`) implementing the Stake Engine RGS
  contract with HTTPS via a locally-signed leaf certificate:
  - `POST /api/rgs/<game>/wallet/authenticate`
  - `POST /api/rgs/<game>/wallet/balance`
  - `POST /api/rgs/<game>/wallet/play`
  - `POST /api/rgs/<game>/wallet/end-round`
  - `POST /api/rgs/<game>/bet/event`
- **Math engine** reading flat layouts (`<game>/index.json`,
  `<game>/lookuptable_*.csv`, `<game>/books_*.jsonl.zst`), with:
  - zstd decoding + byte-offset indexing of events (lazy line parsing,
    zero-copy)
  - weighted RNG via binary search on pre-computed cumulative weights
  - per-mode cache with `OnceCell` warm-up
- **Tauri desktop app** (`crates/desktop`):
  - Game folder picker + auto-detection of `index.json`
  - Embedded LGS lifecycle (start/stop, auto-restart on folder change)
  - Local Root CA generation + install into Windows user trust store (no UAC)
  - Dedicated Chromium launcher with `--max-active-webgl-contexts=64`
  - Profile persistence (game path + URL + resolution snapshot)
- **Test view** (SvelteKit, served over HTTPS by LGS):
  - Side-by-side iframes at 7 built-in resolutions + user-defined customs
  - Per-iframe mute overlay + reload + open-in-new-tab
  - Live sidebar controls: initial balance, currency (35+ + social tokens),
    language (16), device, social-casino toggle
  - Custom Picker component with searchable dropdown + flag icons
- **Session pre-configuration** — the test view writes initial balance,
  currency, and language into the LGS before the game loads, so auth is
  idempotent and reflects the UI state immediately.
- **Admin endpoints** (`/api/admin/…`) for session upsert and settings
  management (toggle / add / delete resolutions).

### Documentation

- `README.md`, `CONTRIBUTING.md`, issue + PR templates.
- MIT licence.

### Platforms

- Windows 10 / 11 (x86_64) — .msi + .exe installers in Releases.
- macOS / Linux — buildable from source, installers not yet packaged.
