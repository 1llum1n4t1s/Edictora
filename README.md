<p align="center">
  <img src="icon/app_icon.png" alt="Edictora icon" width="128">
</p>

# Edictora

English | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**A Windows tool that reads, compares and updates your Chrome Web Store listings (description and URLs) in bulk, using source files kept in each extension's repository.**

The public Chrome Web Store (CWS) API (v2) has no way to read or write store listings, and Chrome doesn't let extensions operate on CWS pages.
Edictora instead drives Chrome with a dedicated profile over CDP and edits the Developer Dashboard as a person would.
It is a single Rust command-line tool (`edictora.exe`) with no runtime dependencies.

> [!IMPORTANT]
> Edictora depends on the layout of the Developer Dashboard. It was verified with the dashboard shown in **Japanese**.
> English dashboards are recognized but not yet verified on a real screen.
> If Edictora can't find a field or button (for example after a dashboard redesign), it stops without changing anything.

## Features

`pull` reads the dashboard's current values, `diff` (the default) shows differences from your source files, `push` rewrites only differing fields, saves a draft and verifies it, and `submit` sends one item for review. Store images and the Privacy practices tab are handled when configured. `login` opens the dedicated profile for signing in, and `update` installs a newer version.

## Documentation

- [Installing and updating](docs/readme/installation.md): Setup, Portable and updates.
- [Configuration](docs/readme/configuration.md): the config file, items, locales and category.
- [Store images](docs/readme/store-images.md): managing icons, tiles and screenshots.
- [Privacy practices tab](docs/readme/privacy-tab.md) settings.
- [Commands](docs/readme/commands.md): signing in, comparing and updating.
- [Exit codes](docs/readme/exit-codes.md)
- [Safeguards](docs/readme/safeguards.md)
- [Display language](docs/readme/display-language.md): nine message languages.
- [MCP server](docs/readme/mcp-server.md): using Edictora from AI agents.
- [MCP registration](docs/readme/mcp-registration.md)
- [Toolport](docs/readme/mcp-toolport.md)
- [Where files are stored](docs/readme/file-locations.md): config, profile and logs.
- [Building](docs/readme/building.md): building from source.

## Distribution

Releases are served from Cloudflare R2 at `https://edictora.kagayoi.com` (Velopack channel `win`). See [docs/release.md](docs/release.md) for the release process.

## License

See [LICENSE](LICENSE). For privacy, see [PRIVACY_POLICY.md](PRIVACY_POLICY.md).
