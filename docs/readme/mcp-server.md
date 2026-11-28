# MCP server (AI agents)

`edictora mcp` is an MCP server that runs over standard input and output. Once it's registered with Claude Code, Codex or another client, the AI can use Edictora when it edits your store listings.

**Setup registers Edictora with Claude Code and Codex automatically**, if their official CLIs (`claude` and `codex`) are on `PATH`. It looks only for `claude.exe` and `codex.exe`, not `.cmd` wrappers (as installed by npm, for example); if you only have a `.cmd`, register by hand: see [Registering by hand](mcp-registration.md). Uninstalling removes only registrations that point to Edictora.
- If a CLI was missing at install time, or registration didn't finish in time, Edictora retries at your next sign-in (also if you install a CLI later).
- If you remove a registration yourself after it was added, Edictora doesn't add it again.
- If Toolport already serves Edictora, direct registration is skipped: see [Toolport and automatic registration](mcp-toolport.md).

For the Portable version or other clients (Claude Desktop, a gateway), see [Registering by hand](mcp-registration.md).

## Tools

| Tool | What it does |
|---|---|
| `edictora_list_items` | Shows what the config covers (items, locales, source files). Doesn't open a browser. |
| `edictora_diff` | Shows differences from the source files (read-only) |
| `edictora_pull` | Shows current values (read-only) |
| `edictora_push` | Rewrites differing fields and saves a draft. Without `confirm: true`, it shows the differences and stops (dry run). |
| `edictora_submit` | Submits one item for review. `item` and `confirm: true` are required. |

- The tools run the same code as the CLI, with the same safeguards and exit codes. Each result starts with the exit code and its meaning. Differences found (3) isn't an error.
- Clients can't show the browser, run `login`, or set the profile or chrome.exe. Chrome runs without a window, except for the sign-in window below.
- If your sign-in has expired, Edictora opens a visible Chrome window, waits for you to sign in (up to 10 minutes), then continues. If that doesn't finish, the tool asks you to run `edictora login` in a terminal.
- Calls run one at a time. Cancel a running call with `notifications/cancelled`.
- A `diff` across all items can take a few minutes, so give your client a generous timeout.
