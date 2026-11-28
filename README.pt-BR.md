<p align="center">
  <img src="icon/app_icon.png" alt="Ícone do Edictora" width="128">
</p>

# Edictora

[English](README.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | Português (Brasil)

**Uma ferramenta para Windows que lê, compara e atualiza em massa as suas listagens da Chrome Web Store (descrição e URLs), usando arquivos de origem mantidos no repositório de cada extensão.**

A API pública da Chrome Web Store (CWS) (v2) não permite ler nem gravar listagens da loja, e o Chrome não deixa extensões operarem nas páginas da CWS.
Por isso, o Edictora controla uma instância do Chrome com um perfil dedicado por meio do Chrome DevTools Protocol (CDP) e edita o Painel do desenvolvedor com as mesmas entradas que uma pessoa usaria.
Ele funciona como uma única ferramenta de linha de comando (`edictora.exe`). É escrito em Rust e não precisa de nenhum runtime, como .NET ou Node.js.

> [!IMPORTANT]
> O Edictora depende do layout do Painel do desenvolvedor. Ele foi verificado com o painel exibido em **japonês**.
> Painéis em inglês também são reconhecidos, mas os rótulos em inglês ainda não foram verificados em uma tela real.
> Se o Edictora não encontrar um campo ou botão (por exemplo, depois de uma reformulação do painel), ele para sem alterar nada.

## Recursos

- **pull**: lê os valores atuais do painel (descrição, URL oficial, URL da página inicial e URL de suporte). Somente leitura.
- **diff**: mostra as diferenças em relação aos seus arquivos de origem. Somente leitura e é o comando padrão.
- **push**: reescreve apenas os campos que diferem, **salva como rascunho**, depois recarrega a página e confere os valores salvos com os seus arquivos de origem.
- **Imagens**: se um item tiver `images` na configuração, `pull`, `diff` e `push` também tratam as imagens da página (ícone da loja, capturas de tela, bloco promocional pequeno e marquee). Restrinja com `--only text|images|privacy` e `--image-scope all|global|localized`.
- **Guia Privacidade**: se um item tiver `privacy` na configuração, `pull`, `diff` e `push` também tratam a guia «Privacidade» do painel (Privacy practices: descrição da finalidade única, justificativa de cada permissão e declaração de uso de dados). Restrinja com `--only privacy`. Salva apenas como rascunho; nunca envia para revisão.
- **submit**: envia um item para revisão. Não altera a configuração "publicar automaticamente após a revisão".
- **login**: abre o Chrome com o perfil dedicado para que você mesmo faça login no Google.
- **update**: baixa e instala uma versão mais recente, se houver. Só funciona quando você instalou o Edictora com o Setup, e também é executado automaticamente quando você faz login no Windows.

## Idioma de exibição

As mensagens, a saída de diff e o texto de ajuda do Edictora estão disponíveis em nove idiomas: japonês, inglês, chinês simplificado, chinês tradicional, coreano, espanhol, francês, alemão e português do Brasil.

- Por padrão, o Edictora decide o idioma nesta ordem: a variável de ambiente `EDICTORA_LANGUAGE`, depois `LC_ALL`, `LC_MESSAGES` e `LANG` (valores vazios, `C` e `POSIX` são ignorados), e por fim o idioma de exibição do Windows. Se nenhum deles indicar um idioma compatível, ele usa inglês.
- Para escolher um idioma explicitamente, defina `EDICTORA_LANGUAGE` como `ja`, `en`, `zh-CN`, `zh-TW`, `ko`, `es`, `fr`, `de` ou `pt-BR`. O nome antigo `EDICTORA_LANG` ainda é lido, mas `EDICTORA_LANGUAGE` tem prioridade.
- `edictora mcp` sempre responde em inglês, seja qual for o idioma de exibição, porque as descrições das ferramentas e as respostas são lidas por agentes de IA.
- Isso é independente do idioma do Painel do desenvolvedor (japonês ou inglês) e dos `locales` das suas listagens da loja.
- Os arquivos de log servem para a solução de problemas pelo autor, então suas mensagens fixas continuam em japonês. As mensagens de erro incluídas no log seguem o idioma de exibição.

## Instalação e atualização

Execute `Edictora-win-Setup.exe` (um instalador Velopack). Ele instala em `%LOCALAPPDATA%\Edictora` sem direitos de administrador.

- Depois de instalar, `edictora` funciona em qualquer janela nova do Prompt de Comando ou do PowerShell, porque o Setup adiciona `%LOCALAPPDATA%\Edictora\current` ao `Path` do seu usuário.
- Quando você faz login no Windows, o `edictora-updater.exe` é executado sem janela. Se `https://edictora.kagayoi.com` tiver uma versão mais recente, ele a baixa, confere o tamanho e o hash e, em seguida, a instala.
  - Ele é registrado como `Edictora Updater` em `Run` no HKCU, então você pode desativá-lo em "Aplicativos de inicialização" no Gerenciador de Tarefas.
  - O resultado é gravado no log.
- Para atualizar manualmente, execute `edictora update`. Ele informa pela saída de erro padrão e termina com código 0 quando você já está atualizado ou a atualização foi iniciada, e com código 1 se falhar. O Update.exe substitui os arquivos depois que o `edictora` termina.
- A versão Portable (`Edictora-win-Portable.zip`) não é atualizada automaticamente no login e não se adiciona ao `Path`. Para atualizá-la, execute `current\edictora.exe update` na pasta em que você a extraiu. Executáveis compilados a partir do código-fonte não se atualizam sozinhos.
- As atualizações nunca são aplicadas enquanto `login`, `pull`, `diff`, `push` ou `submit` estiver em execução. O Edictora conclui o download e aplica a atualização depois.
  - Se você executar um desses comandos enquanto uma atualização está sendo aplicada, ele termina com código 1 sem abrir o Chrome. Aguarde um momento e execute-o novamente.
  - Depois que o processo de atualização termina, o Update.exe ainda precisa de alguns segundos para substituir os arquivos, e o Edictora não consegue proteger esse intervalo. Logo depois de uma atualização, aguarde um momento antes de executar um comando.
- Desinstalar remove `%LOCALAPPDATA%\Edictora` e as entradas de `Run` e `Path`. Seus dados (`%LOCALAPPDATA%\Edictora.Data`) e configurações (`%APPDATA%\Edictora`) são mantidos, então exclua-os você mesmo se não precisar mais deles.

## Requisitos

- Windows 10 ou Windows 11 (x86_64)
- Google Chrome. O Edictora o encontra automaticamente, ou você pode definir `chromePath` no arquivo de configuração ou passar `--chrome`.
- Uma conta de desenvolvedor da CWS
- Rust 1.100.1 se você compilar a partir do código-fonte (fixado em `rust-toolchain.toml`). O Bun só é necessário quando você altera os scripts executados dentro das páginas.

## Uso

### 1. Escreva o arquivo de configuração

O local padrão é `%APPDATA%\Edictora\config.json`; você pode alterá-lo com `--config`.
Para cada item, associe cada idioma aos seus arquivos de origem. **Os campos que você omitir nunca são lidos, comparados nem gravados.**

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

- Se você informar apenas `description.file`, o arquivo inteiro será usado como descrição.
- Quando uma seção é extraída, o Edictora pega as linhas depois do título inicial e antes do título final. Ele remove as linhas em branco do começo e do fim e os espaços em branco no final das linhas, e normaliza as quebras de linha para `\n`. Se um título não for encontrado, ele para com um erro.
- Um campo de URL sem chave é ignorado. `""` significa que o campo deve ficar vazio.
- Chaves desconhecidas (como erros de digitação) são erros. Comentários e vírgulas finais são permitidos.

#### Categoria (opcional)

Adicione `category` a um item para ler, comparar e salvar a categoria da página (o campo “Categoria” do painel, comum a todos os idiomas). Escreva uma chave em minúsculas. Como em qualquer outro campo, **se você a omitir, a categoria nunca é alterada**.

```jsonc
"category": "productivity"    // one of the keys below; leave it out to skip the category
```

- Chaves: `productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`. Uma chave desconhecida é um erro de configuração (código de saída 2). “Nenhuma categoria selecionada” não pode ser um valor de origem.
- A categoria é comum a todos os idiomas, então é lida e gravada uma única vez por item (não uma vez por idioma). Ela é incluída em `--only text`, e `--locale` não a restringe. `push --yes` só a grava quando há diferença, salva um rascunho, recarrega e confere de novo.
- Um item pode ter `category` sem `locales` (somente a categoria).
- Só funciona enquanto o painel é exibido em japonês (os textos em inglês não foram verificados).

#### Imagens da página (opcional)

Adicione `images` a um item para gerenciar as imagens dele. Como nos demais campos, **as chaves que você não escrever nunca são tocadas**. Os caminhos são relativos a `baseDir`.

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

- `dir` usa os arquivos `screenshot-*.png` diretamente na pasta, em ordem de nome (1–5 arquivos). `files` usa os arquivos na ordem em que você os escrever (1–5).
- Só há suporte a PNG de 24 bits sem alfa, com 1280x800 ou 640x400 (capturas), 440x280 (bloco pequeno) ou 1400x560 (marquee). O Edictora verifica isso, e que nenhuma imagem se repita dentro de um conjunto, antes de abrir o Chrome (código de saída 2). O ícone da loja (`icon`) é a exceção: deve ter 128x128 e pode ser um PNG transparente (RGB ou RGBA de 8 bits). A comparação inclui o alfa e ignora a cor dos pixels totalmente transparentes. Ele é comum a todos os idiomas, então `--image-scope global` o inclui.
- Um item pode ter `images` sem `locales` (somente imagens).
- **As imagens são comparadas pelos pixels decodificados, não pelos bytes do arquivo.** O painel recodifica os PNG enviados, então o arquivo guardado nunca tem os mesmos bytes do seu, embora os pixels sejam idênticos. O Edictora baixa cada imagem do painel (o original `=s0`, somente por HTTPS a partir de `*.googleusercontent.com` e sem cookies) e compara os hashes dos pixels em ordem. Se uma imagem não puder ser baixada ou decodificada, o resultado é "não é possível comparar": nunca "igual" nem "substituir".
- `push --yes` substitui um local inteiro (apaga tudo e envia em ordem) somente se ele diferir, salva um rascunho, recarrega e confere de novo. A ordem é: ícone da loja, bloco pequeno, marquee, capturas globais e, depois, as capturas localizadas de cada idioma.
- **Excluir uma imagem tem efeito imediato no painel** (ele pergunta "isso não pode ser desfeito" e, ao recarregar sem salvar, o local continuava vazio). Se um push parar no meio, o rascunho pode ficar sem algumas imagens: execute-o de novo para concluir. O Edictora nunca mexe em um item pendente de revisão.
- Um item com um único idioma não tem capturas localizadas; use `--image-scope global`. Se um idioma de `localizedScreenshots` não estiver no painel, o Edictora para antes de alterar qualquer coisa: use `--image-scope global` ou `--locale`.
- A gravação de imagens só funciona com o painel em japonês (os rótulos em inglês não foram verificados).

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

#### Guia Privacidade (opcional)

Ao adicionar uma permissão a uma extensão, a guia **Privacidade** do painel (Privacy practices) exige uma justificativa antes que o item possa ser enviado para revisão. Adicione `privacy` a um item para ler, comparar e salvar esses campos como rascunho a partir dos seus arquivos de origem. Como nos demais campos, **as chaves que você não escrever nunca são tocadas**.

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

- O texto é extraído do arquivo como em `description` (veja acima). As linhas em branco iniciais e o recuo comum são removidos, então justificativas recuadas sob `  activeTab:` funcionam como estão. Nenhum texto pode ficar vazio e todos devem caber em 1.000 caracteres; o Edictora verifica isso antes de abrir o Chrome (código de saída 2).
- `collected` aceita `personallyIdentifiable`, `health`, `financial`, `authentication`, `personalCommunications`, `location`, `webHistory`, `userActivity` e `websiteContent` (só os indicados são marcados; os demais são desmarcados). `certify` só pode ser `true` (marca as três declarações, como «não vendo…»); `false` ou omiti-lo é um erro de configuração.
- **O conjunto de permissões também é conferido.** Se você escrever `permissions`, uma permissão que está na configuração mas não no painel (ou o contrário) aparece no resultado e o `push` para antes de gravar qualquer coisa: adicione-a à configuração ou corrija o manifest.
- `policyUrl` é o campo da URL da política de privacidade: uma URL http(s) absoluta (até 2.048 caracteres) ou `""` para indicar que o campo deve ficar vazio. Uma chave omitida nunca é alterada. Ela é salva junto com os outros campos desta guia.
- `remoteCode` (`{ "uses": false }` ou `{ "uses": true, "justification": {...} }`) é aceito na configuração, mas a seção correspondente do painel ainda não pôde ser conferida em uma tela real; se você o escrever, o `diff` relata um erro e o `push` para. Por enquanto, não o escreva.
- Só funciona com o painel em japonês (os rótulos em inglês não foram verificados). Salva somente como rascunho; o Edictora nunca envia para revisão.
- Um item pode ter `privacy` sem `locales` nem `images` (somente a guia Privacidade).

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```

### 2. Faça login

```powershell
edictora login
```

O Chrome abre com o perfil dedicado. Faça login no Google nele e abra o Painel do desenvolvedor.
- Assim que o Edictora vê o painel, ele mantém o login no perfil e encerra.
- Ele espera até 10 minutos por padrão. Use `--wait <minutes>` para definir de 1 a 120 minutos.
- O Edictora nunca lida com a sua senha nem com códigos de verificação; você mesmo os digita.

### 3. Compare e atualize

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

| Código de saída | Significado |
|---|---|
| 0 | Sucesso (sem diferenças) |
| 1 | Erro de execução (falha em uma operação do painel, divergência na verificação etc.) |
| 2 | Argumentos ou configuração inválidos (o Chrome não é aberto) |
| 3 | Diferenças encontradas (`diff`, ou `push` sem `--yes`) |
| 4 | Gravação interrompida (revisão pendente ou não foi possível determinar o estado) |
| 5 | Login necessário (a espera automática pelo login não foi concluída, ou o login expirou no meio do trabalho; ao executar de novo, uma janela de login é aberta se necessário) |
| 130 | Interrompido (Ctrl+C) |

O andamento vai para a saída de erro padrão e os resultados vão para a saída padrão.

`pull`, `diff`, `push` e `submit` executam o Chrome sem janela (headless), então o Chrome nunca aparece na frente do seu trabalho.
Para acompanhar o que acontece, adicione `--show-browser`. O `login` sempre mostra uma janela, porque você o opera manualmente. Se o login do Google expirou, `pull`, `diff`, `push` e `submit` abrem por conta própria uma janela do Chrome, aguardam você entrar (até 10 minutos), fecham a janela e continuam. Terminam com o código de saída 5 se esse login não for concluído (tempo esgotado, janela fechada ou um Chrome sem janela com o mesmo perfil impedindo a exibição) ou se o login expirar no meio do trabalho (nesse caso nenhuma janela é aberta; execute de novo).

> [!WARNING]
> `--show-browser` é para quem acompanha a tela. Não o use ao executar o Edictora a partir de um agente de IA ou de um script: uma janela do Chrome apareceria na tela do usuário, e a IA não conseguiria vê-la de qualquer forma.
> Para encontrar a causa de uma falha, use a mensagem de erro e o instantâneo de texto da página salvo na falha.
> - A mensagem de erro inclui o elemento que cobre o alvo, os limites do elemento e o tamanho da área visível.
> - Os instantâneos da página (arquivos de texto que listam, linha por linha, a função, o nome, o valor, o estado e a posição de cada elemento visível) são salvos em `%LOCALAPPDATA%\Edictora.Data\diagnostics`, e a mensagem de erro mostra o caminho. Um instantâneo também é salvo quando `pull` ou `diff` não consegue ler o painel. O Edictora não faz capturas de tela.

### 4. Uso a partir de agentes de IA (MCP)

`edictora mcp` é um servidor MCP que funciona pela entrada e saída padrão. Depois de registrado no Claude Code, no Codex ou em outro cliente, a IA consegue encontrar e usar o Edictora quando edita as suas listagens da loja.

**O Setup registra o Edictora automaticamente no Claude Code e no Codex**, se as CLIs oficiais deles (`claude` e `codex`) estiverem no `PATH`. A desinstalação remove apenas os registros que apontam para o Edictora.
- Se uma CLI estava ausente na hora da instalação, ou o registro não terminou a tempo, o Edictora tenta de novo no seu próximo login. O mesmo vale se você instalar uma CLI depois.
- Se você remover um registro por conta própria depois de ele ter sido adicionado, o Edictora não o adiciona novamente.

Para a versão Portable, ou para clientes diferentes desses dois (como o Claude Desktop ou um gateway), faça o registro uma única vez por um dos métodos abaixo.
Informe o exe instalado como caminho absoluto:
- Versão Setup: `%LOCALAPPDATA%\Edictora\current\edictora.exe`. Esse caminho continua o mesmo quando as atualizações automáticas substituem os arquivos.
- Versão Portable: `current\edictora.exe` na pasta em que você a extraiu.

Usar `edictora` no `PATH` também funciona, mas os clientes iniciados antes da instalação não enxergam o novo `PATH`, então um caminho absoluto é mais seguro.

**Claude Code** (para todos os projetos)

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

Ou adicione ao arquivo `%USERPROFILE%\.codex\config.toml` (substitua `<username>`):

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop e outros clientes configurados com um arquivo JSON** (substitua `<username>`):

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

Se você usa um gateway MCP (como o Toolport), registre o mesmo comando (stdio, argumento `mcp`) no gateway.

**Toolport e o registro automático (versão Setup)**
- A verificação é feita em qualquer sistema operacional (Windows, macOS e Linux); o próprio Setup é distribuído atualmente apenas para Windows.
- Para o Claude Code e o Codex separadamente, o Setup pula `claude mcp add` / `codex mcp add` quando as duas condições a seguir são atendidas: (1) as configurações desse cliente contêm o gateway do Toolport (`mcpServers` em `~/.claude.json`, ou `~/.codex/config.toml`), e (2) o registry do Toolport (`%APPDATA%\Toolport\registry.json`; o local pode ser alterado com `TOOLPORT_REGISTRY` / `TOOLPORT_DATA_DIR`) tem um servidor habilitado que inicia um `edictora.exe` com o argumento `mcp`, em um perfil em uso. Se for o mesmo arquivo que o Setup registra (comparado como o mesmo arquivo), o motivo anotado é `toolport`; se for outro arquivo, `toolport-foreign`; nos dois casos, o Setup pula o registro direto. Se isso não puder ser determinado, o Edictora registra diretamente como antes. O Edictora faz essa verificação no Windows, macOS e Linux (local padrão do registry: Windows `%APPDATA%\Toolport\registry.json`, macOS `~/Library/Application Support/Toolport/registry.json`, Linux `$XDG_CONFIG_HOME/Toolport/registry.json` quando for um caminho absoluto; caso contrário, `~/.config/Toolport/registry.json`); em qualquer outro sistema, não faz a verificação e registra diretamente.
- Um registro que já existe nunca é removido. Se mais tarde você remover o gateway do Toolport desse cliente, ou o executável para o qual o Toolport aponta deixar de existir, o Edictora adiciona o registro direto na próxima atualização ou no próximo início de sessão (o executável de atualização que roda ao iniciar a sessão). Se você desabilitar ou excluir o Edictora no Toolport, o Edictora não o adiciona. Um cliente que o Setup já registrou diretamente uma vez não é adicionado de novo automaticamente, mesmo que você remova o registro; só é adicionado de volta um cliente que o Setup pulou.
- Se você adicionar o Toolport depois e quiser remover apenas o registro direto, execute `claude mcp remove edictora --scope user` ou `codex mcp remove edictora`.
- Se o Toolport fornece um `edictora.exe` diferente (não o que o Setup registra), o Setup também pula o registro direto, anota o motivo `toolport-foreign` e grava um aviso em `%LOCALAPPDATA%\Edictora.Data\logs\Edictora_yyyy-MM-dd.log`. Para corrigir, aponte o `command` do servidor `edictora` no Toolport para o executável da versão Setup (`%LOCALAPPDATA%\Edictora\current\edictora.exe`). Não copie o executável para outro local; uma cópia não é atualizada. A verificação é refeita a cada Setup, atualização e login: quando o Toolport passa a apontar para o executável do Setup, o registro muda para `toolport`, e se o executável apontado pelo Toolport desaparecer, o Edictora registra diretamente.
- A quarentena de ferramentas do Toolport (as ferramentas ficam em quarentena quando uma atualização as altera) não é verificada.
- Você pode escolher o comportamento com `mcpRegistration` em `%APPDATA%\Edictora\config.json`: `auto` (padrão; o comportamento acima), `direct` (sempre registrar diretamente) ou `off` (nunca registrar nem reparar automaticamente; a desinstalação continua removendo os registros que apontam para o Edictora). O instalador lê apenas o caminho de configuração padrão, não uma configuração indicada com `--config`. Os demais comandos relatam qualquer outro valor como erro de configuração. O instalador trata um valor inválido, ou um arquivo de configuração que não é JSON válido, como `auto` e o registra no log.

| Ferramenta | O que faz |
|---|---|
| `edictora_list_items` | Mostra o que o arquivo de configuração abrange (itens, idiomas, arquivos de origem). Não abre um navegador. |
| `edictora_diff` | Mostra as diferenças em relação aos arquivos de origem (somente leitura) |
| `edictora_pull` | Mostra os valores atuais (somente leitura) |
| `edictora_push` | Reescreve os campos que diferem e os salva como rascunho. Sem `confirm: true`, mostra as diferenças e para (uma simulação). |
| `edictora_submit` | Envia um item para revisão. `item` e `confirm: true` são obrigatórios. |

- As ferramentas executam o mesmo código da CLI, com as mesmas salvaguardas e os mesmos códigos de saída. Cada resultado começa com o código de saída e o seu significado. Diferenças encontradas (3) não é tratado como erro.
- Os clientes não podem optar por mostrar o navegador (`--show-browser`), executar `login` nem definir o perfil ou chrome.exe. O Chrome sempre é executado sem janela, exceto a janela de login descrita a seguir.
- Se o seu login expirou, o Edictora abre na sua tela uma janela do Chrome para o login, aguarda você concluir (até 10 minutos) e então continua a chamada da ferramenta. Se não for concluído, a ferramenta pede que você execute `edictora login` em um terminal.
- As chamadas são executadas uma por vez, em ordem. Você pode cancelar uma chamada em andamento com `notifications/cancelled`.
- Um `diff` em todos os itens pode levar alguns minutos, então dê ao seu cliente um tempo limite generoso.

## Salvaguardas

- Por padrão, o Edictora apenas mostra as diferenças e não altera nada. Ele só grava com `push --yes` e só envia com `submit --item <id> --yes`.
- Salvar um rascunho e enviar para revisão são operações separadas. O envio é feito um item por vez; não há envio em massa.
- Antes de salvar, o Edictora confere se cada campo reescrito coincide exatamente com o arquivo de origem. Depois de salvar, ele recarrega a página e confere novamente. Se algo não coincidir, ele não salva ou para com um erro.
- Para cada idioma, o Edictora troca o idioma, reescreve os campos, verifica, salva, depois recarrega e verifica de novo. Se um idioma falhar, o restante desse item é ignorado.
  - Não há reversão entre idiomas: os idiomas salvos antes da falha continuam salvos como rascunhos.
  - Nada se torna público até que você envie o item para revisão.
- O Edictora não grava quando não é possível confirmar que a situação do item é segura para edição, como "Revisão pendente". Ele nunca cancela uma revisão.
- Os campos e botões são encontrados por rótulo, função e texto, nunca por nomes de classe CSS. Se uma busca não corresponder a exatamente um elemento, o Edictora para.
- O Chrome é iniciado sem flags de automação e encerrado de forma limpa. **O perfil nunca é excluído.**
- Os instantâneos de texto da página feitos em caso de falha são salvos em `%LOCALAPPDATA%\Edictora.Data\diagnostics`, nunca no seu repositório. Podem conter os valores dos campos da página do painel (o publisherId e o nome e o endereço de e-mail da sua conta do Google são ocultados).
- As atualizações de imagens substituem apenas os locais que diferem. Não reordenam imagens, e nunca podem enviar para revisão (o código de imagens não tem como chamá-lo).
- As atualizações da guia Privacidade alteram apenas os campos que diferem, nunca enviam para revisão (o código não tem como chamá-lo) e param sem gravar nada se as permissões do painel não corresponderem à configuração.

## Onde os arquivos ficam

| Conteúdo | Local |
|---|---|
| Arquivo de configuração (mapeamento dos arquivos de origem) | `%APPDATA%\Edictora\config.json` |
| Perfil do Chrome (inclui o seu login) | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| Logs (incluindo resultados de atualização) | `%LOCALAPPDATA%\Edictora.Data\logs` |
| Instantâneos de texto da página em falhas | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| O próprio aplicativo (quando instalado com o Setup) | `%LOCALAPPDATA%\Edictora` (removido por completo na desinstalação) |

Os dados ficam em `Edictora.Data`, separados do aplicativo, para que desinstalar não exclua também o seu perfil do Chrome com login. Os dados em locais mais antigos (como `%LOCALAPPDATA%\Edictora\chrome-profile`) não são migrados automaticamente.

O perfil do Chrome contém os seus cookies de login. Não o compartilhe e exclua-o você mesmo quando não precisar mais dele.
O ID do publicador (parte da URL do painel) é ocultado nos logs.

### Se a configuração parecer ausente (aplicativo de desktop do Claude)

O aplicativo de desktop do Claude é um aplicativo empacotado (MSIX). Quando um programa iniciado dentro dele cria uma pasta **nova** em `%APPDATA%` ou `%LOCALAPPDATA%`, o Windows a coloca no armazenamento privado do aplicativo (`%LOCALAPPDATA%\Packages\<pacote>\LocalCache\...`) em vez do local real, e um terminal normal não consegue vê-la. O Edictora avisa na saída de erro quando lê a configuração ou o perfil do Chrome de um lugar assim e, se a configuração estiver ausente, lista as cópias encontradas lá. Para corrigir, mova o arquivo (ou a pasta) para o local real indicado na mensagem, por exemplo `%APPDATA%\Edictora\config.json`. O Edictora nunca lê nem move essas cópias por conta própria.

## Compilação

O programa principal é escrito em Rust (`crates/edictora/`).

```powershell
cargo build --release          # target\release\edictora.exe and edictora-updater.exe (Bun not needed)
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Os scripts executados dentro das páginas do painel ficam em `page-scripts/` (Bun + TypeScript). O arquivo gerado `page-scripts/dist/edictora.js` é embutido no exe, então você só precisa do Bun quando alterar esses scripts.

```powershell
cd page-scripts
bun install --frozen-lockfile   # install dependencies (TypeScript only)
bun run typecheck      # type-check
bun run build          # regenerate dist/edictora.js
bun run check          # check that dist/edictora.js is up to date
bun test               # run the page-script tests (fake DOM)
```

**Expressões regulares em `sectionStart` / `sectionEnd`**: o Edictora usa o crate `regex` do Rust, então lookahead, lookbehind (`(?=…)`, `(?!…)`, `(?<=…)`, `(?<!…)`) e referências inversas (`\1`) não são compatíveis; eles causam um erro quando a configuração é carregada. Padrões comuns, como `^■ Description`, funcionam como estão.

Para testes, você pode substituir:
- A raiz dos dados locais com `EDICTORA_LOCAL_ROOT` (padrão `%LOCALAPPDATA%\Edictora.Data`)
- A raiz dos dados de roaming com `EDICTORA_ROAMING_ROOT` (padrão `%APPDATA%\Edictora`)
- A origem das atualizações com `EDICTORA_UPDATE_URL` (padrão `https://edictora.kagayoi.com`; `http://` simples é permitido apenas para `127.0.0.1`)

O pacote Velopack contém tanto o `edictora.exe` (o exe principal) quanto o `edictora-updater.exe`.
- As atualizações não usam o crate velopack: o Edictora obtém o feed, baixa e verifica os pacotes por conta própria com o WinHTTP, a API HTTP incluída no Windows.
- A instalação, a substituição de arquivos e a desinstalação ficam a cargo do Setup.exe e do Update.exe do Velopack.

## Distribuição

As versões são servidas pelo Cloudflare R2 em `https://edictora.kagayoi.com` (canal Velopack `win`). Consulte [docs/release.md](docs/release.md) para o processo de lançamento.

## Licença

Consulte [LICENSE](LICENSE). Para a privacidade, consulte [PRIVACY_POLICY.md](PRIVACY_POLICY.md).
