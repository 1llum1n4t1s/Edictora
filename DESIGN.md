# Edictora 設計

Chrome Web Store（CWS）の掲載情報を、各拡張機能リポジトリにある正本ファイルから一括で読み出し・比較・更新する Windows 向けツール。
本リポジトリは旧 ExtWeigh（開発中止）を作り替えたもの。最初は ExtWeigh の CDP クライアント・Chrome 起動処理・ログ・ビルド基盤（C#）を流用したが、現在は Rust で作り直している（下の「実装（Rust + Bun/TypeScript）」）。

## 背景と前提

- CWS API v2 には掲載情報（説明・URL など）の読み書きがない。旧 v1.1 の metadata API は 2026-10-15 で廃止済み。
- 拡張機能（Claude in Chrome など）からの操作は、Chrome が「The extensions gallery cannot be scripted.」で拒否する。
- Chrome DevTools Protocol（CDP）での直接操作は、この制限を受けない（2026-11-13 の検証で確認、下記）。
- そのため Edictora は、専用プロファイルの Chrome を CDP で操作して、デベロッパー ダッシュボードの画面を書き換える。
- ダッシュボードの内部 RPC は呼ばない。画面の欄を、人が操作するのと同じ入力イベントで書き換える。

## 2026-11-13 の検証結果（フェーズ 0）

puppeteer（CDP）で実際のダッシュボードを操作して確かめた事実。実装はこれを満たすこと。

### ログインとプロファイル

- 専用の `--user-data-dir` と `--remote-debugging-port` だけを付けた普通の Chrome（`--enable-automation` なし）で、利用者が手動で Google にログインできた。
  ページ側の `navigator.webdriver` は `false`。
- 自動化フラグや計測用フラグを足すと、ログインが弾かれる可能性がある。起動引数は最小限に保ち、変える場合は再検証する。
- ログイン済みプロファイルは秘密情報として扱う。リポジトリの外に置き、ツールが削除しない（旧 ExtWeigh の `ChromeInstance.DisposeAsync` はプロファイルを削除するため、この経路を通さない）。
- パスワードと確認コードは、ツールもエージェントも扱わない。

### 画面構成

- アイテム一覧: `https://chrome.google.com/webstore/devconsole/<publisherId>`。publisherId は実行時に、ログイン後の URL から取得する（設定やコードに書かない）。
- 編集画面: `.../<publisherId>/<itemId>/edit`。左のナビゲーションに「ステータス・パッケージ・ストアの掲載情報・プライバシー・販売地域」がある。
- ストアの掲載情報で編集できる欄:
  - 説明（`textarea`、最大 16,000 文字、文字数カウンタ「n/16,000」付き）
  - 公式 URL・ホームページ URL・サポート URL（`input[type=text]`）
  - カテゴリ（`[role=combobox]`）、プロモーション動画の URL、画像
- 「パッケージのタイトル」「パッケージの概要」は manifest から入る読み取り専用表示で、ダッシュボードでは編集できない。
- 編集中の言語は「言語」ラベルを持つ `[role=combobox]` で切り替える。選択肢は `[role=option]` で、表記は「英語 – en （デフォルト）」「日本語 – ja」の形。
  - 最初に見つかる `[role=combobox]` はパブリッシャーの選択欄なので、ラベルで絞り込むこと。
  - 言語欄は `aria-labelledby` が指す要素の文字が「言語」。開閉は `aria-expanded`（`"true"`/`"false"`）で分かり、開くと `aria-controls` が一覧（`[role=listbox]`）の id を指す。
- ボタン: 「下書きとして保存する」「審査のため送信」「公開」。変更がなくても無効化はされていなかった。
- URL 欄は `aria-label`（例:「公式 URL」）や、ラベルの行で名前が分かる。カテゴリ欄の後ろには「すべての言語用」という説明がある。

### 欄の探し方（実画面で見つかった落とし穴、2026-11-13）

- **ラベルは「1 行の完全一致」で照合する。** ラベル候補には、欄の親要素の文字列（説明文を含む）も入る。部分一致だと、カテゴリ欄の説明「すべての言語用」が「言語」に当たり、言語欄が 2 つあると誤判定して止まった。改行で区切った 1 行が、空白を無視してラベルと完全一致するものだけを選ぶ。URL 欄も同じ（説明文に別の欄名が含まれていても当たらないようにする）。
- **選択肢の一覧は、閉じても要素が残る。** 閉じた一覧（`[role=listbox]`）は枠が 0×0 になるだけで、中の `[role=option]` は表示扱いのまま残る。さらにパブリッシャー名の選択肢は常に表示されている。そのため次のようにする。
  - 選択肢は、枠に大きさがある一覧の中のものだけを数える。
  - 一覧が開いたかは、「言語 – code」形式の選択肢が現れたかで判断する（パブリッシャー名の選択肢では判断しない）。
  - 一覧を閉じたかは、言語欄の `aria-expanded` が `"false"` になったかで判断する（「選択肢が 0 件になったか」では永久に判断できない）。
- **候補がちょうど 1 つに絞れないときは必ず止める。** 曖昧なまま最初の 1 つを選ぶと、別の欄を書き換える事故につながる。

### 操作のしかた

- ウィンドウが裏に回る・画面外にあると描画が止まり、クリックが応答しなかった。当初は `Page.bringToFront` と `Emulation.setFocusEmulationEnabled` で解決したが、作業中に Chrome が最前面へ割り込むため、同日に次の方式へ改めた。
  - 起動引数に `--disable-features=CalculateNativeWinOcclusion`（と描画の間引き抑止フラグ）を付けると、前面に出さなくてもクリック・言語切替が動いた。
  - `--headless=new` でも同じプロファイルのログイン状態をそのまま使え、言語切替・説明欄の読み出しができた。実行中、前面のウィンドウは一度も変わらなかった（画面外配置・最小化での起動は、起動時に Chrome が前面へ来た）。
  - ヘッドレスの UA に含まれる `HeadlessChrome` は `Emulation.setUserAgentOverride` で `Chrome` に置き換えている（置き換えない状態は未検証）。
  旧 ExtWeigh の起動引数（`--disable-backgrounding-occluded-windows` など）も併用を検討する。画面外配置（`--window-position=-32000,0`）は、使う場合に再検証する。
- 説明欄の書き換え: 欄にフォーカスして全選択し、`Input.insertText` で正本の文を入れる。ページ側は通常の入力として受け付け、文字数カウンタも更新された。
  DOM の `value` を直接代入する方法は、フォームに変更として認識されない可能性があるので使わない。
- 書き換えたあと、欄の値が正本と完全一致することを確かめてから保存する。一致しなければ保存しない。
- 保存後は「アイテムが保存されました。」が表示される。ページを読み込み直して、各言語の値が正本と一致することを確認した。
- 「審査のため送信」を押すと確認ダイアログ（`[role=dialog]`）が出る。
  - 文面:「「<名前>」を審査のために送信しますか？」
  - チェックボックス「審査の合格後に「<名前>」を自動的に公開する」（既定 ON）
  - ボタン「キャンセル」「審査のため送信」
  確定すると「アイテムを送信しました。」が出て、ステータスが「審査待ち」になる。
- 「ガイドやヒントが必要な場合は…」という常設の通知が `[aria-live]` 領域にあるので、結果メッセージの判定では除外する。

### まだ確かめていないこと

- 審査待ちのアイテムの編集画面がどう見えるか（編集がロックされるか）。
- 言語が 1 つだけのアイテムや、ja / en 以外の言語を持つアイテム。
- URL 欄（公式・ホームページ・サポート）の書き換えと保存。
- 15 件の全アイテムでの読み出し。

## 構成方針

- **CLI だけを持つ。** ほかの人にも配る前提のツール。画面（GUI）は持たない（C# + Avalonia 版は 2026-11-13 に廃止し、Rust へ移した。経緯は下の「実装（Rust + Bun/TypeScript）」）。
  - `crates/edictora`（Rust、lib + bin `edictora.exe`）: 設定・比較・ダッシュボード操作・手順・CLI。外部の引数パーサに依存しない。終了コードで成否を区別する。
  - `page-scripts`（Bun + TypeScript）: ページ内で評価するスクリプトだけ。生成物を exe に埋め込む。
  - テスト（`cargo test`）: ブラウザ不要な部分（設定・区間切り出し・差分判定・要素の特定・書き込み可否・CLI 引数・手順・CDP の受信）を網羅する。実ダッシュボードに接続するテストは書かない。
- **本体の内訳**（`crates/edictora/src/`）
  - `cdp/`: CDP クライアント（外部の CDP ライブラリなし）と、ページ単位の操作（評価・マウス・キー・遷移）。
  - `chrome/`: Chrome の検出と起動。専用プロファイルで起動するか、そのプロファイルの Chrome がデバッグポート付きで開いていれば接続する。
    - 起動引数は `--user-data-dir`・`--remote-debugging-port=0`（DevToolsActivePort でポート取得）・`--no-first-run`・`--no-default-browser-check` と、描画の間引き抑止の 3 つ・`--disable-features=CalculateNativeWinOcclusion`、ヘッドレス時だけ `--headless=new --window-size=1280,900`。自動化フラグは付けない。画面外配置もしない。
    - 終了は `Browser.close` で正常終了させ、応答しないときだけプロセスを止める。**プロファイルを削除する経路は持たない。**
  - `config/`: 設定ファイルの読込と検証（未知のキーはエラー）、正本の区間切り出し（push.mjs と同じ結果）。
  - `listing/`: 欄の定義、正本と現在値の比較（完全一致）、行単位の差分表示。
  - `dashboard/`: ダッシュボード操作。責務ごとに分ける。
    - `urls.rs`: publisherId をログイン後の URL から取り出す（コード・設定・ログに書かない。ログは伏せ字にする）。
    - `page_scripts.rs`: 埋め込んだページ内スクリプトの呼び出し。利用者の入力値は埋め込まない。
    - `selectors.rs`: 集めた要素から目的の 1 つを選ぶ純粋関数。ちょうど 1 つに絞れなければ止める。
    - `write_gate.rs`: 書き込み可否の判定。確認済みの状態以外はすべて止める（許可リスト方式）。
    - `item_page.rs`: 1 操作ずつの実装（開く・言語切替・読む・書く・保存・送信）。
  - `services.rs`: pull / diff / push / submit の手順と中止判断。trait `ListingPage` 越しに画面を操作するので、手順はブラウザなしでテストできる。
- **入力（正本）**: `%APPDATA%\Edictora\config.json`（`--config` で変更可）。`profileDir`（任意）・`chromePath`（任意）・`items`（`itemId`・`name`・`baseDir`・`locales`）。
  ロケールごとに `description`（`file`、任意で `sectionStart` / `sectionEnd`）と `homepageUrl` / `supportUrl` / `officialUrl` を書く。書いていない欄は対象外。
  例: `ja` → `webstore/store-listing.txt` の「■ 説明」から「■ カテゴリ」の手前まで。
- **保存先**: Chrome プロファイル（既定 `%LOCALAPPDATA%\Edictora\chrome-profile`、設定と `--profile` で変更可）・ログ・診断情報は `%LOCALAPPDATA%\Edictora`、設定は `%APPDATA%\Edictora`。いずれもリポジトリの外。
- **コマンド**
  - `login`: 専用プロファイルの Chrome を開き、利用者が手動ログインしてダッシュボード（publisherId 付き URL）に着くまで待つ
  - `pull`: ダッシュボードの現在値を読み出す（読み取りのみ）
  - `diff`: 正本との差分を表示する（読み取りのみ・既定）
  - `push`: 差分のある欄を書き換えて下書き保存し、読み直して照合する。`--yes` が無ければ差分を表示して止まる
  - `submit`: 審査に送信する。`--item` 1 件と `--yes` を明示したときだけ行う
- **操作の手順**
  - `login` 以外はヘッドレスで起動する（`--show-browser` でウィンドウ表示）。`login` は利用者が操作するため常にウィンドウを表示する。
  - 操作の前に毎回 `Emulation.setFocusEmulationEnabled` を行う。`Page.bringToFront` は使わない（Chrome を最前面へ出さない）。
  - クリックは要素を画面内へスクロールし、矩形の中心へ実際のマウス入力（`clickCount: 1`）を送る。中心が他の要素に覆われていたら押さない。
  - 書き換えはフォーカス → 全選択 → `Input.insertText`（空にするときは Delete キー）。DOM の `value` への直接代入はしない。
  - 待ちは固定の sleep ではなく、条件を一定間隔で確かめる（上限時間つき）。
  - 言語ごとに「切替 → 書換 → 全欄照合 → 保存（「保存されました」を確認）→ 再読込 → 照合」を行い、最後に全言語を読み直して照合する。
  - 再読込・遷移で beforeunload の確認が出たら、未保存の変更を捨てずに中止する。
- **安全策**
  - 既定は差分表示だけで、何も変更しない。
  - 下書き保存と審査送信は別の操作にする。審査送信は 1 件ずつ。「自動的に公開する」の状態は変えずに報告する。
  - 審査待ち・ロック中・状態を判定できないアイテムには書き込まない。審査の取り消しはしない。
  - 失敗時の画面保存は `%LOCALAPPDATA%\Edictora\diagnostics` に置き、リポジトリには置かない。
- **配布**: 配る前提だが、配布先（R2 バケット・ドメイン）は未確定。決まるまで配布スクリプトは「配布先が未設定」で止める。配布物は署名済み `edictora.exe` の Portable ZIP（手順は docs/release.md）。

## cws-mcp との分担

- cws-mcp: パッケージ（ZIP）のアップロードと公開（CWS API v2）
- Edictora: 掲載情報の更新（API がないため画面操作）

## 実装（Rust + Bun/TypeScript）

最初の実装は C#（.NET 11、CLI + Avalonia の画面）だったが、CLI で足りるため画面をやめ、「なるべく Rust、Rust が苦手なページ内スクリプトだけ Bun + TypeScript」の方針で作り直した（2026-11-13）。挙動・文言・終了コードは C# 版に合わせ、実ダッシュボードの `diff`（ヘッドレス、前面に出ないこと）で一致を確かめてから C# 版を削除した。C# 版は履歴の `b1adbc4` から参照できる。

- **構成**: `crates/edictora/`（lib + bin `edictora.exe`）。ページ内で評価する JavaScript だけを `page-scripts/`（Bun + TypeScript）で書き、生成物 `page-scripts/dist/edictora.js` をコミットして `include_str!` で埋め込む。`cargo build` と配布 exe に Bun は要らない。
- **役割分担は C# 版と同じ**: JS は候補要素の情報（タグ・文字・ラベル候補・無効/読み取り専用・aria-expanded・チェック状態）を集めて返すだけ。どれを選ぶか・書き込んでよいかは Rust の純粋関数（`dashboard/selectors.rs`・`dashboard/write_gate.rs`）で決め、ブラウザなしでテストする。
- **CDP**: localhost の WebSocket だけ（TLS なし、`tungstenite` の同期 API）。入出力スレッド 1 本がソケットを非ブロッキングで持ち、コマンドの応答は id ごと、イベントは購読ごとのチャネルへ配る。応答待ちの間も `Page.loadEventFired` と beforeunload の `Page.javascriptDialogOpening` を受け取れる。切断時は応答待ちと購読をすべて失敗させる。
- **Chrome の起動**: 起動引数・ヘッドレスの扱い・既存 Chrome への接続（DevToolsActivePort と `/json/version` の照合）・`Browser.close` による終了は C# 版と同じ。Chrome の標準出力・標準エラーは捨てる（C# 版では Chrome のログ行が CLI の標準エラーに漏れていた）。プロファイルを削除する経路は持たない。
- **中断**: Ctrl+C は `SetConsoleCtrlHandler` で受け、待ちの各所が中断の合図を見て抜ける（終了コード 130）。その後の `Browser.close` と終了待ちは合図を見ないので、Chrome は正常終了する。
- **保存先の注入**: `AppPaths` を値として持ち回し、テストは `target/tmp` 配下を根にする。`main` だけが環境（`EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` による上書きを含む）から決める。
- **正規表現の方言**: 設定の `sectionStart` / `sectionEnd` は Rust の `regex` で解釈する。先読み・後読み・後方参照は使えない（読み込み時にエラー）。どのパターンも線形時間で終わるので、C# 版の 2 秒の打ち切りは持たない。ログの publisherId 伏せ字は、C# 版の否定先読みが無くても同じ結果になる（置き換える文字の集合が `<` を含まないため）。
