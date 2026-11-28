<p align="center">
  <img src="icon/app_icon.png" alt="Edictora 图标" width="128">
</p>

# Edictora

[English](README.md) | [日本語](README.ja.md) | 简体中文 | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**一款 Windows 工具，可根据保存在各扩展程序仓库中的源文件，批量读取、比较并更新您的 Chrome Web Store 商品详情（说明和 URL）。**

公开的 Chrome Web Store（CWS）API（v2）无法读取或写入商品详情，Chrome 也不允许扩展程序操作 CWS 页面。
因此 Edictora 通过 Chrome DevTools Protocol（CDP）驱动使用专用配置文件的 Chrome 实例，并以与真人相同的输入方式编辑开发者信息中心。
它是一个单一的命令行工具（`edictora.exe`），使用 Rust 编写，不需要 .NET 或 Node.js 等运行时。

> [!IMPORTANT]
> Edictora 依赖开发者信息中心的页面布局。它是在信息中心以**日语**显示的情况下验证的。
> 英语信息中心也能识别，但英语的界面标签尚未在真实屏幕上验证。
> 如果 Edictora 找不到某个字段或按钮（例如信息中心改版之后），它会在不做任何更改的情况下停止。

## 功能

- **pull**：从信息中心读取当前值（说明、官方 URL、主页 URL 和支持 URL）。只读。
- **diff**：显示与源文件的差异。只读，也是默认命令。
- **push**：仅重写有差异的字段，并**保存为草稿**，然后重新加载页面，将保存的值与源文件进行核对。
- **图片**：如果配置中的项目写了 `images`，`pull`、`diff`、`push` 也会处理商店图片（商店图标、截图、小型宣传图块、宣传横幅）。可用 `--only text|images|privacy` 和 `--image-scope all|global|localized` 缩小范围。
- **“隐私”标签页**：如果配置中的项目写了 `privacy`，`pull`、`diff`、`push` 也会处理控制台的“隐私”标签页（Privacy practices：单一用途说明、各权限的理由、数据使用披露）。可用 `--only privacy` 缩小范围。只保存为草稿，不会提交审核。
- **submit**：将一个项目提交审核。不会更改“审核后自动发布”设置。
- **login**：使用专用配置文件打开 Chrome，让您自行登录 Google。
- **update**：如有新版本，则下载并安装。仅在您通过 Setup 安装 Edictora 时有效，并且会在您登录 Windows 时自动运行。

## 显示语言

Edictora 的消息、差异输出和帮助文本提供九种语言：日语、英语、简体中文、繁体中文、韩语、西班牙语、法语、德语和巴西葡萄牙语。

- 默认情况下，Edictora 按以下顺序决定语言：环境变量 `EDICTORA_LANGUAGE`，然后是 `LC_ALL`、`LC_MESSAGES` 和 `LANG`（空值、`C` 和 `POSIX` 会被跳过），最后是 Windows 显示语言。如果都没有给出受支持的语言，则使用英语。
- 要明确指定语言，请将 `EDICTORA_LANGUAGE` 设置为 `ja`、`en`、`zh-CN`、`zh-TW`、`ko`、`es`、`fr`、`de` 或 `pt-BR` 之一。旧名称 `EDICTORA_LANG` 仍会被读取，但 `EDICTORA_LANGUAGE` 优先。
- `edictora mcp` 无论显示语言是什么，始终以英语回答，因为它的工具说明和响应是由 AI 代理读取的。
- 这与开发者信息中心的语言（日语或英语）以及您商品详情的 `locales` 无关。
- 日志文件供作者排查问题使用，因此其中的固定消息保持为日语。日志中包含的错误消息则遵循显示语言。

## 安装与更新

运行 `Edictora-win-Setup.exe`（Velopack 安装程序）。它无需管理员权限，会安装到 `%LOCALAPPDATA%\Edictora`。

- 安装后，`edictora` 可在任何新打开的命令提示符或 PowerShell 窗口中使用，因为 Setup 会把 `%LOCALAPPDATA%\Edictora\current` 添加到您的用户 `Path`。
- 当您登录 Windows 时，`edictora-updater.exe` 会在无窗口状态下运行。如果 `https://edictora.kagayoi.com` 上有更新的版本，它会下载该版本，检查其大小和哈希值，然后进行安装。
  - 它以 `Edictora Updater` 为名注册在 HKCU 的 `Run` 下，因此您可以在任务管理器的“启动应用”中将其关闭。
  - 结果会写入日志。
- 要手动更新，请运行 `edictora update`。它会向标准错误输出报告；当您已是最新版本或更新已开始时以退出码 0 退出，失败时以退出码 1 退出。在 `edictora` 退出后，Update.exe 会替换文件。
- Portable 版（`Edictora-win-Portable.zip`）不会在登录时自动更新，也不会把自己添加到 `Path`。要更新它，请在您解压的文件夹中运行 `current\edictora.exe update`。从源代码构建的可执行文件不会自行更新。
- 当 `login`、`pull`、`diff`、`push` 或 `submit` 正在运行时，绝不会应用更新。Edictora 会先完成下载，稍后再应用更新。
  - 如果在应用更新期间运行这些命令之一，它会在不打开 Chrome 的情况下以退出码 1 退出。请稍候，然后重新运行。
  - 更新进程退出后，Update.exe 还需要几秒钟来替换文件，Edictora 无法保护这段间隙。更新之后，请稍等片刻再运行命令。
- 卸载会移除 `%LOCALAPPDATA%\Edictora` 以及 `Run` 和 `Path` 条目。您的数据（`%LOCALAPPDATA%\Edictora.Data`）和设置（`%APPDATA%\Edictora`）会被保留，如果不再需要，请自行删除。

## 系统要求

- Windows 10 或 Windows 11（x86_64）
- Google Chrome。Edictora 会自动查找它，您也可以在配置文件中设置 `chromePath`，或传入 `--chrome`。
- CWS 开发者账号
- 如果从源代码构建，需要 Rust 1.100.1（在 `rust-toolchain.toml` 中固定）。只有在修改页面内脚本时才需要 Bun。

## 使用方法

### 1. 编写配置文件

默认位置是 `%APPDATA%\Edictora\config.json`；您可以用 `--config` 更改。
对于每个项目，将每个语言区域映射到它的源文件。**您省略的字段绝不会被读取、比较或写入。**

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

- 如果只给出 `description.file`，则整个文件都会用作说明。
- 提取区段时，Edictora 取起始标题之后、结束标题之前的行。它会去除首尾的空行和行尾空白，并将换行符统一为 `\n`。如果找不到标题，则报错停止。
- 没有键的 URL 字段会被忽略。`""` 表示该字段应为空。
- 未知的键（例如拼写错误）会报错。允许使用注释和尾随逗号。

#### 类别（可选）

在项目中添加 `category`，即可读取、比较并保存商店的类别（信息中心的“类别”字段，所有语言共用）。请写小写的键。与其他字段一样，**不写就不会改动类别**。

```jsonc
"category": "productivity"    // one of the keys below; leave it out to skip the category
```

- 键：`productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`。未知的键属于配置错误（退出代码 2）。“未选择类别”不能作为源值。
- 类别由所有语言共用，因此每个项目只读写一次（而不是每种语言一次）。它包含在 `--only text` 中，`--locale` 不会缩小其范围。`push --yes` 仅在有差异时才改写，保存为草稿，重新加载后再次核对。
- 项目可以只有 `category` 而没有 `locales`（仅管理类别）。
- 仅在信息中心以日语显示时可用（英语界面的文案尚未验证）。

#### 商店图片（可选）

在项目中添加 `images` 即可管理商店图片。与其他字段一样，**没有写的键不会被触碰**。路径相对于 `baseDir`。

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

- `dir` 按文件名升序使用文件夹直下的 `screenshot-*.png`（1–5 张）。`files` 按书写顺序使用（1–5 张）。
- 仅支持 PNG：24 位、无 Alpha，截图为 1280x800 或 640x400，小型图块为 440x280，横幅为 1400x560。Edictora 会在打开 Chrome 之前检查这些规格以及同一组内是否有重复图片（退出码 2）。但商店图标（`icon`）是例外：必须是 128x128，可以是带透明的 PNG（8 位 RGB 或 RGBA）。比较时包含 Alpha，完全透明的像素的颜色会被忽略。它由所有语言共用，所以 `--image-scope global` 包含它。
- 项目可以只写 `images` 而不写 `locales`（仅图片）。
- **图片按解码后的像素比较，而不是按文件字节。** 控制台会重新编码上传的 PNG，所以保存的文件字节与原文件不同，但像素相同。Edictora 会下载控制台上的每张图片（`=s0` 原图，仅通过 HTTPS 访问 `*.googleusercontent.com`，不带 Cookie），并按顺序比较像素哈希。无法下载或解码的图片结果为“无法比较”，既不视为一致，也不会替换。
- `push --yes` 只在有差异时整体替换一个位置（先全部删除，再按顺序上传），然后保存草稿、重新载入并再次核对。处理顺序为：商店图标 → 小型图块 → 横幅 → 全语言截图 → 各语言的本地化截图。
- **删除图片会立即在控制台生效**（会提示“此操作无法撤销”，且未保存就重新载入后该位置仍为空）。如果 push 中途停止，草稿中可能缺少图片：再次运行即可补全。待审核的项目不会被触碰。
- 只有一种语言的项目没有本地化截图，请使用 `--image-scope global`。如果 `localizedScreenshots` 中的语言不在控制台上，Edictora 会在更改任何内容之前停止：请使用 `--image-scope global` 或 `--locale`。
- 仅当控制台以日语显示时才能写入图片（英文文案尚未确认）。

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

#### “隐私”标签页（可选）

为扩展程序添加权限后，必须在控制台的**“隐私”标签页**（Privacy practices）中填写权限理由等内容，才能提交审核。在项目中添加 `privacy`，即可从源文件读取、比较并以草稿保存这些字段。与其他字段一样，**没有写的键不会被触碰**。

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

- 正文与 `description` 一样，从文件中截取区段（见上文）。会去掉开头的空行，并去掉所有行共有的缩进，因此缩进写在 `  activeTab:` 下面的理由也可以直接使用。每段正文不能为空，且不超过 1,000 个字符；Edictora 会在打开 Chrome 之前检查（退出码 2）。
- `collected` 可写 `personallyIdentifiable`、`health`、`financial`、`authentication`、`personalCommunications`、`location`、`webHistory`、`userActivity`、`websiteContent`（只勾选写出的，其余取消勾选）。`certify` 只能为 `true`（表示勾选“不出售”等三项声明）；`false` 或省略都会报配置错误。
- **权限列表本身也会核对。** 写了 `permissions` 时，配置中有而控制台上没有的权限（或相反）会显示在比较结果中，`push` 会在写入任何内容之前停止：请在配置中补上，或修正 manifest。
- `policyUrl` 是隐私权政策网址字段：http(s) 绝对 URL（最多 2,048 个字符），或表示应为空的 `""`。不写则不会改动。它与此标签页的其他字段一起保存为草稿。
- `remoteCode`（`{ "uses": false }`，或 `{ "uses": true, "justification": {...} }`）可以写在配置中，但控制台上对应的字段尚未在实际界面中确认，因此写了之后 `diff` 会报告错误，`push` 会停止。目前请不要写。
- 仅当控制台以日语显示时可用（英文文案尚未确认）。只保存为草稿，Edictora 绝不会提交审核。
- 项目可以只写 `privacy` 而不写 `locales` 和 `images`（仅“隐私”标签页）。

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```

### 2. 登录

```powershell
edictora login
```

Chrome 会使用专用配置文件打开。请在其中登录 Google 并打开开发者信息中心。
- 一旦 Edictora 看到信息中心，就会把登录状态保存在配置文件中并退出。
- 默认最多等待 10 分钟。可用 `--wait <minutes>` 设置为 1–120 分钟。
- Edictora 绝不会处理您的密码或验证码；这些都由您自己输入。

### 3. 比较与更新

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

| 退出码 | 含义 |
|---|---|
| 0 | 成功（无差异） |
| 1 | 运行时错误（信息中心操作失败、验证不一致等） |
| 2 | 参数或配置无效（不会打开 Chrome） |
| 3 | 发现差异（`diff`，或未带 `--yes` 的 `push`） |
| 4 | 写入已中止（审核中，或无法确定状态） |
| 5 | 需要登录（自动等待登录未完成，或工作途中登录已过期；再次运行时，如有需要会打开登录窗口） |
| 130 | 已中断（Ctrl+C） |

进度输出到标准错误，结果输出到标准输出。

`pull`、`diff`、`push` 和 `submit` 在无窗口（无头）状态下运行 Chrome，因此 Chrome 绝不会跳到您的工作窗口前面。
如果想观看运行过程，请添加 `--show-browser`。`login` 始终显示窗口，因为需要您手动操作。 如果 Google 登录已过期，`pull`、`diff`、`push` 和 `submit` 会自行打开带窗口的 Chrome，等待您完成登录（最长 10 分钟），然后关闭窗口并继续。登录未完成（超时、窗口被关闭，或同一配置文件下仍有无窗口的 Chrome 导致无法显示窗口），或工作途中登录过期（此时不会打开窗口，请再次运行）时，会以退出代码 5 停止。

> [!WARNING]
> `--show-browser` 供人观看屏幕时使用。通过 AI 代理或脚本运行 Edictora 时请勿使用：Chrome 窗口会弹出到用户的屏幕上，而且 AI 本来也看不到它。
> 要查明失败原因，请使用错误消息和失败时保存的页面文本快照。
> - 错误消息包含覆盖元素、元素的边界以及视口大小。
> - 页面快照（按行列出每个可见元素的角色、名称、值、状态和位置的文本文件）保存到 `%LOCALAPPDATA%\Edictora.Data\diagnostics`，错误消息会显示路径。当 `pull` 或 `diff` 读取信息中心失败时，也会保存快照。Edictora 不会截取屏幕截图。

### 4. 从 AI 代理使用（MCP）

`edictora mcp` 是通过标准输入输出运行的 MCP 服务器。把它注册到 Claude Code、Codex 或其他客户端后，AI 在编辑您的商品详情时就能找到并使用 Edictora。

**如果 Claude Code 和 Codex 的官方 CLI（`claude` 和 `codex`）在 `PATH` 中，Setup 会自动把 Edictora 注册到这两者。** 卸载时只会移除指向 Edictora 的注册。
- 如果安装时缺少某个 CLI，或注册没有及时完成，Edictora 会在您下次登录时重试。之后才安装 CLI 的情况也一样。
- 如果您在注册添加之后自行移除了它，Edictora 不会再次添加。

对于 Portable 版，或这两者之外的客户端（例如 Claude Desktop 或网关），请使用下面的某种方法注册一次。
请以绝对路径给出已安装的 exe：
- Setup 版：`%LOCALAPPDATA%\Edictora\current\edictora.exe`。自动更新替换文件时，此路径保持不变。
- Portable 版：您解压的文件夹中的 `current\edictora.exe`。

使用 `PATH` 中的 `edictora` 也可以，但在安装之前启动的客户端看不到新的 `PATH`，所以使用绝对路径更稳妥。

**Claude Code**（适用于所有项目）

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

或者把它添加到 `%USERPROFILE%\.codex\config.toml`（请替换 `<username>`）：

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop 以及其他使用 JSON 文件配置的客户端**（请替换 `<username>`）：

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

如果您使用 MCP 网关（例如 Toolport），请在网关上注册相同的命令（stdio，参数为 `mcp`）。

**Toolport 与自动注册（Setup 版）**
- 此判断在任何操作系统上都会进行（Windows、macOS、Linux）；Setup 本身目前只提供 Windows 版。
- 对 Claude Code 和 Codex 分别判断：同时满足以下两点时，Setup 会跳过 `claude mcp add` / `codex mcp add`。(1) 该客户端的设置中有 Toolport 网关（`~/.claude.json` 的 `mcpServers`，或 `~/.codex/config.toml`）。(2) Toolport 的 registry（`%APPDATA%\Toolport\registry.json`；位置可通过 `TOOLPORT_REGISTRY` / `TOOLPORT_DATA_DIR` 更改）中有一个已启用的服务器，以 `mcp` 参数启动 `edictora.exe`，并且位于正在使用的配置文件中。若与 Setup 所注册的是同一实体，则记录原因 `toolport`；若是另一个实体，则记录 `toolport-foreign`，两种情况下 Setup 都会跳过直接注册。无法判断时，仍按以往方式直接注册。此判断在 Windows、macOS 和 Linux 上进行（registry 的默认位置：Windows 为 `%APPDATA%\Toolport\registry.json`，macOS 为 `~/Library/Application Support/Toolport/registry.json`，Linux 为 `$XDG_CONFIG_HOME/Toolport/registry.json`（为绝对路径时；否则为 `~/.config/Toolport/registry.json`））；在其他系统上不做此判断，直接注册。
- 已有的注册不会被移除。跳过之后，如果您从该客户端移除了 Toolport 网关，或 Toolport 所指向的可执行文件已不存在，Edictora 会在下次更新或下次登录时（登录时运行的更新用 exe）添加直接注册。如果您在 Toolport 中停用或删除了 Edictora，则不会添加。Setup 已直接注册过一次的客户端，即使您自行移除，也不会被自动重新添加（只会补回 Setup 跳过的客户端）。
- 如果之后才添加 Toolport，并且只想移除直接注册，请运行 `claude mcp remove edictora --scope user` 或 `codex mcp remove edictora`。
- 如果 Toolport 提供的是另一个实体的 `edictora.exe`（不是 Setup 所注册的那个），Setup 同样会跳过直接注册，记录原因 `toolport-foreign`，并在 `%LOCALAPPDATA%\Edictora.Data\logs\Edictora_yyyy-MM-dd.log` 中写入警告。要修复，请把 Toolport 中 `edictora` 服务器的 command 改为指向 Setup 版的可执行文件（`%LOCALAPPDATA%\Edictora\current\edictora.exe`）。请勿把可执行文件复制到其他位置（副本不会被更新）。每次 Setup、更新和登录时都会重新判断：Toolport 改为指向 Setup 版后，记录会变为 `toolport`；Toolport 所指向的可执行文件不存在时，则直接注册。
- 不检查 Toolport 的工具隔离（更新使工具发生变化时会被隔离的功能）。
- 可以通过 `%APPDATA%\Edictora\config.json` 中的 `mcpRegistration` 选择行为：`auto`（默认；上述行为）、`direct`（始终直接注册）、`off`（不自动注册或修复；卸载时仍会移除指向 Edictora 的注册）。安装程序只读取默认的配置路径，不读取用 `--config` 指定的配置。其他命令会把这三个值以外的值报告为配置错误。安装程序把无效值或不是有效 JSON 的配置文件当作 `auto` 处理，并记入日志。

| 工具 | 功能 |
|---|---|
| `edictora_list_items` | 显示配置文件涵盖的内容（项目、语言区域、源文件）。不会打开浏览器。 |
| `edictora_diff` | 显示与源文件的差异（只读） |
| `edictora_pull` | 显示当前值（只读） |
| `edictora_push` | 重写有差异的字段并保存为草稿。未带 `confirm: true` 时，只显示差异并停止（试运行）。 |
| `edictora_submit` | 将一个项目提交审核。必须提供 `item` 和 `confirm: true`。 |

- 这些工具运行与 CLI 相同的代码，具有相同的保护措施和退出码。每个结果都以退出码及其含义开头。发现差异（3）不被视为错误。
- 客户端无法选择显示浏览器（`--show-browser`）、运行 `login`，也无法设置配置文件或 chrome.exe。除下面所说的登录窗口外，Chrome 始终在无窗口状态下运行。
- 如果您的登录已过期，Edictora 会在您的屏幕上打开登录用的 Chrome，等待您完成登录（最长 10 分钟），然后继续执行工具调用。若登录未完成，工具会请您在终端中运行 `edictora login`。
- 调用按顺序逐个运行。您可以用 `notifications/cancelled` 取消正在运行的调用。
- 对所有项目执行 `diff` 可能需要几分钟，因此请给您的客户端设置较宽裕的超时时间。

## 保护措施

- 默认情况下 Edictora 只显示差异，不做任何更改。只有 `push --yes` 才会写入，只有 `submit --item <id> --yes` 才会提交审核。
- 保存草稿和提交审核是互相独立的操作。提交一次只处理一个项目；没有批量提交。
- 保存之前，Edictora 会检查每个重写的字段是否与源文件完全一致。保存之后，它会重新加载页面并再次检查。如果有任何不一致，就不保存，或者报错停止。
- 对每个语言区域，Edictora 依次切换语言、重写字段、验证、保存，然后重新加载并再次验证。如果某个语言区域失败，该项目其余的部分会被跳过。
  - 不会跨语言区域回滚：失败之前已保存的语言区域会作为草稿保留。
  - 在您将项目提交审核之前，不会有任何内容公开。
- 当无法确认项目状态可以安全编辑时（例如“审核中”），Edictora 不会写入。它绝不会取消审核。
- 字段和按钮通过标签、角色和文本查找，绝不使用 CSS 类名。如果一次查找没有恰好匹配一个元素，Edictora 就会停止。
- Chrome 不带自动化标志启动，并会被正常关闭。**配置文件绝不会被删除。**
- 失败时保存的页面文本快照保存到 `%LOCALAPPDATA%\Edictora.Data\diagnostics`，绝不会保存到您的仓库中。其中可能包含信息中心页面上字段的值（publisherId 以及 Google 账号的姓名和电子邮件地址会被遮盖）。
- 图片更新只替换有差异的位置。不会调整顺序，更不可能提交审核（图片处理中没有调用提交的途径）。
- “隐私”标签页的更新只改动有差异的字段，不会提交审核（代码中没有可调用的途径）；如果控制台上的权限与配置不一致，会在写入任何内容之前停止。

## 文件存放位置

| 内容 | 位置 |
|---|---|
| 配置文件（源文件映射） | `%APPDATA%\Edictora\config.json` |
| Chrome 配置文件（包含您的登录状态） | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| 日志（包括更新结果） | `%LOCALAPPDATA%\Edictora.Data\logs` |
| 失败时的页面文本快照 | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| 应用本身（通过 Setup 安装时） | `%LOCALAPPDATA%\Edictora`（卸载时整体移除） |

数据保存在 `Edictora.Data` 中，与应用分开，这样卸载时就不会同时删除您已登录的 Chrome 配置文件。旧位置的数据（例如 `%LOCALAPPDATA%\Edictora\chrome-profile`）不会自动迁移。

Chrome 配置文件包含您的登录 Cookie。请勿分享它，不再需要时请自行删除。
发布者 ID（信息中心 URL 的一部分）在日志中会被遮蔽。

### 找不到配置文件时（Claude 桌面应用）

Claude 桌面应用是打包（MSIX）应用。从它内部启动的程序在 `%APPDATA%` 或 `%LOCALAPPDATA%` 下创建**新**文件夹时，Windows 会把它放进应用专用存储（`%LOCALAPPDATA%\Packages\<包>\LocalCache\...`），而不是真实位置，普通终端看不到。当配置或 Chrome 配置文件是从这样的位置读取时，Edictora 会在标准错误输出警告；配置不存在时会列出在那里找到的副本。解决办法是将该文件（文件夹）移到消息中显示的真实位置，例如 `%APPDATA%\Edictora\config.json`。Edictora 不会自动读取或移动这些副本。

## 构建

主程序使用 Rust 编写（`crates/edictora/`）。

```powershell
cargo build --release          # target\release\edictora.exe and edictora-updater.exe (Bun not needed)
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

在信息中心页面内运行的脚本位于 `page-scripts/`（Bun + TypeScript）。生成的 `page-scripts/dist/edictora.js` 会嵌入 exe 中，因此只有在修改这些脚本时才需要 Bun。

```powershell
cd page-scripts
bun install --frozen-lockfile   # install dependencies (TypeScript only)
bun run typecheck      # type-check
bun run build          # regenerate dist/edictora.js
bun run check          # check that dist/edictora.js is up to date
bun test               # run the page-script tests (fake DOM)
```

**`sectionStart` / `sectionEnd` 中的正则表达式**：Edictora 使用 Rust 的 `regex` crate，因此不支持先行断言、后行断言（`(?=…)`、`(?!…)`、`(?<=…)`、`(?<!…)`）和反向引用（`\1`）；它们会在加载配置时导致错误。像 `^■ Description` 这样的普通模式可以照常使用。

测试时，您可以覆盖：
- 本地数据根目录，用 `EDICTORA_LOCAL_ROOT`（默认 `%LOCALAPPDATA%\Edictora.Data`）
- 漫游根目录，用 `EDICTORA_ROAMING_ROOT`（默认 `%APPDATA%\Edictora`）
- 更新源，用 `EDICTORA_UPDATE_URL`（默认 `https://edictora.kagayoi.com`；明文 `http://` 仅允许用于 `127.0.0.1`）

Velopack 包同时包含 `edictora.exe`（主 exe）和 `edictora-updater.exe`。
- 更新不使用 velopack crate：Edictora 自己通过 WinHTTP（Windows 内置的 HTTP API）获取源、下载并验证包。
- 安装、替换文件和卸载交给 Velopack 的 Setup.exe 和 Update.exe。

## 分发

发布版本由 Cloudflare R2 在 `https://edictora.kagayoi.com` 提供（Velopack 渠道 `win`）。发布流程请参阅 [docs/release.md](docs/release.md)。

## 许可证

请参阅 [LICENSE](LICENSE)。隐私方面请参阅 [PRIVACY_POLICY.md](PRIVACY_POLICY.md)。
