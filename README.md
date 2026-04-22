<h1 align="center">Stake Dev Tool</h1>

<p align="center">
  A desktop development tool for slot games built on the
  <a href="https://stake-engine.com/">Stake Engine</a> RGS contract.<br />
  Bundles a high-performance Rust Local Game Server with a multi-resolution
  test view so you can run, debug, and visually QA slots locally — no
  production infra, no browser warnings, no hacks in game code.
</p>

<p align="center">
  <a href="https://github.com/simnJS/stake-dev-tool/releases/latest">
    <img alt="Latest release" src="https://img.shields.io/github/v/release/simnJS/stake-dev-tool?style=flat-square&color=emerald" />
  </a>
  <a href="https://github.com/simnJS/stake-dev-tool/releases">
    <img alt="Total downloads" src="https://img.shields.io/github/downloads/simnJS/stake-dev-tool/total?style=flat-square&color=blue" />
  </a>
  <a href="https://github.com/simnJS/stake-dev-tool/stargazers">
    <img alt="GitHub stars" src="https://img.shields.io/github/stars/simnJS/stake-dev-tool?style=flat-square" />
  </a>
  <a href="https://github.com/simnJS/stake-dev-tool/actions/workflows/ci.yml">
    <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/simnJS/stake-dev-tool/ci.yml?branch=main&label=CI&style=flat-square" />
  </a>
  <a href="https://github.com/simnJS/stake-dev-tool/commits/main">
    <img alt="Last commit" src="https://img.shields.io/github/last-commit/simnJS/stake-dev-tool?style=flat-square" />
  </a>
  <a href="LICENSE">
    <img alt="License" src="https://img.shields.io/github/license/simnJS/stake-dev-tool?style=flat-square" />
  </a>
</p>

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.90%2B-CE412B?style=flat-square&logo=rust&logoColor=white" />
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24C8DB?style=flat-square&logo=tauri&logoColor=white" />
  <img alt="SvelteKit" src="https://img.shields.io/badge/SvelteKit-5-FF3E00?style=flat-square&logo=svelte&logoColor=white" />
  <img alt="axum" src="https://img.shields.io/badge/axum-0.7-000000?style=flat-square" />
</p>

<p align="center">
  <b>Built for slot devs, by slot devs.</b>
</p>

---

## Download

Pick your platform and grab the latest build from the
[Releases page](https://github.com/simnJS/stake-dev-tool/releases/latest):

| Platform                               | File                                                | Notes                                               |
| -------------------------------------- | --------------------------------------------------- | --------------------------------------------------- |
| Windows 10/11 (x86_64)                 | `Stake-Dev-Tool-vX.Y.Z-windows-x64.exe`             | NSIS installer                                      |
| macOS Apple Silicon (M1/M2/M3…)        | `Stake-Dev-Tool-vX.Y.Z-macos-arm64.app.tar.gz`      | Extract → drag `.app` into `/Applications`          |
| Debian / Ubuntu (x86_64) **preferred** | `Stake-Dev-Tool-vX.Y.Z-linux-x64.deb`               | `sudo apt install ./<file>.deb` — pulls NSS tools   |
| Linux (x86_64, other distros)          | `Stake-Dev-Tool-vX.Y.Z-linux-x64.AppImage`          | `chmod +x` then run (install `libnss3-tools` first) |

> Intel Macs are not supported — if that's a blocker, open an issue.

## Table of Contents

- [Features](#features)
- [Quick start](#quick-start)
- [Math folder layout](#math-folder-layout)
- [Architecture](#architecture)
- [HTTP endpoints](#http-endpoints)
- [Environment variables](#environment-variables)
- [Run the LGS standalone](#run-the-lgs-standalone)
- [Build from source](#build-from-source)
- [Auto-updater](#auto-updater)
- [Contributing](#contributing)
- [License](#license)

## Features

- **Fast Rust LGS** — drop-in replacement for the reference RGS contract
  (`/api/rgs/<game>/wallet/…`). Serves `index.json`, `lookuptable_*.csv`,
  `books_*.jsonl.zst` directly from a local folder, with zstd-decoded books
  indexed once per mode, weighted RNG via binary search on pre-computed
  cumulative weights, and zero-copy event reads.
- **Multi-resolution test view** — open your game side-by-side at 7 built-in
  resolutions (Desktop, Laptop, Popout S/L, Mobile S/M/L) plus any custom sizes
  you define. Each iframe runs an isolated session so concurrent state bugs
  surface immediately.
- **SSE event stream** — one persistent connection per frame pushes every spin
  result to the test view in real time. Zero polling, zero wasted traffic.
- **Local HTTPS with trusted cert** — the LGS serves TLS with a local Root CA
  installed in your user trust store. Zero browser warnings, zero hacks in your
  game code. Works on Windows (no UAC), macOS (login keychain, one password
  prompt), and Linux (Chromium NSS store — `.deb` pulls `libnss3-tools`
  automatically).
- **Dedicated Chromium launcher** — opens a separate Chrome/Edge/Brave process
  with `--max-active-webgl-contexts=64` so PixiJS shader compilation survives
  running 7+ instances at once.
- **Saved rounds (bookmarks)** — flag interesting `{mode, eventId}` combos
  with a description and re-force them one-click from the sidebar, per game.
- **Notable rounds panel** — auto-picks `min` / `avg win` / `max win` bet ids
  per mode from the book data. Instant "find me a big win" without scrolling
  the lookup table.
- **Force event & Replay** — pin any event id for the next spin, or jump
  straight to a specific outcome via the Stake Engine replay contract.
- **Live per-session config** — balance, currency (35+ fiat + Stake Cash /
  Gold Coin), language (16), device, social-casino mode. Flags from
  flagcdn.com, switchable without reloading the main view.
- **Dynamic mode dropdowns** — read straight from the current game's
  `index.json`, not a hardcoded list.
- **Mute overlay** — each iframe is click-blocked by default so you don't get
  7 games screaming at you on launch. Click the speaker to unmute.
- **Profiles** — save math folder + front URL + resolution snapshot per game,
  reload with one click.
- **Auto-updater** — Minisign-signed release artifacts, silent install on
  Windows, replace-in-place on macOS/Linux.

## Quick start

1. **Launch the app**. On first start, click **Install Local CA** in the amber
   banner:
   - **Windows** — silent install into the user "Root" store, no UAC prompt.
   - **macOS** — one password prompt (login keychain). Covers Safari, Chrome,
     Edge, Brave. Firefox has its own NSS store; trust manually in
     `about:preferences#privacy` the first time.
   - **Linux** — installs into `~/.pki/nssdb` (Chromium family). Requires
     `libnss3-tools`; the `.deb` package declares it as a dependency so apt
     installs it automatically. AppImage users need to install it manually
     (`sudo apt install libnss3-tools` on Debian/Ubuntu, `sudo dnf install
     nss-tools` on Fedora). Firefox — same as macOS, trust manually.
2. **Browse…** to your game's math folder (containing `index.json` plus the
   `lookuptable_*.csv` and `books_*.jsonl.zst` files).
3. Enter the **Front URL** of your game's frontend (e.g.
   `http://localhost:5174` if you have a local Vite dev server).
4. Click **Launch test view** — a dedicated Chromium window opens on
   `https://localhost:3001/test/…` with your game rendered at all enabled
   resolutions.
5. From the test view sidebar you can change balance, currency, language,
   device, social mode, add custom resolutions, force / bookmark / replay
   events, and mute individual frames.
6. Back in the desktop app, click **Save** next to the folder picker to store
   the current math + URL + resolution snapshot as a profile. One-click reload
   next time.

## Math folder layout

Each game lives in its own directory:

```
<math_root>/
└── <game-slug>/
    ├── index.json                 # { "modes": [{ "name", "cost", "events", "weights" }, …] }
    ├── lookuptable_<mode>.csv     # eventId,weight,payoutMultiplier
    └── books_<mode>.jsonl.zst     # one event per line, zstd-compressed
```

Drop a game folder, point the picker at it — the LGS auto-detects modes from
`index.json`.

## Architecture

```
┌─────────────────────────── Tauri desktop app ────────────────────────────┐
│                                                                          │
│  WebView2 UI (SvelteKit)       Rust backend                              │
│  ┌─────────────────────┐       ┌──────────────────┐                      │
│  │  Main window        │  IPC  │  Commands        │                      │
│  │  - folder picker    │ ◄───► │  - start_lgs     │                      │
│  │  - profiles         │       │  - inspect_game  │                      │
│  │  - CA install       │       │  - install_ca    │                      │
│  │  - launch           │       │  - profiles …    │                      │
│  └─────────────────────┘       └────────┬─────────┘                      │
│                                         │                                │
│                                         ▼                                │
│                             ┌──────────────────────┐                     │
│                             │  Embedded LGS (lib)  │                     │
│                             │  axum + rustls + zstd│                     │
│                             │  (127.0.0.1:3001)    │                     │
│                             └──────────────────────┘                     │
└──────────────────────────────────┬───────────────────────────────────────┘
                                   │  HTTPS + CORS
                                   ▼
                    ┌──────────────────────────────┐
                    │  External Chromium           │
                    │  (spawned with               │
                    │   --max-active-webgl-        │
                    │    contexts=64)              │
                    │                              │
                    │  ┌──────┐ ┌──────┐ ┌──────┐  │
                    │  │iframe│ │iframe│ │iframe│  │
                    │  │ game │ │ game │ │ game │  │
                    │  └──────┘ └──────┘ └──────┘  │
                    │  …                           │
                    └──────────────────────────────┘
```

The test view is **served by the embedded LGS** (not via Tauri's custom
protocol) so iframes load from a real-browser context with full WebGL support.

### Crates

- [`crates/lgs`](crates/lgs/) — LGS library + standalone binary. Can be run
  without the desktop app via `cargo run -p lgs --release`.
- [`crates/desktop`](crates/desktop/) — Tauri shell. Thin layer around the LGS
  lib with commands for file picking, profile management, CA install, and
  browser launching.
- [`ui/`](ui/) — SvelteKit frontend, builds to static HTML/JS. Serves as both
  the Tauri WebView content AND (for `/test/`) the browser test view.

## HTTP endpoints

**RGS contract** (Stake Engine compatible):

- `POST /api/rgs/<game>/wallet/authenticate`
- `POST /api/rgs/<game>/wallet/balance`
- `POST /api/rgs/<game>/wallet/play`
- `POST /api/rgs/<game>/wallet/end-round`
- `POST /api/rgs/<game>/bet/event`

**Devtool** (used by the test view + desktop app — no auth, just tooling):

- `GET    /api/devtool/status`
- `POST   /api/devtool/sessions/prepare`
- `GET    /api/devtool/sessions/:sid/last-event`
- `GET    /api/devtool/sessions/:sid/events`
- `GET    /api/devtool/sessions/:sid/stream` &nbsp;← SSE
- `GET    /api/devtool/games/:game/modes`
- `GET    /api/devtool/bet-stats/:game`
- `GET    /api/devtool/saved-rounds`
- `POST   /api/devtool/saved-rounds`
- `PATCH  /api/devtool/saved-rounds/:id`
- `DELETE /api/devtool/saved-rounds/:id`
- `GET    /api/devtool/settings`
- `POST   /api/devtool/settings/toggle`
- `POST   /api/devtool/settings/custom`
- `DELETE /api/devtool/settings/custom/:id`
- `GET    /api/devtool/force-event`
- `POST   /api/devtool/force-event`
- `DELETE /api/devtool/force-event`

**Replay** (Stake Engine canonical contract):

- `GET /bet/replay/:game/:version/:mode/:event`

## Environment variables

| Variable          | Default          | Purpose                                          |
| ----------------- | ---------------- | ------------------------------------------------ |
| `LGS_BIND_ADDR`   | `0.0.0.0:3001`   | Where the standalone LGS binds                   |
| `LGS_MATH_DIR`    | `./math`         | Root folder containing game subfolders           |
| `LGS_UI_DIR`      | (auto-detected)  | Override the path to `ui/build/` for serving UI  |
| `RUST_LOG`        | `info`           | `tracing-subscriber` filter                      |

The desktop app sets these automatically; they're only useful when running
`cargo run -p lgs` standalone.

## Run the LGS standalone

```bash
LGS_BIND_ADDR=127.0.0.1:3001 \
LGS_MATH_DIR=./math \
cargo run -p lgs --release
```

## Build from source

**Prerequisites:**

- Rust 1.90+ (`rustup` recommended)
- Node.js 20+ and pnpm 10+
- On Windows: [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
  (pre-installed on Win 11)
- On Linux: `libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev
  libssl-dev libayatana-appindicator3-dev librsvg2-dev`
- On macOS: Xcode Command Line Tools

```bash
git clone https://github.com/simnJS/stake-dev-tool.git
cd stake-dev-tool

# install JS deps (root pnpm workspace)
pnpm install

# dev mode (hot reload)
pnpm tauri:dev

# release build (produces installer + portable exe)
pnpm tauri:build
```

Artifacts land under `target/release/bundle/`.

## Auto-updater

Once installed, the app checks GitHub Releases for a newer version on startup
and shows a banner with release notes + "Download & install" button. Updates
are downloaded, verified with a Minisign signature, and installed silently
(passive NSIS mode on Windows, replace-in-place on macOS/Linux).

The updater only works **from** a build that already has the plugin — so the
first upgrade that uses it is the one **to** the release that introduced it.
Earlier versions have to be reinstalled manually.

### Release signing (maintainer setup)

Releases are signed with a Tauri [Minisign](https://jedisct1.github.io/minisign/)
keypair. The public key is embedded in the app at build time; the private key
lives in GitHub Secrets.

To produce a new signing keypair (one-time, already done for this repo):

```bash
pnpm exec tauri signer generate -w ~/.tauri/stake-dev-tool.key
```

Then add the following secrets to the GitHub repo (Settings → Secrets →
Actions):

- `TAURI_SIGNING_PRIVATE_KEY` — contents of the `.key` file (multiline, paste
  as-is).
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — the password entered during
  `signer generate` (empty string if none was set).

Tag a release (`git tag v0.x.y && git push origin v0.x.y`) and the workflow
signs artefacts, uploads them + a `latest.json` manifest to the release.
Rotating keys requires re-shipping the app with the new public key, so treat
them carefully.

## Contributing

Pull requests, issues, and discussions are welcome. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the setup and conventions.

## License

MIT — see [LICENSE](LICENSE).

## Acknowledgements

- [Stake Engine](https://stake-engine.com/) — the RGS contract this tool
  targets.
- [Tauri](https://tauri.app/), [SvelteKit](https://svelte.dev/),
  [axum](https://github.com/tokio-rs/axum), [rustls](https://github.com/rustls/rustls),
  [rcgen](https://github.com/rustls/rcgen) — doing the heavy lifting.
- [flagcdn.com](https://flagcdn.com/) — country flag PNGs.

---

<p align="center">
  <sub>
    Made with ❤️ by <a href="https://github.com/simnJS">@simnJS</a> ·
    <a href="CHANGELOG.md">Changelog</a> ·
    <a href="https://github.com/simnJS/stake-dev-tool/issues/new">Report a bug</a>
  </sub>
</p>
