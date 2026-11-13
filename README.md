<p align="center">
  <img src="icon/app_icon.png" alt="Edictora アイコン" width="128">
</p>

# Edictora

**Chrome Web Store の掲載情報（説明文・URL）を、各リポジトリの正本ファイルから一括で読み出し・比較・更新する Windows ツール**

Chrome Web Store（CWS）の公開 API（v2）には、掲載情報の読み書きがありません。拡張機能から CWS のページを操作することも Chrome が許可しません。
Edictora は、専用プロファイルの Chrome を Chrome DevTools Protocol（CDP）で操作し、デベロッパー ダッシュボードの画面を人と同じ入力で書き換えます。
コマンドライン ツール（`edictora.exe`）1 つで動きます。Rust 製で、実行に .NET や Node.js などのランタイムは要りません。

> [!IMPORTANT]
> ダッシュボードの画面構成に依存するツールです。ダッシュボードは**日本語表示**で使ってください（英語表示は未対応）。
> 画面が変わって欄やボタンを特定できなくなった場合は、何も書き換えずに止まります。

## できること

- **pull** — ダッシュボードの現在値（説明・公式 URL・ホームページ URL・サポート URL）を読み出す（読み取りのみ）
- **diff** — 正本ファイルとの差分を表示する（読み取りのみ。既定の動作）
- **push** — 差分のある欄だけを書き換えて**下書き保存**し、読み込み直して正本と一致するか照合する
- **submit** — 1 件ずつ審査に送信する（「審査の合格後に自動的に公開する」の設定は変更しません）
- **login** — 専用プロファイルの Chrome を開き、Google へのログインを利用者に行ってもらう
- **update** — 新しい版があればダウンロードして差し替える（Setup でインストールしたときだけ。ログオン時にも自動で行う）

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
| 5 | ログインが必要 |
| 130 | 中断（Ctrl+C） |

進捗は標準エラー、結果は標準出力に出ます。

`pull` / `diff` / `push` / `submit` は、Chrome をウィンドウなし（ヘッドレス）で動かすので、作業中に Chrome が前面へ出てきません。
動きを目で確かめたいときは `--show-browser` を付けるとウィンドウを表示します。`login` は手で操作するため、常にウィンドウを表示します。

## 安全策

- 既定は差分表示だけで、何も変更しません。書き換えは `push --yes`、送信は `submit --item <id> --yes` を明示したときだけです。
- 下書き保存と審査送信は別の操作です。審査送信は 1 件ずつで、複数アイテムの一括送信はできません。
- 書き換えた欄の値が正本と完全一致することを確かめてから保存し、保存後は読み込み直して再度照合します。一致しなければ保存しない・エラーで止めます。
- 言語ごとに「切り替え → 書き換え → 照合 → 保存 → 読み込み直して照合」を行います。途中で失敗したら、そのアイテムの残りは行いません。
- ステータスが「審査待ち」など、書き込んでよいと確認できない状態では書き込みません。審査の取り消しはしません。
- 欄やボタンはラベル・役割・文言で探し、ちょうど 1 つに絞れないときは止めます（クラス名には依存しません）。
- Chrome は自動化フラグを付けずに起動し、終了時は正常終了させます。**プロファイルは削除しません。**
- 失敗時の画面は `%LOCALAPPDATA%\Edictora.Data\diagnostics` に保存します（リポジトリには置きません）。

## 保存場所

| 内容 | 場所 |
|---|---|
| 設定ファイル（正本の対応づけ） | `%APPDATA%\Edictora\config.json` |
| Chrome プロファイル（ログイン情報を含む） | `%LOCALAPPDATA%\Edictora.Data\chrome-profile` |
| ログ（更新の結果を含む） | `%LOCALAPPDATA%\Edictora.Data\logs` |
| 失敗時の画面 | `%LOCALAPPDATA%\Edictora.Data\diagnostics` |
| アプリ本体（Setup でインストールしたとき） | `%LOCALAPPDATA%\Edictora`（アンインストールでフォルダーごと消えます） |

データの置き場を `Edictora.Data` としてアプリ本体と分けているのは、アンインストールでログイン済みの Chrome プロファイルまで消えないようにするためです。以前の場所（`%LOCALAPPDATA%\Edictora\chrome-profile` など）からの自動移行はしません。

Chrome プロファイルにはログイン情報（Cookie）が入ります。他人と共有せず、不要になったら利用者自身で削除してください。
ログには、パブリッシャー ID（ダッシュボード URL の一部）を伏せて記録します。

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
```

**`sectionStart` / `sectionEnd` の正規表現**: Rust の `regex` を使うため、先読み・後読み（`(?=…)` `(?!…)` `(?<=…)` `(?<!…)`）と後方参照（`\1`）は使えません（設定の読み込み時にエラーになります）。`^■ 説明` のような通常の書き方はそのまま使えます。

検証用に、保存先の根を環境変数 `EDICTORA_LOCAL_ROOT`（既定 `%LOCALAPPDATA%\Edictora.Data`）と `EDICTORA_ROAMING_ROOT`（既定 `%APPDATA%\Edictora`）で、更新の配信元を `EDICTORA_UPDATE_URL`（既定 `https://edictora.kagayoi.com`。平文の `http://` は `127.0.0.1` だけ）で差し替えられます。

Velopack のパッケージには `edictora.exe`（主 exe）と `edictora-updater.exe` の両方を入れます。更新処理は velopack crate を使わず、フィードの取得・ダウンロード・検証を WinHTTP（Windows 標準の HTTP 機能）で直接行い、インストール・差し替え・アンインストールは Velopack の Setup.exe / Update.exe に任せます。

## 配布

配信元は Cloudflare R2 の `https://edictora.kagayoi.com`（Velopack のチャンネル `win`）です。リリースの手順は [docs/release.md](docs/release.md) を参照してください。

## ライセンス

[LICENSE](LICENSE) を参照してください。プライバシーについては [PRIVACY_POLICY.md](PRIVACY_POLICY.md) を参照してください。
