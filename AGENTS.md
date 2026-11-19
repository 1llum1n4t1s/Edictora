# AGENTS.md — Edictora 開発ガイド（LLM / 開発者向け）

Chrome Web Store（CWS）の掲載情報（説明・公式 URL・ホームページ URL・サポート URL と、掲載画像、「プライバシー」タブ）を、各拡張機能リポジトリの正本ファイルから一括で読み出し・比較・更新する Windows 向け CLI（`edictora.exe`）。ほかの人にも配る前提。
CWS API v2 に掲載情報の読み書きがなく、拡張機能からの CWS 操作は Chrome が拒否するため、専用プロファイルの Chrome を CDP で操作してデベロッパー ダッシュボードの画面を書き換える。
全体設計・2026-11-13 の実画面検証で確かめた画面構成と操作方法は [DESIGN.md](DESIGN.md) を正本とする。

実装は **Rust**（本体・CDP・Chrome 起動・設定・比較・CLI）と、ページ内で評価する JavaScript だけを書く **Bun + TypeScript** の 2 本立て。画面（GUI）は持たない（C# + Avalonia 版は 2026-11-13 に廃止。必要なら履歴の `b1adbc4` から参照できる）。

## アーキテクチャ

```
Cargo.toml / rust-toolchain.toml … ワークスペース（edition 2024・Rust 1.100.0・resolver 3）。版は [workspace.package] の version
crates/edictora/
├─ src/
│  ├─ main.rs / lib.rs
│  ├─ cdp/            … client.rs（CDP JSON-RPC・flatten sessionId・入出力スレッド）/ page.rs（評価・マウス・キー・遷移）
│  ├─ chrome/         … locator.rs（chrome.exe 検出）/ launcher.rs（専用プロファイルで起動 or 既存へ接続、Browser.close で終了）
│  ├─ config/         … mod.rs（config.json の読込・検証）/ section.rs（正本の区間切り出し）/ images.rs（`images` の読込）/ privacy.rs（`privacy` の読込）
│  ├─ images/         … mod.rs（掲載画像のモデル・正本の検証・画素ダイジェストの比較）/ png.rs（PNG のデコードと画素ダイジェスト。zlib 展開は自前）/ fetch.rs（原本を `=s0` で取得。trait `ImageFetcher`）
│  ├─ privacy/        … 「プライバシー」タブのモデル・本文の字下げ除去と文字数の検証・比較（権限の一覧の照合を含む）。画面操作は持たない
│  ├─ listing/        … 欄・正本と現在値のモデル・完全一致の比較 / text_diff.rs（行差分）/ category.rs（カテゴリのキーと data-value の対応表・比較）
│  ├─ dashboard/      … urls.rs / text.rs（画面の文言）/ page_scripts.rs（埋め込み JS の呼び出し）/ selectors.rs（要素の選択）
│  │                    write_gate.rs（書き込み可否）/ item_page.rs（1 操作ずつの実装）/ browser.rs（接続とログイン確認）
│                    mod.rs の trait `EditorPage`（共通）/ `ListingPage`（テキストと審査送信）/ `ImagePage`（画像。審査送信なし）/ `PrivacyPage`（「プライバシー」タブ。審査送信なし）が手順と画面操作の境目
│  ├─ net/winhttp.rs  … WinHTTP の FFI（GET だけ、メモリかファイルへ受ける）。自動更新と、画像の原本の取得が使う
│  ├─ update/         … 自動更新: locate.rs / manifest.rs（sq.version）/ feed.rs / version.rs（SemVer）/ source.rs（配信元）
│  │                    download.rs（ダウンロードと検証）/ hash.rs（CNG）/ lock.rs / apply.rs（Update.exe の起動）
│  ├─ install/        … Velopack のフック（hooks.rs）と HKCU の Run・PATH（registry.rs）/ Claude Code・Codex への MCP 自動登録・解除（connect.rs）
│  ├─ updater_main.rs … 更新用 exe（edictora-updater.exe、windows_subsystem = "windows"）の入口
│  ├─ services.rs     … pull・diff・push・submit の手順と中止判断
│  ├─ cli/            … args.rs（引数解析）/ mod.rs（実行と終了コード）/ diff_formatter.rs
│  ├─ mcp/mod.rs      … MCP サーバー（`edictora mcp`、標準入出力の JSON-RPC）。ツールの中身は CliApp::run を呼ぶだけ
│  ├─ i18n.rs         … 利用者向け出力の表示言語（9 言語）と辞書の引き方（`tr!` / `trs!`、辞書は `locales/<タグ>.json`）
│  ├─ logging.rs      … %LOCALAPPDATA%\Edictora.Data\logs。publisherId を伏せる
│  ├─ redirect.rs     … MSIX パッケージ内のアプリから起動されたときの保存先リダイレクトの検出（`detect_redirect` ほか）・警告・専用領域の候補探し
│  ├─ app_paths.rs / error.rs / cancel.rs（Ctrl+C）/ util.rs
└─ tests/             … 結合テスト（ブラウザ不要。CDP は 127.0.0.1 の偽サーバー）とフィクスチャ
page-scripts/         … ページ内で評価する TypeScript（src/edictora.ts）→ Bun で dist/edictora.js を生成してコミット
scripts/              … リリース用 PowerShell（release-local.ps1 / Sigilune.Common.ps1 / R2 配信）
tests/R2Transfer/     … R2 転送スクリプトの検証（CI の ci.yml が Run.ps1 を実行）
.github/workflows/    … ci.yml（検査・テスト）/ codeql.yml / release.yml（署名付きリリース）
```

## 設計上の重要な前提（このとおりに実装する）

- **手順は `services.rs` に置き、CLI は呼ぶだけにする。** 画面操作は trait（`ListingPage`・`ImagePage`・`PrivacyPage`）越しに行い、手順はテスト用の偽ページでブラウザなしに検証する。
- **ページ内の JS は候補要素の情報を集めて返すだけ。** どれを選ぶかは Rust の純粋関数（`dashboard/selectors.rs`）で判断し、テストで固める。dist は直接編集せず `bun run build` で作り、`bun run check` で最新か確かめる。exe と `cargo build` に Bun を要らせない（`include_str!` で埋め込む）。
- **Chrome の起動引数は最小限**（`--user-data-dir`・`--remote-debugging-port=0`・`--no-first-run`・`--no-default-browser-check` と描画の間引き抑止 3 つ・`--disable-features=CalculateNativeWinOcclusion`、ヘッドレス時だけ `--headless=new --window-size=1280,900`）。`--enable-automation` などの自動化フラグや画面外配置は付けない（Google ログインを弾かれる恐れ）。変える場合は実画面でログインを再検証する。Chrome の stdout/stderr は捨てる。
- **プロファイルは秘密情報で、ツールは絶対に削除しない。** 終了は `Browser.close`、応答しないときだけプロセスを止める。ログイン待ちの間はタブに接続しない。
- **publisherId はログイン後の URL から実行時に取り出し、コード・設定・ログに書かない。** ログと CLI のエラー出力は伏せ字処理を通す。
- **要素はラベル・役割・文言で探し、クラス名に頼らない。** 文言は `dashboard/text.rs` に集める。ラベルは 1 行単位の完全一致、言語一覧の開閉は `aria-expanded`、0×0 の一覧は数えない。候補がちょうど 1 つに絞れなければ止める（最初の 1 つを選ばない）。外部のサービスに選ばせる経路は持たない（判断は `dashboard/selectors.rs` の純粋関数だけ）。画面言語は `<html lang>` で 1 回だけ判定して文言セット（`text.rs` の JA / EN）を 1 つ選ぶ（lang が空なら日本語、ja / en 以外は止める）。英語の文言は実画面で未確認の暫定値（確認済みは日本語だけ）で、英語で足す判定は「止める」側だけにし、「書いてよい」側へ推測で足さない（`write_gate.rs` の `UNCONFIRMED_LOOKALIKE_KEYWORDS` など）。
- **Chrome を前面に出さない。** `login` 以外はヘッドレス（`--show-browser` で表示。**唯一の例外**: pull / diff / push / submit の入口の確認で再ログインが要る（`Error::LoginRequired`）と分かったときだけ、`cli/mod.rs` の `open_logged_in` が `cli/relogin.rs` の `open_with_relogin` で、ヘッドレスのブラウザを閉じ → 表示つきで開いて `wait_for_login`（既定 10 分）→ 閉じ → 元の指定で開き直して確認し直す。再試行は 1 回だけで、失敗・時間切れ・利用者が画面を閉じた・2 回目もログイン画面は終了コード 5、Ctrl+C / MCP の cancelled は 130。作業の途中でのセッション切れはこの対象外）、UA の HeadlessChrome は Chrome に置き換え、操作の前に毎回 `Emulation.setFocusEmulationEnabled`。`Page.bringToFront` は使わない。クリックは scrollIntoView 後に矩形中心へ実際のマウス入力（`clickCount: 1`）、覆われていたら押さない。書き換えはフォーカス → 全選択 → `Input.insertText`。DOM の `value` 直接代入はしない。待ちは固定 sleep でなく上限つきの条件確認。
- **ページ内スクリプトに利用者の入力値を埋め込まない。** 書き込む文字列は CDP の `Input.insertText` で渡す。
- **書き込みの安全策**: `write_gate.rs` は確認済みの状態（許可リスト）以外を止める。審査の取り消しはしない。言語ごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」、最後に全言語を読み直す。途中で失敗したら、そのアイテムの残りは行わない。beforeunload が出たら「留まる」を選び、変更を捨てずに中止する。
- **審査送信は 1 件ずつ明示**（`submit --item <id> --yes`）。「自動的に公開する」チェックは変えずに報告する。
- **掲載画像**（`images`・`services::push_images`）: 手順は `&mut dyn ImagePage` だけを受け取る（審査送信は無い。`EditorPage` ← `ListingPage` / `ImagePage` の分離を崩さない）。**比較はファイルのバイト列ではなくデコードした画素の SHA-256**（ダッシュボードは PNG を再エンコードして保存する。`images/png.rs`）。原本は `=s0` を Cookie なしの `net/winhttp.rs`（https の `*.googleusercontent.com` だけ、16 MB まで）で取得し、取得・デコードできなければ「比較不能」で止める（一致扱いにも常に置換にもしない）。スロットは `selectors::pick_image_slot` が寸法の表記と見出しの行で 1 つに絞る（絞れなければ止める）。画像まわりのクリックは削除ボタンと削除確認ダイアログの「削除」だけで、並べ替えには触れない。**ストア アイコン（`images.icon`、128x128、「ショップ アイコン」）も同じ仕組みで、全言語共通なので `--image-scope global` に含める。アイコンだけは透過（RGBA）の PNG を許し、アルファ込みの画素ダイジェスト（`png::pixel_digest_rgba`）で比べる。**書き込みは日本語の画面だけ。**画像の削除はサーバーへ即座に反映される**ので、途中で止まると下書きから画像が欠ける（再実行で整える）。ローカライズ版が対象でロケールがダッシュボードに無いなら、何も触らず止めて `--image-scope global` か `--locale` を案内する。CLI は `--only text|images|privacy`・`--image-scope all|global|localized`（MCP は `only`・`imageScope`）。詳細は DESIGN.md「掲載画像」
- **「プライバシー」タブ**（`privacy`・`services::push_privacy`）: 権限を足すと、「プライバシー」タブ（`<編集画面>/privacy`）に正当性などを入れないと審査に送信できない。**設定の `privacy` に書いた欄だけ**（`singlePurpose`・`permissions`・`remoteCode`・`dataUsage`・`policyUrl`）を、説明文と同じ「ファイル + sectionStart / sectionEnd」の正本（`config/section.rs` を再利用。固定書式は解析しない）から読み取り・比較・下書き保存する。本文は前後の空行を除き共通の字下げを取り（`privacy::dedent`）、空・1,000 文字超過は Chrome を開く前に終了コード 2 で止める。`permissions` のキーは manifest の権限名、ホスト権限だけ `host`。`dataUsage.certify` は true だけを許す。手順は `&mut dyn PrivacyPage` だけを受け取る（審査送信は無い。`EditorPage` ← `ListingPage` / `ImagePage` / `PrivacyPage` の分離を崩さない）。**`permissions` を書いたときは権限の一覧そのものも照合し**、設定と画面で食い違えば `diff` は終了コード 1、`push` は何も書かず `WriteBlocked` で止める。欄は `selectors::classify_privacy_textareas` などでラベル・aria-label の 1 行完全一致から 1 つに絞る（絞れなければ止める）。保存はテキスト・画像とは別に最後に行う。**日本語の画面だけ**扱う。**リモートコードは未対応**（設定に書くと `diff` がエラー、`push` が止まる）。`--only privacy` で絞れ、`--locale` では絞らない。詳細は DESIGN.md「「プライバシー」タブ」
- **区間切り出しは検証スクリプト（push.mjs）と同じ結果にする。** 見出しが無ければエラー。`sectionStart` / `sectionEnd` は Rust の `regex` で解釈するため、先読み・後読み・後方参照は使えない（設定の契約）。`crates/edictora/tests/fixtures/ReplaceTranslator` の SHA-256・文字数（ja=1492 / en=3172）のテストを壊さない。
- **設定に書いていない欄は、読み出し・比較・書き換えの対象外。** 未知のキーはエラー。URL 欄はキーなし＝対象外、`""`＝空欄が正本。
- **カテゴリ**（`category`・`listing/category.rs`・`ListingPage::read_category` / `write_category`）: 全言語共通のアイテム単位の欄なので `ListingField`（ロケール単位）には入れず、`ExpectedItem::category`（キー）で持つ。値は言語に依らない小文字のキーで、`data-value` との対応表は `listing/category.rs` の 1 か所（未知のキーは Chrome を開く前に終了コード 2。プレースホルダー＝未選択は正本にできず選びもしない）。画面は `ul[role=listbox][aria-label="カテゴリ"]` の選択肢（閉じていても DOM にある）の `aria-selected="true"` の `data-value` を**閉じたまま**読み（page-scripts の `listboxOption` が一覧の通し番号 `group`・`data-value`・`aria-selected`・一覧の枠の大きさを返すだけ。一覧の特定と曖昧時の停止は `selectors::category_options` / `selected_category` / `pick_category_option` の純粋関数）、書き込みはコンボボックスを実マウスで開く → 一覧が見えるまで待つ → `data-value` が一致する選択肢をちょうど 1 つ実クリック → 閉じて選択されたことを確かめる（閉じなければ Escape）。**日本語の画面だけ**（`DashboardText::category` が EN は None）。手順（`services::push_core`）はロケールのループの前に独立した 1 サイクル（読む → 差分なら書く → 保存前の照合 → 書き込み可否 → 下書き保存 → 読み込み直し → 照合）で行い、不足ロケールの確認より後なので、不足で止まる push はカテゴリも書かない。読み出しの失敗はこの欄だけのエラー（`ItemSnapshot::category` が `Err`）にしてロケールの読み出しは続ける。`--only text` に含み、`--locale` では絞らない
- **プライバシー ポリシー URL**（`privacy.policyUrl`・`PrivacyField::PolicyUrl`・`PrivacyPage::write_policy_url`）: 短い 1 行の欄（`input[type=text]`、textarea 用の検査は広げず `selectors::ensure_privacy_url_writable` を別に持つ）。ラベル「プライバシー ポリシーの URL」の 1 行完全一致で 1 つに絞る（`find_policy_url_input`）。契約は公式 URL などと同じ（キーなし＝対象外、`""`＝空欄が正本、http(s) の絶対 URL、2,048 文字以内）。`privacy` の「1 欄も無ければエラー」では policyUrl も 1 欄に数える
- **引数と設定の誤りは Chrome を開く前に終了コード 2 で止める。** 終了コードは 0 / 1 / 2 / 3（差分あり）/ 4（書き込み中止）/ 5（ログインが必要）/ 130（中断）。進捗は標準エラー、結果は標準出力。
- 失敗時の画面保存は `%LOCALAPPDATA%\Edictora.Data\diagnostics` に置き、リポジトリには置かない。push / submit の失敗と、pull / diff の読み出し失敗（ログイン切れ以外）で保存し、保存先を結果のエラー文に含める。
- **MCP（`edictora mcp`）は CLI の薄い皮にする**: ツールは `edictora_list_items` / `edictora_diff` / `edictora_pull` / `edictora_push` / `edictora_submit` だけで、中身は引数の配列を組み立てて `CliApp::run` を呼び、出力をメモリで受ける（手順・安全策・終了コード・伏せ字を二重に持たない）。**画面表示・login・update・プロファイル・chrome.exe の指定はツールに持たせない**（ログインが要るときに CLI 側が表示つきの Chrome を自動で開いて待つ動きだけは例外で、ツールの引数では制御できない。説明文・INSTRUCTIONS に英語で書く）（スキーマは `additionalProperties: false`）。push / submit は `confirm: true` が無ければ書かず、`destructiveHint` を付ける。終了コード 3 は `isError: false`。呼び出しは作業スレッド 1 本で順に実行し、`notifications/cancelled` で中断する。標準出力には JSON-RPC 以外を書かない。**実行中ロックはサーバーの生存期間中ずっと共有で持つ**（`main.rs`。更新の apply が待機中の MCP を止めないため）。説明文には検索で見つかる英語の語（Chrome Web Store・store listing・description）を入れ、日本語は混ぜない（ASCII だけ。テストで固定）。**Setup 版はフックで Claude Code・Codex へ自動登録・解除する**（`install/connect.rs`。公式 CLI の `.exe` だけを時間制限つきで呼ぶ、既に自分を指す登録は触らない、登録済みの CLI は `Edictora.Data\setup-registration.json` に覚えて二度と呼ばない、解除は自分の exe を指す登録だけ、Portable 版は登録しない。テストは本物の CLI・設定に触れずスタブで検証する）。Claude Desktop・ゲートウェイは README の手順で手動登録する（作者の環境では Toolport の `registry.json` に stdio 項目として登録）。詳細は DESIGN.md「MCP サーバー」
- **AI エージェントは edictora を `--show-browser` 付きで実行しない**（利用者の画面にウィンドウが出る。2026-11-15 に実際に起きた）。原因調査はエラー文と diagnostics の画面で行う。「覆われていて押せません」のエラー文には、中心にあった要素・要素の枠・画面の大きさを含める（`selectors::covered_error_message`）。
- **自動更新（Velopack）**: インストール・差し替え・アンインストールは Velopack の Setup.exe / Update.exe に任せ、Edictora は更新の確認とダウンロードだけを自前で行う（velopack crate は使わない）。約束（フック引数・`sq.version`・`releases.{channel}.json`・`.partial` → 検証 → 改名・`apply --package … --waitPid <自分> --silent --norestart --root … --packageDir …`）は vendored の Velopack 1.2.161 Rust SDK に合わせ、推測で変えない。SDK との意図的な違いは DESIGN.md「配布・インストール・自動更新」。
- **フックは引数の解釈より前に処理して必ず 0 で終える**（`edictora.exe`・`edictora-updater.exe` の両方）。install / updated で HKCU の Run に更新用 exe、ユーザー PATH に `<根>\current` を登録し、uninstall で取り除く。名前と要素は根から決める。PATH は型と他の要素を壊さない。インストール先から起動されていなければレジストリに触れない。テストは RegLoadAppKeyW の専用ハイブを使い、実際の Run・PATH に触れない。
- **Chrome を開くコマンドの実行中は更新を適用しない**（契約）。apply がインストール先配下のプロセスを止め、push の途中で落ちると書き込みの安全策が崩れるため。login / pull / diff / push / submit は、設定の検証の後・ブラウザ操作の間だけ `<データの根>\edictora-running.lock`（`AppPaths::running_lock_path`）を共有で持つ（開けないだけなら続行）。更新側（`edictora update`・`edictora-updater.exe`）はダウンロードと検証の後、Update.exe を起動する直前にこれを排他で取り、取れなければ適用せず `AppInUse`（`update` は終了コード 1、更新用 exe は間を空けて試し直す）。取れたらプロセスの終了まで持つ。適用中に Chrome を開くコマンド（MCP サーバーを含む）を実行したら、Chrome を開かずに終了コード 1。`update` 自身はこのロックを持たず、更新プロセスの終了から差し替えまでの数秒の隙間は塞げない。Run の同名の値が「別の場所に実在する」更新用 exe を指すときだけ上書きしない（指す先が消えた古い登録は上書きする）
- **Portable 版（根に `.portable`）では install / updated フックで Run・PATH を登録しない**（手動の `edictora update` は使える）。Run の値は名前だけで扱わない: 削除は現在の値が自分の値と一致するときだけ、同じ名前の値が別の場所の更新用 exe を指していれば上書きせず警告をログへ残す
- **更新は `edictora update` とログオン時の `edictora-updater.exe` だけ**。pull / diff / push / submit の中では確かめない。`update` は設定を読まず Chrome を開かない。検証（サイズと SHA256、無ければ SHA1）に通らないパッケージは適用しない。配信元は定数 `https://edictora.kagayoi.com`、検証用に `EDICTORA_UPDATE_URL`（平文は 127.0.0.1 だけ）。テストで本物の配信元・Update.exe を使わない（127.0.0.1 の偽配信元と偽の `ApplyLauncher`、CLI は偽の更新処理）。

## 保存先

- **MSIX パッケージ内のアプリ（Claude デスクトップの Bash など）から実行すると、`%APPDATA%` / `%LOCALAPPDATA%` に新規作成したフォルダーは本物ではなくパッケージ専用領域（`%LOCALAPPDATA%\Packages\<名前>\LocalCache\...`）へ入り、通常のターミナルから見えなくなる**（本物に既にあるフォルダーへの書き込みは本物に入る）。Edictora は既定の設定とプロファイルがそこから読まれていれば標準エラーへ警告し（`redirect.rs`）、既定の設定が無いときは専用領域の候補をエラー文に添える。自動で読み込んだり移したりしない。AI エージェントは設定・データのフォルダーを新規に作らず、設定を作るときは本物の場所にあるか確かめる。詳細は DESIGN.md「パッケージ内のアプリからの保存先リダイレクト」

- 設定（正本の対応づけ）: `%APPDATA%\Edictora\config.json`（`--config` で変更可）
- Chrome プロファイル: `%LOCALAPPDATA%\Edictora.Data\chrome-profile`（設定の `profileDir` / `--profile` で変更可）
- ログ: `%LOCALAPPDATA%\Edictora.Data\logs`、失敗時の画面: `%LOCALAPPDATA%\Edictora.Data\diagnostics`
- アプリ本体（Velopack のインストール先）: `%LOCALAPPDATA%\Edictora`（`Update.exe`・`current\`・`packages\`）。アンインストールでフォルダーごと消えるので、データはここに置かない
- 検証用に環境変数 `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` で保存先の根を、`EDICTORA_UPDATE_URL` で更新の配信元を差し替えられる

## コーディング規約

- コメント・コミットメッセージは **日本語**。コメントは目的・設計意図・制約を多めに書く
- 人が読む文言（CLI の進捗・結果・エラー、差分、ヘルプ）は 9 言語（ja・en・zh-CN・zh-TW・ko・es・fr・de・pt-BR）で、`crates/edictora/src/locales/<タグ>.json` の辞書に持ち、`tr!("キー", 名前 = 値)` / `trs!("キー")`（`i18n.rs`）で参照する。プレースホルダーは `{name}` で全言語同じ集合、辞書に無いキーは英語。表示言語は `EDICTORA_LANGUAGE`（旧名 `EDICTORA_LANG`）→ `LC_ALL` → `LC_MESSAGES` → `LANG` → OS の表示言語 → 英語。**人が読む文言を足したら、9 言語の辞書を同時に更新し、整合テスト（`tests/localization.rs`）を通す**。`cargo test` は `.cargo/config.toml` で `EDICTORA_LANGUAGE=ja` に固定される。AI が読む文言（MCP のツール名・説明・スキーマ・応答とエラー）は英語に統一し辞書を通さない。利用者のデータ（掲載文）と識別子・コマンド・設定キー・OS や外部 API のエラー原文は訳さない。ログの定型文は日本語のまま。ダッシュボードの画面文言（`dashboard/text.rs`）と設定の `locales` は UI 言語とは別。**README は英語の `README.md` を正本とし、`README.<タグ>.md`（ja・zh-CN・zh-TW・ko・es・fr・de・pt-BR）を置く。README を変えたら全言語版を同時に直す**。リリースのパッケージに同梱するのは英語の `README.md` だけ
- 依存は最小限（`serde` / `serde_json` / `regex` / `tungstenite`（同期・TLS なし・`handshake` のみ）、テスト専用に `sha2`）。HTTP/TLS crate は使わない（DevTools の確認は localhost への素の HTTP/1.1）。**外部への HTTPS（自動更新の `edictora.kagayoi.com`、画像の原本）は、crate を足さず WinHTTP を FFI で直接使い、`net/winhttp.rs` に集める**（リダイレクト無効・操作ごとのタイムアウトと全体の期限・ワーカースレッドで実行し Ctrl+C で抜ける・応答の上限・平文は 127.0.0.1 のテストだけ）。Ctrl+C・レジストリ・ローカル時刻・環境変数展開・ハッシュ（CNG）・WM_SETTINGCHANGE は Windows API を直接宣言して使う
- CDP は入出力スレッド 1 本が非ブロッキングのソケットを持ち、応答待ちの間もイベントを配る
- `AppPaths` は値として注入する。テストは `target/tmp` 配下を根にし、実プロファイル・実設定を解決しない。CLI のテストはブラウザを開く関数に「呼ばれたら panic」を渡す
- Bun 側のパッケージ管理は Bun（`packageManager: bun@1.4.4`、`bunfig.toml` の `minimumReleaseAge`）。依存は `typescript` だけ
- バージョンは `Cargo.toml` の `[workspace.package] version` に一元化。**更新は /vava 指示時のみ**

## ビルド / テスト

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release          # target\release\edictora.exe と edictora-updater.exe
cd page-scripts; bun install --frozen-lockfile; bun run typecheck; bun run check
```

- `cargo` は `~/.cargo/bin` にある
- 実ダッシュボードに接続するテストは書かない。**テスト・動作確認で Chrome を起動しない**（実画面の確認は手動で行う）
- **テストで Velopack の Setup / Update.exe を実行せず、実際の HKCU の Run・PATH を書き換えない**（結合確認は packId を変えた検証用パッケージで手動で行う）

- **Chrome を起動する経路は `seed_local_state` を通す**: `chrome::launcher::launch_or_attach` は新規起動の直前に user-data-dir の `Local State` へ `password_manager.os_password_blank=false` と `os_password_last_changed="9223372036854775000"` を書く（既存キーは保ち、同じ値なら書かず、壊れた JSON には触れない。`tests/chrome_launcher.rs` で固定）。Windows の Chromium は新規プロファイルごとに空パスワードで LogonUser を呼び、ログオン失敗（Security 4625）として数えられ、並列・連続起動で Windows アカウントがロックされるため（2026-11-15 に実害）。Chrome を起動する別経路を足すときも同じシードを先に行う。

## 既知の制約 / 今後の課題

- 実画面で確認済み: `pull` / `diff`（ヘッドレス）、9 言語アイテムでの `push`（説明欄）と `submit`、ステータス「拒否」での書き込み、掲載画像の読み出しと全言語向け画像の `push`（画素比較へ変えた後の置換を伴う実行は未実施）。未確認: 画像の規格外アップロード時のエラー表示、ローカライズ版の書き込み、Ctrl+C での中断、beforeunload の扱い。詳細は DESIGN.md「実画面での確認状況」
- 2026-11-19 にカテゴリ欄とプライバシー ポリシー URL 欄の DOM を実画面（SearchClock・日本語）で**読み取りだけ**確認し、実装後の `diff` で両欄の読み出しが画面と一致した。同日、公開済みの SearchClock で両欄の `push`（書き換え → 下書き保存 → 照合 → 元へ戻す）も確認した。英語の画面は未確認（DESIGN.md「実画面での確認状況」）
- 「プライバシー」タブは実画面で `diff`・`push`・不整合の設定での停止を確認済み。宣誓チェックの付け直し・リモートコードの欄・審査待ちでの書き込み拒否は未確認（DESIGN.md「実画面での確認状況」）
- 未確認の画面状態: 審査待ちアイテムの編集画面、URL 欄の書き換えと保存、言語が 1 つだけのアイテム。`write_gate.rs` の許可リストは実画面の表記で確かめて更新する
- **「拒否」を許可したことの未確認のリスク**: 拒否のアイテムを再提出した直後も表記は「拒否」のままだった。審査中でも「拒否」と出続けるなら、審査中のアイテムに書き込めてしまう。再提出後の表記を実画面で確かめるまで、拒否のアイテムへの push は利用者の確認のうえで行う
- MCP の未確認: 実画面での操作、`notifications/cancelled` での中断、本物の Update.exe での「待機中の MCP があると適用を見送る」動作、本物の `claude` / `codex` CLI での自動登録・解除（`mcp get` の出力形式と健康診断の挙動、`codex mcp add` の書式）
- exe にアイコンとバージョン情報のリソースがまだ無い
- 自動更新の結合は、検証用 packId `EdictoraTest` で実機確認済み（DESIGN.md「実画面での確認状況」）。未確認: 実署名での一連、R2 への実配信、本番の HTTPS、本番 packId `Edictora` での Setup、Portable 版の一連、Setup 直後の初回起動の見え方、実行中ロックで適用を後に回す動作。nupkg 内の `Squirrel.exe` で Update.exe を差し替える処理（SDK にある）は持たない

## リリース

詳細は [docs/release.md](docs/release.md)。通常の公開は、`/vava` が版を上げた main へ軽量タグ `v<version>` を付けて push すると、そのタグの push で CI の `release.yml`（Environment `release`、承認待ちなし）が自動で起動して行い、手元から重ねて配信しない。`workflow_dispatch` は同じタグを指定した再実行にだけ使う（ブランチ指定は `guard` ジョブと Environment `release` の tag ポリシー `v[0-9]*.[0-9]*.[0-9]*` が拒否する。`guard` は Secrets なしで、タグであること・軽量タグで同じ commit を指すこと・main の祖先であること・`Cargo.toml` の版とタグの一致を確かめる。GitHub Release は作らない）。`vpk pack`（Velopack 1.2.161、packId `Edictora`、チャンネル `win`）の中で Sigilune（Certum SimplySign のクラウド鍵）が 1 ファイルずつ署名し、nupkg・Setup・Portable とフィード `releases.win.json` を R2（`edictora-updates`、`https://edictora.kagayoi.com`）へ配信する。CI は `SIGILUNE_EMAIL` / `SIGILUNE_TOTP_URI` / `CLOUDFLARE_API_TOKEN` を Environment の Secrets から受け取る。

```powershell
gh workflow run release.yml --repo 1llum1n4t1s/Edictora --ref v<version>
```

ローカル実行は検証（`-SkipUpload`）に使う。事前に Sigilune のクラウドログインを確認し、失敗時は `sigilune status` で登録を確認するよう案内し、ログインを自動再試行しない。

```powershell
$sigilune = if ($env:SIGILUNE_EXE) { $env:SIGILUNE_EXE } else { Join-Path $env:USERPROFILE '.local\Sigilune\bin\sigilune.exe' }
& $sigilune login
if ($LASTEXITCODE -eq 0) { pwsh -NoProfile -File scripts/release-local.ps1 -SkipUpload } else { throw 'Sigilune でクラウドにログインできなかったよ。sigilune status で登録を確認してね' }
```
