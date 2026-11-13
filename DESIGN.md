# Edictora 設計

Chrome Web Store（CWS）の掲載情報（説明・公式 URL・ホームページ URL・サポート URL）を、各拡張機能リポジトリにある正本ファイルから一括で読み出し・比較・更新する Windows 向け CLI（`edictora.exe`）。ほかの人にも配る前提で、画面（GUI）は持たない。
利用者向けの使い方は [README.md](README.md)、開発者・LLM 向けの規約とコマンドは [AGENTS.md](AGENTS.md)、リリース手順は [docs/release.md](docs/release.md) を参照する。

## 背景と目的

- CWS API v2 には掲載情報（説明・URL など）の読み書きがない。旧 v1.1 の metadata API は 2026-10-15 で廃止済み。
- 拡張機能（Claude in Chrome など）からの操作は、Chrome が「The extensions gallery cannot be scripted.」で拒否する。
- Chrome DevTools Protocol（CDP）での直接操作はこの制限を受けない（2026-11-13 の実画面検証で確認）。
- そのため Edictora は、専用プロファイルの Chrome を CDP で操作し、デベロッパー ダッシュボードの画面を人と同じ入力イベントで書き換える。ダッシュボードの内部 RPC は呼ばない。
- 分担: cws-mcp は パッケージ（ZIP）のアップロードと公開（CWS API v2）、Edictora は掲載情報の更新（API がないため画面操作）。

## 構成

2 本立てで、画面操作の「判断」は Rust、ページ内での「情報収集」だけを TypeScript に分ける。

- `crates/edictora`（Rust、lib + bin `edictora.exe`・`edictora-updater.exe`）: 設定・比較・Chrome 起動・CDP・ダッシュボード操作・手順・CLI・自動更新。外部の引数パーサ・CDP ライブラリ・HTTP/TLS crate は使わない（自動更新の HTTPS は WinHTTP を FFI で直接使う）。
- `page-scripts`（Bun + TypeScript）: ページ内で評価するスクリプトだけ。生成物 `page-scripts/dist/edictora.js` をコミットし、`include_str!` で exe に埋め込む。`cargo build` と配布 exe に Bun は要らない。

### 本体の内訳（`crates/edictora/src/`）

| モジュール | 責務 |
|---|---|
| `cdp/` | CDP クライアント（localhost の WebSocket、TLS なし、`tungstenite` の同期 API）とページ単位の操作（評価・マウス・キー・遷移） |
| `chrome/` | Chrome の検出と起動。専用プロファイルで起動するか、そのプロファイルの Chrome がデバッグポート付きで開いていれば接続する。終了は `Browser.close` |
| `config/` | 設定ファイルの読込と検証（未知のキーはエラー）、正本の区間切り出し |
| `listing/` | 欄の定義、正本と現在値の完全一致の比較、行単位の差分表示 |
| `dashboard/` | ダッシュボード操作。`urls.rs`（publisherId の取り出し）/ `text.rs`（画面の文言）/ `page_scripts.rs`（埋め込み JS の呼び出し）/ `selectors.rs`（要素の選択）/ `write_gate.rs`（書き込み可否）/ `item_page.rs`（1 操作ずつの実装）/ `browser.rs`（接続とログイン確認）。trait `ListingPage` が手順と画面操作の境目 |
| `net/` | `winhttp.rs`: WinHTTP を FFI で直接使う HTTP クライアント（GET だけ、メモリかファイルへ受ける）。自動更新が使う |
| `update/` | 自動更新。`locate.rs`（インストールの判定）/ `manifest.rs`（`sq.version`）/ `feed.rs`（フィードと更新先の選択）/ `version.rs`（SemVer）/ `source.rs`（配信元）/ `download.rs`（ダウンロードと検証）/ `hash.rs`（CNG）/ `lock.rs` / `apply.rs`（Update.exe の起動。trait `ApplyLauncher` が境目） |
| `install/` | Velopack のフック（`hooks.rs`）と HKCU の Run・PATH の読み書き（`registry.rs`） |
| `services.rs` | pull・diff・push・submit の手順と中止判断 |
| `cli/` | 引数解析・実行と終了コード・差分表示 |
| `logging.rs` ほか | ログ（publisherId を伏せる）、保存先（`AppPaths`）、エラー型、Ctrl+C |

### 役割分担の考え方

- **手順は `services.rs`、CLI は呼ぶだけ。** 画面操作は trait `ListingPage` 越しに行うので、手順は偽ページでブラウザなしにテストできる。
- **ページ内 JS は候補要素の情報（タグ・文字・ラベル候補・無効/読み取り専用・`aria-expanded`・チェック状態）を集めて返すだけ。** どれを選ぶか・書き込んでよいかは Rust の純粋関数（`selectors.rs`・`write_gate.rs`）が決め、テストで固める。利用者の入力値はページ内スクリプトに埋め込まない（書き込む文字列は `Input.insertText` で渡す）。
- **CDP**: 入出力スレッド 1 本が非ブロッキングのソケットを持ち、応答は id ごと、イベントは購読ごとのチャネルへ配る。応答待ちの間も `Page.loadEventFired` と beforeunload の `Page.javascriptDialogOpening` を受け取れる。切断時は応答待ちと購読をすべて失敗させる。
- **中断**: Ctrl+C は `SetConsoleCtrlHandler` で受け、待ちの各所が合図を見て抜ける（終了コード 130）。その後の `Browser.close` と終了待ちは合図を見ないので、Chrome は正常終了する。
- **保存先の注入**: `AppPaths` を値として持ち回し、`main` だけが環境（`EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` による上書きを含む）から決める。テストは `target/tmp` 配下を根にする。
- **環境変数は入口だけが読む**: `EDICTORA_UPDATE_URL` は `main`（と更新用 exe の入口）が読み、値で渡す。
- **正規表現**: 設定の `sectionStart` / `sectionEnd` は Rust の `regex` で解釈する。先読み・後読み・後方参照は使えない（読み込み時にエラー）。どのパターンも線形時間で終わるので、時間による打ち切りは持たない。ログの publisherId 伏せ字も `regex` で行う。

## データフロー

1. **入力（正本）**: `%APPDATA%\Edictora\config.json`（`--config` で変更可）。`profileDir`・`chromePath`（任意）と `items`（`itemId`・`name`・`baseDir`・`locales`）。ロケールごとに `description`（`file`、任意で `sectionStart` / `sectionEnd`）と `homepageUrl` / `supportUrl` / `officialUrl` を書く。書いていない欄は読み出し・比較・書き換えの対象外。URL 欄はキーなし＝対象外、`""`＝空欄が正本。
2. **正本の読み取り**: 区間は、見出し行の次から終端見出しの手前までを取り、前後の空行と末尾の空白を除き、改行を `\n` に揃える。見出しが無ければエラー。
3. **Chrome 起動**: `login` 以外はヘッドレス。専用プロファイルの Chrome を最小の引数で起動し、ログイン状態を引き継ぐ。
4. **ダッシュボード操作**: アイテムの編集画面を開き、言語を切り替え、欄を読む。
5. **比較**: 正本と現在値を完全一致で比べ、差分を行単位で表示する（`diff`・`push` の確認）。
6. **書き込み（`push --yes`）**: 言語ごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」を行い、最後に全言語を読み直して照合する。
7. **審査送信（`submit --item <id> --yes`）**: 確認ダイアログで送信する。

`update` は 1〜7 を通らず、配信元のフィードを見て新しい版をダウンロードし、Update.exe に差し替えを任せる（下の「配布・インストール・自動更新」）。

結果は標準出力、進捗は標準エラー。終了コードは 0（成功・差分なし）/ 1（実行時エラー）/ 2（引数・設定の誤り、Chrome は開かない）/ 3（差分あり）/ 4（書き込み中止）/ 5（ログインが必要）/ 130（中断）。

保存先: Chrome プロファイル（既定 `%LOCALAPPDATA%\Edictora.Data\chrome-profile`、`profileDir` / `--profile` で変更可）・ログ（`...\logs`）・失敗時の画面（`...\diagnostics`）は `%LOCALAPPDATA%\Edictora.Data`、設定は `%APPDATA%\Edictora`。いずれもリポジトリの外。`%LOCALAPPDATA%\Edictora` は Velopack のインストール先（アンインストールで消える）なので、データを置かない。

## 重要な不変条件

- **プロファイルは秘密情報で、ツールは削除しない。** 削除する経路を持たない。終了は `Browser.close`、応答しないときだけプロセスを止める。ログイン待ちの間はタブに接続しない。パスワードと確認コードはツールが扱わない。
- **publisherId はログイン後の URL から実行時に取り出し、コード・設定・ログに書かない。** ログと CLI のエラー出力は伏せ字処理を通す。
- **既定は読み取りのみ。** 書き換えは `push --yes`、審査送信は `submit --item <id> --yes` を明示したときだけ。下書き保存と審査送信は別の操作で、審査送信は 1 件ずつ。「審査の合格後に自動的に公開する」の状態は変えずに報告する。
- **書き込み可否は許可リスト方式**（`write_gate.rs`）。確認済みの状態（公開済み・下書き）以外、審査待ち・ロック中・判定不能なアイテムには書き込まない。審査の取り消しはしない。
- **保存前と保存後に正本と照合する。** 一致しなければ保存しない・エラーで止める。途中で失敗したら、そのアイテムの残りは行わない。再読込・遷移で beforeunload が出たら、「留まる」を選んで未保存の変更を捨てずに中止する。
- **要素の特定が曖昧なら止める。** 候補がちょうど 1 つに絞れなければ、最初の 1 つを選ばず止める（別の欄を書き換える事故を防ぐ）。
- **Chrome を前面に出さない。** `Page.bringToFront` は使わず、操作の前に毎回 `Emulation.setFocusEmulationEnabled` を行う。
- **引数と設定の誤りは Chrome を開く前に終了コード 2 で止める。**
- **失敗時の画面保存はリポジトリの外**（`%LOCALAPPDATA%\Edictora.Data\diagnostics`）に置く。
- **検証していないパッケージは適用しない。** サイズとハッシュがフィードと一致したものだけを Update.exe に渡す。

## 採用済みの設計判断

### Chrome の起動

- 引数は `--user-data-dir`・`--remote-debugging-port=0`（`DevToolsActivePort` でポート取得）・`--no-first-run`・`--no-default-browser-check`、描画の間引き抑止 3 つ（`--disable-backgrounding-occluded-windows`・`--disable-renderer-backgrounding`・`--disable-background-timer-throttling`）、`--disable-features=CalculateNativeWinOcclusion`、ヘッドレス時だけ `--headless=new --window-size=1280,900`。Chrome の標準出力・標準エラーは捨てる。
- **自動化フラグ（`--enable-automation` など）と画面外配置は付けない。** 実画面で、これらを付けない普通の Chrome なら利用者が手動で Google にログインでき、`navigator.webdriver` も `false` だった。足すとログインが弾かれる恐れがあるので、変える場合は実画面でログインを再検証する。
- **ヘッドレスで動かす。** ウィンドウが裏に回る・画面外にあると描画が止まりクリックが応答しないが、`CalculateNativeWinOcclusion` の無効化と間引き抑止フラグがあれば前面に出さなくても動く。`--headless=new` は同じプロファイルのログイン状態をそのまま使え、実行中に前面のウィンドウが変わらなかった（画面外配置・最小化の起動は、起動時に Chrome が前面へ来た）。`HeadlessChrome` を含む UA は `Emulation.setUserAgentOverride` で `Chrome` に置き換える（置き換えない状態は未検証）。
- トレードオフ: ヘッドレスでは利用者が動きを見られない。`--show-browser` で表示できる。`login` は手動操作のため常に表示する。

### 入力操作

- クリックは scrollIntoView 後に矩形中心へ実際のマウス入力（`clickCount: 1`）を送り、中心が他の要素に覆われていたら押さない。
- 説明欄の書き換えはフォーカス → 全選択 → `Input.insertText`（空にするときは Delete キー）。ページ側は通常の入力として受け付け、文字数カウンタも更新される。DOM の `value` への直接代入は、フォームに変更として認識されない可能性があるので使わない。
- 待ちは固定の sleep ではなく、条件を一定間隔で確かめる（上限時間つき）。

### 画面構成と欄の探し方

- アイテム一覧は `https://chrome.google.com/webstore/devconsole/<publisherId>`、編集画面は `.../<publisherId>/<itemId>/edit`。ダッシュボードは日本語表示が前提で、文言は `dashboard/text.rs` に集める。
- 「ストアの掲載情報」で編集できる欄: 説明（`textarea`、最大 16,000 文字、カウンタ「n/16,000」付き）、公式 URL・ホームページ URL・サポート URL（`input[type=text]`）。カテゴリ・プロモーション動画・画像は対象外。「パッケージのタイトル／概要」は manifest 由来の読み取り専用。
- 編集中の言語は「言語」ラベルを持つ `[role=combobox]` で切り替える。選択肢は `[role=option]` で、表記は「英語 – en （デフォルト）」「日本語 – ja」の形。最初に見つかる `[role=combobox]` はパブリッシャーの選択欄なので、ラベルで絞り込む。
- **ラベルは「1 行の完全一致」で照合する。** ラベル候補には欄の親要素の文字列（説明文を含む）も入るため、部分一致だとカテゴリ欄の説明「すべての言語用」が「言語」に当たって誤判定する。改行で区切った 1 行が、空白を無視してラベルと完全一致するものだけを選ぶ。URL 欄も同じ。
- **選択肢の一覧は、閉じても要素が残る。** 閉じた `[role=listbox]` は枠が 0×0 になるだけで、中の `[role=option]` は表示扱いのまま残る。パブリッシャー名の選択肢は常に表示されている。そのため、選択肢は枠に大きさのある一覧の中のものだけ数え、開いたかは「言語 – code」形式の選択肢の出現で、閉じたかは言語欄の `aria-expanded` が `"false"` になったかで判断する。
- ボタンは「下書きとして保存する」「審査のため送信」「公開」。変更がなくても無効化されない。保存後は「アイテムが保存されました。」が出る。
- 「審査のため送信」を押すと確認ダイアログ（`[role=dialog]`）が出る。文面は「「<名前>」を審査のために送信しますか？」、チェックボックス「審査の合格後に「<名前>」を自動的に公開する」（既定 ON）、ボタン「キャンセル」「審査のため送信」。確定すると「アイテムを送信しました。」が出てステータスが「審査待ち」になる。
- 「ガイドやヒントが必要な場合は…」という常設の通知が `[aria-live]` 領域にあるので、結果メッセージの判定では除外する。

### 区間切り出し

- 切り出し結果は検証スクリプト（push.mjs）と同じにする。`crates/edictora/tests/fixtures/ReplaceTranslator` の SHA-256・文字数（ja=1492 / en=3172）のテストで固める。

### 配布・インストール・自動更新（Velopack）

- **配布物**: Velopack（vpk 1.2.161、packId `Edictora`、チャンネル `win`）のパッケージ。主 exe は `edictora.exe`、同じパッケージに更新用の `edictora-updater.exe` を入れる。Setup で `%LOCALAPPDATA%\Edictora`（以下「根」）に入る（`Update.exe`・`current\`・`packages\`）。配信元は Cloudflare R2 の `https://edictora.kagayoi.com`。署名・パッケージ作成・配信の手順は [docs/release.md](docs/release.md)。
- **役割の分け方**: インストール・アンインストール・`current` の差し替えは Velopack の Setup.exe / Update.exe に任せる。Edictora は**更新の確認とダウンロードだけ**を自前で行う（`update/`）。velopack crate は ureq / rustls を引き込むので使わない。通信は `net/winhttp.rs`。
- **約束は vendored の Velopack 1.2.161 Rust SDK に合わせる**（Petarin の `desktop/src-tauri/vendor/velopack-1.2.161`）:
  - インストールの判定（`locator.rs`）: exe の親か、パスの最後の `\current\` より前に `Update.exe` があればそこが根。`<根>\Update.exe` と `<根>\current\sq.version` が無ければ「インストールされていない」。現在の版・id・チャンネル・mainExe は `sq.version`（nuspec XML）から読む。チャンネルが空なら `win`。
  - フィード（`sources/http.rs`・`manager.rs`）: `<配信元>/releases.{channel}.json`。`{"Assets":[{PackageId, Version, Type, FileName, SHA1, SHA256, Size, …}]}` でキーは大文字小文字を区別しない。Type が `Full` の最大の版が現在より新しければ更新（ダウングレードしない・差分パッケージは使わない）。問い合わせ文字列は `localVersion` と `id` だけ（SDK が付ける端末固有の `stagingId` は送らない）。
  - ダウンロード（`manager.rs` の `download_updates`）: `<packages>\.velopack_lock` を持ち、`<FileName の拡張子を .partial にしたもの>` に保存 → サイズと SHA256（無ければ SHA1）を検証 → `<packages>\<FileName>` へ改名 → 前からあった `*.nupkg`・`*.partial`・`*-delta.nupkg` を消す。検証に失敗したら適用しない。ハッシュは CNG（bcrypt.dll）で計算する。
  - 適用（`manager.rs` の `unsafe_apply_updates`）: `Update.exe apply --package <nupkg> --waitPid <自分> --silent --norestart --root <根> --packageDir <packages>` を `CREATE_NO_WINDOW` で起動し、起動した側はすぐに終了する。Update.exe が終了を待って差し替える。
  - フック（`app.rs`・`constants.rs`）: 引数が 2 つ以上で 1 つ目（小文字にして）が `--veloapp-install` / `--veloapp-updated` / `--veloapp-obsolete` / `--veloapp-uninstall` ならフック。処理後は必ず終了コード 0。install / uninstall は 30 秒、updated / obsolete は 15 秒で止められる。
- **SDK と違う点（意図的）**: ダウンロード済みのパッケージも検証し直す（SDK は検証せずに使う）。宣言の Size を上限にダウンロードを打ち切る。nupkg から `Squirrel.exe` を取り出して `Update.exe` を差し替える処理は持たない（zip を展開しないため。Update.exe はインストール時の版のまま）。起動時の自動適用（SDK の `VelopackApp::run` がする、ダウンロード済みの新しい版の適用）はしない（ダウンロードしたその場で適用を任せるため）。
- **フックで行うこと**（`install/hooks.rs`。`edictora.exe` と `edictora-updater.exe` の両方が引数の解釈より前に処理する）: install / updated で HKCU の `Run` に `"<根>\current\edictora-updater.exe"` を登録し、ユーザー環境変数 `Path` に `<根>\current` を足して `WM_SETTINGCHANGE` を通知する。uninstall で両方を取り除く（Run は現在の値が自分の値と一致するときだけ消す）。obsolete は何もしない。Portable 版（根に `.portable`）では install / updated でも登録しない（利用者が置いた場所をログオン時に起動させない。手動の `edictora update` は使える）。Run の同じ名前の値が別の場所の更新用 exe を指していれば上書きせず、警告をログへ残す（根のフォルダー名が同じ別のインストールを壊さない）。Run の値の名前（`<根のフォルダー名> Updater`）と PATH の要素は根から決めるので、packId を変えた検証用インストールと衝突しない。PATH は既存の型（REG_SZ / REG_EXPAND_SZ）を保ち、他の要素は変えない（比較のときだけ `%VAR%` を展開）。インストール先から起動されていなければレジストリに触れない。
- **初回起動の扱い**: Velopack に引数なしで起動された（`VELOPACK_FIRSTRUN` / `VELOPACK_RESTART` がある）`edictora.exe` は、既定の `diff`（Chrome を開く）を走らせずに終える。
- **更新の入口**: ログオン時の `edictora-updater.exe`（`windows_subsystem = "windows"`、結果はログ、失敗は 30 秒・90 秒の間隔で 2 回まで試し直す）と、手動の `edictora update`（設定を読まない・Chrome を開かない・結果は標準エラー、最新と更新の開始は 0、失敗と「別の更新処理が動いている」は 1、配信元の差し替えの誤りは 2、中断は 130）。両者は `<packages>\.edictora-update.lock` で排他する。
- **実行中の edictora との排他**: Update.exe の apply はインストール先配下のプロセスを止める可能性があるので、Chrome を開くコマンド（login / pull / diff / push / submit）はブラウザ操作の間だけ `<データの根>\edictora-running.lock` を共有で持つ（CLI 同士は同時に動ける）。更新側はダウンロードと検証の後、Update.exe を起動する直前にこれを排他で取り、取れなければ適用を後に回す（`AppInUse`。`update` は 1、更新用 exe は試し直す。次回はダウンロード済みのものを検証して使う）。取れたらプロセスの終了まで持つ。適用中に Chrome を開くコマンドを実行すると、Chrome を開かずに 1 で止まる。ロックファイルを開けないだけなら CLI は続行する（インストールされていない実行でも無害）。`pull` / `diff` / `push` / `submit` の中では更新を確かめない。インストールされていなければ、更新用 exe は何もせず、`edictora update` は失敗（1）にする。
- **配信元の差し替え**: 検証用に環境変数 `EDICTORA_UPDATE_URL`（読むのは各 `main` だけ）。平文の http は 127.0.0.1 だけ。CLI のテストは偽の更新処理を差し込み、`CliApp` の既定（`refuse_update`）では本物の配信元・Update.exe に届かない。
- **データの置き場を根と分ける**: 根はアンインストールでフォルダーごと消えるので、Chrome プロファイル・ログ・診断情報は `%LOCALAPPDATA%\Edictora.Data` に置く（旧 `%LOCALAPPDATA%\Edictora` からの自動移行はしない）。

## 実画面での確認状況

- 確認済み: ログインとプロファイル、`pull` / `diff`（ヘッドレス、Chrome が前面に出ないこと）、言語切替、説明欄の書き換えと保存・読み込み直し後の照合（フェーズ 0 の検証）、審査送信ダイアログの文面。
- 未確認（`write_gate.rs` の許可リストはこれらを確かめて更新する）:
  - Rust 版の `push` / `submit`、Ctrl+C での中断、beforeunload の扱い。
  - 審査待ちのアイテムの編集画面（編集がロックされるか）。
  - 言語が 1 つだけのアイテム、ja / en 以外の言語を持つアイテム。
  - URL 欄（公式・ホームページ・サポート）の書き換えと保存。
  - 15 件の全アイテムでの読み出し。
- 単体テストは 127.0.0.1 の偽配信元・専用ハイブ・偽の Update.exe 起動で行う。自動更新の結合は、検証用 packId `EdictoraTest` で実機確認済み: Install フックで Run（`EdictoraTest Updater`）と PATH（REG_EXPAND_SZ のまま）を登録、`edictora update` で 1.0.5 → 1.0.6（Obsolete / Updated フック）、`edictora-updater.exe` で 1.0.6 → 1.0.7、`Update.exe uninstall` で Uninstall フックが Run と PATH から自分の分だけを消し `Edictora.Data` は残る。未確認: 実署名での一連、R2 への実配信、本番の HTTPS（edictora.kagayoi.com）、本番 packId `Edictora` での Setup、Portable 版の一連、Setup 直後の初回起動の見え方（`--silent` の Setup は主 exe を起動しないらしい）、実行中ロックで適用を後に回す動作の実機確認。
- ダッシュボードの英語表示は未対応（文言で要素を探すため）。
