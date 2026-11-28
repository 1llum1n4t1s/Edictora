# MCP サーバー

`edictora mcp`。詳細は [DESIGN.md](../../DESIGN.md)「MCP サーバー」。

**CLI の薄い皮にする**。

- ツールは `edictora_list_items` / `edictora_diff` / `edictora_pull` / `edictora_push` / `edictora_submit` だけで、中身は `CliApp::run` を呼び出力をメモリで受ける（手順・安全策・終了コード・伏せ字を二重に持たない）。
- **画面表示・login・update・プロファイル・chrome.exe の指定はツールに持たせない**（再ログイン時に CLI 側が表示つきの Chrome を自動で開く動きだけは例外で、説明文・INSTRUCTIONS に英語で書く。スキーマは `additionalProperties: false`）。
- push / submit は `confirm: true` が無ければ書かず、`destructiveHint` を付ける。終了コード 3 は `isError: false`。
- 呼び出しは作業スレッド 1 本で順に実行し、`notifications/cancelled` で中断する。標準出力には JSON-RPC 以外を書かない。
- **実行中ロックはサーバーの生存期間中ずっと共有で持つ**（`main.rs`）。プロファイルのロック（排他）は `CliApp::run` がツールの実行中だけ持つ。
- 説明文には英語の検索語（Chrome Web Store・store listing・description）を入れ、ASCII だけにする（テストで固定）。
- **Setup 版はフックで Claude Code・Codex へ自動登録・解除する**（`install/connect.rs`）。公式 CLI の `.exe` だけを時間制限つきで呼ぶ、既に自分を指す登録は触らない、解除は自分の exe を指す登録だけ、Portable 版は登録しない。テストはスタブ。
