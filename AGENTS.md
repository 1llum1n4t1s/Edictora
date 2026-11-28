# AGENTS.md — Edictora 開発ガイド

Chrome Web Store（CWS）の掲載情報（説明・URL・画像・「プライバシー」タブ）を、各拡張機能リポジトリの正本ファイルから一括で読み出し・比較・更新する Windows 向け CLI（`edictora.exe`）。CWS API に掲載情報の読み書きが無いため、専用プロファイルの Chrome を CDP で操作してダッシュボードを書き換える。実装は Rust と、ページ内 JS 用の Bun + TypeScript。GUI は無い。全体設計は [DESIGN.md](DESIGN.md) が正本。

## 守る前提（要約。詳細は各ファイル）

- 手順は `services.rs`、画面操作は trait 越し、要素の選択は Rust の純粋関数: [architecture](docs/agents/architecture.md)・構成は [source-layout](docs/agents/source-layout.md)
- Chrome は最小の起動引数・前面に出さない・プロファイルは削除しない・要素はラベルで探す。AI は `--show-browser` を使わない: [browser-automation](docs/agents/browser-automation.md)
- 書き込みは許可リスト以外を止める。審査送信は 1 件ずつ明示。誤りは Chrome 前に終了コード 2: [write-safety](docs/agents/write-safety.md)
- 掲載画像は画素ダイジェストで比較: [listing-images](docs/agents/listing-images.md)
- 「プライバシー」タブは設定に書いた欄だけ扱う: [privacy-tab](docs/agents/privacy-tab.md)
- MCP は CLI の薄い皮。ツールは 5 つだけ: [mcp-server](docs/agents/mcp-server.md)
- 自動更新（Velopack）と実行中ロック: [auto-update](docs/agents/auto-update.md)
- コメント・コミットは日本語、人向け文言は 9 言語、依存は最小限: [coding-conventions](docs/agents/coding-conventions.md)
- 設定・プロファイル・ログの保存先と MSIX リダイレクト: [storage-paths](docs/agents/storage-paths.md)
- fmt・clippy・test。テストで Chrome を起動しない: [build-and-test](docs/agents/build-and-test.md)
- 未確認の画面状態と既知のリスク: [known-issues](docs/agents/known-issues.md)
- リリースは `/vava` とタグ push の CI: [release-process](docs/agents/release-process.md)（[docs/release.md](docs/release.md)）
