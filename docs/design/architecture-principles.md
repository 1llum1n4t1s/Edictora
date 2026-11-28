# 構成の設計原則

[構成](architecture.md)の補足。役割分担の考え方を示す。

- **手順は `services.rs`、CLI は呼ぶだけ。** 画面操作は trait 越しに行うので、手順は偽ページでブラウザなしにテストできる。画像の手順は `&mut dyn ImagePage` だけを、「プライバシー」タブの手順は `&mut dyn PrivacyPage` だけを受け取り、型の上で審査送信を呼べない。
- **ページ内 JS は候補要素の情報（タグ・文字・ラベル候補・無効/読み取り専用・`aria-expanded`・チェック状態）を集めて返すだけ。** どれを選ぶか・書き込んでよいかは Rust の純粋関数（`selectors.rs`・`write_gate.rs`）が決め、テストで固める。利用者の入力値はページ内スクリプトに埋め込まない（`Input.insertText` で渡す）。
- **CDP**: 入出力スレッド 1 本が非ブロッキングのソケットを持ち、応答は id ごと、イベントは購読ごとのチャネルへ配る。応答待ちの間も `Page.loadEventFired` と beforeunload の `Page.javascriptDialogOpening` を受け取れる。切断時は応答待ちと購読をすべて失敗させる。
- **中断**: Ctrl+C は `SetConsoleCtrlHandler` で受け、待ちの各所が合図を見て抜ける（終了コード 130）。その後の `Browser.close` と終了待ちは合図を見ないので、Chrome は正常終了する。
- **保存先の注入**: `AppPaths` を値として持ち回し、`main` だけが環境（`EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` による上書きを含む）から決める。テストは `target/tmp` 配下を根にする。`EDICTORA_UPDATE_URL` も入口（`main` と更新用 exe）だけが読み、値で渡す。
- **正規表現**: `sectionStart` / `sectionEnd` は Rust の `regex` で解釈する。先読み・後読み・後方参照は使えない（読み込み時にエラー）。どのパターンも線形時間なので時間による打ち切りは持たない。ログの publisherId 伏せ字も `regex` で行う。
