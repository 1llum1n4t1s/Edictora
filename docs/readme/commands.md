# Signing in and commands

## Commands

- **pull**: reads the current values from the dashboard. Read-only.
- **diff**: shows the differences from your source files. Read-only, and the default command.
- **push**: rewrites only the differing fields, **saves them as a draft**, then reloads and checks the saved values.
- **submit**: submits one item for review. It doesn't change the "publish automatically after review" setting.
- **login**: opens Chrome with the dedicated profile so you can sign in to Google yourself.
- **update**: installs a newer version if there is one. The Portable version can update by hand with `current\edictora.exe update`; the automatic update at sign-in runs only for Setup installs.

## Sign in

```powershell
edictora login
```

Chrome opens with the dedicated profile. Sign in to Google there and open the Developer Dashboard. Once Edictora sees the dashboard, it keeps the sign-in in the profile and exits. It waits up to 10 minutes by default; use `--wait <minutes>` for 1-120. Edictora never handles your password or verification codes.

## Compare and update

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

## Exit codes and browser behavior

Exit codes (0 success, 3 differences found, 5 sign-in required, and so on), the windowless Chrome behavior, expired sign-ins and failure diagnostics are in [Exit codes and browser behavior](exit-codes.md).

> [!WARNING]
> `--show-browser` is for people watching the screen. Don't use it from an AI agent or a script: a window would pop up on the user's screen. Use the error message and saved page snapshot instead.
