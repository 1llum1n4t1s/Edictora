<p align="center">
  <img src="icon/app_icon.png" alt="Edictora icon" width="128">
</p>

# Edictora

English | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**A Windows tool that reads, compares and updates your Chrome Web Store listings in bulk, using source files kept in each extension's repository.**

The Chrome Web Store API can't read or write store listings, so Edictora drives a dedicated-profile Chrome over CDP and edits the Developer Dashboard like a person would. It is a single Rust command-line tool (`edictora.exe`) with no runtime needed.

> [!IMPORTANT]
> Verified with the dashboard shown in **Japanese**; English labels are unverified. If Edictora can't find a field or button, it stops without changing anything.

Commands: `pull`, `diff` (default), `push` (saves a draft), `submit`, `login`, `update`, `mcp`.

## Documentation

- [Installing and updating](docs/readme/installation.md): Setup, Portable, auto-update, requirements.
- [Config file](docs/readme/configuration.md): map source files to listings, plus category.
- [Store images](docs/readme/store-images.md) and [Privacy practices tab](docs/readme/privacy-tab.md): optional config.
- [Signing in and commands](docs/readme/commands.md): usage, exit codes, diagnostics.
- [MCP server](docs/readme/mcp-server.md): use Edictora from AI agents.
- [Safeguards](docs/readme/safeguards.md): what it never does.
- [File locations](docs/readme/file-locations.md): config, profile, logs.
- [Display language](docs/readme/display-language.md): nine languages.
- [Building](docs/readme/building.md): build and test from source.

Releases: [docs/release.md](docs/release.md). See [LICENSE](LICENSE) and [PRIVACY_POLICY.md](PRIVACY_POLICY.md).
