# 保存先

- **MSIX パッケージ内のアプリ（Claude デスクトップの Bash など）から実行すると、`%APPDATA%` / `%LOCALAPPDATA%` に新規作成したフォルダーはパッケージ専用領域（`%LOCALAPPDATA%\Packages\<名前>\LocalCache\...`）へ入り、通常のターミナルから見えなくなる**。Edictora は既定の設定とプロファイルがそこから読まれていれば標準エラーへ警告し（`redirect.rs`）、既定の設定が無ければ専用領域の候補をエラー文に添える。AI エージェントは設定・データのフォルダーを新規に作らず、設定を作るときは本物の場所にあるか確かめる。詳細は [DESIGN.md](../../DESIGN.md)「パッケージ内のアプリからの保存先リダイレクト」。
- 設定（正本の対応づけ）: `%APPDATA%\Edictora\config.json`（`--config` で変更可）
- Chrome プロファイル: `%LOCALAPPDATA%\Edictora.Data\chrome-profile`（設定の `profileDir` / `--profile` で変更可）
- ログ: `%LOCALAPPDATA%\Edictora.Data\logs`、失敗時のページのスナップショット（テキスト）: `%LOCALAPPDATA%\Edictora.Data\diagnostics`
- アプリ本体（Velopack のインストール先）: `%LOCALAPPDATA%\Edictora`。アンインストールで消えるので、データはここに置かない
- 検証用に環境変数 `EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` で保存先の根を、`EDICTORA_UPDATE_URL` で更新の配信元を差し替えられる
