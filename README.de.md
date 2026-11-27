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
- **Bilder**: Hat ein Eintrag in der Konfiguration `images`, behandeln `pull`, `diff` und `push` auch seine Store-Bilder (Shop-Symbol, Screenshots, kleine Werbekachel und Marquee). Mit `--only text|images|privacy` und `--image-scope all|global|localized` lässt sich das eingrenzen.
- **Registerkarte „Datenschutz“**: Hat ein Eintrag in der Konfiguration `privacy`, behandeln `pull`, `diff` und `push` auch die Registerkarte „Datenschutz“ des Dashboards (Privacy practices: Beschreibung des einzigen Zwecks, Begründung jeder Berechtigung und Angaben zur Datennutzung). Mit `--only privacy` lässt sich das eingrenzen. Es wird nur als Entwurf gespeichert und nie zur Prüfung eingereicht.
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

#### Kategorie (optional)

Fügen Sie einem Eintrag `category` hinzu, um die Store-Kategorie (das Feld „Kategorie“ im Dashboard, für alle Sprachen gemeinsam) zu lesen, zu vergleichen und zu speichern. Schreiben Sie einen Schlüssel in Kleinbuchstaben. Wie bei jedem anderen Feld wird **die Kategorie nie angerührt, wenn Sie sie weglassen**.

```jsonc
"category": "productivity"    // one of the keys below; leave it out to skip the category
```

- Schlüssel: `productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`. Ein unbekannter Schlüssel ist ein Konfigurationsfehler (Exitcode 2). „Keine Kategorie ausgewählt“ kann kein Quellwert sein.
- Die Kategorie gilt für alle Sprachen und wird daher nur einmal pro Eintrag gelesen und geschrieben (nicht einmal pro Sprache). Sie ist in `--only text` enthalten, und `--locale` schränkt sie nicht ein. `push --yes` schreibt sie nur bei einem Unterschied, speichert einen Entwurf, lädt neu und prüft erneut.
- Ein Eintrag kann `category` ohne `locales` haben (nur Kategorie).
- Funktioniert nur, solange das Dashboard auf Japanisch angezeigt wird (die englischen Bezeichnungen sind nicht verifiziert).

#### Store-Bilder (optional)

Fügen Sie einem Eintrag `images` hinzu, um seine Store-Bilder zu verwalten. Wie bei allen anderen Feldern **werden Schlüssel, die Sie weglassen, nie angetastet**. Pfade sind relativ zu `baseDir`.

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

- `dir` verwendet die `screenshot-*.png` direkt im Ordner in Namensreihenfolge (1–5 Dateien). `files` verwendet die Dateien in der angegebenen Reihenfolge (1–5).
- Unterstützt wird nur PNG: 24 Bit ohne Alpha, 1280x800 oder 640x400 (Screenshots), 440x280 (kleine Kachel) oder 1400x560 (Marquee). Edictora prüft das sowie, dass sich innerhalb eines Satzes kein Bild wiederholt, bevor Chrome geöffnet wird (Exitcode 2). Eine Ausnahme ist das Shop-Symbol (`icon`): Es muss 128x128 groß sein und darf ein transparentes PNG sein (8-Bit-RGB oder -RGBA). Verglichen wird einschließlich Alpha; die Farbe vollständig transparenter Pixel wird ignoriert. Es gilt für alle Sprachen, daher schließt `--image-scope global` es ein.
- Ein Eintrag darf `images` ohne `locales` haben (nur Bilder).
- **Bilder werden anhand ihrer dekodierten Pixel verglichen, nicht anhand der Dateibytes.** Das Dashboard kodiert hochgeladene PNGs neu, die gespeicherte Datei hat also nie dieselben Bytes wie Ihre, die Pixel sind aber identisch. Edictora lädt jedes Bild aus dem Dashboard herunter (das Original `=s0`, nur per HTTPS von `*.googleusercontent.com` und ohne Cookies) und vergleicht die Pixel-Hashes der Reihe nach. Lässt sich ein Bild nicht herunterladen oder dekodieren, lautet das Ergebnis „nicht vergleichbar“: nie „gleich“ und nie „ersetzen“.
- `push --yes` ersetzt einen Platz komplett (alles löschen, dann der Reihe nach hochladen) nur bei Abweichung, speichert einen Entwurf, lädt neu und prüft erneut. Reihenfolge: Shop-Symbol, kleine Kachel, Marquee, globale Screenshots, dann die lokalisierten Screenshots je Sprache.
- **Das Löschen eines Bildes wirkt im Dashboard sofort** (es fragt „Das kann nicht rückgängig gemacht werden“, und nach dem Neuladen ohne Speichern blieb der Platz leer). Bricht ein Push mittendrin ab, kann der Entwurf ohne einzelne Bilder zurückbleiben: Führen Sie ihn erneut aus, um ihn abzuschließen. Edictora fasst nie einen Eintrag an, der in Prüfung ist.
- Ein Eintrag mit nur einer Sprache hat keine lokalisierten Screenshots; verwenden Sie `--image-scope global`. Ist eine Sprache aus `localizedScreenshots` nicht im Dashboard, hält Edictora an, bevor etwas geändert wird: Verwenden Sie `--image-scope global` oder `--locale`.
- Bilder lassen sich nur schreiben, solange das Dashboard auf Japanisch angezeigt wird (die englischen Bezeichnungen sind nicht verifiziert).

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

#### Registerkarte „Datenschutz“ (optional)

Wenn Sie einer Erweiterung eine Berechtigung hinzufügen, verlangt die Registerkarte **„Datenschutz“** des Dashboards (Privacy practices) eine Begründung, bevor der Eintrag zur Prüfung eingereicht werden kann. Fügen Sie einem Eintrag `privacy` hinzu, um diese Felder aus Ihren Quelldateien zu lesen, zu vergleichen und als Entwurf zu speichern. Wie bei allen anderen Feldern **werden Schlüssel, die Sie weglassen, nie angetastet**.

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

- Die Texte werden wie `description` aus der Datei geschnitten (siehe oben). Führende Leerzeilen und der gemeinsame Einzug werden entfernt, sodass auch unter `  activeTab:` eingerückte Begründungen direkt funktionieren. Kein Text darf leer sein, und jeder muss in 1.000 Zeichen passen; Edictora prüft das, bevor Chrome geöffnet wird (Exitcode 2).
- `collected` akzeptiert `personallyIdentifiable`, `health`, `financial`, `authentication`, `personalCommunications`, `location`, `webHistory`, `userActivity` und `websiteContent` (nur die genannten werden angehakt, alle anderen abgewählt). `certify` darf nur `true` sein (hakt die drei Erklärungen wie „Ich verkaufe nicht …“ an); `false` oder Weglassen ist ein Konfigurationsfehler.
- **Auch die Menge der Berechtigungen wird geprüft.** Schreiben Sie `permissions`, erscheint eine Berechtigung, die in der Konfiguration steht, im Dashboard aber fehlt (oder umgekehrt), im Ergebnis, und `push` hält an, bevor etwas geschrieben wird: Ergänzen Sie sie in der Konfiguration oder korrigieren Sie das Manifest.
- `policyUrl` ist das Feld für die URL der Datenschutzerklärung: eine absolute http(s)-URL (bis zu 2.048 Zeichen) oder `""`, wenn das Feld leer sein soll. Ein weggelassener Schlüssel wird nie angerührt. Er wird zusammen mit den anderen Feldern dieser Registerkarte gespeichert.
- `remoteCode` (`{ "uses": false }` oder `{ "uses": true, "justification": {...} }`) wird in der Konfiguration akzeptiert, aber der zugehörige Bereich im Dashboard konnte noch nicht an einer echten Oberfläche geprüft werden; schreiben Sie ihn, meldet `diff` einen Fehler und `push` hält an. Lassen Sie ihn vorerst weg.
- Funktioniert nur, solange das Dashboard auf Japanisch angezeigt wird (die englischen Bezeichnungen sind nicht verifiziert). Es wird nur als Entwurf gespeichert; Edictora reicht nie zur Prüfung ein.
- Ein Eintrag darf `privacy` ohne `locales` und `images` haben (nur die Registerkarte „Datenschutz“).

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```

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
| 5 | Anmeldung erforderlich (das automatische Warten auf die Anmeldung wurde nicht abgeschlossen, oder die Anmeldung ist mitten in der Arbeit abgelaufen; bei erneutem Ausführen wird bei Bedarf ein Anmeldefenster geöffnet) |
| 130 | Unterbrochen (Strg+C) |

Fortschrittsmeldungen gehen an die Standardfehlerausgabe, Ergebnisse an die Standardausgabe.

`pull`, `diff`, `push` und `submit` führen Chrome ohne Fenster aus (headless), sodass sich Chrome nie vor Ihre Arbeit schiebt.
Um zu beobachten, was passiert, fügen Sie `--show-browser` hinzu. `login` zeigt immer ein Fenster, weil Sie es von Hand bedienen. Ist die Google-Anmeldung abgelaufen, öffnen `pull`, `diff`, `push` und `submit` selbst ein Chrome-Fenster, warten, bis Sie sich angemeldet haben (höchstens 10 Minuten), schließen es und machen weiter. Sie enden mit dem Exitcode 5, wenn diese Anmeldung nicht abgeschlossen wird (Zeitüberschreitung, Fenster geschlossen oder ein Chrome ohne Fenster mit demselben Profil verhindert die Anzeige) oder wenn die Anmeldung mitten in der Arbeit abläuft (dann öffnet sich kein Fenster; führen Sie den Befehl erneut aus).

> [!WARNING]
> `--show-browser` ist für Personen gedacht, die den Bildschirm beobachten. Verwenden Sie es nicht, wenn Sie Edictora über einen KI-Agenten oder ein Skript ausführen: Auf dem Bildschirm des Benutzers würde ein Chrome-Fenster aufspringen, und die KI kann es ohnehin nicht sehen.
> Um die Ursache eines Fehlers zu finden, nutzen Sie die Fehlermeldung und den bei einem Fehler gespeicherten Text-Snapshot der Seite.
> - Die Fehlermeldung enthält das verdeckende Element, die Abmessungen des Elements und die Größe des Viewports.
> - Seiten-Snapshots (Textdateien, die zeilenweise Rolle, Name, Wert, Zustand und Position jedes sichtbaren Elements auflisten) werden unter `%LOCALAPPDATA%\Edictora.Data\diagnostics` gespeichert, und die Fehlermeldung nennt den Pfad. Ein Snapshot wird auch gespeichert, wenn `pull` oder `diff` das Dashboard nicht lesen kann. Edictora erstellt keine Screenshots.

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

**Toolport und automatische Registrierung (Setup-Version)**
- Die Prüfung läuft auf jedem Betriebssystem (Windows, macOS und Linux); Setup selbst wird derzeit nur für Windows verteilt.
- Für Claude Code und Codex jeweils einzeln überspringt Setup `claude mcp add` / `codex mcp add`, wenn beides zutrifft: (1) Die Einstellungen des Clients enthalten das Toolport-Gateway (`mcpServers` in `~/.claude.json` bzw. `~/.codex/config.toml`), und (2) die Registry von Toolport (`%APPDATA%\Toolport\registry.json`; der Ort lässt sich mit `TOOLPORT_REGISTRY` / `TOOLPORT_DATA_DIR` ändern) enthält einen aktivierten Server, der eine `edictora.exe` mit dem Argument `mcp` startet, in einem verwendeten Profil. Ist es dieselbe Datei wie die von Setup registrierte (als dieselbe Datei verglichen), lautet der gespeicherte Grund `toolport`, bei einer anderen Datei `toolport-foreign`; in beiden Fällen überspringt Setup die direkte Registrierung. Lässt sich das nicht feststellen, registriert Edictora wie bisher direkt. Edictora führt diese Prüfung unter Windows, macOS und Linux durch (Standardort der Registry: Windows `%APPDATA%\Toolport\registry.json`, macOS `~/Library/Application Support/Toolport/registry.json`, Linux `$XDG_CONFIG_HOME/Toolport/registry.json`, sofern es ein absoluter Pfad ist, sonst `~/.config/Toolport/registry.json`); auf jedem anderen System wird nicht geprüft, sondern direkt registriert.
- Eine bereits vorhandene Registrierung wird nie entfernt. Entfernen Sie das Toolport-Gateway später aus diesem Client, oder existiert die ausführbare Datei, auf die Toolport zeigt, nicht mehr, fügt Edictora beim nächsten Update oder bei der nächsten Anmeldung (die beim Anmelden laufende Update-Exe) die direkte Registrierung hinzu. Deaktivieren oder löschen Sie Edictora in Toolport, fügt Edictora sie nicht hinzu. Einen Client, den Setup einmal direkt registriert hat, fügt Edictora nicht automatisch erneut hinzu, auch wenn Sie die Registrierung selbst entfernen; nur ein von Setup ausgelassener Client wird wieder hinzugefügt.
- Wenn Sie Toolport später hinzufügen und nur die direkte Registrierung entfernen möchten, führen Sie `claude mcp remove edictora --scope user` bzw. `codex mcp remove edictora` aus.
- Stellt Toolport eine andere `edictora.exe` bereit (nicht die von Setup registrierte), überspringt Setup die direkte Registrierung ebenfalls, speichert den Grund `toolport-foreign` und schreibt eine Warnung in `%LOCALAPPDATA%\Edictora.Data\logs\Edictora_yyyy-MM-dd.log`. Richten Sie zur Behebung den `command` des Servers `edictora` in Toolport auf die ausführbare Datei der Setup-Version (`%LOCALAPPDATA%\Edictora\current\edictora.exe`). Kopieren Sie die ausführbare Datei nicht an einen anderen Ort; eine Kopie wird nicht aktualisiert. Die Prüfung wird bei jedem Setup, Update und jeder Anmeldung neu durchgeführt: Zeigt Toolport auf die Setup-Version, wechselt der Eintrag zu `toolport`; fehlt die ausführbare Datei, auf die Toolport zeigt, registriert Edictora direkt.
- Die Werkzeug-Quarantäne von Toolport (Werkzeuge werden in Quarantäne gestellt, wenn ein Update sie ändert) wird nicht geprüft.
- Das Verhalten wählen Sie mit `mcpRegistration` in `%APPDATA%\Edictora\config.json`: `auto` (Standard; das obige Verhalten), `direct` (immer direkt registrieren) oder `off` (nie automatisch registrieren oder reparieren; die Deinstallation entfernt weiterhin die Registrierungen, die auf Edictora zeigen). Das Installationsprogramm liest nur den Standardpfad der Konfiguration, nicht eine mit `--config` angegebene. Andere Befehle melden jeden anderen Wert als Konfigurationsfehler. Das Installationsprogramm behandelt einen ungültigen Wert oder eine Konfigurationsdatei ohne gültiges JSON als `auto` und protokolliert ihn.

| Tool | Funktion |
|---|---|
| `edictora_list_items` | Zeigt, was die Konfigurationsdatei abdeckt (Elemente, Locales, Quelldateien). Öffnet keinen Browser. |
| `edictora_diff` | Zeigt die Unterschiede zu den Quelldateien (nur lesend) |
| `edictora_pull` | Zeigt die aktuellen Werte (nur lesend) |
| `edictora_push` | Schreibt abweichende Felder um und speichert sie als Entwurf. Ohne `confirm: true` zeigt es die Unterschiede und hält an (ein Probelauf). |
| `edictora_submit` | Reicht ein Element zur Prüfung ein. `item` und `confirm: true` sind erforderlich. |

- Die Tools führen denselben Code wie die CLI aus, mit denselben Schutzmechanismen und Exitcodes. Jedes Ergebnis beginnt mit dem Exitcode und seiner Bedeutung. „Unterschiede gefunden“ (3) gilt nicht als Fehler.
- Clients können nicht festlegen, den Browser anzuzeigen (`--show-browser`), `login` auszuführen oder das Profil bzw. chrome.exe zu setzen. Chrome läuft immer ohne Fenster, außer dem unten beschriebenen Anmeldefenster.
- Ist Ihre Anmeldung abgelaufen, öffnet Edictora auf Ihrem Bildschirm ein Chrome-Fenster zur Anmeldung, wartet, bis Sie fertig sind (höchstens 10 Minuten), und setzt dann den Tool-Aufruf fort. Wird sie nicht abgeschlossen, fordert das Tool Sie auf, `edictora login` in einem Terminal auszuführen.
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
- Bei einem Fehler gespeicherte Text-Snapshots der Seite werden unter `%LOCALAPPDATA%\Edictora.Data\diagnostics` gespeichert, nie in Ihrem Repository. Sie können Feldwerte der Dashboard-Seite enthalten (die publisherId sowie Name und E-Mail-Adresse Ihres Google-Kontos werden maskiert).
- Bild-Updates ersetzen nur die Plätze, die abweichen. Sie ordnen keine Bilder um, und sie können nie zur Prüfung einreichen (der Bildcode hat keinen Weg, das aufzurufen).
- Aktualisierungen der Registerkarte „Datenschutz“ ändern nur die abweichenden Felder, reichen nie zur Prüfung ein (der Code hat keine Möglichkeit, das aufzurufen) und halten an, ohne etwas zu schreiben, wenn die Berechtigungen im Dashboard nicht zur Konfiguration passen.

## Speicherorte der Dateien

| Inhalt | Ort |
|---|---|
| Konfigurationsdatei (Zuordnung der Quelldateien) | `%APPDATA%\Edictora\config.json` |
| Chrome-Profil (enthält Ihre Anmeldung) | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| Logs (einschließlich Aktualisierungsergebnissen) | `%LOCALAPPDATA%\Edictora.Data\logs` |
| Text-Snapshots der Seite bei Fehlern | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| Die App selbst (bei Installation mit Setup) | `%LOCALAPPDATA%\Edictora` (wird bei der Deinstallation vollständig entfernt) |

Die Daten liegen getrennt von der App in `Edictora.Data`, damit beim Deinstallieren nicht auch Ihr angemeldetes Chrome-Profil gelöscht wird. Daten an älteren Speicherorten (etwa `%LOCALAPPDATA%\Edictora\chrome-profile`) werden nicht automatisch migriert.

Das Chrome-Profil enthält Ihre Anmelde-Cookies. Geben Sie es nicht weiter und löschen Sie es selbst, wenn Sie es nicht mehr brauchen.
Die Publisher-ID (Teil der Dashboard-URL) wird in den Logs maskiert.

### Wenn die Konfiguration zu fehlen scheint (Claude-Desktop-App)

Die Claude-Desktop-App ist eine gepackte (MSIX-)App. Legt ein aus ihr gestartetes Programm einen **neuen** Ordner unter `%APPDATA%` oder `%LOCALAPPDATA%` an, landet er im privaten Speicher der App (`%LOCALAPPDATA%\Packages\<Paket>\LocalCache\...`) statt am echten Ort, und ein normales Terminal sieht ihn nicht. Edictora warnt auf der Fehlerausgabe, wenn es Konfiguration oder Chrome-Profil von so einem Ort liest, und listet bei fehlender Konfiguration die dort gefundenen Kopien auf. Zur Behebung verschieben Sie die Datei (bzw. den Ordner) an den in der Meldung genannten echten Ort, z. B. `%APPDATA%\Edictora\config.json`. Edictora liest oder verschiebt diese Kopien nie von sich aus.

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
bun test               # run the page-script tests (fake DOM)
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
