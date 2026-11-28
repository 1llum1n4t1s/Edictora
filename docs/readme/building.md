# Building

The main program is written in Rust (`crates/edictora/`).

```powershell
cargo build --release          # target\release\edictora.exe and edictora-updater.exe (Bun not needed)
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The scripts that run inside dashboard pages live in `page-scripts/` (Bun + TypeScript). The generated `page-scripts/dist/edictora.js` is embedded in the exe, so you need Bun only when you change those scripts.

```powershell
cd page-scripts
bun install --frozen-lockfile   # install dependencies (TypeScript only)
bun run typecheck      # type-check
bun run build          # regenerate dist/edictora.js
bun run check          # check that dist/edictora.js is up to date
bun test               # run the page-script tests (fake DOM)
```

For testing, you can override:
- The local data root with `EDICTORA_LOCAL_ROOT` (default `%LOCALAPPDATA%\Edictora.Data`)
- The roaming root with `EDICTORA_ROAMING_ROOT` (default `%APPDATA%\Edictora`)
- The update source with `EDICTORA_UPDATE_URL` (default `https://edictora.kagayoi.com`; plain `http://` only for `127.0.0.1`)

The Velopack package contains both `edictora.exe` and `edictora-updater.exe`.
- Updates don't use the velopack crate: Edictora fetches the feed, downloads and verifies packages itself with WinHTTP.
- Installing, replacing files and uninstalling are left to Velopack's Setup.exe and Update.exe.

Releases are served from Cloudflare R2 at `https://edictora.kagayoi.com` (Velopack channel `win`). See [the release process](../release.md).
