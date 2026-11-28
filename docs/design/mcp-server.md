# MCP サーバー（`edictora mcp`）

- 背景: 2026-11-15、別の作業の AI が掲載情報を直そうとして Edictora に気付かず、期限切れの CWS メタデータ API（`cws_mcp__update_metadata`、2026-10-15 で終了）を探していた。CLI を教えると今度は `--show-browser` を付けた。AI が見つけられ、画面を出さない形でしか呼べない入口が要る。
- 形: 同じ exe のサブコマンドにし、別の exe・crate は足さない。プロトコルは 2025-06-18（2025-03-26・2024-11-05 を要求されればその版で応える）。`initialize` / `ping` / `tools/list` / `tools/call` に応え、通知には応えない。一括送信（配列）は -32600。
- ツール: `edictora_list_items`（設定の対象を表示、ブラウザなし）、`edictora_diff` / `edictora_pull`（読み取り専用）、`edictora_push`（`confirm: true` で `--yes`）、`edictora_submit`（`item` と `confirm: true` が必須）。中身は CLI の引数の配列を作って `CliApp::run` を呼ぶ。CLI と同じ設定の検証・終了コード・伏せ字・書き込みの安全策・実行中ロック・失敗時のスナップショット保存を通すため。
- 選ばせないもの: 画面表示（`--show-browser`）・`login`・`update`・`--profile` / `--chrome` / `--config`。スキーマは `additionalProperties: false` で、無い引数は黙って無視せずエラーにする。ログイン切れは CLI が表示つきの Chrome を自動で開いて待ち、完了しなかったとき（終了コード 5）だけ「利用者が端末で `edictora login`」と案内する。
- 結果: 先頭に終了コードとその意味、続けて標準出力（結果）と標準エラー（進捗・エラー）。0 と 3（差分あり）は `isError: false`、ほかは `true`。
- 並行: 作業スレッド 1 本で 1 件ずつ（同じプロファイルの Chrome を同時に開かない）。受信は続け、`notifications/cancelled` で実行中の呼び出しを中断する（CLI の Ctrl+C と同じ経路で Chrome を正常終了させる）。入力が閉じても、受け付け済みの呼び出しは最後まで実行してから終わる（書き込みを半端にしない）。
- 更新との関係: Velopack の `Update.exe apply` はインストール先から動いている他のプロセスを強制終了してから差し替える（`force_stop_package`、Velopack のソースで確認済み）。待機中でも止められるとクライアントのセッションから Edictora が消えるので、**`main` がサーバーの生存期間中ずっと実行中ロックを共有で持つ**（複数セッションの MCP 同士、呼び出しの中の `CliApp::run` とも両立する）。ロックの契約は [配布と自動更新](distribution-update.md)。サーバーが動いている間に更新が来たら、ダウンロードと検証だけ済ませて適用を見送り、手動の `edictora update` は 1 で終わってクライアントを閉じてから実行し直すよう案内する。更新の適用中に起動されたサーバーは、標準出力に何も書かず終了コード 1 で終わる。残る隙: 更新プロセスが終わってから Update.exe が差し替え終えるまでの数秒に起動したサーバーは守れない（CLI と同じ）。ログオン直後にクライアントが自動で起動する環境では、適用が延び続けることがある。
- 登録: Setup 版はフックで Claude Code・Codex へ自動で登録・解除する。`mcpRegistration`（`auto` / `direct` / `off`）で動作を選べ、Toolport が既に提供していれば直接登録を省く。詳細は [MCP の自動登録と Toolport 判定](mcp-registration.md)。
- 見つけやすさ: ツールの説明に英語の検索語（Chrome Web Store・store listing・description）を入れ、日本語は混ぜない（ASCII だけ。テストで固定する）。
