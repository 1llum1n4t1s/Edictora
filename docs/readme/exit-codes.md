# Exit codes and browser behavior

## Exit codes

| Exit code | Meaning |
|---|---|
| 0 | Success (no differences) |
| 1 | Runtime error (a dashboard operation failed, verification mismatch, etc.) |
| 2 | Invalid arguments or config (Chrome isn't opened) |
| 3 | Differences found (`diff`, or `push` without `--yes`) |
| 4 | Write aborted (pending review, or the state couldn't be determined) |
| 5 | Sign-in required (the sign-in wait didn't finish, or the sign-in expired mid-work; run it again) |
| 130 | Interrupted (Ctrl+C) |

Progress goes to standard error and results to standard output.

## Windowless Chrome and expired sign-ins

`pull`, `diff`, `push` and `submit` run Chrome without a window (headless). Add `--show-browser` to watch. `login` always shows a window.

If your Google sign-in has expired, the four commands open a visible Chrome window by themselves, wait for you to sign in (up to 10 minutes), close it and carry on. They stop with exit code 5 if that sign-in doesn't finish (timeout, window closed, or a windowless Chrome on the same profile still running), or if it expires mid-work (no window opens then; run it again).

## Diagnosing failures

> [!WARNING]
> `--show-browser` is for people watching the screen. Don't use it from an AI agent or a script: a window would pop up on the user's screen.
> To find the cause of a failure, use the error message and the saved page snapshot.
> - The error message includes the covering element, its bounds and the viewport size.
> - Page snapshots (text files listing the role, name, value, state and rectangle of each visible element) are saved to `%LOCALAPPDATA%\Edictora.Data\diagnostics`, and the error message shows the path. One is also saved when `pull` or `diff` fails to read the dashboard. Edictora takes no screenshots.
