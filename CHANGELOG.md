# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.3.7] — 2026-04-22

### Fixed

- **Force / replay now returns the state for the requested event id** —
  the books indexer was mapping line N in `books_*.jsonl.zst` to event
  id N, but Stake math-sdk actually writes id N-1 on line N (because
  `library[sim + 1] = Book(sim).to_json()`, so the library key is sim+1
  while the book's `id` field is sim, 0-indexed). Force-spinning event
  4676 used to credit the correct payout (looked up in the weights
  table, which does use the `id` value) but display the visuals from
  event 4675. `BooksIndex` now keys by each line's `id` field instead
  of by line position, so any id numbering works — 0-indexed current
  math-sdk output, 1-indexed if the convention ever flips, or even
  non-contiguous ids.

## [0.3.6] — 2026-04-22

### Added

- **Debian/Ubuntu `.deb` package** alongside the existing AppImage, with
  `libnss3-tools` declared as an apt dependency. Installing via `.deb`
  pulls `certutil` automatically, so the "Install Local CA" step works
  out of the box with no extra manual command. AppImage still requires a
  manual `sudo apt install libnss3-tools` since AppImage can't declare
  system deps. Released as
  `Stake-Dev-Tool-vX.Y.Z-linux-x64.deb`.

## [0.3.5] — 2026-04-21

### Fixed

- **`/play` no longer credits winnings immediately** — previously a winning
  spin returned `balance = pre-bet - cost + payout` straight from `/play`,
  skipping the credit animation in the game client. The RGS contract
  expects `/play` to reflect only the bet deduction (`balance = pre-bet -
  cost`) and `/end-round` to credit the payout. Moved `add_winnings` from
  `play()` into `end_round()` and flipped the round's `active` flag to
  `true` until end-round resolves it. Also: if the client sends a second
  `/play` without `/end-round`, the previous round's pending payout is
  credited before the new bet is taken instead of being silently lost.

## [0.3.4] — 2026-04-21

### Fixed

- **Mode dropdowns now read from `index.json`** instead of a hardcoded list
  (`base` / `baseante` / `bonus` / `bonus5` / `duel` / `duel5`) — that list
  was specific to one game and left every other user staring at options
  their game didn't have. The test view's **Force event** and **Replay
  event** selects now populate from `GET /api/devtool/games/:game/modes`,
  which reads the current game's `index.json`. `forcedMode` / `replayMode`
  snap to `modes[0]` on load if their default (`base`) isn't present.

## [0.3.3] — 2026-04-21

### Changed

- **Test view now streams session events via SSE** instead of polling
  `/last-event` + `/events` once per second per frame. A single
  persistent `EventSource` per frame consumes `GET
  /api/devtool/sessions/:sid/stream`, which emits a `snapshot` on
  connect followed by an `event` for each new spin. Network tab stays
  empty when nothing spins; updates arrive instantly when they do;
  cost scales linearly in active frames. No user-visible feature
  changes.

## [0.3.2] — 2026-04-21

### Added

- **Notable rounds panel** — new collapsible in the test view's Events
  section. Scans each mode's lookup table and surfaces three notable bet
  ids: a no-win round (`min`), the round whose `payoutMultiplier` is
  closest to the weight-weighted average of winning multipliers (`avg`),
  and the highest payout in the table (`max`). One-click Force on each
  + ★ to bookmark with an auto-set description (`min` / `average win` /
  `max win`). Stats are computed lazily on first panel open via
  `GET /api/devtool/bet-stats/:game`.

## [0.3.1] — 2026-04-21

### Added

- **Bookmark from Bet History** — each row in the per-frame history table
  now has a ★ button that opens a popup to attach an optional description
  before saving the round. Already-bookmarked rows show a filled ★.

### Changed

- **History toggle relocated** — the "Bet history" button now sits directly
  below the iframe (next to the panel it opens) instead of above it, so the
  trigger and the content live in the same vertical zone.
- **Release asset names cleaned up** — installers and updater artefacts now
  follow a single consistent scheme:
  `Stake-Dev-Tool-vX.Y.Z-{windows,macos,linux}-{x64,arm64}.<ext>`. Replaces
  the inconsistent default Tauri names (`Stake.Dev.Tool_0.x.y_amd64.AppImage`,
  `Stake.Dev.Tool_aarch64.app.tar.gz`, …).

## [0.3.0] — 2026-04-20

### Added

- **Saved rounds (bookmarks)** — flag interesting `{mode, eventId}` combos
  with a free-form description and re-force them in one click. Persisted
  to `saved-rounds.json` alongside profiles/settings. New endpoints:
  `GET/POST /api/devtool/saved-rounds` and
  `PATCH/DELETE /api/devtool/saved-rounds/:id`.
- **★ Save button** next to the Force-event input — bookmarks the current
  mode + eventId from the test view in one click, with optional description.

### Changed

- **Test-view sidebar restructured** into three clearly labeled sections
  (Session / Events / Layout). Collapsible panels now expand inline under
  their own header instead of all stacking at the bottom of the aside.
  Sticky footer for status/error messages.

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
