# 🎰 Stake Dev Tool **v1.0** is out

Two new superpowers + a UI redesign.

## 🚀 What's new

- **👥 Teams** — collaborative workspaces backed by a private GitHub repo. Share profiles, math files, and bookmarked rounds with your team in one click.
- **🌐 Share preview links** — your slot becomes a static page on GitHub Pages, with math + game running in-browser via a new WASM RGS. No infra, anyone with the URL can play. 3 privacy modes (Sampled / Partial / Full).
- **🎨 UI redesign** — full shadcn-svelte rewrite, redesigned topbar with GitHub sign-in + configurable LGS port, test view rebuilt for readability.

## 🐞 Notable fixes

- Bet history / last-event in the test view works again (SSE 404 fixed)
- No more cross-team round leak when syncing
- Preview repos no longer orphan when you rename a profile
- Replay path no longer 500s in the WASM preview

## ⬇️ Download

> https://github.com/simnJS/stake-dev-tool/releases/latest

Existing installs (≥ v0.3.9) get the update prompt automatically.

📜 Full changelog: <https://github.com/simnJS/stake-dev-tool/blob/main/CHANGELOG.md>

Built with ❤️ by @simnJS — happy spinning 🎲
