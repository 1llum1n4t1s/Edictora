# 書き込みの安全策・終了コード・設定の契約

- **書き込みの安全策**: `write_gate.rs` は確認済みの状態（許可リスト）以外を止める。審査の取り消しはしない。言語ごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」、最後に全言語を読み直す。途中で失敗したら、そのアイテムの残りは行わない。beforeunload は「留まる」を選んで中止する。
- **審査送信は 1 件ずつ明示**（`submit --item <id> --yes`）。「自動的に公開する」チェックは変えずに報告する。
- **引数と設定の誤りは Chrome を開く前に終了コード 2 で止める。** 終了コードは 0 / 1 / 2 / 3（差分あり）/ 4（書き込み中止）/ 5（ログインが必要）/ 130（中断）。進捗は標準エラー、結果は標準出力。
- **区間切り出しは検証スクリプト（push.mjs）と同じ結果にする。** 見出しが無ければエラー。`sectionStart` / `sectionEnd` は Rust の `regex` で解釈するため、先読み・後読み・後方参照は使えない（設定の契約）。`crates/edictora/tests/fixtures/ReplaceTranslator` の SHA-256・文字数（ja=1492 / en=3172）のテストを壊さない。
- **設定に書いていない欄は、読み出し・比較・書き換えの対象外。** 未知のキーはエラー。URL 欄はキーなし＝対象外、`""`＝空欄が正本。
- **カテゴリ**（`category`・`listing/category.rs`・`ListingPage::read_category` / `write_category`）: 全言語共通のアイテム単位の欄なので `ListingField`（ロケール単位）には入れず、`ExpectedItem::category`（キー）で持つ。値は言語に依らない小文字のキーで、`data-value` との対応表は `listing/category.rs` の 1 か所（未知のキーは Chrome を開く前に終了コード 2。未選択のプレースホルダーは正本にできない）。画面は `ul[role=listbox][aria-label="カテゴリ"]` の `aria-selected="true"` の `data-value` を**閉じたまま**読み（判断は `selectors::category_options` / `selected_category` / `pick_category_option`）、書き込みはコンボボックスを実マウスで開く → 一覧が見えるまで待つ → 一致する選択肢をちょうど 1 つ実クリック → 閉じて選択を確かめる。**日本語の画面だけ**（`DashboardText::category` が EN は None）。手順（`services::push_core`）はロケールのループの前に独立した 1 サイクルで、不足ロケールの確認より後。読み出しの失敗はこの欄だけのエラー（`ItemSnapshot::category` が `Err`）にしてロケールの読み出しは続ける。`--only text` に含み、`--locale` では絞らない。
