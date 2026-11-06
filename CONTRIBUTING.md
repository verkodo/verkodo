# Contributing to verkodo

Thank you for helping. The rules for every verkodo repository – sign-off, licences, everything included, no
telemetry – are in the [organisation's CONTRIBUTING.md](https://github.com/verkodo/.github/blob/main/CONTRIBUTING.md).
This file adds how to work on this one.

## Build and test

Rust from `rust-toolchain.toml`, Node 26 and pnpm (the version in `web/package.json`).

```sh
(cd web && pnpm install && pnpm build)
cargo fmt --check && cargo clippy --all-targets && cargo test
cargo run -q -- openapi > api/openapi.json && (cd web && pnpm api)   # after changing the API
(cd web && pnpm check)
cargo deny check
scripts/check-age.py                                                  # after changing Cargo.lock
```

## Rules

- **One SDK, many interfaces:** the command line, the language server and the API work through `verkodo-sdk` only;
  the web editor uses the API and verkodo-wasm.
- **Dependencies:** in `Cargo.toml` the lowest version a crate needs (a range: the crates are on crates.io, and
  programs building them in resolve them with their own dependencies); the exact versions in `Cargo.lock`, and in
  `package.json` exact (no `^`) – each the newest that is at least 14 days old: `scripts/check-age.py` for Cargo,
  `minimumReleaseAge` in `web/pnpm-workspace.yaml` for pnpm; a fix for a security advisory may be younger.
- **Releases:** the version in the workspace's `Cargo.toml`, then a tag `v<version>` on `main`: CI publishes every
  crate but `verkodo-wasm` to crates.io (job `crates`, approved by the maintainer).
