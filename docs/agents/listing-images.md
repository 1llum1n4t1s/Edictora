# 掲載画像

`images`・`services::push_images`。詳細は [DESIGN.md](../../DESIGN.md)「掲載画像」。

- 手順は `&mut dyn ImagePage` だけを受け取る（審査送信は無い）。
- **比較はファイルのバイト列ではなくデコードした画素の SHA-256**（ダッシュボードは PNG を再エンコードする。`images/png.rs`）。
- 原本は `=s0` を Cookie なしの `net/winhttp.rs` で取得し、取得・デコードできなければ「比較不能」で止める（一致扱いにも常に置換にもしない）。
- スロットは `selectors::pick_image_slot` が絞る（絞れなければ止める）。クリックは削除ボタンと削除確認だけで、並べ替えには触れない。
- ストア アイコン（`images.icon`、128x128）も同じ仕組みで、全言語共通なので `--image-scope global` に含める。アイコンだけは透過（RGBA）を許し、`png::pixel_digest_rgba` で比べる。
- 書き込みは日本語の画面だけ。**画像の削除はサーバーへ即座に反映される**ので、途中で止まると下書きから画像が欠ける（再実行で整える）。
- ローカライズ版が対象でロケールがダッシュボードに無ければ、何も触らず止めて `--image-scope global` か `--locale` を案内する。
- CLI は `--only text|images|privacy`・`--image-scope all|global|localized`（MCP は `only`・`imageScope`）。
