<p align="center">
  <img src="icon/app_icon.png" alt="Edictora 圖示" width="128">
</p>

# Edictora

[English](README.md) | [日本語](README.ja.md) | [简体中文](README.zh-CN.md) | 繁體中文 | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**一款 Windows 工具，可依據各擴充功能儲存庫中的來源檔案，批次讀取、比較並更新您的 Chrome Web Store 商店資訊（說明與 URL）。**

公開的 Chrome Web Store（CWS）API（v2）無法讀取或寫入商店資訊，而 Chrome 也不允許擴充功能操作 CWS 頁面。
因此 Edictora 改為透過 Chrome DevTools Protocol（CDP）驅動使用專用設定檔的 Chrome 執行個體，並以與真人相同的輸入方式編輯開發人員資訊主頁。
它是單一命令列工具（`edictora.exe`），以 Rust 撰寫，不需要 .NET 或 Node.js 等執行階段。

> [!IMPORTANT]
> Edictora 依賴開發人員資訊主頁的版面配置，並已在以**日文**顯示的資訊主頁上驗證。
> 英文資訊主頁也能辨識，但英文的畫面標籤尚未在實際畫面上驗證。
> 若 Edictora 找不到欄位或按鈕（例如資訊主頁改版後），它會在不做任何變更的情況下停止。

## 功能

- **pull**：從資訊主頁讀取目前的值（說明、官方 URL、首頁 URL 與支援 URL）。唯讀。
- **diff**：顯示與您的來源檔案之間的差異。唯讀，也是預設指令。
- **push**：只重寫有差異的欄位並**儲存為草稿**，然後重新載入頁面，將儲存的值與您的來源檔案核對。
- **圖片**：如果設定中的項目寫了 `images`，`pull`、`diff`、`push` 也會處理商店圖片（商店圖示、截圖、小型宣傳圖塊、宣傳橫幅）。可用 `--only text|images|privacy` 與 `--image-scope all|global|localized` 縮小範圍。
- **「隱私」分頁**：如果設定中的項目寫了 `privacy`，`pull`、`diff`、`push` 也會處理後台的「隱私」分頁（Privacy practices：單一用途說明、各權限的理由、資料使用揭露）。可用 `--only privacy` 縮小範圍。只儲存為草稿，不會送出審核。
- **submit**：將一個項目提交審查。不會變更「審查通過後自動發布」的設定。
- **login**：以專用設定檔開啟 Chrome，讓您自行登入 Google。
- **update**：若有新版本，則下載並安裝。僅在您以 Setup 安裝 Edictora 時有效，並且會在您登入 Windows 時自動執行。

## 顯示語言

Edictora 的訊息、差異輸出與說明文字提供九種語言：日文、英文、簡體中文、繁體中文、韓文、西班牙文、法文、德文與巴西葡萄牙文。

- 預設情況下，Edictora 依下列順序決定語言：環境變數 `EDICTORA_LANGUAGE`，接著是 `LC_ALL`、`LC_MESSAGES` 與 `LANG`（空值、`C` 與 `POSIX` 會略過），最後是 Windows 的顯示語言。若都沒有得到支援的語言，則使用英文。
- 若要明確指定語言，請將 `EDICTORA_LANGUAGE` 設為 `ja`、`en`、`zh-CN`、`zh-TW`、`ko`、`es`、`fr`、`de` 或 `pt-BR` 其中之一。舊名稱 `EDICTORA_LANG` 仍會被讀取，但 `EDICTORA_LANGUAGE` 的優先順序較高。
- `edictora mcp` 無論顯示語言為何，一律以英文回應，因為它的工具說明與回應是由 AI 代理程式讀取的。
- 這與開發人員資訊主頁的語言（日文或英文），以及您商店資訊的 `locales` 無關。
- 記錄檔供作者排解問題之用，因此其中的固定訊息維持日文。記錄中包含的錯誤訊息則依顯示語言而定。

## 安裝與更新

執行 `Edictora-win-Setup.exe`（Velopack 安裝程式）。它會安裝到 `%LOCALAPPDATA%\Edictora`，不需要系統管理員權限。

- 安裝後，`edictora` 可在任何新開啟的命令提示字元或 PowerShell 視窗中使用，因為 Setup 會將 `%LOCALAPPDATA%\Edictora\current` 加入您的使用者 `Path`。
- 當您登入 Windows 時，`edictora-updater.exe` 會在無視窗的情況下執行。若 `https://edictora.kagayoi.com` 有較新的版本，它會下載、檢查大小與雜湊值，然後安裝。
  - 它以 `Edictora Updater` 為名登錄在 HKCU 的 `Run` 之下，因此您可以在工作管理員的「啟動應用程式」中將它關閉。
  - 結果會寫入記錄檔。
- 若要手動更新，請執行 `edictora update`。已是最新版本或更新已開始時，它會向標準錯誤輸出報告並以結束代碼 0 結束；失敗時以結束代碼 1 結束。`edictora` 結束後，Update.exe 會替換檔案。
- Portable 版（`Edictora-win-Portable.zip`）不會在登入時自動更新，也不會將自己加入 `Path`。若要更新，請在解壓縮的資料夾中執行 `current\edictora.exe update`。從原始碼建置的可執行檔不會自行更新。
- 在 `login`、`pull`、`diff`、`push` 或 `submit` 執行期間，絕不會套用更新。Edictora 會先完成下載，之後再套用更新。
  - 若在更新套用期間執行這些指令，它會在不開啟 Chrome 的情況下以結束代碼 1 結束。請稍候再重新執行。
  - 更新處理序結束後，Update.exe 還需要幾秒鐘來替換檔案，而 Edictora 無法保護這段空檔。更新完成後，請先稍候片刻再執行指令。
- 解除安裝會移除 `%LOCALAPPDATA%\Edictora` 以及 `Run` 與 `Path` 的項目。您的資料（`%LOCALAPPDATA%\Edictora.Data`）與設定（`%APPDATA%\Edictora`）會保留，若不再需要，請自行刪除。

## 系統需求

- Windows 10 或 Windows 11（x86_64）
- Google Chrome。Edictora 會自動尋找，您也可以在組態檔中設定 `chromePath`，或傳入 `--chrome`。
- CWS 開發人員帳戶
- 若從原始碼建置，需要 Rust 1.100.0（固定於 `rust-toolchain.toml`）。只有在變更頁面內指令碼時才需要 Bun。

## 使用方式

### 1. 撰寫組態檔

預設位置為 `%APPDATA%\Edictora\config.json`，可用 `--config` 變更。
針對每個項目，將每個語系對應到其來源檔案。**您省略的欄位絕不會被讀取、比較或寫入。**

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

- 若只提供 `description.file`，則整個檔案都會作為說明使用。
- 擷取區段時，Edictora 會取起始標題之後、結束標題之前的行。它會去除前後的空白行與行尾空白，並將換行字元正規化為 `\n`。若找不到標題，就會以錯誤停止。
- 沒有鍵的 URL 欄位會被忽略。`""` 表示該欄位應為空。
- 未知的鍵（例如拼字錯誤）會視為錯誤。允許註解與尾端逗號。

#### 類別（選用）

在項目中加入 `category`，即可讀取、比較並儲存商店的類別（資訊主頁的「類別」欄位，所有語言共用）。請寫小寫的鍵。與其他欄位一樣，**不寫就不會更動類別**。

```jsonc
"category": "productivity"    // one of the keys below; leave it out to skip the category
```

- 鍵：`productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`。未知的鍵屬於設定錯誤（結束代碼 2）。「未選擇類別」不能作為來源值。
- 類別由所有語言共用，因此每個項目只讀寫一次（而不是每種語言一次）。它包含在 `--only text` 中，`--locale` 不會縮小其範圍。`push --yes` 只在有差異時才改寫，儲存為草稿，重新載入後再次核對。
- 項目可以只有 `category` 而沒有 `locales`（僅管理類別）。
- 僅在資訊主頁以日文顯示時可用（英文介面的文字尚未驗證）。

#### 商店圖片（選用）

在項目中加入 `images` 即可管理商店圖片。與其他欄位相同，**沒有寫的鍵不會被觸碰**。路徑相對於 `baseDir`。

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

- `dir` 依檔名升冪使用資料夾直下的 `screenshot-*.png`（1–5 張）。`files` 依書寫順序使用（1–5 張）。
- 僅支援 PNG：24 位元、無 Alpha，截圖為 1280x800 或 640x400，小型圖塊為 440x280，橫幅為 1400x560。Edictora 會在開啟 Chrome 之前檢查這些規格，以及同一組內是否有重複的圖片（結束代碼 2）。但商店圖示（`icon`）是例外：必須是 128x128，可以是帶透明的 PNG（8 位元 RGB 或 RGBA）。比較時包含 Alpha，完全透明的像素的顏色會被忽略。它由所有語言共用，所以 `--image-scope global` 包含它。
- 項目可以只寫 `images` 而不寫 `locales`（僅圖片）。
- **圖片依解碼後的像素比較，而不是依檔案位元組。** 後台會重新編碼上傳的 PNG，所以儲存的檔案位元組與原檔不同，但像素相同。Edictora 會下載後台上的每張圖片（`=s0` 原圖，僅透過 HTTPS 存取 `*.googleusercontent.com`，不帶 Cookie），並依序比較像素雜湊。無法下載或解碼的圖片結果為「無法比較」，既不視為一致，也不會取代。
- `push --yes` 只在有差異時整體取代一個位置（先全部刪除，再依序上傳），然後儲存草稿、重新載入並再次核對。處理順序為：商店圖示 → 小型圖塊 → 橫幅 → 全語言截圖 → 各語言的在地化截圖。
- **刪除圖片會立即在後台生效**（會提示「此操作無法復原」，且未儲存就重新載入後該位置仍為空）。如果 push 中途停止，草稿中可能缺少圖片：再次執行即可補齊。審核中的項目不會被觸碰。
- 只有一種語言的項目沒有在地化截圖，請使用 `--image-scope global`。如果 `localizedScreenshots` 中的語言不在後台上，Edictora 會在更動任何內容之前停止：請使用 `--image-scope global` 或 `--locale`。
- 僅當後台以日語顯示時才能寫入圖片（英文文案尚未確認）。

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

#### 「隱私」分頁（選用）

為擴充功能加入權限後，必須在後台的**「隱私」分頁**（Privacy practices）填寫權限理由等內容，才能送出審核。在項目中加入 `privacy`，即可從來源檔案讀取、比較並以草稿儲存這些欄位。與其他欄位相同，**沒有寫的鍵不會被觸碰**。

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

- 本文與 `description` 相同，從檔案中擷取區段（見上文）。會去掉開頭的空行，並去掉所有行共有的縮排，因此縮排寫在 `  activeTab:` 底下的理由也可以直接使用。每段本文不可為空，且不超過 1,000 個字元；Edictora 會在開啟 Chrome 之前檢查（結束代碼 2）。
- `collected` 可寫 `personallyIdentifiable`、`health`、`financial`、`authentication`、`personalCommunications`、`location`、`webHistory`、`userActivity`、`websiteContent`（只勾選寫出的，其餘取消勾選）。`certify` 只能為 `true`（表示勾選「不出售」等三項聲明）；`false` 或省略都會是設定錯誤。
- **權限清單本身也會核對。** 寫了 `permissions` 時，設定中有而後台上沒有的權限（或相反）會顯示在比較結果中，`push` 會在寫入任何內容之前停止：請在設定中補上，或修正 manifest。
- `policyUrl` 是隱私權政策網址欄位：http(s) 絕對 URL（最多 2,048 個字元），或表示應為空白的 `""`。不寫則不會更動。它會與此分頁的其他欄位一起儲存為草稿。
- `remoteCode`（`{ "uses": false }`，或 `{ "uses": true, "justification": {...} }`）可以寫在設定中，但後台上對應的欄位尚未在實際畫面中確認，因此寫了之後 `diff` 會回報錯誤，`push` 會停止。目前請不要寫。
- 僅當後台以日語顯示時可用（英文文案尚未確認）。只儲存為草稿，Edictora 絕不會送出審核。
- 項目可以只寫 `privacy` 而不寫 `locales` 和 `images`（僅「隱私」分頁）。

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```

### 2. 登入

```powershell
edictora login
```

Chrome 會以專用設定檔開啟。請在其中登入 Google 並開啟開發人員資訊主頁。
- Edictora 一偵測到資訊主頁，就會將登入狀態保留在設定檔中並結束。
- 預設最多等待 10 分鐘。可使用 `--wait <minutes>` 設定 1 至 120 分鐘。
- Edictora 絕不會處理您的密碼或驗證碼，這些都由您自行輸入。

### 3. 比較與更新

```powershell
edictora diff                                   # Show differences for all items (read-only)
edictora pull --item <id>                       # Show current values (read-only)
edictora push --item <id> --locale en           # Show differences and stop (no --yes, so nothing is written)
edictora push --item <id> --locale en --yes     # Rewrite differing fields, save as draft, verify
edictora submit --item <id> --yes               # Submit one item for review
```

| 結束代碼 | 意義 |
|---|---|
| 0 | 成功（無差異） |
| 1 | 執行階段錯誤（資訊主頁操作失敗、驗證不一致等） |
| 2 | 引數或組態無效（不會開啟 Chrome） |
| 3 | 發現差異（`diff`，或未加 `--yes` 的 `push`） |
| 4 | 寫入已中止（審查中，或無法判斷狀態） |
| 5 | 需要登入（自動等待登入未完成，或作業途中登入已過期；再次執行時，如有需要會開啟登入視窗） |
| 130 | 已中斷（Ctrl+C） |

進度會輸出到標準錯誤，結果會輸出到標準輸出。

`pull`、`diff`、`push` 與 `submit` 會以無視窗（headless）方式執行 Chrome，因此 Chrome 絕不會跳到您的工作視窗前面。
若要觀看執行過程，請加上 `--show-browser`。`login` 一律顯示視窗，因為需要由您手動操作。 若 Google 登入已過期，`pull`、`diff`、`push` 和 `submit` 會自行開啟有視窗的 Chrome，等待您完成登入（最長 10 分鐘），然後關閉視窗並繼續。登入未完成（逾時、視窗被關閉，或同一設定檔仍有無視窗的 Chrome 導致無法顯示視窗），或作業途中登入過期（此時不會開啟視窗，請再次執行）時，會以結束代碼 5 停止。

> [!WARNING]
> `--show-browser` 是給在螢幕前觀看的人使用的。從 AI 代理程式或指令碼執行 Edictora 時請勿使用：Chrome 視窗會跳出在使用者的螢幕上，而 AI 本來也看不到它。
> 若要找出失敗的原因，請使用錯誤訊息，以及失敗時儲存的螢幕截圖。
> - 錯誤訊息包含遮住目標的元素、該元素的邊界，以及可視區域的大小。
> - 螢幕截圖會儲存到 `%LOCALAPPDATA%\Edictora.Data\diagnostics`，錯誤訊息會顯示其路徑。當 `pull` 或 `diff` 讀取資訊主頁失敗時，也會儲存螢幕截圖。

### 4. 從 AI 代理程式使用（MCP）

`edictora mcp` 是透過標準輸入與輸出執行的 MCP 伺服器。將它登錄到 Claude Code、Codex 或其他用戶端後，AI 在編輯您的商店資訊時就能找到並使用 Edictora。

若 Claude Code 與 Codex 的官方 CLI（`claude` 與 `codex`）位於 `PATH` 上，**Setup 會自動將 Edictora 登錄到這兩者**。解除安裝時，只會移除指向 Edictora 的登錄。
- 若安裝時找不到 CLI，或登錄未能及時完成，Edictora 會在您下次登入時重試。之後才安裝 CLI 的情況也一樣。
- 若您在登錄加入後自行移除，Edictora 不會再次加入。

對於 Portable 版，或這兩者以外的用戶端（例如 Claude Desktop 或閘道），請使用下列其中一種方法登錄一次。
請以絕對路徑指定已安裝的 exe：
- Setup 版：`%LOCALAPPDATA%\Edictora\current\edictora.exe`。自動更新替換檔案時，此路徑保持不變。
- Portable 版：解壓縮資料夾中的 `current\edictora.exe`。

使用 `PATH` 上的 `edictora` 也可以，但在安裝之前就已啟動的用戶端看不到新的 `PATH`，因此使用絕對路徑較為保險。

**Claude Code**（適用於所有專案）

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

或將它加入 `%USERPROFILE%\.codex\config.toml`（請替換 `<username>`）：

```toml
[mcp_servers.edictora]
command = 'C:\Users\<username>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop 與其他以 JSON 檔案設定的用戶端**（請替換 `<username>`）：

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

若您使用 MCP 閘道（例如 Toolport），請在閘道上登錄相同的指令（stdio，引數為 `mcp`）。

| 工具 | 功能 |
|---|---|
| `edictora_list_items` | 顯示組態檔涵蓋的內容（項目、語系、來源檔案）。不會開啟瀏覽器。 |
| `edictora_diff` | 顯示與來源檔案的差異（唯讀） |
| `edictora_pull` | 顯示目前的值（唯讀） |
| `edictora_push` | 重寫有差異的欄位並儲存為草稿。未指定 `confirm: true` 時，只顯示差異並停止（試執行）。 |
| `edictora_submit` | 將一個項目提交審查。必須提供 `item` 與 `confirm: true`。 |

- 這些工具執行的程式碼與 CLI 相同，具有相同的防護機制與結束代碼。每個結果都以結束代碼及其意義開頭。發現差異（3）不會被視為錯誤。
- 用戶端無法選擇顯示瀏覽器（`--show-browser`）、執行 `login`，也無法設定設定檔或 chrome.exe。除下面所說的登入視窗外，Chrome 一律以無視窗方式執行。
- 若您的登入已過期，Edictora 會在您的螢幕上開啟登入用的 Chrome，等待您完成登入（最長 10 分鐘），然後繼續執行工具呼叫。若登入未完成，工具會請您在終端機中執行 `edictora login`。
- 呼叫會依序逐一執行。您可以用 `notifications/cancelled` 取消執行中的呼叫。
- 對所有項目執行 `diff` 可能需要數分鐘，因此請為您的用戶端設定寬裕的逾時時間。

## 防護機制

- 預設情況下，Edictora 只顯示差異，不做任何變更。只有 `push --yes` 才會寫入，只有 `submit --item <id> --yes` 才會提交審查。
- 儲存草稿與提交審查是不同的操作。提交一次只處理一個項目，沒有批次提交。
- 儲存之前，Edictora 會檢查每個重寫的欄位是否與來源檔案完全一致。儲存之後，它會重新載入頁面再檢查一次。若有任何不一致，它就不會儲存，或以錯誤停止。
- 對於每個語系，Edictora 會切換語言、重寫欄位、驗證、儲存，然後重新載入並再次驗證。若某個語系失敗，該項目其餘的語系會被略過。
  - 跨語系沒有還原機制：失敗之前已儲存的語系會保留為草稿。
  - 在您將項目提交審查之前，不會公開任何內容。
- 當項目的狀態無法確認可安全編輯時（例如「審查中」），Edictora 不會寫入。它絕不會取消審查。
- 欄位與按鈕是依標籤、角色與文字尋找的，絕不依賴 CSS 類別名稱。若搜尋結果不是剛好一個元素，Edictora 就會停止。
- Chrome 不帶自動化旗標啟動，並會正常關閉。**設定檔絕不會被刪除。**
- 失敗時擷取的螢幕截圖會儲存到 `%LOCALAPPDATA%\Edictora.Data\diagnostics`，絕不會儲存到您的儲存庫。
- 圖片更新只取代有差異的位置。不會調整順序，更不可能送出審核（圖片處理中沒有呼叫送出的途徑）。
- 「隱私」分頁的更新只更動有差異的欄位，不會送出審核（程式碼中沒有可呼叫的途徑）；如果後台上的權限與設定不一致，會在寫入任何內容之前停止。

## 檔案儲存位置

| 內容 | 位置 |
|---|---|
| 組態檔（來源檔案對應） | `%APPDATA%\Edictora\config.json` |
| Chrome 設定檔（包含您的登入狀態） | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| 記錄檔（包含更新結果） | `%LOCALAPPDATA%\Edictora.Data\logs` |
| 失敗時的螢幕截圖 | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| 應用程式本身（以 Setup 安裝時） | `%LOCALAPPDATA%\Edictora`（解除安裝時會整個移除） |

資料保存在 `Edictora.Data` 中，與應用程式分開，這樣解除安裝時就不會連同您已登入的 Chrome 設定檔一併刪除。舊位置的資料（例如 `%LOCALAPPDATA%\Edictora\chrome-profile`）不會自動遷移。

Chrome 設定檔包含您的登入 Cookie。請勿分享，並在不再需要時自行刪除。
發布者 ID（資訊主頁 URL 的一部分）在記錄中會被遮蔽。

### 找不到設定檔時（Claude 桌面應用程式）

Claude 桌面應用程式是封裝（MSIX）應用程式。從它內部啟動的程式在 `%APPDATA%` 或 `%LOCALAPPDATA%` 下建立**新**資料夾時，Windows 會把它放進應用程式專用儲存空間（`%LOCALAPPDATA%\Packages\<套件>\LocalCache\...`），而不是實際位置，一般終端機看不到。當設定檔或 Chrome 設定檔是從這樣的位置讀取時，Edictora 會在標準錯誤輸出警告；設定檔不存在時會列出在那裡找到的副本。解決方法是將該檔案（資料夾）移到訊息中顯示的實際位置，例如 `%APPDATA%\Edictora\config.json`。Edictora 不會自動讀取或移動這些副本。

## 建置

主程式以 Rust 撰寫（`crates/edictora/`）。

```powershell
cargo build --release          # target\release\edictora.exe and edictora-updater.exe (Bun not needed)
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

在資訊主頁頁面內執行的指令碼位於 `page-scripts/`（Bun + TypeScript）。產生的 `page-scripts/dist/edictora.js` 會內嵌到 exe 中，因此只有在變更這些指令碼時才需要 Bun。

```powershell
cd page-scripts
bun install --frozen-lockfile   # install dependencies (TypeScript only)
bun run typecheck      # type-check
bun run build          # regenerate dist/edictora.js
bun run check          # check that dist/edictora.js is up to date
```

**`sectionStart` / `sectionEnd` 中的規則運算式**：Edictora 使用 Rust 的 `regex` crate，因此不支援正向預測、反向預測（`(?=…)`、`(?!…)`、`(?<=…)`、`(?<!…)`）與反向參照（`\1`），載入組態時會導致錯誤。像 `^■ Description` 這樣的一般模式可照原樣使用。

測試時，您可以覆寫：
- 以 `EDICTORA_LOCAL_ROOT` 覆寫本機資料根目錄（預設為 `%LOCALAPPDATA%\Edictora.Data`）
- 以 `EDICTORA_ROAMING_ROOT` 覆寫漫遊根目錄（預設為 `%APPDATA%\Edictora`）
- 以 `EDICTORA_UPDATE_URL` 覆寫更新來源（預設為 `https://edictora.kagayoi.com`；純文字 `http://` 僅允許用於 `127.0.0.1`）

Velopack 套件同時包含 `edictora.exe`（主要 exe）與 `edictora-updater.exe`。
- 更新不使用 velopack crate：Edictora 透過 WinHTTP（Windows 內建的 HTTP API）自行取得摘要來源、下載並驗證套件。
- 安裝、替換檔案與解除安裝，則交由 Velopack 的 Setup.exe 與 Update.exe 處理。

## 發行

發行版本由 Cloudflare R2 於 `https://edictora.kagayoi.com`（Velopack 通道 `win`）提供。發行流程請參閱 [docs/release.md](docs/release.md)。

## 授權

請參閱 [LICENSE](LICENSE)。隱私權相關內容請參閱 [PRIVACY_POLICY.md](PRIVACY_POLICY.md)。
