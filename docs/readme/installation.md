# Installing and updating

## Requirements

- Windows 10 or Windows 11 (x86_64)
- Google Chrome. Edictora finds it automatically, or set `chromePath` in the config file or pass `--chrome`.
- A CWS developer account
- Rust 1.100.1 if you build from source (pinned in `rust-toolchain.toml`). Bun is needed only when you change the in-page scripts.

## Installing

Run `Edictora-win-Setup.exe` (a Velopack installer). It installs to `%LOCALAPPDATA%\Edictora` without administrator rights.

- Setup adds `%LOCALAPPDATA%\Edictora\current` to your user `Path`, so `edictora` works in any newly opened Command Prompt or PowerShell window.
- When you sign in to Windows, `edictora-updater.exe` runs without a window. If `https://edictora.kagayoi.com` has a newer version, it downloads it, checks its size and hash, and installs it.
  - It's registered as `Edictora Updater` under `Run` in HKCU, so you can turn it off under "Startup apps" in Task Manager.
  - The result is written to the log.
- To update by hand, run `edictora update`. It reports to standard error and exits with code 0 when you're up to date or the update has started, and with code 1 on failure. Update.exe replaces the files after `edictora` exits.
- The Portable version (`Edictora-win-Portable.zip`) doesn't update at sign-in and doesn't add itself to `Path`. To update it, run `current\edictora.exe update` in the folder you extracted it to. Executables built from source don't update themselves.
- Updates are never applied while `login`, `pull`, `diff`, `push` or `submit` is running; Edictora applies them later.
  - If you run one of these commands while an update is being applied, it exits with code 1 without opening Chrome. Wait a moment and run it again.
  - This also applies after the update process exits, until Update.exe has finished replacing the files.
- Uninstalling removes `%LOCALAPPDATA%\Edictora` and the `Run` and `Path` entries. Your data (`%LOCALAPPDATA%\Edictora.Data`) and settings (`%APPDATA%\Edictora`) are kept; delete them yourself if you no longer need them.
