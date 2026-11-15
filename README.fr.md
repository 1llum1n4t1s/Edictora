<p align="center">
  <img src="icon/app_icon.png" alt="Icône d'Edictora" width="128">
</p>

# Edictora

[English](README.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | Français | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**Un outil Windows qui lit, compare et met à jour en masse vos fiches du Chrome Web Store (description et URL), à partir de fichiers sources conservés dans le dépôt de chaque extension.**

L'API publique du Chrome Web Store (CWS) (v2) ne permet ni de lire ni d'écrire les fiches de présentation, et Chrome n'autorise pas les extensions à agir sur les pages du CWS.
Edictora pilote donc une instance de Chrome dotée d'un profil dédié via le Chrome DevTools Protocol (CDP) et modifie le tableau de bord développeur avec les mêmes entrées qu'une personne.
Il s'exécute sous la forme d'un unique outil en ligne de commande (`edictora.exe`). Il est écrit en Rust et n'a besoin d'aucun environnement d'exécution comme .NET ou Node.js.

> [!IMPORTANT]
> Edictora dépend de la disposition du tableau de bord développeur. Il a été vérifié avec le tableau de bord affiché en **japonais**.
> Les tableaux de bord en anglais sont également reconnus, mais les libellés anglais à l'écran n'ont pas encore été vérifiés sur un écran réel.
> Si Edictora ne trouve pas un champ ou un bouton (par exemple après une refonte du tableau de bord), il s'arrête sans rien modifier.

## Fonctionnalités

- **pull** : lit les valeurs actuelles du tableau de bord (description, URL officielle, URL de la page d'accueil et URL d'assistance). Lecture seule.
- **diff** : affiche les différences avec vos fichiers sources. Lecture seule, et commande par défaut.
- **push** : réécrit uniquement les champs qui diffèrent, **les enregistre en brouillon**, puis recharge la page et compare les valeurs enregistrées à vos fichiers sources.
- **Images** : si un élément a `images` dans la configuration, `pull`, `diff` et `push` gèrent aussi ses images de fiche (icône de la boutique, captures d'écran, petite tuile promotionnelle et marquee). Limitez-les avec `--only text|images` et `--image-scope all|global|localized`.
- **submit** : envoie un élément pour examen. Il ne modifie pas le paramètre « publier automatiquement après examen ».
- **login** : ouvre Chrome avec le profil dédié pour que vous puissiez vous connecter vous-même à Google.
- **update** : télécharge et installe une version plus récente, s'il y en a une. Cela ne fonctionne que si vous avez installé Edictora avec Setup, et la commande s'exécute aussi automatiquement à l'ouverture de votre session Windows.

## Langue d'affichage

Les messages, les différences affichées et l'aide d'Edictora sont disponibles en neuf langues : japonais, anglais, chinois simplifié, chinois traditionnel, coréen, espagnol, français, allemand et portugais brésilien.

- Par défaut, Edictora détermine la langue dans cet ordre : la variable d'environnement `EDICTORA_LANGUAGE`, puis `LC_ALL`, `LC_MESSAGES` et `LANG` (les valeurs vides, `C` et `POSIX` sont ignorées), puis la langue d'affichage de Windows. Si aucune ne correspond à une langue prise en charge, il utilise l'anglais.
- Pour choisir explicitement une langue, définissez `EDICTORA_LANGUAGE` sur l'une des valeurs `ja`, `en`, `zh-CN`, `zh-TW`, `ko`, `es`, `fr`, `de` ou `pt-BR`. L'ancien nom `EDICTORA_LANG` est toujours lu, mais `EDICTORA_LANGUAGE` est prioritaire.
- `edictora mcp` répond toujours en anglais, quelle que soit la langue d'affichage, car les descriptions et les réponses de ses outils sont lues par des agents IA.
- Cela est indépendant de la langue du tableau de bord développeur (japonais ou anglais) et des `locales` de vos fiches.
- Les fichiers journaux sont destinés au dépannage par l'auteur, leurs messages fixes restent donc en japonais. Les messages d'erreur inclus dans le journal suivent la langue d'affichage.

## Installation et mise à jour

Exécutez `Edictora-win-Setup.exe` (un programme d'installation Velopack). Il s'installe dans `%LOCALAPPDATA%\Edictora` sans droits d'administrateur.

- Après l'installation, `edictora` fonctionne dans toute nouvelle fenêtre d'Invite de commandes ou de PowerShell, car Setup ajoute `%LOCALAPPDATA%\Edictora\current` au `Path` de votre utilisateur.
- À l'ouverture de votre session Windows, `edictora-updater.exe` s'exécute sans fenêtre. Si `https://edictora.kagayoi.com` propose une version plus récente, il la télécharge, vérifie sa taille et son hachage, puis l'installe.
  - Il est enregistré sous le nom `Edictora Updater` dans `Run` de HKCU ; vous pouvez donc le désactiver dans « Applications de démarrage » du Gestionnaire des tâches.
  - Le résultat est écrit dans le journal.
- Pour mettre à jour manuellement, exécutez `edictora update`. La commande rend compte sur la sortie d'erreur standard et se termine avec le code 0 si vous êtes déjà à jour ou si la mise à jour a démarré, et avec le code 1 en cas d'échec. Update.exe remplace les fichiers après la fermeture d'`edictora`.
- La version Portable (`Edictora-win-Portable.zip`) ne se met pas à jour automatiquement à l'ouverture de session et ne s'ajoute pas au `Path`. Pour la mettre à jour, exécutez `current\edictora.exe update` dans le dossier où vous l'avez extraite. Les exécutables compilés à partir des sources ne se mettent pas à jour eux-mêmes.
- Les mises à jour ne sont jamais appliquées pendant l'exécution de `login`, `pull`, `diff`, `push` ou `submit`. Edictora termine le téléchargement et applique la mise à jour plus tard.
  - Si vous lancez l'une de ces commandes pendant l'application d'une mise à jour, elle se termine avec le code 1 sans ouvrir Chrome. Patientez un instant, puis relancez-la.
  - Une fois le processus de mise à jour terminé, Update.exe a besoin de quelques secondes de plus pour remplacer les fichiers, et Edictora ne peut pas protéger cet intervalle. Juste après une mise à jour, patientez un instant avant d'exécuter une commande.
- La désinstallation supprime `%LOCALAPPDATA%\Edictora` ainsi que les entrées `Run` et `Path`. Vos données (`%LOCALAPPDATA%\Edictora.Data`) et vos paramètres (`%APPDATA%\Edictora`) sont conservés ; supprimez-les vous-même si vous n'en avez plus besoin.

## Configuration requise

- Windows 10 ou Windows 11 (x86_64)
- Google Chrome. Edictora le trouve automatiquement, ou vous pouvez définir `chromePath` dans le fichier de configuration ou passer `--chrome`.
- Un compte développeur CWS
- Rust 1.100.0 si vous compilez à partir des sources (fixé dans `rust-toolchain.toml`). Bun n'est nécessaire que si vous modifiez les scripts exécutés dans les pages.

## Utilisation

### 1. Écrire le fichier de configuration

L'emplacement par défaut est `%APPDATA%\Edictora\config.json` ; vous pouvez le changer avec `--config`.
Pour chaque élément, associez chaque locale à ses fichiers sources. **Les champs que vous omettez ne sont jamais lus, comparés ni écrits.**

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

- Si vous indiquez uniquement `description.file`, le fichier entier sert de description.
- Lorsqu'une section est extraite, Edictora prend les lignes situées après le titre de début et avant le titre de fin. Il supprime les lignes vides au début et à la fin ainsi que les espaces en fin de ligne, et normalise les fins de ligne en `\n`. Si un titre est introuvable, il s'arrête avec une erreur.
- Un champ d'URL sans clé est ignoré. `""` signifie que le champ doit être vide.
- Les clés inconnues (comme les fautes de frappe) sont des erreurs. Les commentaires et les virgules finales sont autorisés.

#### Images de la fiche (facultatif)

Ajoutez `images` à un élément pour gérer ses images. Comme pour les autres champs, **les clés que vous n'écrivez pas ne sont jamais touchées**. Les chemins sont relatifs à `baseDir`.

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

- `dir` utilise les fichiers `screenshot-*.png` situés directement dans le dossier, par ordre de nom (1 à 5 fichiers). `files` utilise les fichiers dans l'ordre où vous les écrivez (1 à 5).
- Seul le PNG est pris en charge : 24 bits sans alpha, 1280x800 ou 640x400 pour les captures, 440x280 pour la petite tuile et 1400x560 pour le marquee. Edictora le vérifie, ainsi qu'une éventuelle image en double dans un même jeu, avant d'ouvrir Chrome (code de sortie 2). L'icône de la boutique (`icon`) fait exception : elle doit faire 128x128 et peut être un PNG transparent (RVB ou RVBA 8 bits). Elle est comparée alpha compris, en ignorant la couleur des pixels totalement transparents. Elle est commune à toutes les langues, donc `--image-scope global` l'inclut.
- Un élément peut avoir `images` sans `locales` (images seulement).
- **Les images sont comparées par leurs pixels décodés, pas par les octets du fichier.** Le tableau de bord ré-encode les PNG téléversés : le fichier stocké n'a jamais les mêmes octets que le vôtre, mais les pixels sont identiques. Edictora télécharge chaque image du tableau de bord (l'original `=s0`, en HTTPS uniquement depuis `*.googleusercontent.com`, sans cookie) et compare les empreintes des pixels dans l'ordre. Si une image ne peut pas être téléchargée ou décodée, le résultat est « comparaison impossible » : jamais « identique » ni « remplacer ».
- `push --yes` remplace un emplacement entier (tout supprimer, puis téléverser dans l'ordre) uniquement s'il diffère, enregistre un brouillon, recharge et vérifie à nouveau. Ordre : icône de la boutique, petite tuile, marquee, captures globales, puis les captures localisées de chaque langue.
- **La suppression d'une image prend effet immédiatement sur le tableau de bord** (il demande « cette action est irréversible » et, après rechargement sans enregistrer, l'emplacement restait vide). Si un push s'arrête en cours de route, le brouillon peut rester privé de certaines images : relancez-le pour terminer. Edictora ne touche jamais un élément en attente d'examen.
- Un élément à une seule langue n'a pas de captures localisées ; utilisez `--image-scope global`. Si une langue de `localizedScreenshots` n'est pas sur le tableau de bord, Edictora s'arrête avant de rien modifier : utilisez `--image-scope global` ou `--locale`.
- L'écriture d'images ne fonctionne que lorsque le tableau de bord est en japonais (les libellés anglais ne sont pas vérifiés).

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

### 2. Se connecter

```powershell
edictora login
```

Chrome s'ouvre avec le profil dédié. Connectez-vous-y à Google et ouvrez le tableau de bord développeur.
- Dès qu'Edictora voit le tableau de bord, il conserve la connexion dans le profil et se ferme.
- Il attend jusqu'à 10 minutes par défaut. Utilisez `--wait <minutes>` pour définir une durée de 1 à 120 minutes.
- Edictora ne manipule jamais votre mot de passe ni vos codes de vérification ; vous les saisissez vous-même.

### 3. Comparer et mettre à jour

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

| Code de sortie | Signification |
|---|---|
| 0 | Succès (aucune différence) |
| 1 | Erreur d'exécution (échec d'une opération sur le tableau de bord, vérification non concordante, etc.) |
| 2 | Arguments ou configuration non valides (Chrome n'est pas ouvert) |
| 3 | Différences détectées (`diff`, ou `push` sans `--yes`) |
| 4 | Écriture interrompue (examen en attente, ou état impossible à déterminer) |
| 5 | Connexion requise |
| 130 | Interrompu (Ctrl+C) |

La progression est envoyée sur la sortie d'erreur standard et les résultats sur la sortie standard.

`pull`, `diff`, `push` et `submit` exécutent Chrome sans fenêtre (headless), de sorte que Chrome ne vienne jamais se placer devant votre travail.
Pour observer ce qui se passe, ajoutez `--show-browser`. `login` affiche toujours une fenêtre, car vous la manipulez à la main.

> [!WARNING]
> `--show-browser` est destiné aux personnes qui regardent l'écran. Ne l'utilisez pas lorsque vous exécutez Edictora depuis un agent IA ou un script : une fenêtre Chrome apparaîtrait sur l'écran de l'utilisateur, et l'IA ne peut de toute façon pas la voir.
> Pour trouver la cause d'un échec, utilisez le message d'erreur et la capture d'écran enregistrée lors de l'échec.
> - Le message d'erreur indique l'élément qui recouvre, les limites de l'élément et la taille de la fenêtre d'affichage.
> - Les captures d'écran sont enregistrées dans `%LOCALAPPDATA%\Edictora.Data\diagnostics`, et le message d'erreur indique le chemin. Une capture d'écran est aussi enregistrée lorsque `pull` ou `diff` ne parvient pas à lire le tableau de bord.

### 4. L'utiliser depuis des agents IA (MCP)

`edictora mcp` est un serveur MCP qui fonctionne via l'entrée et la sortie standard. Une fois enregistré auprès de Claude Code, de Codex ou d'un autre client, l'IA peut trouver et utiliser Edictora lorsqu'elle modifie vos fiches du Chrome Web Store.

**Setup enregistre automatiquement Edictora auprès de Claude Code et de Codex**, si leurs CLI officielles (`claude` et `codex`) sont dans le `PATH`. La désinstallation supprime uniquement les enregistrements qui pointent vers Edictora.
- Si une CLI était absente au moment de l'installation, ou si l'enregistrement n'a pas abouti à temps, Edictora réessaie à votre prochaine ouverture de session. Il en va de même si vous installez une CLI plus tard.
- Si vous supprimez vous-même un enregistrement après son ajout, Edictora ne le rajoute pas.

Pour la version Portable, ou pour les clients autres que ces deux-là (comme Claude Desktop ou une passerelle), enregistrez-le une fois avec l'une des méthodes ci-dessous.
Indiquez l'exe installé par un chemin absolu :
- Version Setup : `%LOCALAPPDATA%\Edictora\current\edictora.exe`. Ce chemin reste le même lorsque les mises à jour automatiques remplacent les fichiers.
- Version Portable : `current\edictora.exe` dans le dossier où vous l'avez extraite.

`edictora` dans le `PATH` fonctionne aussi, mais les clients lancés avant l'installation ne voient pas le nouveau `PATH` ; un chemin absolu est donc plus sûr.

**Claude Code** (pour tous les projets)

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

Ou ajoutez-le à `%USERPROFILE%\.codex\config.toml` (remplacez `<username>`) :

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop et les autres clients configurés avec un fichier JSON** (remplacez `<username>`) :

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

Si vous utilisez une passerelle MCP (comme Toolport), enregistrez la même commande (stdio, argument `mcp`) sur la passerelle.

| Outil | Ce qu'il fait |
|---|---|
| `edictora_list_items` | Affiche ce que couvre le fichier de configuration (éléments, locales, fichiers sources). N'ouvre pas de navigateur. |
| `edictora_diff` | Affiche les différences avec les fichiers sources (lecture seule) |
| `edictora_pull` | Affiche les valeurs actuelles (lecture seule) |
| `edictora_push` | Réécrit les champs qui diffèrent et les enregistre en brouillon. Sans `confirm: true`, il affiche les différences et s'arrête (simulation). |
| `edictora_submit` | Envoie un élément pour examen. `item` et `confirm: true` sont obligatoires. |

- Les outils exécutent le même code que la CLI, avec les mêmes protections et les mêmes codes de sortie. Chaque résultat commence par le code de sortie et sa signification. Des différences détectées (3) ne sont pas traitées comme une erreur.
- Les clients ne peuvent pas choisir d'afficher le navigateur (`--show-browser`), exécuter `login`, ni définir le profil ou chrome.exe. Chrome s'exécute toujours sans fenêtre.
- Si votre connexion a expiré, l'outil vous demande d'exécuter `edictora login` dans un terminal. Vous vous connectez vous-même.
- Les appels s'exécutent un par un, dans l'ordre. Vous pouvez annuler un appel en cours avec `notifications/cancelled`.
- Un `diff` portant sur tous les éléments peut prendre plusieurs minutes ; prévoyez donc un délai d'expiration généreux dans votre client.

## Protections

- Par défaut, Edictora se contente d'afficher les différences et ne change rien. Il n'écrit qu'avec `push --yes` et n'envoie pour examen qu'avec `submit --item <id> --yes`.
- L'enregistrement d'un brouillon et l'envoi pour examen sont deux opérations distinctes. L'envoi se fait un élément à la fois ; il n'y a pas d'envoi groupé.
- Avant d'enregistrer, Edictora vérifie que chaque champ réécrit correspond exactement au fichier source. Après l'enregistrement, il recharge la page et vérifie de nouveau. Si quoi que ce soit ne correspond pas, il n'enregistre pas, ou il s'arrête avec une erreur.
- Pour chaque locale, Edictora change de langue, réécrit les champs, les vérifie, enregistre, puis recharge et vérifie de nouveau. Si une locale échoue, le reste de cet élément est ignoré.
  - Il n'y a pas de retour en arrière entre les locales : les locales enregistrées avant l'échec restent enregistrées en brouillon.
  - Rien n'est publié tant que vous n'envoyez pas l'élément pour examen.
- Edictora n'écrit pas lorsque le statut de l'élément ne peut pas être confirmé comme sûr pour la modification, par exemple « Examen en attente ». Il n'annule jamais un examen.
- Les champs et les boutons sont trouvés par leur libellé, leur rôle et leur texte, jamais par des noms de classes CSS. Si une recherche ne correspond pas à exactement un élément, Edictora s'arrête.
- Chrome est démarré sans indicateurs d'automatisation et fermé proprement. **Le profil n'est jamais supprimé.**
- Les captures d'écran prises en cas d'échec sont enregistrées dans `%LOCALAPPDATA%\Edictora.Data\diagnostics`, jamais dans votre dépôt.
- Les mises à jour d'images ne remplacent que les emplacements qui diffèrent. Elles ne touchent pas à l'ordre des images, et ne peuvent jamais soumettre à l'examen (le code des images n'a aucun moyen de l'appeler).

## Où les fichiers sont stockés

| Contenu | Emplacement |
|---|---|
| Fichier de configuration (association des fichiers sources) | `%APPDATA%\Edictora\config.json` |
| Profil Chrome (contient votre connexion) | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| Journaux (y compris les résultats des mises à jour) | `%LOCALAPPDATA%\Edictora.Data\logs` |
| Captures d'écran en cas d'échec | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| L'application elle-même (installée avec Setup) | `%LOCALAPPDATA%\Edictora` (entièrement supprimée à la désinstallation) |

Les données sont conservées dans `Edictora.Data`, séparément de l'application, afin que la désinstallation ne supprime pas aussi votre profil Chrome connecté. Les données situées dans d'anciens emplacements (comme `%LOCALAPPDATA%\Edictora\chrome-profile`) ne sont pas migrées automatiquement.

Le profil Chrome contient vos cookies de connexion. Ne le partagez pas et supprimez-le vous-même lorsque vous n'en avez plus besoin.
L'ID de l'éditeur (une partie de l'URL du tableau de bord) est masqué dans les journaux.

### Si la configuration semble absente (application de bureau Claude)

L'application de bureau Claude est une application empaquetée (MSIX). Lorsqu'un programme lancé depuis celle-ci crée un **nouveau** dossier sous `%APPDATA%` ou `%LOCALAPPDATA%`, Windows le place dans le stockage privé de l'application (`%LOCALAPPDATA%\Packages\<paquet>\LocalCache\...`) au lieu de l'emplacement réel, et un terminal normal ne peut pas le voir. Edictora avertit sur la sortie d'erreur lorsqu'il lit la configuration ou le profil Chrome depuis un tel endroit et, si la configuration est absente, liste les copies trouvées à cet endroit. Pour corriger, déplacez le fichier (ou le dossier) vers l'emplacement réel indiqué dans le message, par exemple `%APPDATA%\Edictora\config.json`. Edictora ne lit ni ne déplace jamais ces copies de lui-même.

## Compilation

Le programme principal est écrit en Rust (`crates/edictora/`).

```powershell
cargo build --release          # target\release\edictora.exe and edictora-updater.exe (Bun not needed)
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Les scripts qui s'exécutent dans les pages du tableau de bord se trouvent dans `page-scripts/` (Bun + TypeScript). Le fichier généré `page-scripts/dist/edictora.js` est intégré à l'exe ; vous n'avez donc besoin de Bun que si vous modifiez ces scripts.

```powershell
cd page-scripts
bun install --frozen-lockfile   # install dependencies (TypeScript only)
bun run typecheck      # type-check
bun run build          # regenerate dist/edictora.js
bun run check          # check that dist/edictora.js is up to date
```

**Expressions régulières dans `sectionStart` / `sectionEnd`** : Edictora utilise la crate `regex` de Rust ; les assertions avant (lookahead), arrière (lookbehind) (`(?=…)`, `(?!…)`, `(?<=…)`, `(?<!…)`) et les références arrière (`\1`) ne sont donc pas prises en charge ; elles provoquent une erreur au chargement de la configuration. Les motifs ordinaires comme `^■ Description` fonctionnent tels quels.

Pour les tests, vous pouvez remplacer :
- La racine des données locales avec `EDICTORA_LOCAL_ROOT` (par défaut `%LOCALAPPDATA%\Edictora.Data`)
- La racine itinérante avec `EDICTORA_ROAMING_ROOT` (par défaut `%APPDATA%\Edictora`)
- La source des mises à jour avec `EDICTORA_UPDATE_URL` (par défaut `https://edictora.kagayoi.com` ; le `http://` simple n'est autorisé que pour `127.0.0.1`)

Le paquet Velopack contient à la fois `edictora.exe` (l'exe principal) et `edictora-updater.exe`.
- Les mises à jour n'utilisent pas la crate velopack : Edictora récupère lui-même le flux, télécharge et vérifie les paquets avec WinHTTP, l'API HTTP intégrée à Windows.
- L'installation, le remplacement des fichiers et la désinstallation sont confiés à Setup.exe et Update.exe de Velopack.

## Distribution

Les versions sont servies depuis Cloudflare R2 à l'adresse `https://edictora.kagayoi.com` (canal Velopack `win`). Consultez [docs/release.md](docs/release.md) pour le processus de publication.

## Licence

Consultez [LICENSE](LICENSE). Pour la confidentialité, consultez [PRIVACY_POLICY.md](PRIVACY_POLICY.md).
