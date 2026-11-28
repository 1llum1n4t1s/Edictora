# 「プライバシー」タブ

`privacy`・`services::push_privacy`。詳細は [DESIGN.md](../../DESIGN.md)「「プライバシー」タブ」。

- 権限を足すと、タブ（`<編集画面>/privacy`）に正当性などを入れないと審査に送信できない。
- **設定の `privacy` に書いた欄だけ**（`singlePurpose`・`permissions`・`remoteCode`・`dataUsage`・`policyUrl`）を、説明文と同じ区間の正本（`config/section.rs`）から読み取り・比較・下書き保存する。1 欄も無ければエラー（policyUrl も 1 欄に数える）。
- 本文は共通の字下げを取り（`privacy::dedent`）、空・1,000 文字超過は Chrome を開く前に終了コード 2 で止める。`dataUsage.certify` は true だけ許す。
- 手順は `&mut dyn PrivacyPage` だけを受け取る（審査送信は無い。`EditorPage` ← `ListingPage` / `ImagePage` / `PrivacyPage` の分離を崩さない）。
- **`permissions` を書いたときは権限の一覧そのものも照合し**、食い違えば `diff` は終了コード 1、`push` は何も書かず `WriteBlocked` で止める。
- 欄は `selectors::classify_privacy_textareas` などで 1 つに絞る（絞れなければ止める）。保存はテキスト・画像とは別に最後に行う。
- **日本語の画面だけ**扱う。**リモートコードは未対応**（設定に書くと `diff` がエラー、`push` が止まる）。`--only privacy` で絞れ、`--locale` では絞らない。
- **プライバシー ポリシー URL**（`privacy.policyUrl`・`PrivacyField::PolicyUrl`・`PrivacyPage::write_policy_url`）: 短い 1 行の欄（`input[type=text]`、検査は `selectors::ensure_privacy_url_writable`）。ラベル「プライバシー ポリシーの URL」の 1 行完全一致で 1 つに絞る（`find_policy_url_input`）。契約は公式 URL などと同じ（http(s) の絶対 URL、2,048 文字以内）。
