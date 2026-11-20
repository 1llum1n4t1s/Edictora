<p align="center">
  <img src="icon/app_icon.png" alt="Edictora アイコン" width="128">
</p>

# Edictora

[English](README.md) | 日本語 | [简体中文](README.zh-CN.md) | [繁體中文](README.zh-TW.md) | [한국어](README.ko.md) | [Español](README.es.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Português (Brasil)](README.pt-BR.md)

**Chrome Web Store の掲載情報（説明文・URL）を、各リポジトリの正本ファイルから一括で読み出し・比較・更新する Windows ツール**

Chrome Web Store（CWS）の公開 API（v2）には、掲載情報の読み書きがありません。拡張機能から CWS のページを操作することも Chrome が許可しません。
Edictora は、専用プロファイルの Chrome を Chrome DevTools Protocol（CDP）で操作し、デベロッパー ダッシュボードの画面を人と同じ入力で書き換えます。
コマンドライン ツール（`edictora.exe`）1 つで動きます。Rust 製で、実行に .NET や Node.js などのランタイムは要りません。

> [!IMPORTANT]
> ダッシュボードの画面構成に依存するツールです。実画面で確かめているのは**日本語表示**のダッシュボードです。
> 英語表示のダッシュボードも認識しますが、英語の画面の文言はまだ実画面で確かめていません。
> 画面が変わって欄やボタンを特定できなくなった場合は、何も書き換えずに止まります。

## できること

- **pull** — ダッシュボードの現在値（説明・公式 URL・ホームページ URL・サポート URL）を読み出す（読み取りのみ）
- **diff** — 正本ファイルとの差分を表示する（読み取りのみ。既定の動作）
- **push** — 差分のある欄だけを書き換えて**下書き保存**し、読み込み直して正本と一致するか照合する
- **画像**: 設定のアイテムに `images` があれば、`pull`・`diff`・`push` はストアの掲載画像（ショップ アイコン・スクリーンショット・小タイル・マーキー）も扱います。`--only text|images|privacy` と `--image-scope all|global|localized` で絞れます。
- **「プライバシー」タブ**: 設定のアイテムに `privacy` があれば、`pull`・`diff`・`push` はダッシュボードの「プライバシー」タブ（Privacy practices: 単一用途の説明・権限ごとの正当性・データ使用の開示）も扱います。`--only privacy` で絞れます。下書き保存までで、審査には送信しません。
- **submit** — 1 件ずつ審査に送信する（「審査の合格後に自動的に公開する」の設定は変更しません）
- **login** — 専用プロファイルの Chrome を開き、Google へのログインを利用者に行ってもらう
- **update** — 新しい版があればダウンロードして差し替える（Setup でインストールしたときだけ。ログオン時にも自動で行う）

## 表示言語

メッセージ・差分の表示・ヘルプは、日本語・英語・簡体字中国語・繁体字中国語・韓国語・スペイン語・フランス語・ドイツ語・ブラジル・ポルトガル語の 9 言語に対応しています。

- 既定では、環境変数 `EDICTORA_LANGUAGE` → `LC_ALL` → `LC_MESSAGES` → `LANG`（空・`C`・`POSIX` は除く）→ Windows の表示言語の順で決めます。どれでも判定できない、または未対応の言語なら英語です。
- 明示するときは、環境変数 `EDICTORA_LANGUAGE` に `ja`・`en`・`zh-CN`・`zh-TW`・`ko`・`es`・`fr`・`de`・`pt-BR` のいずれかを設定します。旧名の `EDICTORA_LANG` も読みますが、`EDICTORA_LANGUAGE` が優先されます。
- `edictora mcp` は、AI エージェントが読む出力なので、表示言語に関わらず常に英語で返します。
- ダッシュボードの表示言語（日本語・英語）や、掲載情報の `locales` とは別のものです。
- ログは作者が原因を調べるためのものなので、ログの定型文は日本語のままです。ただし、ログに含まれるエラー文などは表示言語で記録されます。

## インストールと更新

配布物の `Edictora-win-Setup.exe`（Velopack のインストーラー）を実行すると、管理者権限なしで `%LOCALAPPDATA%\Edictora` に入ります。

- インストール後は、新しく開いたコマンドプロンプトや PowerShell で `edictora` がそのまま使えます（ユーザー環境変数 `Path` に `%LOCALAPPDATA%\Edictora\current` を足します）。
- Windows へのログオン時に `edictora-updater.exe` がウィンドウを出さずに動き、`https://edictora.kagayoi.com` に新しい版があれば、ダウンロードしてサイズとハッシュを確かめてから差し替えます（HKCU の `Run` に `Edictora Updater` として登録します。タスク マネージャーの「スタートアップ アプリ」で無効にできます）。結果はログに残ります。
- 手動で更新するときは `edictora update` を実行します。結果は標準エラーに出し、最新・更新の開始なら終了コード 0、失敗なら 1 です。差し替えは `edictora` の終了後に Update.exe が行います。`pull` / `diff` / `push` / `submit` の実行中は更新を確かめません。
- Portable 版（`Edictora-win-Portable.zip`）では、ログオン時の自動更新と `Path` の登録をしません。展開先の `current\edictora.exe update` で手動で更新できます。ソースからビルドした exe は更新しません。
- `login` / `pull` / `diff` / `push` / `submit` の実行中は、更新を適用しません（ダウンロードまで済ませ、適用は後に回します）。更新の適用中にこれらを実行すると、Chrome を開かずに終了コード 1 で止まるので、少し待ってから実行し直してください。ただし、更新処理が終了してから Update.exe が差し替えを終えるまでの数秒間は防げないので、更新の直後は少し待ってから実行してください。
- アンインストールすると、`%LOCALAPPDATA%\Edictora` と `Run`・`Path` の登録が取り除かれます。データ（`%LOCALAPPDATA%\Edictora.Data`）と設定（`%APPDATA%\Edictora`）は残るので、不要なら利用者自身で削除してください。

## 必要環境

- Windows 10 / Windows 11（x86_64）
- Google Chrome（自動検出。設定ファイルの `chromePath` か `--chrome` でも指定可能）
- CWS デベロッパー アカウント
- Rust 1.100.0（ソースからビルドする場合。`rust-toolchain.toml` で固定）。ページ内スクリプトを変更するときだけ Bun

## 使い方

### 1. 設定ファイルを書く

既定の場所は `%APPDATA%\Edictora\config.json` です（`--config` で変更できます）。
アイテムごとに、ロケールと正本ファイルの対応を書きます。**書いていない欄は、読み出し・比較・書き換えのどれにも使いません。**

```jsonc
{
  // 任意。省略時は %LOCALAPPDATA%\Edictora.Data\chrome-profile
  "profileDir": "%LOCALAPPDATA%\\Edictora.Data\\chrome-profile",
  "items": [
    {
      "itemId": "abcdefghijklmnopabcdefghijklmnop",   // 32 文字の CWS アイテム ID
      "name": "Replace Translator",                     // 任意（表示用）
      "baseDir": "D:\\Dev\\ReplaceTranslator",          // 任意（相対パスの基準。省略時は設定ファイルの場所）
      "locales": {
        "ja": {
          "description": {
            "file": "webstore/store-listing.txt",
            "sectionStart": "^■ 説明",                  // この見出し行の次から
            "sectionEnd": "^■ カテゴリ"                 // この見出し行の手前まで
          },
          "homepageUrl": "https://example.com/",
          "supportUrl": "https://example.com/support"
        },
        "en": {
          "description": {
            "file": "webstore/store-listing.en.txt",
            "sectionStart": "^■ Description",
            "sectionEnd": "^■ Category"
          }
        }
      }
    }
  ]
}
```

- `description.file` だけを書くと、ファイル全体を説明文にします。
- 区間を切り出すときは、見出し行の次から終端見出しの手前までを取り、前後の空行と末尾の空白を除き、改行を `\n` に揃えます。見出しが見つからなければエラーで止まります。
- URL 欄は、キーを書かなければ対象外です。`""` を書くと「空欄が正本」として扱います。
- 未知のキー（綴り間違いなど）はエラーにします。コメントと末尾カンマは使えます。

#### カテゴリ（任意）

アイテムに `category` を書くと、ストアのカテゴリ（ダッシュボードの「カテゴリ」欄。全言語共通）を読み取り・比較・下書き保存できます。値は小文字のキーで書きます。ほかの欄と同じく、**書かなければカテゴリには触りません**。

```jsonc
"category": "productivity"    // one of the keys below; leave it out to skip the category
```

- キー: `productivity`, `communication`, `tools`, `developer_tools`, `workflow_and_planning`, `education`, `lifestyle`, `art_and_design`, `entertainment`, `games`, `shopping`, `social_networking`, `news_and_weather`, `well_being`, `household`, `travel`, `just_for_fun`, `make_chrome_yours`, `privacy_and_security`, `accessibility`, `functionality_and_ui`。未知のキーは設定の誤りです（終了コード 2）。「カテゴリ未選択」は正本にできません。
- カテゴリは全言語共通なので、言語ごとではなくアイテムごとに 1 回だけ読み書きします。`--only text` に含まれ、`--locale` では絞りません。`push --yes` は差分があるときだけ書き換え、下書き保存して読み込み直し、もう一度照合します。
- `locales` を書かず `category` だけを持つアイテムも使えます。
- ダッシュボードが日本語で表示されているときだけ動作します（英語の画面の文言は未確認です）。

#### 掲載画像（任意）

アイテムに `images` を書くと、掲載画像も管理できます。ほかの欄と同じく、**書いていないキーは触りません**。パスは `baseDir` が基準です。

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

- `dir` はフォルダー直下の `screenshot-*.png` を名前の昇順で使います（1〜5 枚）。`files` は書いた順です（1〜5 枚）。
- 使えるのは PNG だけで、アルファなしの 24 ビット、寸法はスクリーンショットが 1280x800 か 640x400、小タイルが 440x280、マーキーが 1400x560 です。Chrome を開く前に、これと同じセット内に同じ画像がないことを確かめます（終了コード 2）。ただしショップ アイコン（`icon`）だけは例外で、寸法は 128x128、透過のある PNG（8 ビットの RGB か RGBA）も使えます。比較はアルファを含めて行い、完全に透明な画素の色は無視します。全言語共通の欄なので、`--image-scope global` に含まれます。
- `locales` を書かず `images` だけのアイテムも作れます。
- **画像はファイルのバイト列ではなく、デコードした画素で比べます。** ダッシュボードはアップロードした PNG を再エンコードして保存するので、保存された画像はバイト列が元のファイルと一致しませんが、画素は同じです。Edictora はダッシュボードの各画像（`=s0` の原本。`*.googleusercontent.com` へ HTTPS だけ、Cookie なし）を取得し、画素のハッシュを順番に比べます。取得・デコードできない画像は「比較不能」で、一致扱いにも置き換えにもしません。
- `push --yes` は差分のあるスロットだけを丸ごと入れ替え（全部削除してから順にアップロード）、下書き保存し、読み込み直して照合します。順番は ショップ アイコン → 小タイル → マーキー → 全言語向けスクリーンショット → 言語ごとのローカライズ版です。
- **画像の削除は、ダッシュボードに即座に反映されます**（「元に戻せません」と確認され、保存せずに読み込み直しても空のままでした）。push が途中で止まると、下書きの画像が欠けたままになることがあります。もう一度実行すれば続きから整います。審査待ちのアイテムには触れません。
- 言語が 1 つだけのアイテムにはローカライズ版がないので、`--image-scope global` を使ってください。`localizedScreenshots` のロケールがダッシュボードになければ、何も変えずに止まります（`--image-scope global` か `--locale` を使います）。
- 画像の書き込みは、ダッシュボードが日本語表示のときだけ行えます（英語の文言は未確認です）。

```powershell
edictora diff --only images --item <id>                                  # Images only (read-only)
edictora push --only images --image-scope global --item <id> --yes      # Replace global images, save as draft, verify
```

#### 「プライバシー」タブ（任意）

拡張機能に権限を足すと、ダッシュボードの**「プライバシー」タブ**（Privacy practices）に権限の正当性などを入力しないと審査に送信できません。アイテムに `privacy` を書くと、その欄を正本ファイルから読み取り・比較・下書き保存できます。ほかの欄と同じく、**書いていないキーは触りません**。

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

- 本文は `description` と同じく、ファイルから区間を切り出します（上を参照）。先頭の空行を除き、全行に共通する字下げを取り除くので、`  activeTab:` の下に字下げして書いた正当性もそのまま使えます。どの本文も空にはできず、1,000 文字以内にします。Chrome を開く前に確かめます（終了コード 2）。
- `collected` に書けるのは `personallyIdentifiable`・`health`・`financial`・`authentication`・`personalCommunications`・`location`・`webHistory`・`userActivity`・`websiteContent` です（書いたものだけチェックし、残りは外します）。`certify` は `true` だけを指定できます（「販売しない」など 3 つの宣誓を入れる意味です）。`false` や省略は設定エラーです。
- **権限の一覧も照合します。** `permissions` を書いたとき、設定にあるのに画面に無い権限、または画面にあるのに設定に無い権限は、比較結果に表示され、`push` は何も書かずに止まります（設定へ足すか、manifest を直してください）。
- `policyUrl` はプライバシー ポリシーの URL 欄です。http(s) の絶対 URL（2,048 文字以内）、または空欄を正本にする `""`。書かなければ触りません。このタブのほかの欄と一緒に下書き保存します。
- `remoteCode`（`{ "uses": false }`、または `{ "uses": true, "justification": {...} }`）は設定に書けますが、ダッシュボード上の該当欄を実画面で確認できていないため、書くと `diff` はエラーを報告し、`push` は止まります。今は書かないでください。
- ダッシュボードが日本語表示のときだけ使えます（英語の文言は未確認です）。下書き保存までで、審査には送信しません。
- `locales` や `images` を書かず `privacy` だけのアイテムも作れます。

```powershell
edictora diff --only privacy --item <id>          # Privacy practices tab only (read-only)
edictora push --only privacy --item <id> --yes    # Rewrite differing fields, save as draft, reload and verify
```

### 2. ログインする

```powershell
edictora login
```

専用プロファイルの Chrome が開きます。その画面で Google にログインし、デベロッパー ダッシュボードを表示してください。到達を確認すると、ログイン状態をプロファイルに残して終了します。
待つ時間の上限は既定 10 分です（`--wait <分>` で 1〜120 分に変更できます）。
パスワードや確認コードは Edictora は扱いません（入力はご自身で行います）。

### 3. 比較して更新する

```powershell
edictora diff                                   # 全アイテムの差分を表示（読み取りのみ）
edictora pull --item <id>                       # 現在値を表示（読み取りのみ）
edictora push --item <id> --locale ja           # 差分を表示して止まる（--yes が無いので書き換えない）
edictora push --item <id> --locale ja --yes     # 差分のある欄を書き換えて下書き保存・照合
edictora submit --item <id> --yes               # 1 件を審査に送信
```

| 終了コード | 意味 |
|---|---|
| 0 | 成功（差分なし） |
| 1 | 実行時エラー（画面操作の失敗・照合の不一致など） |
| 2 | 引数・設定の誤り（Chrome は開きません） |
| 3 | 差分あり（`diff`、または `--yes` なしの `push`） |
| 4 | 書き込みを中止（審査待ち・状態を判定できない） |
| 5 | ログインが必要（自動のログイン待ちが完了しなかったか、作業の途中でログインが切れた。もう一度実行すると、必要ならログイン用の窓が開きます） |
| 130 | 中断（Ctrl+C） |

進捗は標準エラー、結果は標準出力に出ます。

`pull` / `diff` / `push` / `submit` は、Chrome をウィンドウなし（ヘッドレス）で動かすので、作業中に Chrome が前面へ出てきません。
動きを目で確かめたいときは `--show-browser` を付けるとウィンドウを表示します。`login` は手で操作するため、常にウィンドウを表示します。 Google のログインが切れていたときは、`pull`・`diff`・`push`・`submit` が自分でウィンドウつきの Chrome を開き、ログインが終わるまで（最大 10 分）待ち、閉じてから続けます。ログインが完了しなかったとき（時間切れ、ウィンドウを閉じた、同じプロファイルで画面なしの Chrome が残っていて窓を出せない）や、作業の途中でログインが切れたとき（この場合は窓が開かないので、もう一度実行してください）は、終了コード 5 で止まります。

> [!WARNING]
> `--show-browser` は人が画面を見るためのものです。AI エージェントやスクリプトから実行するときは付けないでください。利用者の画面に Chrome のウィンドウが出て、作業の邪魔になります（AI には画面が見えないので、原因調査の役にも立ちません）。
> 失敗の原因は、エラー文（覆っていた要素・要素の枠・画面の大きさを含みます）と、失敗時に保存するページのスナップショット（テキスト。`%LOCALAPPDATA%\Edictora.Data\diagnostics`、保存先はエラー文にも出ます）で調べてください。見えている要素の役割・名前・値・状態・位置を 1 行ずつ並べたものです。`pull` / `diff` の読み出しに失敗したときも保存します。Edictora はスクリーンショットを撮りません。

### 4. AI エージェントから使う（MCP）

`edictora mcp` は、標準入出力で動く MCP サーバーです。Claude Code や Codex などに登録すると、AI が掲載情報を直すときに Edictora を見つけて使えます。

Setup でインストールすると、**Claude Code と Codex には自動で登録します**（それぞれの公式 CLI `claude` / `codex` が PATH にある場合。アンインストールすると、Edictora を指す登録だけを自動で外します）。インストールの時点で CLI が無かった・時間内に終わらなかった場合は、次のログオンでやり直します。一度登録した後に自分で外した登録は、もう一度足しません。
CLI を後から入れた場合も、次のログオンで自動で登録します。Portable 版や、Claude Desktop・ゲートウェイなど上の 2 つ以外のクライアントでは、次のどれかで一度だけ登録してください。
コマンドには、インストール先の exe を絶対パスで指定します（Setup 版は `%LOCALAPPDATA%\Edictora\current\edictora.exe`。自動更新で中身が替わってもこのパスは変わりません。Portable 版は展開した場所の `current\edictora.exe`）。
PATH の `edictora` でも動きますが、インストール前から起動しているクライアントは新しい PATH を知らないため、絶対パスをおすすめします。

**Claude Code**（全プロジェクトで使う）

```powershell
claude mcp add --scope user edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

**Codex**

```powershell
codex mcp add edictora -- "$env:LOCALAPPDATA\Edictora\current\edictora.exe" mcp
```

または `%USERPROFILE%\.codex\config.toml` に書きます（`<ユーザー名>` は置き換える）。

```toml
[mcp_servers.edictora]
command = 'C:\Users\<ユーザー名>\AppData\Local\Edictora\current\edictora.exe'
args = ["mcp"]
```

**Claude Desktop など、JSON の設定ファイルで登録するクライアント**（`<ユーザー名>` は置き換える）

```json
{
  "mcpServers": {
    "edictora": {
      "command": "C:\\Users\\<ユーザー名>\\AppData\\Local\\Edictora\\current\\edictora.exe",
      "args": ["mcp"]
    }
  }
}
```

MCP のゲートウェイ（Toolport など）を使っている場合は、ゲートウェイ側に同じコマンド（stdio、引数 `mcp`）を登録します。

| ツール | 内容 |
|---|---|
| `edictora_list_items` | 設定ファイルの対象（アイテム・ロケール・正本ファイル）を表示（ブラウザを開かない） |
| `edictora_diff` | 正本との差分を表示（読み取りのみ） |
| `edictora_pull` | 現在値を表示（読み取りのみ） |
| `edictora_push` | 差分のある欄を書き換えて下書き保存。`confirm: true` が無ければ差分を表示して止まる |
| `edictora_submit` | 1 件を審査に送信。`item` と `confirm: true` が必須 |

- 中身は CLI と同じ処理で、安全策・終了コードも同じです（結果の先頭に終了コードとその意味が出ます。差分あり（3）は失敗扱いにしません）。
- 画面の表示（`--show-browser`）・`login`・プロファイルや chrome.exe の指定は、ツールから選べません。Chrome は、次に書くログイン用のウィンドウを除き、常にウィンドウなしで動きます。
- ログインが切れていたら、Edictora が利用者の画面にログイン用の Chrome を開き、ログインが終わるまで（最大 10 分）待ってからツールの処理を続けます。完了しなかったときは、ツールが「端末で `edictora login` を実行してください」と返します。
- 呼び出しは 1 件ずつ順に実行します。`notifications/cancelled` で実行中の呼び出しを中断できます。
- 全アイテムの `diff` は数分かかることがあります。クライアントの待ち時間は長めにしてください。

## 安全策

- 既定は差分表示だけで、何も変更しません。書き換えは `push --yes`、送信は `submit --item <id> --yes` を明示したときだけです。
- 下書き保存と審査送信は別の操作です。審査送信は 1 件ずつで、複数アイテムの一括送信はできません。
- 書き換えた欄の値が正本と完全一致することを確かめてから保存し、保存後は読み込み直して再度照合します。一致しなければ保存しない・エラーで止めます。
- 言語ごとに「切り替え → 書き換え → 照合 → 保存 → 読み込み直して照合」を行います。途中で失敗したら、そのアイテムの残りは行いません。
- ステータスが「審査待ち」など、書き込んでよいと確認できない状態では書き込みません。審査の取り消しはしません。
- 欄やボタンはラベル・役割・文言で探し、ちょうど 1 つに絞れないときは止めます（クラス名には依存しません）。
- Chrome は自動化フラグを付けずに起動し、終了時は正常終了させます。**プロファイルは削除しません。**
- 失敗時のページのスナップショット（テキスト）は `%LOCALAPPDATA%\Edictora.Data\diagnostics` に保存します（リポジトリには置きません）。ダッシュボードの欄の値が含まれることがあります（publisherId と、Google アカウントの氏名・メールアドレスは伏せます）。
- 画像の更新は、差分のあるスロットだけを入れ替えます。並べ替えには触れず、審査への送信も呼べません（画像の処理にその経路がありません）。
- 「プライバシー」タブの更新は、差分のある欄だけを書き換え、審査には送信できません（コードに呼び出す手段がありません）。ダッシュボードの権限が設定と合わないときは、何も書かずに止まります。

## 保存場所

| 内容 | 場所 |
|---|---|
| 設定ファイル（正本の対応づけ） | `%APPDATA%\Edictora\config.json` |
| Chrome プロファイル（ログイン情報を含む） | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| ログ（更新の結果を含む） | `%LOCALAPPDATA%\Edictora.Data\logs` |
| 失敗時のページのスナップショット（テキスト） | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| アプリ本体（Setup でインストールしたとき） | `%LOCALAPPDATA%\Edictora`（アンインストールでフォルダーごと消えます） |

データの置き場を `Edictora.Data` としてアプリ本体と分けているのは、アンインストールでログイン済みの Chrome プロファイルまで消えないようにするためです。以前の場所（`%LOCALAPPDATA%\Edictora\chrome-profile` など）からの自動移行はしません。

Chrome プロファイルにはログイン情報（Cookie）が入ります。他人と共有せず、不要になったら利用者自身で削除してください。
ログには、パブリッシャー ID（ダッシュボード URL の一部）を伏せて記録します。

### 設定が見つからないとき（Claude デスクトップアプリ）

Claude デスクトップアプリはパッケージ化（MSIX）されたアプリです。その中から起動したプログラムが `%APPDATA%` や `%LOCALAPPDATA%` の下に**新しい**フォルダーを作ると、Windows は本物の場所ではなくアプリ専用の領域（`%LOCALAPPDATA%\Packages\<パッケージ>\LocalCache\...`）に作り、通常のターミナルからは見えません。Edictora は、設定や Chrome プロファイルをそうした場所から読んでいるときに標準エラーへ警告し、設定が無いときはそこにある複製を案内します。直すには、メッセージに出た本物の場所（例: `%APPDATA%\Edictora\config.json`）へそのファイル（フォルダー）を移してください。Edictora が自動で読んだり移したりすることはありません。

## ビルド

本体は Rust（`crates/edictora/`）です。

```powershell
cargo build --release          # target\release\edictora.exe と edictora-updater.exe（Bun は不要）
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

ダッシュボードのページ内で動かすスクリプトは `page-scripts/`（Bun + TypeScript）にあり、生成した `page-scripts/dist/edictora.js` を exe に埋め込みます。スクリプトを変えたときだけ Bun を使います。

```powershell
cd page-scripts
bun install --frozen-lockfile   # 依存（typescript だけ）を入れる
bun run typecheck      # 型検査
bun run build          # dist/edictora.js を作り直す
bun run check          # dist/edictora.js が最新か確かめる
bun test               # ページ内スクリプトのテスト（偽の DOM で評価）
```

**`sectionStart` / `sectionEnd` の正規表現**: Rust の `regex` を使うため、先読み・後読み（`(?=…)` `(?!…)` `(?<=…)` `(?<!…)`）と後方参照（`\1`）は使えません（設定の読み込み時にエラーになります）。`^■ 説明` のような通常の書き方はそのまま使えます。

検証用に、保存先の根を環境変数 `EDICTORA_LOCAL_ROOT`（既定 `%LOCALAPPDATA%\Edictora.Data`）と `EDICTORA_ROAMING_ROOT`（既定 `%APPDATA%\Edictora`）で、更新の配信元を `EDICTORA_UPDATE_URL`（既定 `https://edictora.kagayoi.com`。平文の `http://` は `127.0.0.1` だけ）で差し替えられます。

Velopack のパッケージには `edictora.exe`（主 exe）と `edictora-updater.exe` の両方を入れます。更新処理は velopack crate を使わず、フィードの取得・ダウンロード・検証を WinHTTP（Windows 標準の HTTP 機能）で直接行い、インストール・差し替え・アンインストールは Velopack の Setup.exe / Update.exe に任せます。

## 配布

配信元は Cloudflare R2 の `https://edictora.kagayoi.com`（Velopack のチャンネル `win`）です。リリースの手順は [docs/release.md](docs/release.md) を参照してください。

## ライセンス

[LICENSE](LICENSE) を参照してください。プライバシーについては [PRIVACY_POLICY.md](PRIVACY_POLICY.md) を参照してください。
