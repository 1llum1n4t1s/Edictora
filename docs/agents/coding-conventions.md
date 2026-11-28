# コーディング規約

- コメント・コミットメッセージは **日本語**。コメントは目的・設計意図・制約を多めに書く。
- バージョンは `Cargo.toml` の `[workspace.package] version` に一元化。**更新は /vava 指示時のみ**。
- 依存は最小限（`serde` / `serde_json` / `regex` / `tungstenite`（同期・TLS なし・`handshake` のみ）、テスト専用に `sha2`）。HTTP/TLS crate は使わない（DevTools の確認は localhost への素の HTTP/1.1）。
- **外部への HTTPS（自動更新、画像の原本）は、crate を足さず WinHTTP を FFI で直接使い、`net/winhttp.rs` に集める**（リダイレクト無効・タイムアウトと全体の期限・Ctrl+C で抜ける・応答の上限・平文は 127.0.0.1 のテストだけ）。Ctrl+C・レジストリ・ハッシュ（CNG）などは Windows API を直接宣言して使う。
- `AppPaths` は値として注入する。テストは `target/tmp` 配下を根にし、実プロファイル・実設定を解決しない。CLI のテストはブラウザを開く関数に「呼ばれたら panic」を渡す。
- Bun 側のパッケージ管理は Bun（`packageManager: bun@1.4.5`、`bunfig.toml` の `minimumReleaseAge`）。依存は `typescript` だけ。

## 表示言語（9 言語）

- 人が読む文言（CLI の進捗・結果・エラー、差分、ヘルプ）は 9 言語（ja・en・zh-CN・zh-TW・ko・es・fr・de・pt-BR）で、`crates/edictora/src/locales/<タグ>.json` の辞書に持ち、`tr!("キー", 名前 = 値)` / `trs!("キー")`（`i18n.rs`）で参照する。プレースホルダーは `{name}` で全言語同じ集合、辞書に無いキーは英語。
- 表示言語は `EDICTORA_LANGUAGE` → `LC_ALL` → `LC_MESSAGES` → `LANG` → OS の表示言語 → 英語。`cargo test` は `.cargo/config.toml` で `EDICTORA_LANGUAGE=ja` に固定される。
- **人が読む文言を足したら、9 言語の辞書を同時に更新し、整合テスト（`tests/localization.rs`）を通す**。
- AI が読む文言（MCP のツール名・説明・スキーマ・応答とエラー）は英語に統一し辞書を通さない。利用者のデータ・識別子・コマンド・設定キー・外部のエラー原文は訳さない。ログの定型文は日本語のまま。
- **README は英語の `README.md` を正本とし、`README.<タグ>.md`（ja・zh-CN・zh-TW・ko・es・fr・de・pt-BR）を置く。README を変えたら全言語版を同時に直す**。リリースのパッケージに同梱するのは英語の `README.md` だけ。
