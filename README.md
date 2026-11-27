<p align="center">
  <img src="icon/app_icon.png" alt="Edictora icon" width="128">
</p>

# Edictora

English | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**A Windows tool that reads, compares and updates your Chrome Web Store listings (description and URLs) in bulk, using source files kept in each extension's repository.**

The public Chrome Web Store (CWS) API (v2) has no way to read or write store listings, and Chrome doesn't let extensions operate on CWS pages.
Edictora instead drives a Chrome instance with a dedicated profile over the Chrome DevTools Protocol (CDP), and edits the Developer Dashboard with the same input a person would use.
It runs as a single command-line tool (`edictora.exe`). It's written in Rust and needs no runtime such as .NET or Node.js.

> [!IMPORTANT]
> Edictora depends on the layout of the Developer Dashboard. It was verified with the dashboard shown in **Japanese**.
> English dashboards are recognized too, but the English on-screen labels haven't been verified on a real screen yet.
> If Edictora can't find a field or button (for example after a dashboard redesign), it stops without changing anything.

## Features

- **pull**: reads the current values from the dashboard (description, official URL, homepage URL and support URL). Read-only.
- **diff**: shows the differences from your source files. Read-only, and the default command.
- **push**: rewrites only the fields that differ, **saves them as a draft**, then reloads the page and checks the saved values against your source files.
- **Images**: when an item has `images` in the config, `pull`, `diff` and `push` also handle its store images (store icon, screenshots, small promo tile and marquee). Narrow them with `--only text|images|privacy` and `--image-scope all|global|localized`.
- **Privacy practices tab**: when an item has `privacy` in the config, `pull`, `diff` and `push` also handle the dashboard's "Privacy" tab (Privacy practices): the single purpose description, the justification of each permission, and the data usage disclosure. Narrow them with `--only privacy`. Saved as a draft only; never submitted for review.
- **submit**: submits one item for review. It doesn't change the "publish automatically after review" setting.
- **login**: opens Chrome with the dedicated profile so you can sign in to Google yourself.
- **update**: downloads and installs a newer version, if there is one. This only works when you installed Edictora with Setup, and it also runs automatically when you sign in to Windows.

## Display language

Edictora's messages, diff output and help text are available in nine languages: Japanese, English, Simplified Chinese, Traditional Chinese, Korean, Spanish, French, German and Brazilian Portuguese.

- By default, Edictora decides the language in this order: the `EDICTORA_LANGUAGE` environment variable, then `LC_ALL`, `LC_MESSAGES` and `LANG` (empty, `C` and `POSIX` are skipped), then the Windows display language. If none of them gives a supported language, it uses English.
- To choose a language explicitly, set `EDICTORA_LANGUAGE` to one of `ja`, `en`, `zh-CN`, `zh-TW`, `ko`, `es`, `fr`, `de` or `pt-BR`. The older name `EDICTORA_LANG` is still read, but `EDICTORA_LANGUAGE` takes priority.
- `edictora mcp` always answers in English, whatever the display language is, because its tool descriptions and responses are read by AI agents.
- This is separate from the language of the Developer Dashboard (Japanese or English) and from the `locales` of your store listings.
- Log files are meant for troubleshooting by the author, so their fixed messages stay in Japanese. Error messages included in the log follow the display language.

## Installing and updating

Run `Edictora-win-Setup.exe` (a Velopack installer). It installs to `%LOCALAPPDATA%\Edictora` without administrator rights.

- After installing, `edictora` works in any newly opened Command Prompt or PowerShell window, because Setup adds `%LOCALAPPDATA%\Edictora\current` to your user `Path`.
- When you sign in to Windows, `edictora-updater.exe` runs without a window. If `https://edictora.kagayoi.com` has a newer version, it downloads it, checks its size and hash, and then installs it.
  - It's registered as `Edictora Updater` under `Run` in HKCU, so you can turn it off under "Startup apps" in Task Manager.
  - The result is written to the log.
- To update by hand, run `edictora update`. It reports to standard error and exits with code 0 when you're already up to date or the update has started, and with code 1 if it fails. Update.exe replaces the files after `edictora` exits.
- The Portable version (`Edictora-win-Portable.zip`) doesn't update automatically at sign-in and doesn't add itself to `Path`. To update it, run `current\edictora.exe update` in the folder you extracted it to. Executables built from source don't update themselves.
- Updates are never applied while `login`, `pull`, `diff`, `push` or `submit` is running. Edictora finishes the download and applies the update later.
  - If you run one of these commands while an update is being applied, it exits with code 1 without opening Chrome. Wait a moment and run it again.
  - After the update process exits, Update.exe needs a few more seconds to replace the files, and Edictora can't protect that gap. Right after an update, wait a moment before running a command.
- Uninstalling removes `%LOCALAPPDATA%\Edictora` and the `Run` and `Path` entries. Your data (`%LOCALAPPDATA%\Edictora.Data`) and settings (`%APPDATA%\Edictora`) are kept, so delete them yourself if you no longer need them.

## Requirements

- Windows 10 or Windows 11 (x86_64)
- Google Chrome. Edictora finds it automatically, or you can set `chromePath` in the config file or pass `--chrome`.
- A CWS developer account
- Rust 1.100.0 if you build from source (pinned in `rust-toolchain.toml`). Bun is needed only when you change the in-page scripts.

## Usage

### 1. Write the config file

The default location is `%APPDATA%\Edictora\config.json`; you can change it with `--config`.
For each item, map each locale to its source files. **Fields you leave out are never read, compared or written.**

```jsonc
{
  // Optional. Defaults to %LOCALAPPDATA%\Edictora.Data\chrome-profile
  "profileDir": "%LOCALAPPDATA%\\Edictora.Data\\chrome-profile",
  "items": [
    {
      "itemId": "abcdefghijklmnopabcdefghijklmnop",   // 32-character CWS item ID
      "name": "Replace Translator",                     // Optional (for display)
      "baseDir": "D:\\Dev\\ReplaceTranslator",          // Optional (base for relative paths; defaults to the config file's folder)
      "locales": {
        "en": {
          "description": {
            "file": "webstore/store-listing.en.txt",
            "sectionStart": "^■ Description",           // Start after this heading line
            "sectionEnd": "^■ Category"                 // Stop before this heading line
          },
          "homepageUrl": "https://example.com/",
          "supportUrl": "https://example.com/support"
        }
      }
    }
  ]
}
```

- If you give only `description.file`, the whole file is used as the description.
- When a section is extracted, Edictora takes the lines after the start heading and before the end heading. It trims leading and trailing blank lines and trailing whitespace, and normalizes line endings to `\n`. If a heading isn't found, it stops with an error.
- A URL field without a key is ignored. `""` means the field should be empty.
- Unknown keys (such as typos) are errors. Comments and trailing commas are allowed.

#### Category (optional)

Add `category` to an item to read, compare and save its store category (the dashboard's "Category" field, shared by all languages). Write a lowercase key. Like every other field, **if you leave it out, the category is never touched**.

```jsonc
"category": "productivity"    // one of the keys below; leave it out to skip the category
```

- Keys: `productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`. An unknown key is a config error (exit code 2). "No category selected" can't be a source value.
- The category is shared by all languages, so it is read and written once per item (not once per language). It is included in `--only text`, and `--locale` does not narrow it. `push --yes` writes it only when it differs, saves a draft, reloads and checks again.
- An item may have `category` without `locales` (category only).
- Works only while the dashboard is shown in Japanese (the English labels are unverified).

#### Store images (optional)

Add `images` to an item to manage its store images. Like every other field, **keys you leave out are never touched**. Paths are relative to `baseDir`.

```jsonc
"images": {
  "icon": "icons/icon-128.png",                             // 128x128 (a transparent PNG is allowed)
  "smallTile": "webstore/images/tile-small.png",            // 440x280
  "marquee": "webstore/images/tile-marquee.png",            // 1400x560
  "globalScreenshots": { "dir": "webstore/images/en" },     // or { "files": ["a.png", "b.png"] }
  "localizedScreenshots": {                                 // zh_CN = zh-CN
    "ja": { "dir": "webstore/images/ja" },
    "zh_CN": { "dir": "webstore/images/zh_CN" }
  }
}
```

- `dir` uses the `screenshot-*.png` files directly inside the folder, in name order (1–5 files). `files` uses the files in the order you write them (1–5).
- Only PNG is supported: 24-bit without alpha, 1280x800 or 640x400 for screenshots, 440x280 for the small tile and 1400x560 for the marquee. Edictora checks this, and that no image repeats inside a set, before opening Chrome (exit code 2). The store icon (`icon`) is the exception: it must be 128x128 and may be transparent (8-bit RGB or RGBA). It is compared including alpha, ignoring the color of fully transparent pixels. It is shared by all languages, so `--image-scope global` includes it.
- An item may have `images` without `locales` (images only).
- **Images are compared by their decoded pixels, not by file bytes.** The dashboard re-encodes uploaded PNGs, so the stored file never has the same bytes as yours even though the pixels are identical. Edictora downloads each image from the dashboard (the `=s0` original, over HTTPS from `*.googleusercontent.com` only, with no cookies) and compares the pixel hashes in order. If an image can't be downloaded or decoded, the result is "cannot compare": never "match" and never "replace".
- `push --yes` replaces a whole slot (delete all, then upload in order) only when it differs, saves a draft, reloads and checks again. Slots are handled in this order: store icon, small tile, marquee, global screenshots, then each language's localized screenshots.
- **Deleting an image takes effect on the dashboard immediately** (it asks "this can't be undone", and the slot stayed empty after reloading without saving). If a push stops halfway, the draft can be left with images missing: run it again to finish. Edictora never touches an item that is pending review.
- An item with a single language has no localized screenshots, so use `--image-scope global`. If a locale in `localizedScreenshots` isn't on the dashboard, Edictora stops before changing anything: use `--image-scope global` or `--locale`.
- Writing images works only while the dashboard is shown in Japanese (the English labels are unverified).

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

#### Privacy practices tab (optional)

When you add a permission to an extension, the dashboard's **Privacy** tab (Privacy practices) needs a justification for it before the item can be submitted for review. Add `privacy` to an item to read, compare and save those fields from your source files as a draft. Like every other field, **keys you leave out are never touched**.

```jsonc
"privacy": {
  // Text fields use the same "file + sectionStart / sectionEnd" as `description`
  "singlePurpose": { "file": "webstore/store-listing.txt", "sectionStart": "^■ Single purpose$", "sectionEnd": "^■ Data usage$" },
  "permissions": {                                  // manifest permission name -> justification; host permissions use the special key "host"
    "activeTab": { "file": "webstore/store-listing.txt", "sectionStart": "^  activeTab:$", "sectionEnd": "^  scripting:$" },
    "host": { "file": "webstore/store-listing.txt", "sectionStart": "^  Host permissions:$", "sectionEnd": "^■ Privacy policy URL$" }
  },
  "dataUsage": {
    "collected": ["personallyIdentifiable", "authentication"],   // the kinds to check; the rest are unchecked
    "certify": true                                            // checks the three declarations; only true is allowed
  },
  "policyUrl": "https://example.com/privacy"            // absolute http(s) URL; "" means the field should be empty
}
```

- Text is cut out of your file like `description` (see above). Leading blank lines are removed and the common indentation is stripped, so justifications indented under `  activeTab:` work as they are. No text may be empty, and each must fit the 1,000-character limit; Edictora checks this before opening Chrome (exit code 2).
- `collected` takes any of `personallyIdentifiable`, `health`, `financial`, `authentication`, `personalCommunications`, `location`, `webHistory`, `userActivity` and `websiteContent` (only the ones you list are checked; the rest are unchecked). `certify` must be `true` (it checks the three declarations such as "I do not sell…"); `false` or leaving it out is a config error.
- **The set of permissions is checked too.** If you write `permissions`, a permission that is in the config but missing on the dashboard (or the other way around) is shown in the result, and `push` stops before writing anything: add it to the config or fix the manifest.
- `policyUrl` is the privacy policy URL field: an absolute http(s) URL (up to 2,048 characters), or `""` to mean the field should be empty. A key you leave out is never touched. It is saved together with the other fields of this tab.
- `remoteCode` (`{ "uses": false }`, or `{ "uses": true, "justification": {...} }`) is accepted in the config, but the dashboard section for it could not be checked against a real screen yet, so writing it makes `diff` report an error and `push` stop. Leave it out for now.
- Works only while the dashboard is shown in Japanese (the English labels are unverified). Saved as a draft only; Edictora never submits for review.
- An item may have `privacy` without `locales` or `images` (Privacy practices tab only).

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```

### 2. Sign in

```powershell
edictora login
```

Chrome opens with the dedicated profile. Sign in to Google there and open the Developer Dashboard.
- Once Edictora sees the dashboard, it keeps the sign-in in the profile and exits.
- It waits up to 10 minutes by default. Use `--wait <minutes>` to set 1–120 minutes.
- Edictora never handles your password or verification codes; you enter them yourself.

### 3. Compare and update

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

| Exit code | Meaning |
|---|---|
| 0 | Success (no differences) |
| 1 | Runtime error (a dashboard operation failed, verification mismatch, etc.) |
| 2 | Invalid arguments or config (Chrome isn't opened) |
| 3 | Differences found (`diff`, or `push` without `--yes`) |
| 4 | Write aborted (pending review, or the state couldn't be determined) |
| 5 | Sign-in required (the automatic sign-in wait didn't finish, or the sign-in expired in the middle of the work; running it again opens a sign-in window if one is needed) |
| 130 | Interrupted (Ctrl+C) |

Progress goes to standard error and results go to standard output.

`pull`, `diff`, `push` and `submit` run Chrome without a window (headless), so Chrome never jumps in front of your work.
To watch what happens, add `--show-browser`. `login` always shows a window, because you operate it by hand. If your Google sign-in has expired, `pull`, `diff`, `push` and `submit` open a visible Chrome window by themselves, wait for you to sign in (up to 10 minutes), close it and carry on. They stop with exit code 5 if that sign-in doesn't finish (timeout, window closed, or a windowless Chrome on the same profile is still running and blocks the window), or if the sign-in expires in the middle of the work (no window opens then; run it again).

> [!WARNING]
> `--show-browser` is for people watching the screen. Don't use it when running Edictora from an AI agent or a script: a Chrome window would pop up on the user's screen, and the AI can't see it anyway.
> To find the cause of a failure, use the error message and the text snapshot of the page saved on failure.
> - The error message includes the covering element, the element's bounds and the viewport size.
> - Page snapshots (text files listing the role, name, value, state and rectangle of each visible element) are saved to `%LOCALAPPDATA%\Edictora.Data\diagnostics`, and the error message shows the path. A snapshot is also saved when `pull` or `diff` fails to read the dashboard. Edictora takes no screenshots.

### 4. Using it from AI agents (MCP)

`edictora mcp` is an MCP server that runs over standard input and output. Once it's registered with Claude Code, Codex or another client, the AI can find and use Edictora when it edits your store listings.

**Setup registers Edictora with Claude Code and Codex automatically**, if their official CLIs (`claude` and `codex`) are on `PATH`. Uninstalling removes only the registrations that point to Edictora.
- If a CLI was missing at install time, or registration didn't finish in time, Edictora retries at your next sign-in. The same applies if you install a CLI later.
- If you remove a registration yourself after it was added, Edictora doesn't add it again.

For the Portable version, or for clients other than those two (such as Claude Desktop or a gateway), register it once using one of the methods below.
Give the installed exe as an absolute path:
- Setup version: `%LOCALAPPDATA%\Edictora\current\edictora.exe`. This path stays the same when automatic updates replace the files.
- Portable version: `current\edictora.exe` in the folder you extracted it to.

`edictora` on `PATH` also works, but clients started before the installation don't see the new `PATH`, so an absolute path is safer.

**Claude Code** (for all projects)

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

Or add it to `%USERPROFILE%\.codex\config.toml` (replace `<username>`):

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop and other clients configured with a JSON file** (replace `<username>`):

```json
{
  "mcpServers": {
    "edictora": {
      "command": "C:\\Users\\<username>\\AppData\\Local\\Edictora\\current\\edictora.exe",
      "args": ["mcp"]
    }
  }
}
```

If you use an MCP gateway (such as Toolport), register the same command (stdio, argument `mcp`) on the gateway.

**Toolport and automatic registration (Setup version, Windows)**
- For Claude Code and Codex separately, Setup skips `claude mcp add` / `codex mcp add` when both of these hold: (1) that client's settings contain the Toolport gateway (`mcpServers` in `~/.claude.json`, or `~/.codex/config.toml`), and (2) Toolport's registry (`%APPDATA%\Toolport\registry.json`; the location can be changed with `TOOLPORT_REGISTRY` / `TOOLPORT_DATA_DIR`) has an enabled server that starts `edictora.exe` with the `mcp` argument, in a profile that is in use. If this can't be determined, Edictora registers directly as before. Edictora makes this check on Windows, macOS and Linux (default registry location: Windows `%APPDATA%\Toolport\registry.json`, macOS `~/Library/Application Support/Toolport/registry.json`, Linux `$XDG_CONFIG_HOME/Toolport/registry.json` when it is an absolute path, otherwise `~/.config/Toolport/registry.json`); on any other OS it doesn't check and registers directly.
- A registration that already exists is never removed. If you later remove the Toolport gateway from that client, or the executable Toolport points to no longer exists, Edictora adds the direct registration at the next update or the next sign-in (the update executable that runs at sign-in). If you disable or delete Edictora in Toolport, Edictora doesn't add it. A client that Setup has registered directly once is not re-added automatically even if you remove the registration yourself; only a client that Setup skipped is added back.
- If you add Toolport later and want to remove only the direct registration, run `claude mcp remove edictora --scope user` or `codex mcp remove edictora`.
- When Setup starts over (first install, a new install location, or no saved registration state) while Toolport serves Edictora, it doesn't call the CLI, so a direct registration that points to the old executable is left as it is. Remove it with the commands above if you no longer need it.
- Toolport's tool quarantine (tools are quarantined when an update changes them) isn't checked.
- You can choose the behavior with `mcpRegistration` in `%APPDATA%\Edictora\config.json`: `auto` (default; the behavior above), `direct` (always register directly), or `off` (never register or repair automatically; uninstalling still removes the registrations that point to Edictora). The installer reads only the default config path, not a config given with `--config`. Other commands report any value other than these three as a config error. The installer treats an invalid value, or a config file that isn't valid JSON, as `auto` and logs it.

| Tool | What it does |
|---|---|
| `edictora_list_items` | Shows what the config file covers (items, locales, source files). Doesn't open a browser. |
| `edictora_diff` | Shows differences from the source files (read-only) |
| `edictora_pull` | Shows current values (read-only) |
| `edictora_push` | Rewrites differing fields and saves them as a draft. Without `confirm: true`, it shows the differences and stops (a dry run). |
| `edictora_submit` | Submits one item for review. `item` and `confirm: true` are required. |

- The tools run the same code as the CLI, with the same safeguards and exit codes. Each result starts with the exit code and its meaning. Differences found (3) isn't treated as an error.
- Clients can't choose to show the browser (`--show-browser`), run `login`, or set the profile or chrome.exe. Chrome runs without a window, except for the sign-in window described next.
- If your sign-in has expired, Edictora opens a visible Chrome window on your screen, waits for you to sign in (up to 10 minutes), then continues the tool call. If that doesn't finish, the tool asks you to run `edictora login` in a terminal.
- Calls run one at a time, in order. You can cancel a running call with `notifications/cancelled`.
- A `diff` across all items can take a few minutes, so give your client a generous timeout.

## Safeguards

- By default Edictora only shows differences and changes nothing. It writes only with `push --yes`, and submits only with `submit --item <id> --yes`.
- Saving a draft and submitting for review are separate operations. Submission is one item at a time; there's no bulk submit.
- Before saving, Edictora checks that each rewritten field exactly matches the source file. After saving, it reloads the page and checks again. If anything doesn't match, it doesn't save, or it stops with an error.
- For each locale, Edictora switches the language, rewrites the fields, verifies them, saves, then reloads and verifies again. If one locale fails, the rest of that item is skipped.
  - There's no rollback across locales: locales saved before the failure stay saved as drafts.
  - Nothing goes public until you submit the item for review.
- Edictora doesn't write when the item's status can't be confirmed as safe to edit, such as "Pending review". It never cancels a review.
- Fields and buttons are found by label, role and text, never by CSS class names. If a search doesn't match exactly one element, Edictora stops.
- Chrome is started without automation flags and shut down cleanly. **The profile is never deleted.**
- Page snapshots (text) taken on failure are saved to `%LOCALAPPDATA%\Edictora.Data\diagnostics`, never to your repository. They can contain the values of fields on the dashboard page; the publisher ID and your Google account name and email address are masked.
- Image updates replace only the slots that differ. They never reorder images, and they can never submit for review (the image code has no way to call it).
- Privacy practices updates change only the fields that differ, never submit for review (the code has no way to call it), and stop without writing anything if the permissions on the dashboard don't match the config.

## Where files are stored

| Content | Location |
|---|---|
| Config file (source file mapping) | `%APPDATA%\Edictora\config.json` |
| Chrome profile (includes your sign-in) | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| Logs (including update results) | `%LOCALAPPDATA%\Edictora.Data\logs` |
| Page snapshots (text) on failure | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| The app itself (when installed with Setup) | `%LOCALAPPDATA%\Edictora` (removed entirely on uninstall) |

Data is kept in `Edictora.Data`, separate from the app, so that uninstalling doesn't also delete your signed-in Chrome profile. Data in older locations (such as `%LOCALAPPDATA%\Edictora\chrome-profile`) isn't migrated automatically.

The Chrome profile contains your sign-in cookies. Don't share it, and delete it yourself when you no longer need it.
The publisher ID (part of the dashboard URL) is masked in the logs.

### If the config seems to be missing (Claude desktop app)

The Claude desktop app is a packaged (MSIX) app. When a program started from inside it creates a **new** folder under `%APPDATA%` or `%LOCALAPPDATA%`, Windows puts it in the app's private storage (`%LOCALAPPDATA%\Packages\<package>\LocalCache\...`) instead of the real location, and a normal terminal can't see it. Edictora warns on stderr when it is reading the config or the Chrome profile from such a place, and when the config is missing it lists any copy found there. To fix it, move the file (or folder) to the real location shown in the message, for example `%APPDATA%\Edictora\config.json`. Edictora never moves or reads those copies on its own.

## Building

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

**Regular expressions in `sectionStart` / `sectionEnd`**: Edictora uses Rust's `regex` crate, so lookahead, lookbehind (`(?=…)`, `(?!…)`, `(?<=…)`, `(?<!…)`) and backreferences (`\1`) aren't supported; they cause an error when the config is loaded. Ordinary patterns such as `^■ Description` work as written.

For testing, you can override:
- The local data root with `EDICTORA_LOCAL_ROOT` (default `%LOCALAPPDATA%\Edictora.Data`)
- The roaming root with `EDICTORA_ROAMING_ROOT` (default `%APPDATA%\Edictora`)
- The update source with `EDICTORA_UPDATE_URL` (default `https://edictora.kagayoi.com`; plain `http://` is allowed only for `127.0.0.1`)

The Velopack package contains both `edictora.exe` (the main exe) and `edictora-updater.exe`.
- Updates don't use the velopack crate: Edictora fetches the feed, downloads and verifies packages itself with WinHTTP, the HTTP API built into Windows.
- Installing, replacing files and uninstalling are left to Velopack's Setup.exe and Update.exe.

## Distribution

Releases are served from Cloudflare R2 at `https://edictora.kagayoi.com` (Velopack channel `win`). See [docs/release.md](docs/release.md) for the release process.

## License

See [LICENSE](LICENSE). For privacy, see [PRIVACY_POLICY.md](PRIVACY_POLICY.md).
