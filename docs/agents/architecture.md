# アーキテクチャ

実装は **Rust**（本体・CDP・Chrome 起動・設定・比較・CLI）と、ページ内で評価する JavaScript だけを書く **Bun + TypeScript** の 2 本立て。画面（GUI）は持たない（C# + Avalonia 版は廃止。履歴の `b1adbc4` から参照できる）。全体設計は [DESIGN.md](../../DESIGN.md) が正本。

リポジトリとモジュールの構成は [source-layout.md](source-layout.md) を参照。

## 手順と画面操作の分離

- **手順は `services.rs` に置き、CLI は呼ぶだけにする。** 画面操作は trait（`ListingPage`・`ImagePage`・`PrivacyPage`）越しに行い、手順はテスト用の偽ページでブラウザなしに検証する。
- **ページ内の JS は候補要素の情報を集めて返すだけ。** どれを選ぶかは Rust の純粋関数（`dashboard/selectors.rs`）で判断し、テストで固める。dist は直接編集せず `bun run build` で作り、`bun run check` で最新か確かめる。exe と `cargo build` に Bun を要らせない（`include_str!` で埋め込む）。
