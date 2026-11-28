# Chrome 操作と要素の選択

- **Chrome の起動引数は最小限**（`--user-data-dir`・`--remote-debugging-port=0`・`--no-first-run`・`--no-default-browser-check` と描画の間引き抑止 3 つ・`--disable-features=CalculateNativeWinOcclusion`、ヘッドレス時だけ `--headless=new --window-size=1280,900`）。`--enable-automation` などの自動化フラグや画面外配置は付けない（Google ログインを弾かれる恐れ）。変える場合は実画面でログインを再検証する。
- **Chrome を起動する経路は `seed_local_state` を通す**: `chrome::launcher::launch_or_attach` は新規起動の直前に user-data-dir の `Local State` へ `password_manager.os_password_blank=false` と `os_password_last_changed="9223372036854775000"` を書く（既存キーは保つ。`tests/chrome_launcher.rs` で固定）。新規プロファイルの連続起動で Windows アカウントがロックされるのを防ぐため。別経路を足すときも同じシードを先に行う。
- **プロファイルは秘密情報で、ツールは絶対に削除しない。** 終了は `Browser.close`、応答しないときだけプロセスを止める。ログイン待ちの間はタブに接続しない。
- **publisherId はログイン後の URL から実行時に取り出し、コード・設定・ログに書かない。** ログと CLI のエラー出力は伏せ字処理を通す。
- **要素はラベル・役割・文言で探し、クラス名に頼らない。** 文言は `dashboard/text.rs` に集める。ラベルは 1 行単位の完全一致、言語一覧の開閉は `aria-expanded`、0×0 の一覧は数えない。候補がちょうど 1 つに絞れなければ止める（最初の 1 つを選ばない）。画面言語は `<html lang>` で 1 回だけ判定して文言セット（`text.rs` の JA / EN）を選ぶ（空なら日本語、ja / en 以外は止める）。英語の文言は実画面で未確認の暫定値で、英語で足す判定は「止める」側だけにし、「書いてよい」側へ推測で足さない（`write_gate.rs` の `UNCONFIRMED_LOOKALIKE_KEYWORDS` など）。
- **Chrome を前面に出さない。** `login` 以外はヘッドレス（`--show-browser` で表示）。**唯一の例外**: pull / diff / push / submit の入口で再ログインが要る（`Error::LoginRequired`）と分かったときだけ、`cli/relogin.rs` の `open_with_relogin` が、ヘッドレスを閉じ → 表示つきで開いて `wait_for_login`（既定 10 分）→ 閉じ → 開き直して確認し直す。再試行は 1 回だけで、失敗・時間切れ・2 回目もログイン画面は終了コード 5。作業の途中のセッション切れは対象外。
- UA の HeadlessChrome は Chrome に置き換え、操作の前に毎回 `Emulation.setFocusEmulationEnabled`。`Page.bringToFront` は使わない。クリックは scrollIntoView 後に矩形中心へ実マウス入力、覆われていたら押さない。書き換えはフォーカス → 全選択 → `Input.insertText`（DOM の `value` 直接代入はしない）。待ちは固定 sleep でなく上限つきの条件確認。
- **ページ内スクリプトに利用者の入力値を埋め込まない。** 書き込む文字列は CDP の `Input.insertText` で渡す。
- **AI エージェントは edictora を `--show-browser` 付きで実行しない**（利用者の画面にウィンドウが出る）。原因調査はエラー文と diagnostics のスナップショット（テキスト）で行う。「覆われていて押せません」のエラー文には、中心にあった要素・要素の枠・画面の大きさを含める（`selectors::covered_error_message`）。
- 失敗時の診断は**画像ではなくページのテキスト スナップショット**（`yyyyMMdd-HHmmss-<ラベル>.txt`）を `%LOCALAPPDATA%\Edictora.Data\diagnostics` に置き、リポジトリには置かない（`Page.captureScreenshot` は使わない）。中身はページ内スクリプトの `snapshot()` が作る（見えている要素の 役割・名前・値・状態・矩形 の字下げツリー。1 値 2,000 文字・全体 1,000,000 文字で切る）。保存前に必ず `logging::redact_diagnostics` を通し、publisherId を `<publisher>`、Google アカウントの氏名を `<name>`、メールアドレスを `<email>` にする。push / submit の失敗と、pull / diff の読み出し失敗（ログイン切れ以外）で保存し、保存先を結果のエラー文に含める。
- CDP は入出力スレッド 1 本が非ブロッキングのソケットを持ち、応答待ちの間もイベントを配る。
