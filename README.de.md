<p align="center">
  <img src="icon/app_icon.png" alt="Edictora-Symbol" width="128">
</p>

# Edictora

[English](README.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | Deutsch | [Português (Brasil)](README.pt-BR.md)

**Ein Windows-Tool, das Ihre Chrome-Web-Store-Einträge (Beschreibung und URLs) anhand von Quelldateien aus dem Repository der jeweiligen Erweiterung gesammelt ausliest, vergleicht und aktualisiert.**

Die öffentliche Chrome-Web-Store-API (CWS, v2) bietet keine Möglichkeit, Store-Einträge zu lesen oder zu schreiben, und Chrome erlaubt Erweiterungen nicht, CWS-Seiten zu bedienen.
Edictora steuert stattdessen über das Chrome DevTools Protocol (CDP) eine Chrome-Instanz mit eigenem Profil und bearbeitet das Developer Dashboard mit denselben Eingaben, die auch ein Mensch verwenden würde.
Es läuft als einzelnes Befehlszeilenwerkzeug (`edictora.exe`), ist in Rust geschrieben und benötigt keine Laufzeitumgebung wie .NET oder Node.js.

> [!IMPORTANT]
> Edictora hängt vom Aufbau des Developer Dashboards ab. Es wurde mit dem in **Japanisch** angezeigten Dashboard geprüft.
> Englische Dashboards werden ebenfalls erkannt, die englischen Beschriftungen auf dem Bildschirm wurden aber noch nicht an einem echten Bildschirm überprüft.
> Findet Edictora ein Feld oder eine Schaltfläche nicht (zum Beispiel nach einem Redesign des Dashboards), bricht es ab, ohne etwas zu ändern.

## Funktionen

- **pull**: liest die aktuellen Werte aus dem Dashboard (Beschreibung, offizielle URL, Homepage-URL und Support-URL). Nur lesend.
- **diff**: zeigt die Unterschiede zu Ihren Quelldateien. Nur lesend und der Standardbefehl.
- **push**: schreibt nur die abweichenden Felder um, **speichert sie als Entwurf**, lädt die Seite neu und prüft die gespeicherten Werte anhand Ihrer Quelldateien.
- **submit**: reicht ein Element zur Prüfung ein. Die Einstellung „Nach der Prüfung automatisch veröffentlichen“ wird dabei nicht geändert.
- **login**: öffnet Chrome mit dem eigenen Profil, damit Sie sich selbst bei Google anmelden können.
- **update**: lädt eine neuere Version herunter und installiert sie, falls es eine gibt. Das funktioniert nur, wenn Sie Edictora mit Setup installiert haben, und läuft außerdem automatisch bei Ihrer Windows-Anmeldung.

## Anzeigesprache

Die Meldungen, die Diff-Ausgabe und der Hilfetext von Edictora sind in neun Sprachen verfügbar: Japanisch, Englisch, Vereinfachtes Chinesisch, Traditionelles Chinesisch, Koreanisch, Spanisch, Französisch, Deutsch und Brasilianisches Portugiesisch.

- Standardmäßig bestimmt Edictora die Sprache in dieser Reihenfolge: die Umgebungsvariable `EDICTORA_LANGUAGE`, dann `LC_ALL`, `LC_MESSAGES` und `LANG` (leer, `C` und `POSIX` werden übersprungen), dann die Windows-Anzeigesprache. Ergibt keine davon eine unterstützte Sprache, wird Englisch verwendet.
- Um eine Sprache ausdrücklich festzulegen, setzen Sie `EDICTORA_LANGUAGE` auf `ja`, `en`, `zh-CN`, `zh-TW`, `ko`, `es`, `fr`, `de` oder `pt-BR`. Der ältere Name `EDICTORA_LANG` wird weiterhin gelesen, aber `EDICTORA_LANGUAGE` hat Vorrang.
- `edictora mcp` antwortet unabhängig von der Anzeigesprache immer auf Englisch, weil seine Tool-Beschreibungen und Antworten von KI-Agenten gelesen werden.
- Das ist unabhängig von der Sprache des Developer Dashboards (Japanisch oder Englisch) und von den `locales` Ihrer Store-Einträge.
- Logdateien dienen der Fehlersuche durch den Autor, daher bleiben ihre festen Meldungen auf Japanisch. Fehlermeldungen innerhalb des Logs folgen der Anzeigesprache.

## Installieren und aktualisieren

Führen Sie `Edictora-win-Setup.exe` (ein Velopack-Installer) aus. Er installiert nach `%LOCALAPPDATA%\Edictora`, ohne Administratorrechte zu benötigen.

- Nach der Installation funktioniert `edictora` in jeder neu geöffneten Eingabeaufforderung oder PowerShell, weil Setup `%LOCALAPPDATA%\Edictora\current` zum `Path` Ihres Benutzerkontos hinzufügt.
- Bei der Windows-Anmeldung läuft `edictora-updater.exe` ohne Fenster. Gibt es unter `https://edictora.kagayoi.com` eine neuere Version, lädt es sie herunter, prüft Größe und Hash und installiert sie dann.
  - Es ist als `Edictora Updater` unter `Run` in HKCU registriert, sodass Sie es im Task-Manager unter „Autostart-Apps“ deaktivieren können.
  - Das Ergebnis wird in das Log geschrieben.
- Zum manuellen Aktualisieren führen Sie `edictora update` aus. Es berichtet über die Standardfehlerausgabe und endet mit Exitcode 0, wenn Sie bereits auf dem neuesten Stand sind oder die Aktualisierung gestartet wurde, und mit Exitcode 1, wenn sie fehlschlägt. Update.exe ersetzt die Dateien, nachdem `edictora` beendet wurde.
- Die Portable-Version (`Edictora-win-Portable.zip`) aktualisiert sich bei der Anmeldung nicht automatisch und trägt sich nicht in den `Path` ein. Zum Aktualisieren führen Sie `current\edictora.exe update` in dem Ordner aus, in den Sie sie entpackt haben. Aus dem Quellcode gebaute Programmdateien aktualisieren sich nicht selbst.
- Aktualisierungen werden nie angewendet, während `login`, `pull`, `diff`, `push` oder `submit` läuft. Edictora schließt den Download ab und wendet die Aktualisierung später an.
  - Führen Sie einen dieser Befehle aus, während eine Aktualisierung angewendet wird, endet er mit Exitcode 1, ohne Chrome zu öffnen. Warten Sie einen Moment und führen Sie ihn erneut aus.
  - Nachdem der Aktualisierungsprozess beendet ist, braucht Update.exe noch einige Sekunden, um die Dateien zu ersetzen, und Edictora kann diese Lücke nicht absichern. Warten Sie direkt nach einer Aktualisierung einen Moment, bevor Sie einen Befehl ausführen.
- Beim Deinstallieren werden `%LOCALAPPDATA%\Edictora` sowie die Einträge unter `Run` und `Path` entfernt. Ihre Daten (`%LOCALAPPDATA%\Edictora.Data`) und Einstellungen (`%APPDATA%\Edictora`) bleiben erhalten; löschen Sie sie selbst, wenn Sie sie nicht mehr brauchen.

## Voraussetzungen

- Windows 10 oder Windows 11 (x86_64)
- Google Chrome. Edictora findet es automatisch, oder Sie setzen `chromePath` in der Konfigurationsdatei oder übergeben `--chrome`.
- Ein CWS-Entwicklerkonto
- Rust 1.100.0, wenn Sie aus dem Quellcode bauen (festgelegt in `rust-toolchain.toml`). Bun wird nur benötigt, wenn Sie die In-Page-Skripte ändern.

## Verwendung

### 1. Konfigurationsdatei schreiben

Der Standardort ist `%APPDATA%\Edictora\config.json`; mit `--config` können Sie ihn ändern.
Ordnen Sie für jedes Element jeder Locale ihre Quelldateien zu. **Felder, die Sie weglassen, werden nie gelesen, verglichen oder geschrieben.**

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

- Geben Sie nur `description.file` an, wird die gesamte Datei als Beschreibung verwendet.
- Beim Extrahieren eines Abschnitts übernimmt Edictora die Zeilen nach der Startüberschrift und vor der Endüberschrift. Es entfernt führende und abschließende Leerzeilen sowie abschließende Leerzeichen und vereinheitlicht die Zeilenenden auf `\n`. Wird eine Überschrift nicht gefunden, bricht es mit einem Fehler ab.
- Ein URL-Feld ohne Schlüssel wird ignoriert. `""` bedeutet, dass das Feld leer sein soll.
- Unbekannte Schlüssel (zum Beispiel Tippfehler) sind Fehler. Kommentare und abschließende Kommas sind erlaubt.

### 2. Anmelden

```powershell
edictora login
```

Chrome öffnet sich mit dem eigenen Profil. Melden Sie sich dort bei Google an und öffnen Sie das Developer Dashboard.
- Sobald Edictora das Dashboard erkennt, behält es die Anmeldung im Profil und beendet sich.
- Standardmäßig wartet es bis zu 10 Minuten. Mit `--wait <minutes>` stellen Sie 1 bis 120 Minuten ein.
- Edictora verarbeitet nie Ihr Passwort oder Bestätigungscodes; Sie geben sie selbst ein.

### 3. Vergleichen und aktualisieren

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

| Exitcode | Bedeutung |
|---|---|
| 0 | Erfolg (keine Unterschiede) |
| 1 | Laufzeitfehler (eine Dashboard-Operation ist fehlgeschlagen, Abweichung bei der Prüfung usw.) |
| 2 | Ungültige Argumente oder Konfiguration (Chrome wird nicht geöffnet) |
| 3 | Unterschiede gefunden (`diff` oder `push` ohne `--yes`) |
| 4 | Schreibvorgang abgebrochen (Prüfung ausstehend oder der Status konnte nicht ermittelt werden) |
| 5 | Anmeldung erforderlich |
| 130 | Unterbrochen (Strg+C) |

Fortschrittsmeldungen gehen an die Standardfehlerausgabe, Ergebnisse an die Standardausgabe.

`pull`, `diff`, `push` und `submit` führen Chrome ohne Fenster aus (headless), sodass sich Chrome nie vor Ihre Arbeit schiebt.
Um zu beobachten, was passiert, fügen Sie `--show-browser` hinzu. `login` zeigt immer ein Fenster, weil Sie es von Hand bedienen.

> [!WARNING]
> `--show-browser` ist für Personen gedacht, die den Bildschirm beobachten. Verwenden Sie es nicht, wenn Sie Edictora über einen KI-Agenten oder ein Skript ausführen: Auf dem Bildschirm des Benutzers würde ein Chrome-Fenster aufspringen, und die KI kann es ohnehin nicht sehen.
> Um die Ursache eines Fehlers zu finden, nutzen Sie die Fehlermeldung und den bei einem Fehler gespeicherten Screenshot.
> - Die Fehlermeldung enthält das verdeckende Element, die Abmessungen des Elements und die Größe des Viewports.
> - Screenshots werden unter `%LOCALAPPDATA%\Edictora.Data\diagnostics` gespeichert, und die Fehlermeldung nennt den Pfad. Ein Screenshot wird auch gespeichert, wenn `pull` oder `diff` das Dashboard nicht lesen kann.

### 4. Verwendung mit KI-Agenten (MCP)

`edictora mcp` ist ein MCP-Server, der über Standardeingabe und Standardausgabe läuft. Ist er bei Claude Code, Codex oder einem anderen Client registriert, kann die KI Edictora finden und verwenden, wenn sie Ihre Store-Einträge bearbeitet.

**Setup registriert Edictora automatisch bei Claude Code und Codex**, sofern deren offizielle CLIs (`claude` und `codex`) im `PATH` liegen. Beim Deinstallieren werden nur die Registrierungen entfernt, die auf Edictora zeigen.
- Fehlte eine CLI bei der Installation oder wurde die Registrierung nicht rechtzeitig abgeschlossen, versucht Edictora es bei Ihrer nächsten Anmeldung erneut. Dasselbe gilt, wenn Sie eine CLI später installieren.
- Wenn Sie eine Registrierung nach dem Hinzufügen selbst entfernen, fügt Edictora sie nicht erneut hinzu.

Für die Portable-Version oder für andere Clients als diese beiden (etwa Claude Desktop oder ein Gateway) registrieren Sie es einmalig mit einer der folgenden Methoden.
Geben Sie die installierte Exe als absoluten Pfad an:
- Setup-Version: `%LOCALAPPDATA%\Edictora\current\edictora.exe`. Dieser Pfad bleibt gleich, wenn automatische Aktualisierungen die Dateien ersetzen.
- Portable-Version: `current\edictora.exe` in dem Ordner, in den Sie sie entpackt haben.

`edictora` im `PATH` funktioniert ebenfalls, aber vor der Installation gestartete Clients sehen den neuen `PATH` nicht, daher ist ein absoluter Pfad sicherer.

**Claude Code** (für alle Projekte)

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

Oder fügen Sie es in `%USERPROFILE%\.codex\config.toml` ein (ersetzen Sie `<username>`):

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop und andere Clients, die mit einer JSON-Datei konfiguriert werden** (ersetzen Sie `<username>`):

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

Wenn Sie ein MCP-Gateway verwenden (etwa Toolport), registrieren Sie denselben Befehl (stdio, Argument `mcp`) im Gateway.

| Tool | Funktion |
|---|---|
| `edictora_list_items` | Zeigt, was die Konfigurationsdatei abdeckt (Elemente, Locales, Quelldateien). Öffnet keinen Browser. |
| `edictora_diff` | Zeigt die Unterschiede zu den Quelldateien (nur lesend) |
| `edictora_pull` | Zeigt die aktuellen Werte (nur lesend) |
| `edictora_push` | Schreibt abweichende Felder um und speichert sie als Entwurf. Ohne `confirm: true` zeigt es die Unterschiede und hält an (ein Probelauf). |
| `edictora_submit` | Reicht ein Element zur Prüfung ein. `item` und `confirm: true` sind erforderlich. |

- Die Tools führen denselben Code wie die CLI aus, mit denselben Schutzmechanismen und Exitcodes. Jedes Ergebnis beginnt mit dem Exitcode und seiner Bedeutung. „Unterschiede gefunden“ (3) gilt nicht als Fehler.
- Clients können nicht festlegen, den Browser anzuzeigen (`--show-browser`), `login` auszuführen oder das Profil bzw. chrome.exe zu setzen. Chrome läuft immer ohne Fenster.
- Ist Ihre Anmeldung abgelaufen, fordert das Tool Sie auf, `edictora login` in einem Terminal auszuführen. Sie melden sich selbst an.
- Aufrufe laufen nacheinander in der Reihenfolge ihres Eingangs. Einen laufenden Aufruf können Sie mit `notifications/cancelled` abbrechen.
- Ein `diff` über alle Elemente kann einige Minuten dauern, daher sollten Sie Ihrem Client ein großzügiges Timeout geben.

## Schutzmechanismen

- Standardmäßig zeigt Edictora nur Unterschiede an und ändert nichts. Es schreibt nur mit `push --yes` und reicht nur mit `submit --item <id> --yes` ein.
- Das Speichern eines Entwurfs und das Einreichen zur Prüfung sind getrennte Vorgänge. Eingereicht wird jeweils ein Element; eine Sammel-Einreichung gibt es nicht.
- Vor dem Speichern prüft Edictora, ob jedes umgeschriebene Feld genau der Quelldatei entspricht. Nach dem Speichern lädt es die Seite neu und prüft erneut. Stimmt etwas nicht überein, speichert es nicht oder bricht mit einem Fehler ab.
- Für jede Locale wechselt Edictora die Sprache, schreibt die Felder um, prüft sie, speichert, lädt neu und prüft erneut. Schlägt eine Locale fehl, wird der Rest dieses Elements übersprungen.
  - Es gibt kein Zurückrollen über Locales hinweg: Vor dem Fehler gespeicherte Locales bleiben als Entwürfe gespeichert.
  - Nichts wird öffentlich, bis Sie das Element zur Prüfung einreichen.
- Edictora schreibt nicht, wenn sich nicht bestätigen lässt, dass der Status des Elements eine Bearbeitung sicher zulässt, etwa bei „Prüfung ausstehend“. Eine Prüfung wird nie abgebrochen.
- Felder und Schaltflächen werden über Beschriftung, Rolle und Text gefunden, nie über CSS-Klassennamen. Trifft eine Suche nicht genau ein Element, bricht Edictora ab.
- Chrome wird ohne Automatisierungs-Flags gestartet und sauber beendet. **Das Profil wird nie gelöscht.**
- Bei einem Fehler aufgenommene Screenshots werden unter `%LOCALAPPDATA%\Edictora.Data\diagnostics` gespeichert, nie in Ihrem Repository.

## Speicherorte der Dateien

| Inhalt | Ort |
|---|---|
| Konfigurationsdatei (Zuordnung der Quelldateien) | `%APPDATA%\Edictora\config.json` |
| Chrome-Profil (enthält Ihre Anmeldung) | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| Logs (einschließlich Aktualisierungsergebnissen) | `%LOCALAPPDATA%\Edictora.Data\logs` |
| Screenshots bei Fehlern | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| Die App selbst (bei Installation mit Setup) | `%LOCALAPPDATA%\Edictora` (wird bei der Deinstallation vollständig entfernt) |

Die Daten liegen getrennt von der App in `Edictora.Data`, damit beim Deinstallieren nicht auch Ihr angemeldetes Chrome-Profil gelöscht wird. Daten an älteren Speicherorten (etwa `%LOCALAPPDATA%\Edictora\chrome-profile`) werden nicht automatisch migriert.

Das Chrome-Profil enthält Ihre Anmelde-Cookies. Geben Sie es nicht weiter und löschen Sie es selbst, wenn Sie es nicht mehr brauchen.
Die Publisher-ID (Teil der Dashboard-URL) wird in den Logs maskiert.

## Bauen

Das Hauptprogramm ist in Rust geschrieben (`crates/edictora/`).

```powershell
cargo build --release          # target\release\edictora.exe and edictora-updater.exe (Bun not needed)
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Die Skripte, die innerhalb der Dashboard-Seiten laufen, liegen in `page-scripts/` (Bun + TypeScript). Die erzeugte Datei `page-scripts/dist/edictora.js` ist in die Exe eingebettet, daher brauchen Sie Bun nur, wenn Sie diese Skripte ändern.

```powershell
cd page-scripts
bun install --frozen-lockfile   # install dependencies (TypeScript only)
bun run typecheck      # type-check
bun run build          # regenerate dist/edictora.js
bun run check          # check that dist/edictora.js is up to date
```

**Reguläre Ausdrücke in `sectionStart` / `sectionEnd`**: Edictora verwendet die Rust-Crate `regex`, daher werden Lookahead, Lookbehind (`(?=…)`, `(?!…)`, `(?<=…)`, `(?<!…)`) und Rückreferenzen (`\1`) nicht unterstützt; sie führen beim Laden der Konfiguration zu einem Fehler. Gewöhnliche Muster wie `^■ Description` funktionieren unverändert.

Zum Testen können Sie Folgendes überschreiben:
- Das lokale Datenstammverzeichnis mit `EDICTORA_LOCAL_ROOT` (Standard `%LOCALAPPDATA%\Edictora.Data`)
- Das Roaming-Stammverzeichnis mit `EDICTORA_ROAMING_ROOT` (Standard `%APPDATA%\Edictora`)
- Die Aktualisierungsquelle mit `EDICTORA_UPDATE_URL` (Standard `https://edictora.kagayoi.com`; reines `http://` ist nur für `127.0.0.1` erlaubt)

Das Velopack-Paket enthält sowohl `edictora.exe` (die Haupt-Exe) als auch `edictora-updater.exe`.
- Aktualisierungen verwenden nicht die velopack-Crate: Edictora ruft den Feed ab und lädt und prüft Pakete selbst mit WinHTTP, der in Windows integrierten HTTP-API.
- Installieren, Ersetzen von Dateien und Deinstallieren überlässt Edictora dem Setup.exe und Update.exe von Velopack.

## Verteilung

Releases werden von Cloudflare R2 unter `https://edictora.kagayoi.com` bereitgestellt (Velopack-Kanal `win`). Den Release-Prozess beschreibt [docs/release.md](docs/release.md).

## Lizenz

Siehe [LICENSE](LICENSE). Zum Datenschutz siehe [PRIVACY_POLICY.md](PRIVACY_POLICY.md).
