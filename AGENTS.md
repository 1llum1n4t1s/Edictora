# AGENTS.md — Edictora 開発ガイド（LLM / 開発者向け）

Chrome Web Store（CWS）の掲載情報（説明・公式 URL・ホームページ URL・サポート URL と、掲載画像）を、各拡張機能リポジトリの正本ファイルから一括で読み出し・比較・更新する Windows 向け CLI（`edictora.exe`）。ほかの人にも配る前提。
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
│  ├─ config/         … mod.rs（config.json の読込・検証）/ section.rs（正本の区間切り出し）/ images.rs（`images` の読込）
│  ├─ images/         … mod.rs（掲載画像のモデル・正本の検証・画素ダイジェストの比較）/ png.rs（PNG のデコードと画素ダイジェスト。zlib の展開を自前で持つ）/ fetch.rs（画像の原本を `=s0` で取得。trait `ImageFetcher`）
│  ├─ listing/        … 欄・正本と現在値のモデル・完全一致の比較 / text_diff.rs（行差分）
│  ├─ dashboard/      … urls.rs / text.rs（画面の文言）/ page_scripts.rs（埋め込み JS の呼び出し）/ selectors.rs（要素の選択）
│  │                    write_gate.rs（書き込み可否）/ item_page.rs（1 操作ずつの実装）/ browser.rs（接続とログイン確認）
│                    mod.rs の trait `EditorPage`（共通）/ `ListingPage`（テキストと審査送信）/ `ImagePage`（画像。審査送信なし）が手順と画面操作の境目
│  ├─ net/winhttp.rs  … WinHTTP の FFI（GET だけ、メモリかファイルへ受ける）。自動更新と、画像の原本の取得が使う
│  ├─ update/         … 自動更新: locate.rs / manifest.rs（sq.version）/ feed.rs / version.rs（SemVer）/ source.rs（配信元）
│  │                    download.rs（ダウンロードと検証）/ hash.rs（CNG）/ lock.rs / apply.rs（Update.exe の起動）
│  ├─ install/        … Velopack のフック（hooks.rs）と HKCU の Run・PATH（registry.rs）/ Claude Code・Codex への MCP 自動登録・解除（connect.rs）
│  ├─ updater_main.rs … 更新用 exe（edictora-updater.exe、windows_subsystem = "windows"）の入口
│  ├─ services.rs     … pull・diff・push・submit の手順と中止判断
│  ├─ cli/            … args.rs（引数解析）/ mod.rs（実行と終了コード）/ diff_formatter.rs
│  ├─ mcp/mod.rs      … MCP サーバー（`edictora mcp`、標準入出力の JSON-RPC）。ツールの中身は CliApp::run を呼ぶだけ
│  ├─ i18n.rs         … 利用者向け出力の表示言語（9 言語）と辞書の引き方。`tr!` / `trs!` でキーを参照する。辞書は `locales/<タグ>.json`
│  ├─ logging.rs      … %LOCALAPPDATA%\Edictora.Data\logs。publisherId を伏せる
│  ├─ app_paths.rs / error.rs / cancel.rs（Ctrl+C）/ util.rs
└─ tests/             … 結合テスト（ブラウザ不要。CDP は 127.0.0.1 の偽サーバー）とフィクスチャ
page-scripts/         … ページ内で評価する TypeScript（src/edictora.ts）→ Bun で dist/edictora.js を生成してコミット
scripts/              … リリース用 PowerShell（release-local.ps1 / Sigilune.Common.ps1 / R2 配信）
tests/R2Transfer/     … R2 転送スクリプトの検証（CI の ci.yml が Run.ps1 を実行）
.github/workflows/    … ci.yml（検査・テスト）/ codeql.yml / release.yml（署名付きリリース）
```

## 設計上の重要な前提（このとおりに実装する）

- **手順は `services.rs` に置き、CLI は呼ぶだけにする。** 画面操作は trait（`ListingPage`・`ImagePage`）越しに行い、手順はテスト用の偽ページでブラウザなしに検証する。
- **ページ内の JS は候補要素の情報を集めて返すだけ。** どれを選ぶかは Rust の純粋関数（`dashboard/selectors.rs`）で判断し、テストで固める。dist は直接編集せず `bun run build` で作り、`bun run check` で最新か確かめる。exe と `cargo build` に Bun を要らせない（`include_str!` で埋め込む）。
- **Chrome の起動引数は最小限**（`--user-data-dir`・`--remote-debugging-port=0`・`--no-first-run`・`--no-default-browser-check` と描画の間引き抑止 3 つ・`--disable-features=CalculateNativeWinOcclusion`、ヘッドレス時だけ `--headless=new --window-size=1280,900`）。`--enable-automation` などの自動化フラグや画面外配置は付けない（Google ログインを弾かれる恐れ）。変える場合は実画面でログインを再検証する。Chrome の stdout/stderr は捨てる。
- **プロファイルは秘密情報で、ツールは絶対に削除しない。** 終了は `Browser.close`、応答しないときだけプロセスを止める。ログイン待ちの間はタブに接続しない。
- **publisherId はログイン後の URL から実行時に取り出し、コード・設定・ログに書かない。** ログと CLI のエラー出力は伏せ字処理を通す。
- **要素はラベル・役割・文言で探し、クラス名に頼らない。** 文言は `dashboard/text.rs` に集める。ラベルは 1 行単位の完全一致、言語一覧の開閉は `aria-expanded`、0×0 の一覧は数えない。候補がちょうど 1 つに絞れなければ止める（最初の 1 つを選ばない）。外部のサービスに選ばせる経路は持たない（判断は `dashboard/selectors.rs` の純粋関数だけ）。ダッシュボードの画面言語は `<html lang>` で 1 回だけ判定して文言セット（`text.rs` の JA / EN）を 1 つ選ぶ（要素ごとに両言語で探さない）。lang が空なら日本語、ja / en 以外は止める。英語の文言は実画面で未確認の暫定値で、英語で足す判定は「止める」側だけにし、「書いてよい」側へ推測で足さない（`write_gate.rs` の `UNCONFIRMED_LOOKALIKE_KEYWORDS` など）。
- **Chrome を前面に出さない。** `login` 以外はヘッドレス（`--show-browser` で表示）、UA の HeadlessChrome は Chrome に置き換え、操作の前に毎回 `Emulation.setFocusEmulationEnabled`。`Page.bringToFront` は使わない。クリックは scrollIntoView 後に矩形中心へ実際のマウス入力（`clickCount: 1`）、覆われていたら押さない。書き換えはフォーカス → 全選択 → `Input.insertText`。DOM の `value` 直接代入はしない。待ちは固定 sleep でなく上限つきの条件確認。
- **ページ内スクリプトに利用者の入力値を埋め込まない。** 書き込む文字列は CDP の `Input.insertText` で渡す。
- **書き込みの安全策**: `write_gate.rs` は確認済みの状態（許可リスト）以外を止める。審査の取り消しはしない。言語ごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」、最後に全言語を読み直す。途中で失敗したら、そのアイテムの残りは行わない。beforeunload が出たら「留まる」を選び、変更を捨てずに中止する。
- **審査送信は 1 件ずつ明示**（`submit --item <id> --yes`）。「自動的に公開する」チェックは変えずに報告する。
- **掲載画像**（`images`・`services::push_images`）: 画像の手順は `&mut dyn ImagePage` だけを受け取る（`ImagePage` に審査送信は無い。`EditorPage` ← `ListingPage` / `ImagePage` の分離を崩さない）。**比較はファイルのバイト列ではなく、デコードした画素の SHA-256**（ダッシュボードは PNG を再エンコードして保存する。`images/png.rs`）。原本は `=s0` を Cookie なしの `net/winhttp.rs`（https の `*.googleusercontent.com` だけ、16 MB まで）で取得し、取得・デコードできなければ「比較不能」で止める（一致扱いにも常に置換にもしない）。スロットは `selectors::pick_image_slot` が寸法の表記と見出しの行で 1 つに絞る（絞れなければ止める）。画像まわりのクリックは削除ボタン（aria-label が「画像を削除 <alt>」に完全一致）と削除確認ダイアログの「削除」だけ。アイコン・並べ替えには触れない。書き込みは日本語の画面だけ。**画像の削除はサーバーへ即座に反映される**ので、途中で止まると下書きから画像が欠ける（再実行で整える）。ローカライズ版が対象でロケールがダッシュボードに無い・言語が 1 つのアイテムなら、何も触らず止めて `--image-scope global` か `--locale` を案内する。審査待ちなどでは最初の削除の前と各保存の直前に止める。CLI は `--only text|images`・`--image-scope all|global|localized`（MCP は `only`・`imageScope`）。
- **区間切り出しは検証スクリプト（push.mjs）と同じ結果にする。** 見出しが無ければエラー。`sectionStart` / `sectionEnd` は Rust の `regex` で解釈するため、先読み・後読み・後方参照は使えない（設定の契約）。`crates/edictora/tests/fixtures/ReplaceTranslator` の SHA-256・文字数（ja=1492 / en=3172）のテストを壊さない。
- **設定に書いていない欄は、読み出し・比較・書き換えの対象外。** 未知のキーはエラー。URL 欄はキーなし＝対象外、`""`＝空欄が正本。
- **引数と設定の誤りは Chrome を開く前に終了コード 2 で止める。** 終了コードは 0 / 1 / 2 / 3（差分あり）/ 4（書き込み中止）/ 5（ログインが必要）/ 130（中断）。進捗は標準エラー、結果は標準出力。
- 失敗時の画面保存は `%LOCALAPPDATA%\Edictora.Data\diagnostics` に置き、リポジトリには置かない。push / submit の失敗に加え、pull / diff の読み出し失敗（ログイン切れ以外）でも保存し、保存先を結果のエラー文に含める（標準エラーを捨てて実行されても分かるように）。
- **MCP（`edictora mcp`）は CLI の薄い皮にする**: ツールは `edictora_list_items` / `edictora_diff` / `edictora_pull` / `edictora_push` / `edictora_submit` だけで、中身は引数の配列を組み立てて `CliApp::run` を呼び、出力をメモリで受ける（手順・安全策・終了コード・伏せ字を二重に持たない）。**画面表示・login・update・プロファイル・chrome.exe の指定はツールに持たせない**（スキーマは `additionalProperties: false`、無い引数はエラー）。push / submit は `confirm: true` が無ければ書かない、`destructiveHint` を付ける。終了コード 3（差分あり）は `isError: false`。呼び出しは作業スレッド 1 本で順に実行（同じプロファイルの Chrome を同時に開かない）、`notifications/cancelled` で中断、入力が閉じても受け付け済みは最後まで実行する。標準出力には JSON-RPC 以外を書かない。**実行中ロックはサーバーの生存期間中ずっと共有で持つ**（`main.rs`。Update.exe の apply はインストール先の他プロセスを強制終了するので、待機中の MCP も守る。サーバーが動いている間の更新は `AppInUse` で見送り、次のログオンへ回る。適用中に起動されたら終了コード 1）。説明文には検索で見つかる語（Chrome Web Store・store listing・description・掲載情報・説明文）を入れる（テストで固定）。**Setup 版はフックで Claude Code・Codex へ自動登録・解除する**（`install/` の登録処理。公式 CLI の `.exe` だけを時間制限つきで呼ぶ、既に自分を指す登録は触らない、登録済みの CLI は `Edictora.Data\setup-registration.json` に覚えて二度と呼ばない、終わらなかった分はログオン時の更新用 exe がやり直す、解除は自分の exe を指す登録だけ、Portable 版は登録しない。テストは本物の CLI・設定に触れずスタブで検証する）。Claude Desktop・ゲートウェイの登録方法は README に、クライアントごとに書く（Setup 版の固定パス `%LOCALAPPDATA%\Edictora\current\edictora.exe` を絶対パスで指定）。作者の環境では Toolport の `%APPDATA%\Toolport\registry.json` に stdio 項目（`args: ["mcp"]`）として登録している
- **AI エージェントは edictora を `--show-browser` 付きで実行しない**（利用者の画面にウィンドウが出る。2026-11-15 に実際に起きた）。原因調査はエラー文と diagnostics の画面で行う。そのため「覆われていて押せません」のエラー文には、中心にあった要素・要素の枠・画面の大きさを含める（`selectors::covered_error_message`）。
- **自動更新（Velopack）**: インストール・差し替え・アンインストールは Velopack の Setup.exe / Update.exe に任せ、Edictora は更新の確認とダウンロードだけを自前で行う（velopack crate は使わない）。約束（フック引数・`sq.version`・`releases.{channel}.json`・`.partial` → 検証 → 改名・`apply --package … --waitPid <自分> --silent --norestart --root … --packageDir …`）は vendored の Velopack 1.2.161 Rust SDK（Petarin の `desktop/src-tauri/vendor/velopack-1.2.161`）に合わせ、推測で変えない。詳細と SDK との意図的な違いは DESIGN.md の「配布・インストール・自動更新」。
- **フックは引数の解釈より前に処理して必ず 0 で終える**（`edictora.exe`・`edictora-updater.exe` の両方）。install / updated で HKCU の Run に更新用 exe、ユーザー PATH に `<根>\current` を登録し、uninstall で取り除く。名前と要素は根から決める。PATH は型と他の要素を壊さない。インストール先から起動されていなければレジストリに触れない。テストは RegLoadAppKeyW の専用ハイブを使い、実際の Run・PATH に触れない。
- **Chrome を開くコマンドの実行中は更新を適用しない**（契約）。Update.exe の apply はインストール先配下のプロセスを止める可能性があり、push の途中で落とされると書き込みの安全策が崩れるため。login / pull / diff / push / submit は、設定の検証の後・ブラウザ操作の間だけ `<データの根>\edictora-running.lock`（`AppPaths::running_lock_path`、インストール先に依存しない）を共有で持つ（CLI 同士は同時に動ける。ロックファイルを開けないだけなら続行し、インストールされていない実行でも無害）。更新側（`edictora update`・`edictora-updater.exe`）はダウンロードと検証を済ませた後、Update.exe を起動する直前にこれを排他で取り、取れなければ適用せず `AppInUse`（`update` は終了コード 1、更新用 exe は間を空けて試し直す）。取れたらプロセスの終了まで持ち続ける。適用中（排他で持たれている）に Chrome を開くコマンドを実行したら、Chrome を開かずに終了コード 1。`update` 自身はこのロックを持たない。排他は更新プロセスの終了で外れ、Update.exe の差し替えはその後なので、この数秒の隙間は塞げない（Update.exe 側の動きに依存。文書で案内する）。Run の同名の値が「別の場所に実在する」更新用 exe を指すときだけ上書きしない（指す先が消えた古い登録は上書きする）
- **Portable 版（根に `.portable`）では install / updated フックで Run・PATH を登録しない**（手動の `edictora update` は使える）。Run の値は名前だけで扱わない: 削除は現在の値が自分の値と一致するときだけ、同じ名前の値が別の場所の更新用 exe を指していれば上書きせず警告をログへ残す
- **更新は `edictora update` とログオン時の `edictora-updater.exe` だけ**。pull / diff / push / submit の中では確かめない。`update` は設定を読まず Chrome を開かない。検証（サイズと SHA256、無ければ SHA1）に通らないパッケージは適用しない。配信元は定数 `https://edictora.kagayoi.com`、検証用に `EDICTORA_UPDATE_URL`（平文は 127.0.0.1 だけ）。テストで本物の配信元・Update.exe を使わない（127.0.0.1 の偽配信元と偽の `ApplyLauncher`、CLI は偽の更新処理）。

## 保存先

- 設定（正本の対応づけ）: `%APPDATA%\Edictora\config.json`（`--config` で変更可）
- Chrome プロファイル: `%LOCALAPPDATA%\Edictora.Data\chrome-profile`（設定の `profileDir` / `--profile` で変更可）
- ログ: `%LOCALAPPDATA%\Edictora.Data\logs`、失敗時の画面: `%LOCALAPPDATA%\Edictora.Data\diagnostics`
- アプリ本体（Velopack のインストール先）: `%LOCALAPPDATA%\Edictora`（`Update.exe`・`current\`・`packages\`）。アンインストールでフォルダーごと消えるので、データはここに置かない（旧場所からの自動移行はしない）
- 検証用に環境変数 `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` で保存先の根を、`EDICTORA_UPDATE_URL` で更新の配信元を差し替えられる

## コーディング規約

- コメント・コミットメッセージは **日本語**。コメントは目的・設計意図・制約を多めに書く
- 人が読む文言（CLI の進捗・結果・エラー、差分、ヘルプ）は 9 言語（ja・en・zh-CN・zh-TW・ko・es・fr・de・pt-BR）で、`crates/edictora/src/locales/<タグ>.json` の辞書に持ち、`tr!("キー", 名前 = 値)` / `trs!("キー")`（`i18n.rs`）で参照する。プレースホルダーは `{name}` で全言語同じ集合、辞書に無いキーは英語。表示言語は `EDICTORA_LANGUAGE`（旧名 `EDICTORA_LANG` も読む。優先は前者）→ `LC_ALL` → `LC_MESSAGES` → `LANG` → OS の表示言語 → 英語。**人が読む文言を足したら、9 言語の辞書を同時に更新し、整合テスト（`tests/localization.rs`: キー集合・プレースホルダーの一致、参照キーが辞書にあること、未使用キーが無いこと）を通す**。`cargo test` は `.cargo/config.toml` で `EDICTORA_LANGUAGE=ja` に固定される（日本語の文言を確かめるテストのため）。AI が読む文言（MCP のツール名・説明・スキーマ・AI へ返す応答とエラー）は英語に統一し辞書を通さない（`edictora mcp` はプロセスの言語を英語に固定する）。利用者のデータ（掲載文）と識別子・コマンド・設定キー・OS や外部 API のエラー原文は訳さない。ログの定型文は日本語のまま。ダッシュボードの画面文言（`dashboard/text.rs`）と設定の `locales`（ストアの掲載言語）は UI 言語とは別。**README は英語の `README.md` を正本とし、`README.ja.md`・`README.zh-CN.md`・`README.zh-TW.md`・`README.ko.md`・`README.es.md`・`README.fr.md`・`README.de.md`・`README.pt-BR.md` を置く（先頭に 9 言語の切替行）。README を変えたら全言語版を同時に直す**。リリースのパッケージに同梱するのは英語の `README.md` だけ
- 依存は最小限（`serde` / `serde_json` / `regex` / `tungstenite`（同期・TLS なし・`handshake` のみ）、テスト専用に `sha2`）。HTTP/TLS crate は使わない（DevTools の確認は localhost への素の HTTP/1.1）。**外部への HTTPS（自動更新の `edictora.kagayoi.com`）は、crate を足さず WinHTTP を FFI で直接使い、`net/winhttp.rs` に集める**（リダイレクト無効・操作ごとのタイムアウトと全体の期限・ワーカースレッドで実行し Ctrl+C で抜ける・応答の上限・平文は 127.0.0.1 のテストだけ）。Ctrl+C・レジストリ・ローカル時刻・環境変数展開・ハッシュ（CNG）・WM_SETTINGCHANGE は Windows API を直接宣言して使う
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

- **Chrome を起動する経路は `seed_local_state` を通す**: `chrome::launcher::launch_or_attach` は新規起動の直前に user-data-dir の `Local State` へ `password_manager.os_password_blank=false` と `os_password_last_changed="9223372036854775000"` を書く（既存キーは保ち、同じ値なら書かず、壊れた JSON には触れない。`tests/chrome_launcher.rs` で固定）。Windows の Chromium は新規プロファイルごとに空パスワードで LogonUser を呼び（`password_manager_util_win.cc` の `CheckBlankPasswordWithPrefs`）、ログオン失敗（Security 4625）として数えられ、並列・連続起動で Windows アカウントがロックされる（2026-11-15 に Puppeteer で実害）。Chrome を起動する別経路を足すときも同じシードを先に行う。

## 既知の制約 / 今後の課題

- ダッシュボードの英語表示は認識するが、英語の文言は実画面で未確認（日本語の画面だけ確認済み）
- 実画面で確認済み: `pull` / `diff`（ヘッドレス）、2026-11-15 に 9 言語のアイテムでの `push`（説明欄）と `submit`、ステータス「拒否」での書き込み、掲載画像の読み出し（単一言語・9 言語）と SearchClock での全言語向け画像の `push`（削除・アップロード・保存まで。画素比較に変えた後は置換を伴う実行を未実施で、「変更なし」だけ確認。DESIGN.md「実画面での確認状況」）。画像の規格外アップロード時のエラー表示と、ローカライズ版の書き込みは未確認。Ctrl+C での中断、beforeunload の扱いは Rust 版ではまだ実画面で確かめていない
- 未確認の画面状態: 審査待ちアイテムの編集画面、URL 欄の書き換えと保存、言語が 1 つだけのアイテム。`write_gate.rs` の許可リスト（公開済み・下書き・拒否）は実画面の表記で確かめて更新する
- **「拒否」を許可したことの未確認のリスク**: 拒否のアイテムを再提出した直後も表記は「拒否」のままだった。審査中でも「拒否」と出続けるなら、審査中のアイテムに書き込めてしまう。再提出後の表記を実画面で確かめるまで、拒否のアイテムへの push は利用者の確認のうえで行う（DESIGN.md「実画面での確認状況」）
- MCP の未確認: 実画面での操作、`notifications/cancelled` での中断、本物の Update.exe で「待機中の MCP があると適用を見送る」こと（ロックの保持は結合テスト `tests/mcp_running_lock.rs` で確認済み）、本物の `claude` / `codex` CLI での自動登録・解除（`mcp get` の出力に exe のパスがそのまま出るか、`mcp get` が健康診断でサーバーを起動するか）、`codex mcp add` の書式
- exe にアイコンとバージョン情報のリソースがまだ無い
- 自動更新の結合は、検証用 packId `EdictoraTest` で実機確認済み: Install フックで Run（`EdictoraTest Updater`）と PATH（REG_EXPAND_SZ のまま）を登録、`edictora update` で 1.0.5 → 1.0.6（Obsolete / Updated フック）、`edictora-updater.exe` で 1.0.6 → 1.0.7、`Update.exe uninstall` で Uninstall フックが Run と PATH から自分の分だけを消し `Edictora.Data` は残る。未確認: 実署名での一連、R2 への実配信、本番の HTTPS（edictora.kagayoi.com）、本番 packId `Edictora` での Setup、Portable 版の一連、Setup 直後の初回起動の見え方（`--silent` の Setup は主 exe を起動しないらしい）、実行中ロックで適用を後に回す動作の実機確認。nupkg 内の `Squirrel.exe` で Update.exe を差し替える処理（SDK にある）は持たない
- 配信先の R2 bucket `edictora-updates` と `https://edictora.kagayoi.com` は作成済みだが、作成時点で SSL 証明書は初期化中。初回配信の前に HTTPS で接続できることを確かめる

## リリース

詳細は [docs/release.md](docs/release.md)。通常の公開は、`/vava` で版を上げて push した後に CI の `release.yml`（手動起動・Environment `release`、承認待ちなし）で行い、手元から重ねて配信しない。`edictora.exe` と `edictora-updater.exe` を `vpk pack`（Velopack 1.2.161、packId `Edictora`、チャンネル `win`）でパッケージにし、その中で Sigilune（`scripts/Sigilune.Common.ps1`、Certum SimplySign のクラウド鍵）が 1 ファイルずつ署名する。nupkg・Setup・Portable とフィード `releases.win.json` を R2（`edictora-updates`、`https://edictora.kagayoi.com`）へ配信する。CI は `SIGILUNE_EMAIL` / `SIGILUNE_TOTP_URI` / `CLOUDFLARE_API_TOKEN` を Environment の Secrets から受け取る。

```powershell
gh workflow run release.yml --repo 1llum1n4t1s/Edictora --ref main
```

ローカル実行は検証（`-SkipUpload`）に使う。事前に Sigilune のクラウドログインを確認し、失敗時は `sigilune status` で登録を確認するよう案内し、ログインを自動再試行しない。

```powershell
$sigilune = if ($env:SIGILUNE_EXE) { $env:SIGILUNE_EXE } else { Join-Path $env:USERPROFILE '.local\Sigilune\bin\sigilune.exe' }
& $sigilune login
if ($LASTEXITCODE -eq 0) { pwsh -NoProfile -File scripts/release-local.ps1 -SkipUpload } else { throw 'Sigilune でクラウドにログインできなかったよ。sigilune status で登録を確認してね' }
```
