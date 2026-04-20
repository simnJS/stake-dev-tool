# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.2] — 2026-04-20

### Fixed

- Auto-updater `latest.json` contained draft-only GitHub asset URLs
  (`untagged-<hash>`) which 404 once the release is published. The manifest
  job now constructs stable public URLs from the tag + filename.

## [0.2.1] — 2026-04-20

### Fixed

- Test view (`/test/...`) was returning 404 in installed builds because the
  LGS only served the UI from disk and the bundle didn't ship `ui/build/`
  alongside the exe. Release builds now **embed the UI inside the binary**
  via `include_dir!`. Dev builds keep the disk-based serving for hot reload.

## [0.2.0] — 2026-04-20

### Added

- **Auto-updater** — the app checks GitHub Releases on startup and offers a
  one-click download + install. Signed with Minisign; the public key is
  embedded in the binary, the private key sits in GitHub Secrets.
- **Replay endpoint** — `GET /bet/replay/:game/:version/:mode/:event`, matches
  the canonical Stake Engine contract. No session / no auth.
- **Replay launcher** — sidebar panel to load a specific event into a frame
  with the official replay URL params.
- **Force event** — sidebar input to pin `/play` responses to a specific
  eventId for a given mode. Great for debugging a specific outcome without
  RNG luck.
- **Event history** — every `/play` appends to a bounded (100-entry) per-session
  history. The test view shows the last event prominently and a collapsible
  full-history table per frame.
- **Bigger last-event display** — prominent strip above each iframe with event
  id, multiplier, bet, win. Forced spins are badged.
- **New app icon** — custom S/DT logo across Windows / macOS / Linux bundles.

### Changed

- Admin endpoints moved from `/api/admin/*` → `/api/devtool/*` (they're not
  actually admin — no auth, no privilege escalation; they're tooling).
- Mute now recycles the iframe when silencing an already-playing frame,
  because cross-origin iframes don't let the parent pause their AudioContext.
- Release bundles trimmed to the updater-compatible formats only: NSIS (Windows),
  `.app.tar.gz` (macOS Apple Silicon), AppImage (Linux).

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
  - Local Root CA generation + install into the user trust store, per-OS:
    - Windows — user "Root" store via `certutil`, no UAC
    - macOS — login keychain via `/usr/bin/security` (one password prompt)
    - Linux — user NSS database via `certutil` (needs `libnss3-tools`)
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

- Windows 10 / 11 (x86_64) — `.msi` + `.exe` (NSIS) installers.
- macOS 12+ on Apple Silicon (`aarch64`) — `.dmg`. Intel Macs are not built.
- Linux (`x86_64`) — `.AppImage` + `.deb`.
