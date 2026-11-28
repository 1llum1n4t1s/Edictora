# 構成

画面操作の「判断」は Rust、ページ内での「情報収集」だけを TypeScript に分ける。

- `crates/edictora`（Rust、lib + bin `edictora.exe`・`edictora-updater.exe`）: 設定・比較・Chrome 起動・CDP・ダッシュボード操作・手順・CLI・自動更新。外部の引数パーサ・CDP ライブラリ・HTTP/TLS crate は使わない（HTTPS は WinHTTP を FFI で直接使う）。
- `page-scripts`（Bun + TypeScript）: ページ内で評価するスクリプトだけ。生成物 `page-scripts/dist/edictora.js` をコミットし、`include_str!` で exe に埋め込む。`cargo build` と配布 exe に Bun は要らない。

## 本体の内訳（`crates/edictora/src/`）

| モジュール | 責務 |
|---|---|
| `cdp/` | CDP クライアント（localhost の WebSocket、TLS なし、`tungstenite` の同期 API）とページ単位の操作（評価・マウス・キー・遷移） |
| `chrome/` | Chrome の検出と起動。専用プロファイルで起動するか、そのプロファイルの Chrome がデバッグポート付きで開いていれば接続する。終了は `Browser.close` |
| `config/` | 設定ファイルの読込と検証（未知のキーはエラー）、正本の区間切り出し |
| `listing/` | 欄の定義、正本と現在値の完全一致の比較、行単位の差分表示、画像の正本の組み立て（`build_expected_images`）、カテゴリの対応表 |
| `images/` | 掲載画像のモデル・正本の検証・画素ダイジェストの比較。`png.rs`（zlib の展開とフィルターの復元を自前で持つ）/ `fetch.rs`（原本の取得。trait `ImageFetcher` が境目）。[掲載画像](listing-images.md) |
| `privacy/` | 「プライバシー」タブのモデル・正本の組み立て・比較（権限の一覧の照合を含む）。画面操作は持たない。[「プライバシー」タブ](privacy-tab.md) |
| `dashboard/` | ダッシュボード操作。`urls.rs`（publisherId の取り出し）/ `text.rs`（画面の文言）/ `page_scripts.rs`（埋め込み JS の呼び出し）/ `selectors.rs`（要素の選択）/ `write_gate.rs`（書き込み可否）/ `item_page.rs`（1 操作ずつの実装）/ `browser.rs`（接続とログイン確認）。画面操作の境目の trait は 4 つ: `EditorPage`（共通: 開く・読み直す・状態・言語・下書き保存・診断）、`ListingPage: EditorPage`（テキスト欄と審査送信）、`ImagePage: EditorPage`（画像の読み・削除・アップロード）、`PrivacyPage: EditorPage`（「プライバシー」タブの読み書き）。**審査送信を持つのは `ListingPage` だけ** |
| `net/` | `winhttp.rs`: WinHTTP の FFI（GET だけ、メモリかファイルへ受ける）。自動更新と画像の原本の取得が使う |
| `update/` | 自動更新（`locate.rs`・`manifest.rs`・`feed.rs`・`version.rs`・`source.rs`・`download.rs`・`hash.rs`・`lock.rs`・`apply.rs`。trait `ApplyLauncher` が境目）。[配布と自動更新](distribution-update.md) |
| `install/` | Velopack のフック（`hooks.rs`）、HKCU の Run・PATH（`registry.rs`）、Claude Code・Codex への MCP 自動登録・解除（`connect.rs`・`toolport.rs`） |
| `services.rs` | pull・diff・push・submit の手順と中止判断 |
| `cli/` | 引数解析・実行と終了コード・差分表示・再ログイン（`relogin.rs`） |
| `mcp/` | MCP サーバー。ツールの中身は `CliApp::run` を呼んで出力をメモリで受けるだけ。[MCP サーバー](mcp-server.md) |
| `i18n.rs` | 利用者向け出力の表示言語の決定と、辞書（`locales/<タグ>.json`）をキーで引く `tr!` / `trs!`。[表示言語](display-language.md) |
| `redirect.rs` | MSIX 内のアプリからの保存先リダイレクトの検出と案内。[リダイレクト](storage-redirect.md) |
| `logging.rs` ほか | ログ（publisherId を伏せる）、保存先（`AppPaths`）、エラー型、Ctrl+C |

## 役割分担の考え方

手順は `services.rs` に置き、CLI は呼ぶだけ。ページ内 JS は情報収集だけで、選択と書き込み可否は Rust の純粋関数が決める。CDP・中断・保存先の注入・正規表現の扱いは [構成の設計原則](architecture-principles.md)。
