# Where files are stored

| Content | Location |
|---|---|
| Config file (source file mapping) | `%APPDATA%\Edictora\config.json` |
| Chrome profile (includes your sign-in) | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| Logs (including update results) | `%LOCALAPPDATA%\Edictora.Data\logs` |
| Page snapshots (text) on failure | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| The app itself (when installed with Setup) | `%LOCALAPPDATA%\Edictora` (removed entirely on uninstall) |

Data is kept in `Edictora.Data`, separate from the app, so uninstalling doesn't delete your signed-in Chrome profile. Data in older locations (such as `%LOCALAPPDATA%\Edictora\chrome-profile`) isn't migrated automatically.

The Chrome profile contains your sign-in cookies. Don't share it, and delete it yourself when you no longer need it. The publisher ID (part of the dashboard URL) is masked in the logs.

## If the config seems to be missing (Claude desktop app)

The Claude desktop app is a packaged (MSIX) app. When a program started from inside it creates a **new** folder under `%APPDATA%` or `%LOCALAPPDATA%`, Windows puts it in the app's private storage (`%LOCALAPPDATA%\Packages\<package>\LocalCache\...`), which a normal terminal can't see. Edictora warns on stderr when it reads the config or Chrome profile from such a place, and when the config is missing it lists any copy found there. To fix it, move the file (or folder) to the real location shown in the message, for example `%APPDATA%\Edictora\config.json`. Edictora never moves or reads those copies on its own.
