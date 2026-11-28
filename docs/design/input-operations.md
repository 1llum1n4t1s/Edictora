# 入力操作

- クリックは scrollIntoView 後に矩形中心へ実際のマウス入力（`clickCount: 1`）を送り、中心が他の要素に覆われていたら押さない。
- 欄の書き換えはフォーカス → 全選択 → `Input.insertText`（空にするときは Delete キー）。通常の入力として受け付けられ、文字数カウンタも更新される。DOM の `value` への直接代入は、フォームに変更として認識されない恐れがあるので使わない。
- 画像のアップロードは、ページの `input[type=file]`（非表示のものがある）へ `DOM.setFileInputFiles` でファイルを渡す（`CdpPage::set_file_input_files`）。式を `returnByValue: false` で評価して objectId を得て、パスは CDP の引数で渡し、終わったら `Runtime.releaseObjectGroup` で解放する。
- 画像の削除ボタンはホバーで現れる（普段は 0x0・透明）。画像の上へマウスを動かしてから、現れたボタンを実マウス入力で押す。
- 待ちは固定の sleep ではなく、条件を一定間隔で確かめる（上限時間つき）。
