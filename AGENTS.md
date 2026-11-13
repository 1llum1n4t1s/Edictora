# AGENTS.md — Edictora 開発ガイド（LLM / 開発者向け）

Chrome Web Store（CWS）の掲載情報（説明・公式 URL・ホームページ URL・サポート URL）を、各拡張機能リポジトリの正本ファイルから一括で読み出し・比較・更新する Windows 向け CLI（`edictora.exe`）。ほかの人にも配る前提。
CWS API v2 に掲載情報の読み書きがなく、拡張機能からの CWS 操作は Chrome が拒否するため、専用プロファイルの Chrome を CDP で操作してデベロッパー ダッシュボードの画面を書き換える。
全体設計・2026-11-13 の実画面検証で確かめた画面構成と操作方法は [DESIGN.md](DESIGN.md) を正本とする。

実装は **Rust**（本体・CDP・Chrome 起動・設定・比較・CLI）と、ページ内で評価する JavaScript だけを書く **Bun + TypeScript** の 2 本立て。画面（GUI）は持たない（C# + Avalonia 版は 2026-11-13 に廃止。必要なら履歴の `b1adbc4` から参照できる）。

## アーキテクチャ

```
Cargo.toml / rust-toolchain.toml … ワークスペース（edition 2024・Rust 1.99.1・resolver 3）。版は [workspace.package] の version
crates/edictora/
├─ src/
│  ├─ main.rs / lib.rs
│  ├─ cdp/            … client.rs（CDP JSON-RPC・flatten sessionId・入出力スレッド）/ page.rs（評価・マウス・キー・遷移）
│  ├─ chrome/         … locator.rs（chrome.exe 検出）/ launcher.rs（専用プロファイルで起動 or 既存へ接続、Browser.close で終了）
│  ├─ config/         … mod.rs（config.json の読込・検証）/ section.rs（正本の区間切り出し）
│  ├─ listing/        … 欄・正本と現在値のモデル・完全一致の比較 / text_diff.rs（行差分）
│  ├─ dashboard/      … urls.rs / text.rs（画面の文言）/ page_scripts.rs（埋め込み JS の呼び出し）/ selectors.rs（要素の選択）
│  │                    write_gate.rs（書き込み可否）/ item_page.rs（1 操作ずつの実装）/ browser.rs（接続とログイン確認）
│  │                    mod.rs の trait `ListingPage`（手順と画面操作の境目）
│  ├─ services.rs     … pull・diff・push・submit の手順と中止判断
│  ├─ cli/            … args.rs（引数解析）/ mod.rs（実行と終了コード）/ diff_formatter.rs
│  ├─ logging.rs      … %LOCALAPPDATA%\Edictora\logs。publisherId を伏せる
│  ├─ app_paths.rs / error.rs / cancel.rs（Ctrl+C）/ util.rs
└─ tests/             … 結合テスト（ブラウザ不要。CDP は 127.0.0.1 の偽サーバー）とフィクスチャ
page-scripts/         … ページ内で評価する TypeScript（src/edictora.ts）→ Bun で dist/edictora.js を生成してコミット
scripts/              … リリース用 PowerShell（release-local.ps1 / Sigilune.Common.ps1 / R2 配信）
tests/R2Transfer/     … R2 転送スクリプトの検証（CI の ci.yml が Run.ps1 を実行）
.github/workflows/    … ci.yml（検査・テスト）/ codeql.yml / release.yml（署名付きリリース）
```

## 設計上の重要な前提（このとおりに実装する）

- **手順は `services.rs` に置き、CLI は呼ぶだけにする。** 画面操作は trait `ListingPage` 越しに行い、手順はテスト用の偽ページでブラウザなしに検証する。
- **ページ内の JS は候補要素の情報を集めて返すだけ。** どれを選ぶかは Rust の純粋関数（`dashboard/selectors.rs`）で判断し、テストで固める。dist は直接編集せず `bun run build` で作り、`bun run check` で最新か確かめる。exe と `cargo build` に Bun を要らせない（`include_str!` で埋め込む）。
- **Chrome の起動引数は最小限**（`--user-data-dir`・`--remote-debugging-port=0`・`--no-first-run`・`--no-default-browser-check` と描画の間引き抑止 3 つ・`--disable-features=CalculateNativeWinOcclusion`、ヘッドレス時だけ `--headless=new --window-size=1280,900`）。`--enable-automation` などの自動化フラグや画面外配置は付けない（Google ログインを弾かれる恐れ）。変える場合は実画面でログインを再検証する。Chrome の stdout/stderr は捨てる。
- **プロファイルは秘密情報で、ツールは絶対に削除しない。** 終了は `Browser.close`、応答しないときだけプロセスを止める。ログイン待ちの間はタブに接続しない。
- **publisherId はログイン後の URL から実行時に取り出し、コード・設定・ログに書かない。** ログと CLI のエラー出力は伏せ字処理を通す。
- **要素はラベル・役割・文言で探し、クラス名に頼らない。** 文言は `dashboard/text.rs` に集める。ラベルは 1 行単位の完全一致、言語一覧の開閉は `aria-expanded`、0×0 の一覧は数えない。候補がちょうど 1 つに絞れなければ止める（最初の 1 つを選ばない）。ダッシュボードは日本語表示が前提。
- **Chrome を前面に出さない。** `login` 以外はヘッドレス（`--show-browser` で表示）、UA の HeadlessChrome は Chrome に置き換え、操作の前に毎回 `Emulation.setFocusEmulationEnabled`。`Page.bringToFront` は使わない。クリックは scrollIntoView 後に矩形中心へ実際のマウス入力（`clickCount: 1`）、覆われていたら押さない。書き換えはフォーカス → 全選択 → `Input.insertText`。DOM の `value` 直接代入はしない。待ちは固定 sleep でなく上限つきの条件確認。
- **ページ内スクリプトに利用者の入力値を埋め込まない。** 書き込む文字列は CDP の `Input.insertText` で渡す。
- **書き込みの安全策**: `write_gate.rs` は確認済みの状態（許可リスト）以外を止める。審査の取り消しはしない。言語ごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」、最後に全言語を読み直す。途中で失敗したら、そのアイテムの残りは行わない。beforeunload が出たら「留まる」を選び、変更を捨てずに中止する。
- **審査送信は 1 件ずつ明示**（`submit --item <id> --yes`）。「自動的に公開する」チェックは変えずに報告する。
- **区間切り出しは検証スクリプト（push.mjs）と同じ結果にする。** 見出しが無ければエラー。`sectionStart` / `sectionEnd` は Rust の `regex` で解釈するため、先読み・後読み・後方参照は使えない（設定の契約）。`crates/edictora/tests/fixtures/ReplaceTranslator` の SHA-256・文字数（ja=1492 / en=3172）のテストを壊さない。
- **設定に書いていない欄は、読み出し・比較・書き換えの対象外。** 未知のキーはエラー。URL 欄はキーなし＝対象外、`""`＝空欄が正本。
- **引数と設定の誤りは Chrome を開く前に終了コード 2 で止める。** 終了コードは 0 / 1 / 2 / 3（差分あり）/ 4（書き込み中止）/ 5（ログインが必要）/ 130（中断）。進捗は標準エラー、結果は標準出力。
- 失敗時の画面保存は `%LOCALAPPDATA%\Edictora\diagnostics` に置き、リポジトリには置かない。

## 保存先

- 設定（正本の対応づけ）: `%APPDATA%\Edictora\config.json`（`--config` で変更可）
- Chrome プロファイル: `%LOCALAPPDATA%\Edictora\chrome-profile`（設定の `profileDir` / `--profile` で変更可）
- ログ: `%LOCALAPPDATA%\Edictora\logs`、失敗時の画面: `%LOCALAPPDATA%\Edictora\diagnostics`
- 検証用に環境変数 `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` で保存先の根を差し替えられる

## コーディング規約

- 利用者向けの文言・コメント・コミットメッセージは **日本語**。コメントは目的・設計意図・制約を多めに書く
- 依存は最小限（`serde` / `serde_json` / `regex` / `tungstenite`（同期・TLS なし・`handshake` のみ）、テスト専用に `sha2`）。HTTP/TLS crate は使わない（DevTools の確認は localhost への素の HTTP/1.1）。Ctrl+C・レジストリ・ローカル時刻・環境変数展開は Windows API を直接宣言して使う
- CDP は入出力スレッド 1 本が非ブロッキングのソケットを持ち、応答待ちの間もイベントを配る
- `AppPaths` は値として注入する。テストは `target/tmp` 配下を根にし、実プロファイル・実設定を解決しない。CLI のテストはブラウザを開く関数に「呼ばれたら panic」を渡す
- Bun 側のパッケージ管理は Bun（`packageManager: bun@1.4.4`、`bunfig.toml` の `minimumReleaseAge`）。依存は `typescript` だけ
- バージョンは `Cargo.toml` の `[workspace.package] version` に一元化。**更新は /vava 指示時のみ**

## ビルド / テスト

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release          # target\release\edictora.exe
cd page-scripts; bun install --frozen-lockfile; bun run typecheck; bun run check
```

- `cargo` は `~/.cargo/bin` にある
- 実ダッシュボードに接続するテストは書かない。**テスト・動作確認で Chrome を起動しない**（実画面の確認は手動で行う）

## 既知の制約 / 今後の課題

- ダッシュボードの英語表示は未対応（文言で要素を探すため）
- 実画面で確認済みなのは `pull` / `diff`（ヘッドレス）まで。`push` / `submit`、Ctrl+C での中断、beforeunload の扱いは Rust 版ではまだ実画面で確かめていない
- 未確認の画面状態: 審査待ちアイテムの編集画面、URL 欄の書き換えと保存、言語が 1 つだけ / ja・en 以外のアイテム。`write_gate.rs` の許可リスト（公開済み・下書き）は実画面の表記で確かめて更新する
- exe にアイコンとバージョン情報のリソースがまだ無い
- 配布先（R2 bucket・公開ドメイン）は未確定。`scripts/release-local.ps1` の `$Bucket` / `$BaseUrl` が空のあいだは、配信を伴う実行は「配布先が未設定」で止まる

## リリース

詳細は [docs/release.md](docs/release.md)。通常の公開は、`/vava` で版を上げて push した後に CI の `release.yml`（手動起動・Environment `release` の承認）で行い、手元から重ねて配信しない。`edictora.exe` を Sigilune（`scripts/Sigilune.Common.ps1`、Certum SimplySign のクラウド鍵）で署名し、Portable ZIP にまとめる。CI は `SIGILUNE_EMAIL` / `SIGILUNE_TOTP_URI` / `CLOUDFLARE_API_TOKEN` を Environment の Secrets から受け取る。配布先が決まるまでは `skip_upload=true` での確認だけが通る。

```powershell
gh workflow run release.yml --repo 1llum1n4t1s/Edictora --ref main
```

ローカル実行は検証（`-SkipUpload`）に使う。事前に Sigilune のクラウドログインを確認し、失敗時は `sigilune status` で登録を確認するよう案内し、ログインを自動再試行しない。

```powershell
$sigilune = if ($env:SIGILUNE_EXE) { $env:SIGILUNE_EXE } else { Join-Path $env:USERPROFILE '.local\Sigilune\bin\sigilune.exe' }
& $sigilune login
if ($LASTEXITCODE -eq 0) { pwsh -NoProfile -File scripts/release-local.ps1 -SkipUpload } else { throw 'Sigilune でクラウドにログインできなかったよ。sigilune status で登録を確認してね' }
```
