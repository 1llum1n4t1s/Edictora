# カテゴリ欄の DOM と読み出し

[カテゴリ](category.md)の補足。

- **DOM**（2026-11-19、SearchClock の日本語画面）: コンボボックスは `div[role=combobox]`（`aria-haspopup="listbox"`、`aria-expanded="false"`、aria-label なし、`aria-labelledby` の参照先が「カテゴリ」）。innerText は「カテゴリ\nカテゴリ\nカテゴリ\n機能と UI」（ラベル 3 行 + 現在の表示値）。一覧は `ul[role=listbox][aria-label="カテゴリ"]` で、閉じている間は 0×0 だが `li[role=option]` が全部 DOM にあり、各 option は `data-value` と `aria-selected`（"true"/"false"）を持つ。同じ画面に言語・公式 URL（`input[role=combobox]` と空の listbox）・投稿者アカウント切替の listbox / combobox があるので、一覧はラベルで絞る。option は 22 件（未選択のプレースホルダー「カテゴリを選択してください」＝ `Select a category` と 21 カテゴリ）。
- **読み出し**（一覧を開かず、閉じたまま読める）: page-scripts の種類 `listboxOption`（`kinds::LISTBOX_OPTION`）が、閉じて見えないものも含めて `[role=option]` をすべて集め、各要素の `label`（所属する一覧の aria-label / aria-labelledby）・`data_value`・`selected`（aria-selected）・`group`（ページ内の `[role=listbox]` の通し番号）・一覧の枠の大きさを返すだけにする（既存の `option` 種類の挙動は変えない）。Rust 側の純粋関数（`selectors`）が、`category_options`（ラベルの 1 行が「カテゴリ」の一覧の選択肢だけ。`group` が 1 種類でなければ止める）→ `selected_category`（`aria-selected="true"` がちょうど 1 つでなければ止める。プレースホルダーが選ばれていればその値のまま返し、正本と不一致にする）で選ぶ。`data-value` が対応表に無い値は、生の値を表示して不一致にする。reload 直後は選択肢が揃うまで上限つき（15 秒）で待つ。
