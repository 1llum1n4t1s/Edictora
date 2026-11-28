# Registering the MCP server by hand

For the Portable version, or other clients (such as Claude Desktop or a gateway), register `edictora mcp` once yourself, with the exe as an absolute path:
- Setup version: `%LOCALAPPDATA%\Edictora\current\edictora.exe` (stable across automatic updates).
- Portable version: `current\edictora.exe` in the folder you extracted it to.

**Claude Code** (all projects)

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

Or add to `%USERPROFILE%\.codex\config.toml` (replace `<username>`):

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop and other JSON-configured clients** (replace `<username>`):

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

With an MCP gateway (such as Toolport), register the same command (stdio, argument `mcp`) on the gateway.
