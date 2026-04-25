# 🎰 Stake Dev Tool **v1.0** is out

Massive update — the slot-dev workbench just got two new superpowers and a redesigned UI from top to bottom.

---

## 🚀 Headline features

### 👥 **Teams**
Collaborative workspaces backed by a private GitHub repo per team. Sign in once with GitHub Device Flow (token in your OS keychain), then:
- Push profiles + saved rounds + math files to share with your team
- Pull a teammate's game in one click — math goes to `Documents/stake-dev-tool/teams/.../`, profile is wired up automatically
- Multi-team push picker, role-based ownership, owner-only delete
- Sync button propagates new bookmarks across the team

### 🌐 **Shareable preview links**
One click → your slot is hosted as a static page on GitHub Pages. Math + game both run **in the browser** via a brand-new WASM RGS — no server, no infra, anyone with the URL can play.
- 3 privacy modes:
  - `Sampled` — ~100 books per mode, tiny + fast (recommended for demos)
  - `Partial` — all books, events truncated by half (broken RTP, harder to reverse)
  - `Full` — math as-is, fully public
- Per-preview repo (clean unpublish via `DELETE /repos`)
- Re-publish updates in place

### 🦀 **lgs-wasm** (new crate)
Math engine compiled to wasm32 — pure-Rust zstd, lazy mode loading, fetch + XHR shim that intercepts the game's RGS calls. Same contract as the local LGS, runs in any browser.

---

## 🎨 UI redesign

Full **shadcn-svelte** rewrite (Geist Mono everywhere, dark theme tokens):

- **Topbar reorganized** in 3 hierarchy groups: LGS pill (status + port settings), Account (GitHub user / Sign-in), Navigation (Teams, Updates)
- **Test view** rebuilt with shadcn components — bigger typography for readability, cleaner sidebar (Session / Events / Layout sections)
- **GitHub Sign-in dialog** is now a shared component, reachable from the topbar AND inline in the Share preview dialog (no more detour through Teams just to sign in)
- **LGS port is configurable** from the topbar settings (persisted to localStorage, hot-restarts the LGS when running)

---

## 🐞 Notable fixes

- **Test view bet history works again** — SSE was returning 404 when the session wasn't yet prepared, which permanently kills `EventSource`. Fixed: stream now returns 200 with an empty snapshot until `/play` runs.
- **No more cross-team round leak** — `sync_saved_rounds` now filters local rounds by the games catalogued in each team (push + pull symmetric).
- **Preview repos no longer orphan on rename** — keyed off the immutable profile UUID instead of the editable name.
- **`enable_pages` failures surface** — no more "done" toast for a URL that 404s indefinitely.
- **Replay path no longer 500s** in the WASM preview (was matched but never dispatched).
- **CA banner stops flickering** when navigating back to the main page.
- Plus: `list_team_repos` paginated past 100, `remove_from_catalog` logs DELETE failures, asymmetric pull-filter on team rounds tightened.

---

## ⬇️ Download

> https://github.com/simnJS/stake-dev-tool/releases/latest

| Platform           | File                                            |
| ------------------ | ----------------------------------------------- |
| Windows 10/11      | `Stake-Dev-Tool-v1.0.0-windows-x64.exe`         |
| macOS Apple Silicon| `Stake-Dev-Tool-v1.0.0-macos-arm64.app.tar.gz`  |
| Debian / Ubuntu    | `Stake-Dev-Tool-v1.0.0-linux-x64.deb`           |
| Other Linux        | `Stake-Dev-Tool-v1.0.0-linux-x64.AppImage`      |

Existing installs (≥ v0.3.9) get the update prompt automatically — Minisign-signed, silent install on Windows.

---

📜 Full changelog: <https://github.com/simnJS/stake-dev-tool/blob/main/CHANGELOG.md>
🐛 Bug? <https://github.com/simnJS/stake-dev-tool/issues/new>

Built with ❤️ by @simnJS — happy spinning 🎲
