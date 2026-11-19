<p align="center">
  <img src="icon/app_icon.png" alt="Icono de Edictora" width="128">
</p>

# Edictora

[English](README.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | Español | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**Una herramienta para Windows que lee, compara y actualiza en bloque las fichas de Chrome Web Store (descripción y URL) de sus extensiones, usando archivos de origen que se guardan en el repositorio de cada extensión.**

La API pública (v2) de Chrome Web Store (CWS) no permite leer ni escribir las fichas de la tienda, y Chrome no deja que las extensiones operen sobre las páginas de CWS.
Por eso Edictora controla una instancia de Chrome con un perfil dedicado mediante el Chrome DevTools Protocol (CDP) y edita el Panel para desarrolladores con las mismas entradas que usaría una persona.
Funciona como una única herramienta de línea de comandos (`edictora.exe`). Está escrita en Rust y no necesita ningún entorno de ejecución como .NET o Node.js.

> [!IMPORTANT]
> Edictora depende del diseño del Panel para desarrolladores. Se verificó con el panel mostrado en **japonés**.
> También reconoce los paneles en inglés, pero las etiquetas en inglés que aparecen en pantalla todavía no se han verificado en una pantalla real.
> Si Edictora no encuentra un campo o un botón (por ejemplo, tras un rediseño del panel), se detiene sin cambiar nada.

## Funciones

- **pull**: lee los valores actuales del panel (descripción, URL oficial, URL de la página principal y URL de asistencia). Solo lectura.
- **diff**: muestra las diferencias con sus archivos de origen. Solo lectura y es el comando predeterminado.
- **push**: reescribe solo los campos que difieren, **los guarda como borrador**, vuelve a cargar la página y comprueba los valores guardados con sus archivos de origen.
- **Imágenes**: si un elemento tiene `images` en la configuración, `pull`, `diff` y `push` también tratan sus imágenes de la ficha (icono de la tienda, capturas de pantalla, mosaico promocional pequeño y marquee). Acótelas con `--only text|images|privacy` y `--image-scope all|global|localized`.
- **Pestaña Privacidad**: si un elemento tiene `privacy` en la configuración, `pull`, `diff` y `push` también tratan la pestaña «Privacidad» del panel (Privacy practices: descripción del propósito único, justificación de cada permiso y declaración del uso de datos). Acótela con `--only privacy`. Solo se guarda como borrador; nunca se envía a revisión.
- **submit**: envía un elemento a revisión. No cambia el ajuste «publicar automáticamente tras la revisión».
- **login**: abre Chrome con el perfil dedicado para que inicie sesión en Google usted mismo.
- **update**: descarga e instala una versión más reciente, si la hay. Solo funciona si instaló Edictora con Setup, y también se ejecuta automáticamente cuando inicia sesión en Windows.

## Idioma de visualización

Los mensajes, la salida de diff y el texto de ayuda de Edictora están disponibles en nueve idiomas: japonés, inglés, chino simplificado, chino tradicional, coreano, español, francés, alemán y portugués de Brasil.

- De forma predeterminada, Edictora decide el idioma en este orden: la variable de entorno `EDICTORA_LANGUAGE`, luego `LC_ALL`, `LC_MESSAGES` y `LANG` (se omiten los valores vacíos, `C` y `POSIX`), y después el idioma de visualización de Windows. Si ninguno da un idioma compatible, usa inglés.
- Para elegir un idioma de forma explícita, establezca `EDICTORA_LANGUAGE` en uno de estos valores: `ja`, `en`, `zh-CN`, `zh-TW`, `ko`, `es`, `fr`, `de` o `pt-BR`. El nombre antiguo `EDICTORA_LANG` se sigue leyendo, pero `EDICTORA_LANGUAGE` tiene prioridad.
- `edictora mcp` siempre responde en inglés, sea cual sea el idioma de visualización, porque sus descripciones de herramientas y sus respuestas las leen agentes de IA.
- Esto es independiente del idioma del Panel para desarrolladores (japonés o inglés) y de los `locales` de las fichas de su tienda.
- Los archivos de registro están pensados para que el autor resuelva problemas, así que sus mensajes fijos permanecen en japonés. Los mensajes de error incluidos en el registro siguen el idioma de visualización.

## Instalación y actualización

Ejecute `Edictora-win-Setup.exe` (un instalador de Velopack). Se instala en `%LOCALAPPDATA%\Edictora` sin necesidad de derechos de administrador.

- Tras la instalación, `edictora` funciona en cualquier ventana nueva del Símbolo del sistema o de PowerShell, porque Setup añade `%LOCALAPPDATA%\Edictora\current` al `Path` de su usuario.
- Cuando inicia sesión en Windows, `edictora-updater.exe` se ejecuta sin ventana. Si `https://edictora.kagayoi.com` tiene una versión más reciente, la descarga, comprueba su tamaño y su hash y luego la instala.
  - Se registra como `Edictora Updater` en `Run` de HKCU, por lo que puede desactivarlo en «Aplicaciones de inicio» del Administrador de tareas.
  - El resultado se escribe en el registro.
- Para actualizar manualmente, ejecute `edictora update`. Informa por la salida de error estándar y termina con el código 0 cuando ya está actualizado o la actualización ha comenzado, y con el código 1 si falla. Update.exe reemplaza los archivos cuando `edictora` termina.
- La versión Portable (`Edictora-win-Portable.zip`) no se actualiza automáticamente al iniciar sesión y no se añade a `Path`. Para actualizarla, ejecute `current\edictora.exe update` en la carpeta donde la extrajo. Los ejecutables compilados desde el código fuente no se actualizan por sí solos.
- Las actualizaciones nunca se aplican mientras `login`, `pull`, `diff`, `push` o `submit` se están ejecutando. Edictora termina la descarga y aplica la actualización más tarde.
  - Si ejecuta uno de estos comandos mientras se está aplicando una actualización, termina con el código 1 sin abrir Chrome. Espere un momento y vuelva a ejecutarlo.
  - Cuando el proceso de actualización termina, Update.exe necesita unos segundos más para reemplazar los archivos, y Edictora no puede proteger ese intervalo. Justo después de una actualización, espere un momento antes de ejecutar un comando.
- La desinstalación elimina `%LOCALAPPDATA%\Edictora` y las entradas de `Run` y `Path`. Sus datos (`%LOCALAPPDATA%\Edictora.Data`) y su configuración (`%APPDATA%\Edictora`) se conservan, así que elimínelos usted mismo si ya no los necesita.

## Requisitos

- Windows 10 o Windows 11 (x86_64)
- Google Chrome. Edictora lo encuentra automáticamente, o puede definir `chromePath` en el archivo de configuración o pasar `--chrome`.
- Una cuenta de desarrollador de CWS
- Rust 1.100.0 si compila desde el código fuente (fijado en `rust-toolchain.toml`). Bun solo es necesario cuando cambia los scripts que se ejecutan dentro de las páginas.

## Uso

### 1. Escribir el archivo de configuración

La ubicación predeterminada es `%APPDATA%\Edictora\config.json`; puede cambiarla con `--config`.
Para cada elemento, asocie cada configuración regional (locale) a sus archivos de origen. **Los campos que omita nunca se leen, se comparan ni se escriben.**

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

- Si indica solo `description.file`, se usa el archivo completo como descripción.
- Cuando se extrae una sección, Edictora toma las líneas posteriores al encabezado inicial y anteriores al encabezado final. Elimina las líneas en blanco iniciales y finales y los espacios en blanco finales, y normaliza los saltos de línea a `\n`. Si no se encuentra un encabezado, se detiene con un error.
- Un campo de URL sin clave se ignora. `""` significa que el campo debe estar vacío.
- Las claves desconocidas (por ejemplo, erratas) son errores. Se permiten comentarios y comas finales.

#### Categoría (opcional)

Añada `category` a un elemento para leer, comparar y guardar la categoría de la ficha (el campo «Categoría» del panel, común a todos los idiomas). Escriba una clave en minúsculas. Como en cualquier otro campo, **si no la escribe, la categoría nunca se toca**.

```jsonc
"category": "productivity"    // one of the keys below; leave it out to skip the category
```

- Claves: `productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`. Una clave desconocida es un error de configuración (código de salida 2). «Sin categoría seleccionada» no puede ser un valor de origen.
- La categoría es común a todos los idiomas, así que se lee y se escribe una sola vez por elemento (no una vez por idioma). Se incluye en `--only text` y `--locale` no la restringe. `push --yes` la escribe solo cuando difiere, guarda un borrador, recarga y vuelve a comprobar.
- Un elemento puede tener `category` sin `locales` (solo la categoría).
- Solo funciona mientras el panel se muestra en japonés (los textos en inglés no están verificados).

#### Imágenes de la ficha (opcional)

Añada `images` a un elemento para gestionar sus imágenes. Como en el resto de campos, **las claves que no escriba no se tocan nunca**. Las rutas son relativas a `baseDir`.

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

- `dir` usa los archivos `screenshot-*.png` que están directamente en la carpeta, por orden de nombre (1–5 archivos). `files` usa los archivos en el orden en que los escriba (1–5).
- Solo se admite PNG de 24 bits sin alfa, de 1280x800 o 640x400 (capturas), 440x280 (mosaico pequeño) o 1400x560 (marquee). Edictora lo comprueba, y que no se repita ninguna imagen dentro de un conjunto, antes de abrir Chrome (código de salida 2). El icono de la tienda (`icon`) es la excepción: debe ser de 128x128 y puede ser un PNG transparente (RGB o RGBA de 8 bits). Se compara incluyendo el alfa e ignorando el color de los píxeles totalmente transparentes. Es común a todos los idiomas, así que `--image-scope global` lo incluye.
- Un elemento puede tener `images` sin `locales` (solo imágenes).
- **Las imágenes se comparan por sus píxeles decodificados, no por los bytes del archivo.** El panel vuelve a codificar los PNG que se suben, así que el archivo guardado nunca tiene los mismos bytes que el suyo, aunque los píxeles sean idénticos. Edictora descarga cada imagen del panel (el original `=s0`, solo por HTTPS desde `*.googleusercontent.com` y sin cookies) y compara los hashes de píxeles en orden. Si una imagen no se puede descargar o decodificar, el resultado es «no se puede comparar»: nunca «coincide» ni «reemplazar».
- `push --yes` reemplaza una ubicación entera (borra todo y sube en orden) solo si difiere, guarda un borrador, recarga y vuelve a comprobar. El orden es: icono de la tienda, mosaico pequeño, marquee, capturas globales y, después, las capturas localizadas de cada idioma.
- **Borrar una imagen surte efecto de inmediato en el panel** (pregunta «no se puede deshacer» y, al recargar sin guardar, la ubicación seguía vacía). Si un push se interrumpe a medias, el borrador puede quedar sin algunas imágenes: vuelva a ejecutarlo para terminar. Edictora nunca toca un elemento pendiente de revisión.
- Un elemento con un solo idioma no tiene capturas localizadas; use `--image-scope global`. Si un idioma de `localizedScreenshots` no está en el panel, Edictora se detiene antes de cambiar nada: use `--image-scope global` o `--locale`.
- La escritura de imágenes solo funciona con el panel en japonés (los textos en inglés no están verificados).

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

#### Pestaña Privacidad (opcional)

Cuando añade un permiso a una extensión, la pestaña **Privacidad** del panel (Privacy practices) exige una justificación antes de poder enviar el elemento a revisión. Añada `privacy` a un elemento para leer, comparar y guardar como borrador esos campos desde sus archivos de origen. Como en el resto de campos, **las claves que no escriba no se tocan nunca**.

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

- El texto se extrae del archivo igual que `description` (véase arriba). Se quitan las líneas en blanco iniciales y la sangría común, de modo que sirven las justificaciones sangradas bajo `  activeTab:`. Ningún texto puede estar vacío y todos deben caber en 1000 caracteres; Edictora lo comprueba antes de abrir Chrome (código de salida 2).
- `collected` admite `personallyIdentifiable`, `health`, `financial`, `authentication`, `personalCommunications`, `location`, `webHistory`, `userActivity` y `websiteContent` (solo se marcan los indicados; el resto se desmarca). `certify` solo puede ser `true` (marca las tres declaraciones, como «no vendo…»); `false` u omitirlo es un error de configuración.
- **También se comprueba el conjunto de permisos.** Si escribe `permissions`, un permiso que está en la configuración pero no en el panel (o al revés) aparece en el resultado y `push` se detiene sin escribir nada: añádalo a la configuración o corrija el manifest.
- `policyUrl` es el campo de la URL de la política de privacidad: una URL http(s) absoluta (hasta 2.048 caracteres) o `""` para indicar que el campo debe quedar vacío. Una clave que omita nunca se toca. Se guarda junto con los demás campos de esta pestaña.
- `remoteCode` (`{ "uses": false }` o `{ "uses": true, "justification": {...} }`) se acepta en la configuración, pero la sección correspondiente del panel aún no se ha podido comprobar en una pantalla real, así que, si lo escribe, `diff` informa de un error y `push` se detiene. Por ahora, no lo escriba.
- Solo funciona con el panel en japonés (los textos en inglés no están verificados). Se guarda únicamente como borrador; Edictora nunca envía a revisión.
- Un elemento puede tener `privacy` sin `locales` ni `images` (solo la pestaña Privacidad).

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```

### 2. Iniciar sesión

```powershell
edictora login
```

Chrome se abre con el perfil dedicado. Inicie sesión en Google allí y abra el Panel para desarrolladores.
- Cuando Edictora ve el panel, conserva el inicio de sesión en el perfil y termina.
- Espera hasta 10 minutos de forma predeterminada. Use `--wait <minutos>` para fijar entre 1 y 120 minutos.
- Edictora nunca maneja su contraseña ni sus códigos de verificación; los introduce usted mismo.

### 3. Comparar y actualizar

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

| Código de salida | Significado |
|---|---|
| 0 | Correcto (sin diferencias) |
| 1 | Error de ejecución (falló una operación del panel, la verificación no coincide, etc.) |
| 2 | Argumentos o configuración no válidos (no se abre Chrome) |
| 3 | Se encontraron diferencias (`diff`, o `push` sin `--yes`) |
| 4 | Escritura interrumpida (revisión pendiente, o no se pudo determinar el estado) |
| 5 | Se requiere iniciar sesión (la espera automática de inicio de sesión no se completó, o la sesión caducó a mitad del trabajo; al ejecutarlo de nuevo se abre una ventana de inicio de sesión si hace falta) |
| 130 | Interrumpido (Ctrl+C) |

El progreso se envía a la salida de error estándar y los resultados a la salida estándar.

`pull`, `diff`, `push` y `submit` ejecutan Chrome sin ventana (headless), de modo que Chrome nunca se pone delante de su trabajo.
Para ver lo que ocurre, añada `--show-browser`. `login` siempre muestra una ventana, porque la maneja usted a mano. Si el inicio de sesión de Google ha caducado, `pull`, `diff`, `push` y `submit` abren por sí mismos una ventana de Chrome, esperan a que inicie sesión (hasta 10 minutos), la cierran y continúan. Se detienen con el código de salida 5 si ese inicio de sesión no se completa (tiempo agotado, ventana cerrada o un Chrome sin ventana del mismo perfil que impide mostrarla), o si la sesión caduca a mitad del trabajo (entonces no se abre ninguna ventana; vuelva a ejecutarlo).

> [!WARNING]
> `--show-browser` es para quien observa la pantalla. No lo use al ejecutar Edictora desde un agente de IA o un script: aparecería una ventana de Chrome en la pantalla del usuario, y la IA tampoco puede verla.
> Para averiguar la causa de un fallo, use el mensaje de error y la captura de pantalla que se guarda al fallar.
> - El mensaje de error incluye el elemento que lo cubre, los límites del elemento y el tamaño del área visible.
> - Las capturas de pantalla se guardan en `%LOCALAPPDATA%\Edictora.Data\diagnostics`, y el mensaje de error muestra la ruta. También se guarda una captura cuando `pull` o `diff` no consiguen leer el panel.

### 4. Uso desde agentes de IA (MCP)

`edictora mcp` es un servidor MCP que funciona a través de la entrada y la salida estándar. Una vez registrado en Claude Code, Codex u otro cliente, la IA puede encontrar y usar Edictora cuando edite las fichas de su tienda.

**Setup registra Edictora automáticamente en Claude Code y Codex**, si sus CLI oficiales (`claude` y `codex`) están en `PATH`. La desinstalación elimina solo los registros que apuntan a Edictora.
- Si faltaba una CLI en el momento de la instalación, o el registro no terminó a tiempo, Edictora lo reintenta en su próximo inicio de sesión. Lo mismo ocurre si instala una CLI más tarde.
- Si usted elimina un registro después de que se añadiera, Edictora no lo vuelve a añadir.

Para la versión Portable, o para clientes distintos de esos dos (como Claude Desktop o una pasarela), regístrelo una vez con uno de los métodos siguientes.
Indique el exe instalado con una ruta absoluta:
- Versión Setup: `%LOCALAPPDATA%\Edictora\current\edictora.exe`. Esta ruta no cambia cuando las actualizaciones automáticas reemplazan los archivos.
- Versión Portable: `current\edictora.exe` en la carpeta donde la extrajo.

`edictora` en `PATH` también funciona, pero los clientes iniciados antes de la instalación no ven el nuevo `PATH`, así que una ruta absoluta es más segura.

**Claude Code** (para todos los proyectos)

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

O añádalo a `%USERPROFILE%\.codex\config.toml` (sustituya `<username>`):

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop y otros clientes configurados con un archivo JSON** (sustituya `<username>`):

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

Si usa una pasarela MCP (como Toolport), registre en ella el mismo comando (stdio, argumento `mcp`).

| Herramienta | Qué hace |
|---|---|
| `edictora_list_items` | Muestra lo que cubre el archivo de configuración (elementos, configuraciones regionales, archivos de origen). No abre un navegador. |
| `edictora_diff` | Muestra las diferencias con los archivos de origen (solo lectura) |
| `edictora_pull` | Muestra los valores actuales (solo lectura) |
| `edictora_push` | Reescribe los campos que difieren y los guarda como borrador. Sin `confirm: true`, muestra las diferencias y se detiene (una simulación). |
| `edictora_submit` | Envía un elemento a revisión. Se requieren `item` y `confirm: true`. |

- Las herramientas ejecutan el mismo código que la CLI, con las mismas salvaguardas y los mismos códigos de salida. Cada resultado empieza con el código de salida y su significado. Que se encuentren diferencias (3) no se trata como un error.
- Los clientes no pueden elegir mostrar el navegador (`--show-browser`), ejecutar `login` ni definir el perfil o chrome.exe. Chrome siempre se ejecuta sin ventana, salvo la ventana de inicio de sesión descrita a continuación.
- Si su inicio de sesión ha caducado, Edictora abre en su pantalla una ventana de Chrome para iniciar sesión, espera a que termine (hasta 10 minutos) y luego continúa con la llamada a la herramienta. Si no se completa, la herramienta le pide que ejecute `edictora login` en un terminal.
- Las llamadas se ejecutan de una en una, en orden. Puede cancelar una llamada en curso con `notifications/cancelled`.
- Un `diff` sobre todos los elementos puede tardar unos minutos, así que dé a su cliente un tiempo de espera generoso.

## Salvaguardas

- De forma predeterminada, Edictora solo muestra las diferencias y no cambia nada. Solo escribe con `push --yes` y solo envía a revisión con `submit --item <id> --yes`.
- Guardar un borrador y enviar a revisión son operaciones distintas. El envío se hace de un elemento en un elemento; no hay envío en bloque.
- Antes de guardar, Edictora comprueba que cada campo reescrito coincide exactamente con el archivo de origen. Después de guardar, vuelve a cargar la página y comprueba de nuevo. Si algo no coincide, no guarda o se detiene con un error.
- Para cada configuración regional, Edictora cambia el idioma, reescribe los campos, los verifica, guarda, vuelve a cargar y verifica otra vez. Si una configuración regional falla, se omite el resto de ese elemento.
  - No hay reversión entre configuraciones regionales: las que se guardaron antes del fallo permanecen guardadas como borradores.
  - Nada se hace público hasta que envía el elemento a revisión.
- Edictora no escribe cuando no se puede confirmar que el estado del elemento es seguro para editar, como «Pendiente de revisión». Nunca cancela una revisión.
- Los campos y los botones se encuentran por etiqueta, rol y texto, nunca por nombres de clase CSS. Si una búsqueda no coincide con exactamente un elemento, Edictora se detiene.
- Chrome se inicia sin indicadores de automatización y se cierra de forma limpia. **El perfil nunca se elimina.**
- Las capturas de pantalla tomadas al fallar se guardan en `%LOCALAPPDATA%\Edictora.Data\diagnostics`, nunca en su repositorio.
- Las actualizaciones de imágenes reemplazan solo las ubicaciones que difieren. No reordenan imágenes, y nunca pueden enviar a revisión (el código de imágenes no tiene forma de llamarlo).
- Las actualizaciones de la pestaña Privacidad solo cambian los campos que difieren, nunca envían a revisión (el código no tiene forma de llamarlo) y se detienen sin escribir nada si los permisos del panel no coinciden con la configuración.

## Dónde se guardan los archivos

| Contenido | Ubicación |
|---|---|
| Archivo de configuración (asociación de archivos de origen) | `%APPDATA%\Edictora\config.json` |
| Perfil de Chrome (incluye su inicio de sesión) | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| Registros (incluidos los resultados de actualización) | `%LOCALAPPDATA%\Edictora.Data\logs` |
| Capturas de pantalla al fallar | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| La propia aplicación (cuando se instala con Setup) | `%LOCALAPPDATA%\Edictora` (se elimina por completo al desinstalar) |

Los datos se guardan en `Edictora.Data`, separados de la aplicación, para que al desinstalar no se elimine también su perfil de Chrome con la sesión iniciada. Los datos de ubicaciones anteriores (como `%LOCALAPPDATA%\Edictora\chrome-profile`) no se migran automáticamente.

El perfil de Chrome contiene sus cookies de inicio de sesión. No lo comparta y elimínelo usted mismo cuando ya no lo necesite.
El ID del publicador (parte de la URL del panel) se oculta en los registros.

### Si la configuración parece faltar (aplicación de escritorio de Claude)

La aplicación de escritorio de Claude es una aplicación empaquetada (MSIX). Cuando un programa iniciado desde ella crea una carpeta **nueva** bajo `%APPDATA%` o `%LOCALAPPDATA%`, Windows la coloca en el almacenamiento privado de la aplicación (`%LOCALAPPDATA%\Packages\<paquete>\LocalCache\...`) en lugar de la ubicación real, y un terminal normal no puede verla. Edictora avisa por la salida de error cuando lee la configuración o el perfil de Chrome desde un lugar así y, si falta la configuración, lista las copias que encuentre allí. Para solucionarlo, mueva el archivo (o la carpeta) a la ubicación real indicada en el mensaje, por ejemplo `%APPDATA%\Edictora\config.json`. Edictora nunca lee ni mueve esas copias por su cuenta.

## Compilación

El programa principal está escrito en Rust (`crates/edictora/`).

```powershell
cargo build --release          # target\release\edictora.exe and edictora-updater.exe (Bun not needed)
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Los scripts que se ejecutan dentro de las páginas del panel están en `page-scripts/` (Bun + TypeScript). El archivo generado `page-scripts/dist/edictora.js` está incrustado en el exe, así que solo necesita Bun cuando cambie esos scripts.

```powershell
cd page-scripts
bun install --frozen-lockfile   # install dependencies (TypeScript only)
bun run typecheck      # type-check
bun run build          # regenerate dist/edictora.js
bun run check          # check that dist/edictora.js is up to date
```

**Expresiones regulares en `sectionStart` / `sectionEnd`**: Edictora usa el crate `regex` de Rust, así que no se admiten las aserciones de búsqueda hacia delante ni hacia atrás (`(?=…)`, `(?!…)`, `(?<=…)`, `(?<!…)`) ni las referencias inversas (`\1`); provocan un error al cargar la configuración. Los patrones habituales, como `^■ Description`, funcionan tal cual.

Para las pruebas, puede sustituir:
- La raíz de datos local con `EDICTORA_LOCAL_ROOT` (predeterminada: `%LOCALAPPDATA%\Edictora.Data`)
- La raíz de itinerancia con `EDICTORA_ROAMING_ROOT` (predeterminada: `%APPDATA%\Edictora`)
- El origen de las actualizaciones con `EDICTORA_UPDATE_URL` (predeterminado: `https://edictora.kagayoi.com`; el `http://` sin cifrar solo se permite para `127.0.0.1`)

El paquete de Velopack contiene tanto `edictora.exe` (el exe principal) como `edictora-updater.exe`.
- Las actualizaciones no usan el crate velopack: Edictora obtiene el feed, y descarga y verifica los paquetes por sí misma con WinHTTP, la API HTTP integrada en Windows.
- La instalación, el reemplazo de archivos y la desinstalación se dejan a Setup.exe y Update.exe de Velopack.

## Distribución

Las versiones se sirven desde Cloudflare R2 en `https://edictora.kagayoi.com` (canal de Velopack `win`). Consulte [docs/release.md](docs/release.md) para ver el proceso de publicación.

## Licencia

Consulte [LICENSE](LICENSE). Para la privacidad, consulte [PRIVACY_POLICY.md](PRIVACY_POLICY.md).
