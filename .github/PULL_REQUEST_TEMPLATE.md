<!-- Thanks for your contribution! Fill in the sections below. -->

## What

A short description of the change.

## Why

The motivation — link to the issue if one exists (`Closes #123`).

## How

Brief notes on the approach, any trade-offs, and areas you're unsure about.

## Test plan

- [ ] `cargo check --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `pnpm --filter ui check` passes
- [ ] `pnpm --filter ui build` succeeds
- [ ] Manual smoke test: <describe what you tested and how>

## Screenshots / recordings

If there's a UI change, attach a before/after or short clip.

## Breaking changes?

Any API / config / storage format change that existing users would notice?
