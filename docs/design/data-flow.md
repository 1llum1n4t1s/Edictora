# データフロー

1. **入力（正本）**: `%APPDATA%\Edictora\config.json`（`--config` で変更可）。`profileDir`・`chromePath`（任意）と `items`（`itemId`・`name`・`baseDir`・`locales`・`images`・`privacy`・`category`）。ロケールごとに `description`（`file`、任意で `sectionStart` / `sectionEnd`）と `homepageUrl` / `supportUrl` / `officialUrl` を書く。書いていない欄は読み出し・比較・書き換えの対象外。URL 欄はキーなし＝対象外、`""`＝空欄が正本。`locales`・`images`・`privacy`・`category` の少なくとも 1 つが要る（画像だけのアイテムなども作れる）。`images` は書いたキーだけが対象で、`icon` / `smallTile` / `marquee`（1 枚のパス）・`globalScreenshots`（`{"dir"}` か `{"files"}`、1〜5 枚）・`localizedScreenshots`（ロケール → `{"dir"}` か `{"files"}`。`zh_CN` と `zh-CN` は同じ扱い）。`dir` は直下の `screenshot-*.png` を名前の昇順で並べる。詳細は [掲載画像](listing-images.md)・[「プライバシー」タブ](privacy-tab.md)・[カテゴリ](category.md)。任意の `mcpRegistration`（`auto` / `direct` / `off`）は [MCP サーバー](mcp-server.md)。
2. **正本の読み取り**: 区間は、見出し行の次から終端見出しの手前までを取り、前後の空行と末尾の空白を除き、改行を `\n` に揃える。見出しが無ければエラー。
3. **Chrome 起動**: `login` 以外はヘッドレス。専用プロファイルの Chrome を最小の引数で起動し、ログイン状態を引き継ぐ。入口で再ログインが要ると分かったときだけ、表示つきで開いてログインを待つ（[Chrome の起動](chrome-launch.md)）。
4. **ダッシュボード操作**: アイテムの編集画面を開き、言語を切り替え、欄を読む。
5. **比較**: 正本と現在値を完全一致で比べ、差分を行単位で表示する（`diff`・`push` の確認）。画像は画素のダイジェストで比べる（[掲載画像](listing-images.md)）。
6. **書き込み（`push --yes`）**: 言語ごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」を行い、最後に全言語を読み直して照合する。カテゴリ → 画像 → 「プライバシー」タブは、それぞれ別の手順・別の保存として後に続く（`--only text|images|privacy`・`--image-scope all|global|localized` で絞れる）。
7. **審査送信（`submit --item <id> --yes`）**: 確認ダイアログで送信する。

`update` は 1〜7 を通らず、配信元のフィードを見て新しい版をダウンロードし、Update.exe に差し替えを任せる（[配布と自動更新](distribution-update.md)）。

結果は標準出力、進捗は標準エラー。終了コードは 0（成功・差分なし）/ 1（実行時エラー）/ 2（引数・設定の誤り、Chrome は開かない）/ 3（差分あり）/ 4（書き込み中止）/ 5（ログインが必要。自動のログイン待ちが完了しなかったか、作業の途中でログインが切れた。もう一度実行すると、必要ならログイン用の窓が開く）/ 130（中断）。

保存先: Chrome プロファイル（既定 `%LOCALAPPDATA%\Edictora.Data\chrome-profile`、`profileDir` / `--profile` で変更可）・ログ（`...\logs`）・失敗時のページのスナップショット（`...\diagnostics`）は `%LOCALAPPDATA%\Edictora.Data`、設定は `%APPDATA%\Edictora`。いずれもリポジトリの外。`%LOCALAPPDATA%\Edictora` は Velopack のインストール先でアンインストールで消えるので、データを置かない。
