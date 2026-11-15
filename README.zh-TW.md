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
| 5 | 需要登入 |
| 130 | 已中斷（Ctrl+C） |

進度會輸出到標準錯誤，結果會輸出到標準輸出。

`pull`、`diff`、`push` 與 `submit` 會以無視窗（headless）方式執行 Chrome，因此 Chrome 絕不會跳到您的工作視窗前面。
若要觀看執行過程，請加上 `--show-browser`。`login` 一律顯示視窗，因為需要由您手動操作。

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
- 用戶端無法選擇顯示瀏覽器（`--show-browser`）、執行 `login`，也無法設定設定檔或 chrome.exe。Chrome 一律以無視窗方式執行。
- 若您的登入已過期，工具會請您在終端機中執行 `edictora login`。登入須由您自行完成。
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
