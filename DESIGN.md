# Edictora 設計

Chrome Web Store の掲載情報（説明・URL・掲載画像・「プライバシー」タブ）を、各拡張機能リポジトリの正本から一括で読み出し・比較・更新する Windows 向け CLI。詳細は [docs/design/](docs/design/) にトピックごとに置く。使い方は [README.md](README.md)、規約は [AGENTS.md](AGENTS.md)、リリースは [docs/release.md](docs/release.md)。

## 背景と目的

CWS API に掲載情報の読み書きがないため、専用プロファイルの Chrome を CDP で操作して画面を書き換える。→ [背景と目的](docs/design/background-and-goals.md)

## 構成

Rust（判断）と Bun + TypeScript（ページ内の情報収集）の 2 本立て。→ [構成](docs/design/architecture.md)

### 本体の内訳（`crates/edictora/src/`）

モジュールごとの責務。→ [構成](docs/design/architecture.md)

### 役割分担の考え方

手順は `services.rs`、CLI は呼ぶだけ。→ [構成](docs/design/architecture.md)

## データフロー

設定 → 正本 → Chrome 起動 → 操作 → 比較 → 書き込み → 審査送信。→ [データフロー](docs/design/data-flow.md)

## 重要な不変条件

書き込みは明示時のみ・許可リスト方式・照合してから保存。→ [重要な不変条件](docs/design/invariants.md)

## 採用済みの設計判断

### Chrome の起動

最小の引数のヘッドレス起動。→ [Chrome の起動](docs/design/chrome-launch.md)

### 表示言語（出力とダッシュボード画面）

出力は 9 言語、画面は ja / en で別物。→ [表示言語](docs/design/display-language.md)

### 入力操作

実マウス入力と `Input.insertText`。→ [入力操作](docs/design/input-operations.md)

### 掲載画像

画素ダイジェストで比較して置換する。→ [掲載画像](docs/design/listing-images.md)

### カテゴリ

全言語共通の欄をキーで扱う。→ [カテゴリ](docs/design/category.md)

### 画面構成と欄の探し方

ラベルの 1 行完全一致で探す。→ [画面構成と欄の探し方](docs/design/dashboard-screens.md)

### 「プライバシー」タブ（Privacy practices）

設定に書いた欄だけを扱う。→ [「プライバシー」タブ](docs/design/privacy-tab.md)

### 区間切り出し

検証スクリプトと同じ結果にする。→ [区間切り出し](docs/design/section-extraction.md)

### パッケージ内のアプリからの保存先リダイレクト（`redirect.rs`）

MSIX 内の保存先の仮想化を検出して警告する。→ [リダイレクト](docs/design/storage-redirect.md)

### 配布・インストール・自動更新（Velopack）

更新の確認とダウンロードだけ自前で行う。→ [配布と自動更新](docs/design/distribution-update.md)

### MCP サーバー（`edictora mcp`）

CLI の薄い皮として提供する。→ [MCP サーバー](docs/design/mcp-server.md)

## 実画面での確認状況

確認済み・未確認の一覧。→ [実画面での確認状況](docs/design/verification-status.md)
