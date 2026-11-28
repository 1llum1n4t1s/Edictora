# ソースの配置

[アーキテクチャ](architecture.md)の補足。リポジトリとモジュールの構成。

```
Cargo.toml / rust-toolchain.toml … ワークスペース（edition 2024・Rust 1.100.1・resolver 3）。版は [workspace.package] の version
crates/edictora/
├─ src/
│  ├─ main.rs / lib.rs
│  ├─ cdp/            … client.rs（CDP JSON-RPC・flatten sessionId・入出力スレッド）/ page.rs（評価・マウス・キー・遷移）
│  ├─ chrome/         … locator.rs（chrome.exe 検出）/ launcher.rs（専用プロファイルで起動 or 既存へ接続、Browser.close で終了）
│  ├─ config/         … mod.rs（config.json の読込・検証）/ section.rs（正本の区間切り出し）/ images.rs / privacy.rs（各設定の読込）
│  ├─ images/         … mod.rs（モデル・正本の検証・画素ダイジェストの比較）/ png.rs（PNG デコード。zlib 展開は自前）/ fetch.rs（原本の取得。trait `ImageFetcher`）
│  ├─ privacy/        … 「プライバシー」タブのモデル・本文の検証・比較（権限の一覧の照合を含む）。画面操作は持たない
│  ├─ listing/        … 欄・正本と現在値のモデル・完全一致の比較 / text_diff.rs（行差分）/ category.rs（カテゴリのキーと data-value の対応表・比較）
│  ├─ dashboard/      … urls.rs / text.rs（画面の文言）/ page_scripts.rs（埋め込み JS の呼び出し）/ selectors.rs（要素の選択）
│  │                    write_gate.rs（書き込み可否）/ item_page.rs（1 操作ずつの実装）/ browser.rs（接続とログイン確認）
│  │                    mod.rs の trait `EditorPage`（共通）/ `ListingPage`（テキストと審査送信）/ `ImagePage`（画像。審査送信なし）/ `PrivacyPage`（プライバシー。審査送信なし）が手順と画面操作の境目
│  ├─ net/winhttp.rs  … WinHTTP の FFI（GET だけ、メモリかファイルへ受ける）。自動更新と画像の原本の取得が使う
│  ├─ update/         … 自動更新: locate.rs / manifest.rs（sq.version）/ feed.rs / version.rs（SemVer）/ source.rs（配信元）
│  │                    download.rs（ダウンロードと検証）/ hash.rs（CNG）/ lock.rs / apply.rs（Update.exe の起動）
│  ├─ install/        … Velopack のフック（hooks.rs）と HKCU の Run・PATH（registry.rs）/ Claude Code・Codex への MCP 自動登録・解除（connect.rs）
│  ├─ updater_main.rs … 更新用 exe（edictora-updater.exe、windows_subsystem = "windows"）の入口
│  ├─ services.rs     … pull・diff・push・submit の手順と中止判断
│  ├─ cli/            … args.rs（引数解析）/ mod.rs（実行と終了コード）/ diff_formatter.rs / relogin.rs
│  ├─ mcp/mod.rs      … MCP サーバー（`edictora mcp`）。ツールの中身は CliApp::run を呼ぶだけ
│  ├─ i18n.rs         … 利用者向け出力の表示言語（9 言語）と辞書の引き方（`tr!` / `trs!`、辞書は `locales/<タグ>.json`）
│  ├─ logging.rs      … %LOCALAPPDATA%\Edictora.Data\logs。publisherId を伏せる
│  ├─ redirect.rs     … MSIX 内からの起動時の保存先リダイレクトの検出・警告・専用領域の候補探し
│  ├─ app_paths.rs / error.rs / cancel.rs（Ctrl+C）/ util.rs
└─ tests/             … 結合テスト（ブラウザ不要。CDP は 127.0.0.1 の偽サーバー）とフィクスチャ
page-scripts/         … ページ内で評価する TypeScript（src/edictora.ts）→ Bun で dist/edictora.js を生成してコミット
scripts/              … リリース用 PowerShell（release-local.ps1 / Sigilune.Common.ps1 / R2 配信）
tests/R2Transfer/     … R2 転送スクリプトの検証（CI の ci.yml が Run.ps1 を実行）
.github/workflows/    … ci.yml（検査・テスト）/ codeql.yml / release.yml（署名付きリリース）
```
