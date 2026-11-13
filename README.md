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

## 必要環境

- Windows 10 / Windows 11（x86_64）
- Google Chrome（自動検出。設定ファイルの `chromePath` か `--chrome` でも指定可能）
- CWS デベロッパー アカウント
- Rust 1.99.1（ソースからビルドする場合。`rust-toolchain.toml` で固定）。ページ内スクリプトを変更するときだけ Bun

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

### Jev に操作対象を確かめさせる（`--jev`、任意）

`pull` / `diff` / `push` / `submit` に `--jev` を付けると、入力欄（説明・URL 欄）・言語の選択欄・言語の選択肢を選ぶたびに、[TypeSafe](https://typesafe.ai/) の分類 API「Jev」にも「どれが目的の要素か」を聞き、Edictora のルールの結果と突き合わせます。Jev は答えるだけで、クリックや入力は Edictora が行います。既定はオフで、`login` では使えません。

```powershell
$env:EDICTORA_JEV_API_TOKEN = '<TypeSafe の API トークン>'   # トークンは環境変数でだけ渡す（config.json には書かない）
edictora diff --jev
edictora push --item <id> --jev --jev-model jev-latest --yes  # モデルは既定 jev-latest
```

- トークンが無いと、Chrome を開く前に終了コード 2 で止まります。トークンは画面・ログ・エラー出力に出しません。
- ルールでちょうど 1 つに決まり、Jev も同じものを選んだときだけ進みます。Jev が別のもの（または「該当なし」）を選んだら止めます。
- ルールで 0 個・複数になったときは、Jev の確信度が 0.8 以上ならその選択を採用し、未満なら止めます（複数のときは、ルールに当てはまった中から選んだ場合だけ採用）。
- どれを採用したかと Jev の確信度は、進捗（標準エラー）とログに出ます。
- 下書き保存・審査送信のボタン、送信の確認ダイアログ、「自動的に公開する」チェック、書き込んでよい状態かの判定は Jev に選ばせず、従来どおりルールだけで行います。そのため `submit --jev` は指定できますが、Jev への問い合わせは起きません。
- 書き込んでよい状態かの判定は説明欄（textarea）がちょうど 1 つあることを前提にしているので、説明欄が複数ある画面では `--jev` でも `push` は止まります（終了コード 4）。
- Jev への通信に失敗したとき（接続できない・30 秒のタイムアウト・エラー応答・応答の形の誤り）は、推測で先へ進まず終了コード 1 で止まります。Ctrl+C で中断できます（終了コード 130）。

**外部へ送る情報（`--jev` 指定時だけ）**: `https://api.typesafe.ai/v1/systemone` へ、次のものだけを送ります。

- 操作の目的（例「ストアの掲載情報の「説明」の入力欄」「言語の選択肢のうち、ロケールコードが en のもの」）
- 候補要素ごとの役割（combobox / option / textbox）・タグ名・input の種類・ラベル行（80 文字以下の行を 8 行まで。文字数カウンタは除く）・短い表示テキスト（言語名とロケールコードの表記だけ）

次のものは送りません: 説明文・URL など欄の値と書き込む値、パブリッシャー ID、ページの URL、Cookie・ログイン情報、スクリーンショット、パブリッシャー（アカウント）名の選択欄・選択肢。`://` を含む行や長い行もラベルから除きます。送った本文（トークンは含まない）はログに記録されるので、後から確かめられます。

## 安全策

- 既定は差分表示だけで、何も変更しません。書き換えは `push --yes`、送信は `submit --item <id> --yes` を明示したときだけです。
- 下書き保存と審査送信は別の操作です。審査送信は 1 件ずつで、複数アイテムの一括送信はできません。
- 書き換えた欄の値が正本と完全一致することを確かめてから保存し、保存後は読み込み直して再度照合します。一致しなければ保存しない・エラーで止めます。
- 言語ごとに「切り替え → 書き換え → 照合 → 保存 → 読み込み直して照合」を行います。途中で失敗したら、そのアイテムの残りは行いません。
- ステータスが「審査待ち」など、書き込んでよいと確認できない状態では書き込みません。審査の取り消しはしません。
- 欄やボタンはラベル・役割・文言で探し、ちょうど 1 つに絞れないときは止めます（クラス名には依存しません）。`--jev` のときだけ、入力欄と言語の選択に限って、確信度 0.8 以上の Jev の選択で補います。
- Chrome は自動化フラグを付けずに起動し、終了時は正常終了させます。**プロファイルは削除しません。**
- 失敗時の画面は `%LOCALAPPDATA%\Edictora\diagnostics` に保存します（リポジトリには置きません）。

## 保存場所

| 内容 | 場所 |
|---|---|
| 設定ファイル（正本の対応づけ） | `%APPDATA%\Edictora\config.json` |
| Chrome プロファイル（ログイン情報を含む） | `%LOCALAPPDATA%\Edictora\chrome-profile` |
| ログ | `%LOCALAPPDATA%\Edictora\logs` |
| 失敗時の画面 | `%LOCALAPPDATA%\Edictora\diagnostics` |

Chrome プロファイルにはログイン情報（Cookie）が入ります。他人と共有せず、不要になったら利用者自身で削除してください。
ログには、パブリッシャー ID（ダッシュボード URL の一部）を伏せて記録します。

## ビルド

本体は Rust（`crates/edictora/`）です。

```powershell
cargo build --release          # target\release\edictora.exe（Bun は不要）
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

検証用に、保存先の根を環境変数 `EDICTORA_LOCAL_ROOT`（既定 `%LOCALAPPDATA%\Edictora`）と `EDICTORA_ROAMING_ROOT`（既定 `%APPDATA%\Edictora`）で差し替えられます。

## 配布

配布先（R2 バケット・ドメイン）は未確定です。決まるまで配布物は公開していません。

## ライセンス

[LICENSE](LICENSE) を参照してください。プライバシーについては [PRIVACY_POLICY.md](PRIVACY_POLICY.md) を参照してください。
