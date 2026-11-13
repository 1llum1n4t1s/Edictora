<p align="center">
  <img src="icon/app_icon.png" alt="Edictora アイコン" width="128">
</p>

# Edictora

**Chrome Web Store の掲載情報（説明文・URL）を、各リポジトリの正本ファイルから一括で読み出し・比較・更新する Windows ツール**

Chrome Web Store（CWS）の公開 API（v2）には、掲載情報の読み書きがありません。拡張機能から CWS のページを操作することも Chrome が許可しません。
Edictora は、専用プロファイルの Chrome を Chrome DevTools Protocol（CDP）で操作し、デベロッパー ダッシュボードの画面を人と同じ入力で書き換えます。
コマンドライン（`edictora.exe`）と画面（Edictora.UI）の両方を同梱し、どちらも同じ処理を使います。

> [!IMPORTANT]
> ダッシュボードの画面構成に依存するツールです。ダッシュボードは**日本語表示**で使ってください（英語表示は未対応）。
> 画面が変わって欄やボタンを特定できなくなった場合は、何も書き換えずに止まります。

## できること

- **pull** — ダッシュボードの現在値（説明・公式 URL・ホームページ URL・サポート URL）を読み出す（読み取りのみ）
- **diff** — 正本ファイルとの差分を表示する（読み取りのみ。既定の動作）
- **push** — 差分のある欄だけを書き換えて**下書き保存**し、読み込み直して正本と一致するか照合する
- **submit** — 1 件ずつ審査に送信する（「審査の合格後に自動的に公開する」の設定は変更しません）
- **login** — 専用プロファイルの Chrome を開き、Google へのログインを利用者に行ってもらう

## 必要環境

- Windows 10 (20348) / Windows 11 以降
- Google Chrome（自動検出。設定ファイルの `chromePath` か `--chrome` でも指定可能）
- CWS デベロッパー アカウント
- .NET 11 SDK（ソースからビルドする場合）

## 使い方

### 1. 設定ファイルを書く

既定の場所は `%APPDATA%\Edictora\config.json` です（`--config` で変更できます）。
アイテムごとに、ロケールと正本ファイルの対応を書きます。**書いていない欄は、読み出し・比較・書き換えのどれにも使いません。**

```jsonc
{
  // 任意。省略時は %LOCALAPPDATA%\Edictora\chrome-profile
  "profileDir": "%LOCALAPPDATA%\\Edictora\\chrome-profile",
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

### 2. ログインする

```powershell
edictora login
```

専用プロファイルの Chrome が開きます。その画面で Google にログインし、デベロッパー ダッシュボードを表示してください。到達を確認すると、ログイン状態をプロファイルに残して終了します。
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
| 5 | ログインが必要 |
| 130 | 中断（Ctrl+C） |

進捗は標準エラー、結果は標準出力に出ます。

### 画面（Edictora.UI）

アイテム × ロケールの一覧（一致 / 差分あり / 未取得 / エラー）と、選んだ行の現在値・正本の左右比較を表示します。
「現在値を取得」で読み出し、チェックした行だけを「下書き保存」、選択中のアイテム 1 件を「審査に送信」できます（どちらも確認ダイアログが出ます）。

## 安全策

- 既定は差分表示だけで、何も変更しません。書き換えは `push --yes`、送信は `submit --item <id> --yes` を明示したときだけです。
- 下書き保存と審査送信は別の操作です。審査送信は 1 件ずつで、複数アイテムの一括送信はできません。
- 書き換えた欄の値が正本と完全一致することを確かめてから保存し、保存後は読み込み直して再度照合します。一致しなければ保存しない・エラーで止めます。
- 言語ごとに「切り替え → 書き換え → 照合 → 保存 → 読み込み直して照合」を行います。途中で失敗したら、そのアイテムの残りは行いません。
- ステータスが「審査待ち」など、書き込んでよいと確認できない状態では書き込みません。審査の取り消しはしません。
- 欄やボタンはラベル・役割・文言で探し、ちょうど 1 つに絞れないときは止めます（クラス名には依存しません）。
- Chrome は自動化フラグを付けずに起動し、終了時は正常終了させます。**プロファイルは削除しません。**
- 失敗時の画面は `%LOCALAPPDATA%\Edictora\diagnostics` に保存します（リポジトリには置きません）。

## 保存場所

| 内容 | 場所 |
|---|---|
| 設定ファイル（正本の対応づけ） | `%APPDATA%\Edictora\config.json` |
| 画面の設定 | `%APPDATA%\Edictora\settings.json` |
| Chrome プロファイル（ログイン情報を含む） | `%LOCALAPPDATA%\Edictora\chrome-profile` |
| ログ | `%LOCALAPPDATA%\Edictora\logs` |
| 失敗時の画面 | `%LOCALAPPDATA%\Edictora\diagnostics` |

Chrome プロファイルにはログイン情報（Cookie）が入ります。他人と共有せず、不要になったら利用者自身で削除してください。
ログには、パブリッシャー ID（ダッシュボード URL の一部）を伏せて記録します。

## ビルド

```powershell
dotnet build Edictora.slnx -c Release
dotnet test Edictora.slnx -c Release
dotnet publish src/Edictora.Cli/Edictora.Cli.csproj -c Release -r win-x64   # NativeAOT の edictora.exe
```

## 配布

配布先（R2 バケット・ドメイン）は未確定です。決まるまで配布物は公開していません。

## ライセンス

[LICENSE](LICENSE) を参照してください。プライバシーについては [PRIVACY_POLICY.md](PRIVACY_POLICY.md) を参照してください。
