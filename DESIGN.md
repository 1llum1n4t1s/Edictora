# Edictora 設計

Chrome Web Store（CWS）の掲載情報（説明・公式 URL・ホームページ URL・サポート URL と、掲載画像）を、各拡張機能リポジトリにある正本ファイルから一括で読み出し・比較・更新する Windows 向け CLI（`edictora.exe`）。ほかの人にも配る前提で、画面（GUI）は持たない。
利用者向けの使い方は [README.md](README.md)、開発者・LLM 向けの規約とコマンドは [AGENTS.md](AGENTS.md)、リリース手順は [docs/release.md](docs/release.md) を参照する。

## 背景と目的

- CWS API v2 には掲載情報（説明・URL など）の読み書きがない。旧 v1.1 の metadata API は 2026-10-15 で廃止済み。
- 拡張機能（Claude in Chrome など）からの操作は、Chrome が「The extensions gallery cannot be scripted.」で拒否する。
- Chrome DevTools Protocol（CDP）での直接操作はこの制限を受けない（2026-11-13 の実画面検証で確認）。
- そのため Edictora は、専用プロファイルの Chrome を CDP で操作し、デベロッパー ダッシュボードの画面を人と同じ入力イベントで書き換える。ダッシュボードの内部 RPC は呼ばない。
- 分担: cws-mcp は パッケージ（ZIP）のアップロードと公開（CWS API v2）、Edictora は掲載情報の更新（API がないため画面操作）。

## 構成

2 本立てで、画面操作の「判断」は Rust、ページ内での「情報収集」だけを TypeScript に分ける。

- `crates/edictora`（Rust、lib + bin `edictora.exe`・`edictora-updater.exe`）: 設定・比較・Chrome 起動・CDP・ダッシュボード操作・手順・CLI・自動更新。外部の引数パーサ・CDP ライブラリ・HTTP/TLS crate は使わない（自動更新の HTTPS は WinHTTP を FFI で直接使う）。
- `page-scripts`（Bun + TypeScript）: ページ内で評価するスクリプトだけ。生成物 `page-scripts/dist/edictora.js` をコミットし、`include_str!` で exe に埋め込む。`cargo build` と配布 exe に Bun は要らない。

### 本体の内訳（`crates/edictora/src/`）

| モジュール | 責務 |
|---|---|
| `cdp/` | CDP クライアント（localhost の WebSocket、TLS なし、`tungstenite` の同期 API）とページ単位の操作（評価・マウス・キー・遷移） |
| `chrome/` | Chrome の検出と起動。専用プロファイルで起動するか、そのプロファイルの Chrome がデバッグポート付きで開いていれば接続する。終了は `Browser.close` |
| `config/` | 設定ファイルの読込と検証（未知のキーはエラー）、正本の区間切り出し |
| `listing/` | 欄の定義、正本と現在値の完全一致の比較、行単位の差分表示、画像の正本の組み立て（`build_expected_images`） |
| `images/` | 掲載画像（全言語向けスクリーンショット・ローカライズ版スクリーンショット・小タイル・マーキー）のモデル・正本の検証（PNG・寸法・重複）・画素ダイジェストの比較。`png.rs`（zlib の展開とフィルターの復元を自前で持つ。画素のダイジェスト）/ `fetch.rs`（ダッシュボードの画像の原本を `=s0` で取得。trait `ImageFetcher` が境目） |
| `dashboard/` | ダッシュボード操作。`urls.rs`（publisherId の取り出し）/ `text.rs`（画面の文言）/ `page_scripts.rs`（埋め込み JS の呼び出し）/ `selectors.rs`（要素の選択）/ `write_gate.rs`（書き込み可否）/ `item_page.rs`（1 操作ずつの実装）/ `browser.rs`（接続とログイン確認）。画面操作の境目の trait は 3 つ: `EditorPage`（共通: 開く・読み直す・状態・言語・下書き保存・診断）、`ListingPage: EditorPage`（テキスト欄と審査送信）、`ImagePage: EditorPage`（画像の読み・削除・アップロード。**審査送信を持たない**） |
| `net/` | `winhttp.rs`: WinHTTP を FFI で直接使う HTTP クライアント（GET だけ、メモリかファイルへ受ける）。自動更新と、画像の原本の取得（`images/fetch.rs`）が使う |
| `update/` | 自動更新。`locate.rs`（インストールの判定）/ `manifest.rs`（`sq.version`）/ `feed.rs`（フィードと更新先の選択）/ `version.rs`（SemVer）/ `source.rs`（配信元）/ `download.rs`（ダウンロードと検証）/ `hash.rs`（CNG）/ `lock.rs` / `apply.rs`（Update.exe の起動。trait `ApplyLauncher` が境目） |
| `install/` | Velopack のフック（`hooks.rs`）、HKCU の Run・PATH の読み書き（`registry.rs`）、Claude Code・Codex への MCP 自動登録・解除（`connect.rs`） |
| `services.rs` | pull・diff・push・submit の手順と中止判断 |
| `cli/` | 引数解析・実行と終了コード・差分表示 |
| `mcp/` | MCP サーバー（`edictora mcp`）。標準入出力の改行区切り JSON-RPC。ツールの中身は `CliApp::run` を呼んで出力をメモリで受けるだけ |
| `i18n.rs` | 利用者向け出力の表示言語（9 言語）の決定と、辞書（`locales/<タグ>.json`）をキーで引く `tr!` / `trs!` |
| `redirect.rs` | MSIX パッケージ内のアプリから起動されたときの保存先リダイレクト（アプリ専用領域）の検出と案内。「パッケージ内のアプリからの保存先リダイレクト」の節を参照 |
| `logging.rs` ほか | ログ（publisherId を伏せる）、保存先（`AppPaths`）、エラー型、Ctrl+C |

### 役割分担の考え方

- **手順は `services.rs`、CLI は呼ぶだけ。** 画面操作は trait（`ListingPage`・`ImagePage`）越しに行うので、手順は偽ページでブラウザなしにテストできる。画像の手順（`pull_images` / `diff_images` / `push_images`）は `&mut dyn ImagePage` だけを受け取り、型の上で審査送信を呼べない。
- **ページ内 JS は候補要素の情報（タグ・文字・ラベル候補・無効/読み取り専用・`aria-expanded`・チェック状態）を集めて返すだけ。** どれを選ぶか・書き込んでよいかは Rust の純粋関数（`selectors.rs`・`write_gate.rs`）が決め、テストで固める。利用者の入力値はページ内スクリプトに埋め込まない（書き込む文字列は `Input.insertText` で渡す）。
- **CDP**: 入出力スレッド 1 本が非ブロッキングのソケットを持ち、応答は id ごと、イベントは購読ごとのチャネルへ配る。応答待ちの間も `Page.loadEventFired` と beforeunload の `Page.javascriptDialogOpening` を受け取れる。切断時は応答待ちと購読をすべて失敗させる。
- **中断**: Ctrl+C は `SetConsoleCtrlHandler` で受け、待ちの各所が合図を見て抜ける（終了コード 130）。その後の `Browser.close` と終了待ちは合図を見ないので、Chrome は正常終了する。
- **保存先の注入**: `AppPaths` を値として持ち回し、`main` だけが環境（`EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` による上書きを含む）から決める。テストは `target/tmp` 配下を根にする。
- **環境変数は入口だけが読む**: `EDICTORA_UPDATE_URL` は `main`（と更新用 exe の入口）が読み、値で渡す。
- **正規表現**: 設定の `sectionStart` / `sectionEnd` は Rust の `regex` で解釈する。先読み・後読み・後方参照は使えない（読み込み時にエラー）。どのパターンも線形時間で終わるので、時間による打ち切りは持たない。ログの publisherId 伏せ字も `regex` で行う。

## データフロー

1. **入力（正本）**: `%APPDATA%\Edictora\config.json`（`--config` で変更可）。`profileDir`・`chromePath`（任意）と `items`（`itemId`・`name`・`baseDir`・`locales`・`images`）。ロケールごとに `description`（`file`、任意で `sectionStart` / `sectionEnd`）と `homepageUrl` / `supportUrl` / `officialUrl` を書く。書いていない欄は読み出し・比較・書き換えの対象外。URL 欄はキーなし＝対象外、`""`＝空欄が正本。`locales` と `images` は少なくとも一方が要る（画像だけのアイテムも作れる）。`images` は書いたキーだけが対象で、`icon` / `smallTile` / `marquee`（1 枚のパス）・`globalScreenshots`（`{"dir"}` か `{"files"}`、1〜5 枚）・`localizedScreenshots`（ロケール → `{"dir"}` か `{"files"}`。キーは `zh_CN` と `zh-CN` を同じに扱う）。`dir` は直下の `screenshot-*.png` を名前の昇順で並べる。`icon` は 128x128 の 1 枚で、全言語共通。
2. **正本の読み取り**: 区間は、見出し行の次から終端見出しの手前までを取り、前後の空行と末尾の空白を除き、改行を `\n` に揃える。見出しが無ければエラー。
3. **Chrome 起動**: `login` 以外はヘッドレス。専用プロファイルの Chrome を最小の引数で起動し、ログイン状態を引き継ぐ。
4. **ダッシュボード操作**: アイテムの編集画面を開き、言語を切り替え、欄を読む。
5. **比較**: 正本と現在値を完全一致で比べ、差分を行単位で表示する（`diff`・`push` の確認）。画像は、ページから段ごとの `<img>`（alt と src）を集め、src を `=s0`（原寸の原本）にして取得し、**デコードした画素の SHA-256** を順序つきで正本と比べる（「掲載画像」参照）。
6. **書き込み（`push --yes`）**: 言語ごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」を行い、最後に全言語を読み直して照合する。画像は、その後に別の手順で行う（`--only text|images`・`--image-scope all|global|localized` で絞れる。「掲載画像」参照）。
7. **審査送信（`submit --item <id> --yes`）**: 確認ダイアログで送信する。

`update` は 1〜7 を通らず、配信元のフィードを見て新しい版をダウンロードし、Update.exe に差し替えを任せる（下の「配布・インストール・自動更新」）。

結果は標準出力、進捗は標準エラー。終了コードは 0（成功・差分なし）/ 1（実行時エラー）/ 2（引数・設定の誤り、Chrome は開かない）/ 3（差分あり）/ 4（書き込み中止）/ 5（ログインが必要）/ 130（中断）。

保存先: Chrome プロファイル（既定 `%LOCALAPPDATA%\Edictora.Data\chrome-profile`、`profileDir` / `--profile` で変更可）・ログ（`...\logs`）・失敗時の画面（`...\diagnostics`）は `%LOCALAPPDATA%\Edictora.Data`、設定は `%APPDATA%\Edictora`。いずれもリポジトリの外。`%LOCALAPPDATA%\Edictora` は Velopack のインストール先（アンインストールで消える）なので、データを置かない。

## 重要な不変条件

- **プロファイルは秘密情報で、ツールは削除しない。** 削除する経路を持たない。終了は `Browser.close`、応答しないときだけプロセスを止める。ログイン待ちの間はタブに接続しない。パスワードと確認コードはツールが扱わない。
- **publisherId はログイン後の URL から実行時に取り出し、コード・設定・ログに書かない。** ログと CLI のエラー出力は伏せ字処理を通す。
- **既定は読み取りのみ。** 書き換えは `push --yes`、審査送信は `submit --item <id> --yes` を明示したときだけ。下書き保存と審査送信は別の操作で、審査送信は 1 件ずつ。「審査の合格後に自動的に公開する」の状態は変えずに報告する。
- **書き込み可否は許可リスト方式**（`write_gate.rs`）。確認済みの状態（公開済み・下書き・拒否）以外、審査待ち・ロック中・判定不能なアイテムには書き込まない。審査の取り消しはしない。
- **保存前と保存後に正本と照合する。** 一致しなければ保存しない・エラーで止める。途中で失敗したら、そのアイテムの残りは行わない。再読込・遷移で beforeunload が出たら、「留まる」を選んで未保存の変更を捨てずに中止する。
- **画像の書き込みは審査送信に届かない。** 画像の手順は `ImagePage`（審査送信を持たない trait）だけを受け取る。画像まわりのクリックは、aria-label が「画像を削除 <alt>」の完全一致する削除ボタンと、削除の確認ダイアログの「削除」だけに限り、並べ替えには触れない（アイコンの削除も同じ削除ボタンで、alt は「ショップ アイコン」）。審査待ちなどでは最初の削除の前（`ensure_allowed`）と各保存の直前に止める。テストは、`submit_for_review` が panic する偽ページでテキストと画像の push 全体を通す。
- **画像の比較は画素で行い、比較不能を一致にも置換にもしない。** ダッシュボードは PNG を再エンコードして保存するので、ファイルのバイト列では比べない。原本を取得・デコードできなければ「比較不能」でそのアイテムを止める。
- **画像の削除は即座にダッシュボードへ反映される（保存を待たない）。** 途中で止まると、下書きから画像が欠けたまま残りうる。再実行で整える。
- **要素の特定が曖昧なら止める。** 候補がちょうど 1 つに絞れなければ、最初の 1 つを選ばず止める（別の欄を書き換える事故を防ぐ）。
- **Chrome を前面に出さない。** `Page.bringToFront` は使わず、操作の前に毎回 `Emulation.setFocusEmulationEnabled` を行う。
- **引数と設定の誤りは Chrome を開く前に終了コード 2 で止める。**
- **失敗時の画面保存はリポジトリの外**（`%LOCALAPPDATA%\Edictora.Data\diagnostics`）に置く。
- **検証していないパッケージは適用しない。** サイズとハッシュがフィードと一致したものだけを Update.exe に渡す。

## 採用済みの設計判断

### Chrome の起動

- 引数は `--user-data-dir`・`--remote-debugging-port=0`（`DevToolsActivePort` でポート取得）・`--no-first-run`・`--no-default-browser-check`、描画の間引き抑止 3 つ（`--disable-backgrounding-occluded-windows`・`--disable-renderer-backgrounding`・`--disable-background-timer-throttling`）、`--disable-features=CalculateNativeWinOcclusion`、ヘッドレス時だけ `--headless=new --window-size=1280,900`。Chrome の標準出力・標準エラーは捨てる。
- **自動化フラグ（`--enable-automation` など）と画面外配置は付けない。** 実画面で、これらを付けない普通の Chrome なら利用者が手動で Google にログインでき、`navigator.webdriver` も `false` だった。足すとログインが弾かれる恐れがあるので、変える場合は実画面でログインを再検証する。
- **ヘッドレスで動かす。** ウィンドウが裏に回る・画面外にあると描画が止まりクリックが応答しないが、`CalculateNativeWinOcclusion` の無効化と間引き抑止フラグがあれば前面に出さなくても動く。`--headless=new` は同じプロファイルのログイン状態をそのまま使え、実行中に前面のウィンドウが変わらなかった（画面外配置・最小化の起動は、起動時に Chrome が前面へ来た）。`HeadlessChrome` を含む UA は `Emulation.setUserAgentOverride` で `Chrome` に置き換える（置き換えない状態は未検証）。
- トレードオフ: ヘッドレスでは利用者が動きを見られない。`--show-browser` で表示できる。`login` は手動操作のため常に表示する。
- `--show-browser` は人が見るためのもので、AI エージェント・自動実行では使わない。2026-11-15、別の作業の AI が「言語の選択肢が覆われている」失敗を画面サイズのせいと推測して付けて試し、利用者の画面にウィンドウが出た。推測に頼らなくて済むよう、覆われたときのエラー文に中心にあった要素・要素の枠・画面の大きさを含め、`pull` / `diff` の読み出しに失敗したときも画面を `diagnostics` に保存して、保存先をエラー文（標準出力の結果）に出す。

### 表示言語（出力とダッシュボード画面）

2 つの言語は別物で、日本語の画面を英語の出力で使うこともある。

- **出力の言語**（`i18n.rs`）: CLI の進捗・結果・エラー、差分、ヘルプが対象で、日本語・英語・簡体字中国語・繁体字中国語・韓国語・スペイン語・フランス語・ドイツ語・ブラジル・ポルトガル語の 9 言語。環境変数 `EDICTORA_LANGUAGE`（旧名 `EDICTORA_LANG` も読む。優先は前者）→ `LC_ALL` → `LC_MESSAGES` → `LANG`（空・C・POSIX は除く）→ OS の表示言語（`GetUserDefaultUILanguage`）の順に決め、判定できなければ英語。タグは `zh-Hans`/`zh-CN`/`zh-SG`→zh-CN、`zh-Hant`/`zh-TW`/`zh-HK`/`zh-MO`→zh-TW、`pt*`→pt-BR、それ以外は主言語で照合する。決定はプロセスで 1 回（初回の使用時）。引数・設定に頼らないのは、フックや引数の誤り（終了コード 2）より前でも決められるようにするため。文言は言語ごとの JSON 辞書（`locales/<タグ>.json`、`include_str!`）に持ち、コードは安定したキーで `tr!` / `trs!` 参照する（プレースホルダー `{name}`、辞書に無ければ英語、英語にも無ければキー）。整合テスト（`tests/localization.rs`）が全言語のキー集合・プレースホルダーの一致と、参照キーの存在・未使用キーの無さを固める。AI が読む文言（MCP のツール説明・スキーマ・応答）は英語に統一し、`edictora mcp` はプロセスの言語を英語に固定する。ログの定型文は作者が調べるためのものなので日本語のまま（ログに埋め込むエラー文は表示言語のまま）。ページ内スクリプトのエラーは固定の印を返し、Rust 側で各言語の文にする。ダッシュボードの画面文言と設定の `locales` は UI 言語とは別。テストは `.cargo/config.toml` の `[env]` で `EDICTORA_LANGUAGE=ja` に固定する。README は英語を正本に 9 言語版を置く。
- **ダッシュボード画面の言語**（`dashboard/text.rs`）: `<html lang>` の主サブタグでページごとに 1 回だけ判定し、文言セット（JA / EN）を 1 つ選ぶ。要素ごとに「日本語か英語のどちらかに一致」で探さない（候補が倍になり取り違えの危険が増えるため）。lang が空なら日本語（確認済みの経路を lang 属性の有無で止めない。英語画面なら見つからずに止まる）。ja / en 以外は止める。
- **英語の文言は実画面で未確認**なので、安全側に倒す。英語の成功メッセージの判定は単語単位・大文字小文字無視で、失敗を表す語（"not" など）を含むものは成功と見なさない。英語では送信前に送信完了メッセージが出ていないかを確かめ、出ていれば審査送信を始めない。"Unpublished" / "Not published" は "Published" の部分一致で許可されてしまうので書き込みを止める。

### 入力操作

- クリックは scrollIntoView 後に矩形中心へ実際のマウス入力（`clickCount: 1`）を送り、中心が他の要素に覆われていたら押さない。
- 説明欄の書き換えはフォーカス → 全選択 → `Input.insertText`（空にするときは Delete キー）。ページ側は通常の入力として受け付け、文字数カウンタも更新される。DOM の `value` への直接代入は、フォームに変更として認識されない可能性があるので使わない。
- 画像のアップロードは、ページの `input[type=file]`（非表示のものがある）へ CDP の `DOM.setFileInputFiles` でファイルを渡す（`CdpPage::set_file_input_files`）。式を `returnByValue: false` で評価して objectId を得て、パスは CDP の引数で渡し（ページ内スクリプトには埋め込まない）、終わったら `Runtime.releaseObjectGroup` で解放する。
- 画像の削除ボタンはホバーで現れる（普段は 0x0・透明）。画像の上へマウスを動かしてから、現れたボタンを実際のマウス入力で押す。
- 待ちは固定の sleep ではなく、条件を一定間隔で確かめる（上限時間つき）。

### 掲載画像

- **対象**: ストア アイコン（128x128、1 枚。全言語共通。ダッシュボード上の名前は「ショップ アイコン」）・小タイル（440x280）・マーキー（1400x560）・全言語向けスクリーンショット（1280x800 か 640x400、1〜5 枚）・ローカライズ版スクリーンショット（言語ごと、同じ寸法・枚数）。プロモーション動画・並べ替えは対象外。v1 は PNG だけ（8 ビット・インターレースなし。小タイル・マーキー・スクリーンショットは 24 ビット・アルファなし。**アイコンだけは透過（RGBA）も可**）。アイコンは全言語共通なので `--image-scope global` に含め、処理順は小タイルより前。
- **正本の検証**（Chrome を開く前、終了コード 2）: 実在・PNG ヘッダー・8 ビット RGB（アイコンは RGB か RGBA）・寸法・同一セット内で画素が重複しない・枚数。画素のダイジェストもここで計算する。
- **比較は画素で行う。** 2026-11-15 に SearchClock で、アップロードした PNG はサーバーが再エンコードして保存し（`=s0` の原本はバイト列もサイズも変わる。例: 901,028 → 1,052,742 バイト）、画素は完全に同じことを確かめた。そのためファイルの SHA-256 では比べず、`images/png.rs` でデコードした画素（行ごとにフィルターを戻した RGB）に幅・高さを前置した SHA-256 で比べる（zlib の展開は自前。依存を増やさない）。ダッシュボード側が RGBA（すべて不透明）で返しても RGB に落として比べる。透過・インターレース・8 ビット以外は「比較不能」。**アイコンだけはアルファ込みのダイジェスト**（`png::pixel_digest_rgba`。幅・高さ・RGBA の画素列。完全に透明な画素は RGBA をすべて 0 に正規化して色を無視する。RGB は A=255 として扱う）で比べる（`images::digest_for_slot` が正本とダッシュボードの原本の両方でスロットごとに選ぶ）。アイコンは角丸などで透過画素を持つのが普通のため。
- **現在値の取得**: ページから段ごとの `<img>` の alt と src を集め、src の末尾のサイズ指定（`=w160-h100-rw` など）を `=s0` に置き換えて `net/winhttp.rs` で取得する（上限 16 MB、https の `*.googleusercontent.com` だけ、Cookie なし、リダイレクトなし）。Cookie なしで 200 が返ることを実画面で確認した。取得できなければ「比較不能」で、一致扱いにも常に置換にもしない。
- **スロットの特定**（`selectors::pick_image_slot`）: 各 `input[type=file]` について「その入力だけを含む最大の祖先」の文字列を集め、寸法の表記（`128x128`・`1280x800`・`440×280`・`1400×560`）と見出しの行（「全言語向けアセット」「ローカライズ版アセット」）で 1 つに絞る。アイコンの入力は見出しの行を持たず（枠の文字列は「ショップ アイコン / 128x128 ピクセル / ガイドライン…」だけ）、`128x128` の表記とローカライズ版の見出しを持たないことで選ぶ。全言語向けとローカライズ版のスクリーンショットは同じ寸法なので、見出しで分ける。1 つに絞れなければ止める。言語が 1 つのアイテムにはローカライズ版のスロットが無い。
- **空のスロット**: 画像を削除したスロットには、alt だけを持つ src が空の `<img>` が残る。実画像だけ（src が空でないもの）を数える。
- **削除**: 画像の上へマウスを動かして削除ボタン（aria-label「画像を削除 <alt>」）を出し、クリックする。確認ダイアログ「この画像を削除してもよろしいですか？この操作は元に戻せません。」（ボタン「キャンセル」「削除」）が出るので、文面を確かめて「削除」を押す。文面が違う・複数のダイアログなら押さずに止める。
- **アップロード**: 1 枚ずつ `DOM.setFileInputFiles`。枚数が 1 枚増え、alt が「スクリーンショット N」の並びに揃い、エラー表示（結果メッセージに失敗を表す語）が無いことを待つ（1 枚 60 秒まで）。
- **push の順序**: テキストの push（対象なら）→ 編集画面を開く → 書き込み可否 → 全言語向け（アイコン → 小タイル → マーキー → スクリーンショット。差分のあるスロットだけ全入れ替え）→ 書き込み可否 → 下書き保存 → 読み直して照合 → ロケールごと（切替 → 置換 → 書き込み可否 → 保存 → 読み直し → 切替 → 照合）→ 最後に読み直して全対象スロットを再照合。1 スロットの置換は「既存を末尾から 1 枚ずつ削除（毎回枚数が減るのを待つ）→ 正本を先頭から 1 枚ずつアップロード → 枚数を照合」。
- **ダッシュボードにあるロケールだけ**: ローカライズ版が対象で、そのロケールがダッシュボードに無い（言語が 1 つのアイテムを含む）なら、何も触らず全体を止めて `--image-scope global` か `--locale` を案内する。ローカライズ版を設定していないロケールは、現在 0 枚として扱う。
- **日本語の画面だけ**: 画像の書き込みは日本語の画面だけ許す（英語の文言は未確認）。読み出しは、スロットを特定できれば英語でも動く可能性があるが、未確認。

### 画面構成と欄の探し方

- アイテム一覧は `https://chrome.google.com/webstore/devconsole/<publisherId>`、編集画面は `.../<publisherId>/<itemId>/edit`。文言は `dashboard/text.rs` に集め、画面の言語ごとの文言セット（日本語は実画面で確認済み、英語は未確認の暫定値）を持つ（「表示言語」参照）。
- 「ストアの掲載情報」で編集できる欄: 説明（`textarea`、最大 16,000 文字、カウンタ「n/16,000」付き）、公式 URL・ホームページ URL・サポート URL（`input[type=text]`）。カテゴリ・プロモーション動画は対象外（アイコンを含む掲載画像は下の「掲載画像」）。「パッケージのタイトル／概要」は manifest 由来の読み取り専用。
- 編集中の言語は「言語」ラベルを持つ `[role=combobox]` で切り替える。選択肢は `[role=option]` で、表記は「英語 – en （デフォルト）」「日本語 – ja」の形。最初に見つかる `[role=combobox]` はパブリッシャーの選択欄なので、ラベルで絞り込む。
- **ラベルは「1 行の完全一致」で照合する。** ラベル候補には欄の親要素の文字列（説明文を含む）も入るため、部分一致だとカテゴリ欄の説明「すべての言語用」が「言語」に当たって誤判定する。改行で区切った 1 行が、空白を無視してラベルと完全一致するものだけを選ぶ。URL 欄も同じ。
- **選択肢の一覧は、閉じても要素が残る。** 閉じた `[role=listbox]` は枠が 0×0 になるだけで、中の `[role=option]` は表示扱いのまま残る。パブリッシャー名の選択肢は常に表示されている。そのため、選択肢は枠に大きさのある一覧の中のものだけ数え、開いたかは「言語 – code」形式の選択肢の出現で、閉じたかは言語欄の `aria-expanded` が `"false"` になったかで判断する。
- ボタンは「下書きとして保存する」「審査のため送信」「公開」。変更がなくても無効化されない。保存後は「アイテムが保存されました。」が出る。
- 「審査のため送信」を押すと確認ダイアログ（`[role=dialog]`）が出る。文面は「「<名前>」を審査のために送信しますか？」、チェックボックス「審査の合格後に「<名前>」を自動的に公開する」（既定 ON）、ボタン「キャンセル」「審査のため送信」。確定すると「アイテムを送信しました。」が出てステータスが「審査待ち」になる。
- 「ガイドやヒントが必要な場合は…」という常設の通知が `[aria-live]` 領域にあるので、結果メッセージの判定では除外する。

### 区間切り出し

- 切り出し結果は検証スクリプト（push.mjs）と同じにする。`crates/edictora/tests/fixtures/ReplaceTranslator` の SHA-256・文字数（ja=1492 / en=3172）のテストで固める。

### パッケージ内のアプリからの保存先リダイレクト（`redirect.rs`）

- 背景（実害）: Claude デスクトップアプリは MSIX パッケージで、その中の Bash ツールから起動したプロセスが `%APPDATA%` / `%LOCALAPPDATA%` の下に**本物にまだ無いフォルダー**を作ると、本物ではなくパッケージ専用領域 `%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalCache\{Roaming,Local}\...` に作られる。本物に既にあるフォルダーへの書き込みは本物に入る。両方にあるとパッケージ内のプロセスは専用領域を優先して読む。結果として `%APPDATA%\Edictora\config.json` が専用領域にだけできて、通常のターミナルからは「設定が無い」状態になった（2026-11-15）。
- 検出: 対象を開いて `GetFinalPathNameByHandleW`（kernel32 を FFI で直接。`FILE_FLAG_BACKUP_SEMANTICS`・アクセス権 0 で、何も作らず変えない）で最終パスを得る。仮想化は最終パスに現れる（Claude の Bash から `%APPDATA%\rtk` で実測し、`...\Packages\Claude_...\LocalCache\Roaming\rtk` が返ることを確認）。判定は文字列だけの純粋関数 `detect_redirect(要求, 最終)`: 2 つが（`\\?\`・大文字小文字・区切り・末尾の `\` を除いて）異なり、かつ最終が `...\AppData\Local\Packages\<name>\LocalCache\` の下であること。利用者が最初から専用領域のパスを指定した場合と、専用領域以外への付け替え（ジャンクションなど）は対象にしない。
- 読めたとき: 既定の設定ファイル（`--config` 未指定）と Chrome プロファイル（`profile_directory`）がリダイレクトされていれば、標準エラーへ警告を種類ごとに 1 プロセス 1 回出す（実際の保存先・通常のターミナルから見えないこと・本物の場所へ移す対処）。設定は `CliApp::run` の検証後（Chrome を開く前）、プロファイルはブラウザを開いた直後（login は作成直後に当たる）。MCP では標準エラーは結果の「Progress and errors」に入り、`edictora_list_items` は結果の本文に入る（標準出力の JSON-RPC は壊さない）。文言は 9 言語の辞書（`redirect.*`）で、MCP では英語。
- 見つからないとき: 既定の設定ファイルが無いとき、`AppPaths::packages_root`（既定 `%LOCALAPPDATA%\Packages`、環境変数 `EDICTORA_PACKAGES_ROOT` で差し替え。`EDICTORA_LOCAL_ROOT` / `EDICTORA_ROAMING_ROOT` を使う実行・テストでは `None` で探さない）の下の `*\LocalCache\Roaming\Edictora\config.json` を読み取りだけで探し、あればエラー文に候補のパスと移す先を添える。自動で読み込まない・移さない（利用者のデータで、どちらが正かは利用者が決める）。
- 起きにくくする（判断）: パッケージ内のプロセスから「本物へ確実に」フォルダーを作る手段は無い（子プロセスもパッケージの仮想化を受ける）ので、作成を回避する実装はしない。代わりに、**パッケージの外で動く経路**だけで本物の場所を先に用意する: Velopack の install / updated フックと、ログオン時の `edictora-updater.exe` が `ensure_data_folders` で `%APPDATA%\Edictora` と `%LOCALAPPDATA%\Edictora.Data` を作る（ベストエフォート。既にあれば何もしない。以後パッケージ内のアプリが書いても本物に入る）。パッケージ内から呼ばれて仮想化先へ作られた場合は、作成後に検出してログへ警告を残す。Edictora が新規作成する箇所（login のプロファイル作成・ログ）は、作成後の検出と警告にとどめる。config の雛形作成は無い。
- 未確認: 実際に Setup・ログオン時の更新用 exe で本物の場所にフォルダーができることの実機確認、パッケージ内から login して警告が出る実機の流れ（Chrome を起動する確認は行っていない）。

### 配布・インストール・自動更新（Velopack）

- **配布物**: Velopack（vpk 1.2.161、packId `Edictora`、チャンネル `win`）のパッケージ。主 exe は `edictora.exe`、同じパッケージに更新用の `edictora-updater.exe` を入れる。Setup で `%LOCALAPPDATA%\Edictora`（以下「根」）に入る（`Update.exe`・`current\`・`packages\`）。配信元は Cloudflare R2 の `https://edictora.kagayoi.com`。署名・パッケージ作成・配信の手順は [docs/release.md](docs/release.md)。
- **役割の分け方**: インストール・アンインストール・`current` の差し替えは Velopack の Setup.exe / Update.exe に任せる。Edictora は**更新の確認とダウンロードだけ**を自前で行う（`update/`）。velopack crate は ureq / rustls を引き込むので使わない。通信は `net/winhttp.rs`。
- **約束は vendored の Velopack 1.2.161 Rust SDK に合わせる**（Petarin の `desktop/src-tauri/vendor/velopack-1.2.161`）:
  - インストールの判定（`locator.rs`）: exe の親か、パスの最後の `\current\` より前に `Update.exe` があればそこが根。`<根>\Update.exe` と `<根>\current\sq.version` が無ければ「インストールされていない」。現在の版・id・チャンネル・mainExe は `sq.version`（nuspec XML）から読む。チャンネルが空なら `win`。
  - フィード（`sources/http.rs`・`manager.rs`）: `<配信元>/releases.{channel}.json`。`{"Assets":[{PackageId, Version, Type, FileName, SHA1, SHA256, Size, …}]}` でキーは大文字小文字を区別しない。Type が `Full` の最大の版が現在より新しければ更新（ダウングレードしない・差分パッケージは使わない）。問い合わせ文字列は `localVersion` と `id` だけ（SDK が付ける端末固有の `stagingId` は送らない）。
  - ダウンロード（`manager.rs` の `download_updates`）: `<packages>\.velopack_lock` を持ち、`<FileName の拡張子を .partial にしたもの>` に保存 → サイズと SHA256（無ければ SHA1）を検証 → `<packages>\<FileName>` へ改名 → 前からあった `*.nupkg`・`*.partial`・`*-delta.nupkg` を消す。検証に失敗したら適用しない。ハッシュは CNG（bcrypt.dll）で計算する。
  - 適用（`manager.rs` の `unsafe_apply_updates`）: `Update.exe apply --package <nupkg> --waitPid <自分> --silent --norestart --root <根> --packageDir <packages>` を `CREATE_NO_WINDOW` で起動し、起動した側はすぐに終了する。Update.exe が終了を待って差し替える。
  - フック（`app.rs`・`constants.rs`）: 引数が 2 つ以上で 1 つ目（小文字にして）が `--veloapp-install` / `--veloapp-updated` / `--veloapp-obsolete` / `--veloapp-uninstall` ならフック。処理後は必ず終了コード 0。install / uninstall は 30 秒、updated / obsolete は 15 秒で止められる。
- **SDK と違う点（意図的）**: ダウンロード済みのパッケージも検証し直す（SDK は検証せずに使う）。宣言の Size を上限にダウンロードを打ち切る。nupkg から `Squirrel.exe` を取り出して `Update.exe` を差し替える処理は持たない（zip を展開しないため。Update.exe はインストール時の版のまま）。起動時の自動適用（SDK の `VelopackApp::run` がする、ダウンロード済みの新しい版の適用）はしない（ダウンロードしたその場で適用を任せるため）。
- **フックで行うこと**（`install/hooks.rs`。`edictora.exe` と `edictora-updater.exe` の両方が引数の解釈より前に処理する）: install / updated で HKCU の `Run` に `"<根>\current\edictora-updater.exe"` を登録し、ユーザー環境変数 `Path` に `<根>\current` を足して `WM_SETTINGCHANGE` を通知する。uninstall で両方を取り除く（Run は現在の値が自分の値と一致するときだけ消す）。obsolete は何もしない。Portable 版（根に `.portable`）では install / updated でも登録しない（利用者が置いた場所をログオン時に起動させない。手動の `edictora update` は使える）。Run の同じ名前の値が別の場所の更新用 exe を指していれば上書きせず、警告をログへ残す（根のフォルダー名が同じ別のインストールを壊さない）。Run の値の名前（`<根のフォルダー名> Updater`）と PATH の要素は根から決めるので、packId を変えた検証用インストールと衝突しない。PATH は既存の型（REG_SZ / REG_EXPAND_SZ）を保ち、他の要素は変えない（比較のときだけ `%VAR%` を展開）。インストール先から起動されていなければレジストリに触れない。
- **初回起動の扱い**: Velopack に引数なしで起動された（`VELOPACK_FIRSTRUN` / `VELOPACK_RESTART` がある）`edictora.exe` は、既定の `diff`（Chrome を開く）を走らせずに終える。
- **更新の入口**: ログオン時の `edictora-updater.exe`（`windows_subsystem = "windows"`、結果はログ、失敗は 30 秒・90 秒の間隔で 2 回まで試し直す）と、手動の `edictora update`（設定を読まない・Chrome を開かない・結果は標準エラー、最新と更新の開始は 0、失敗と「別の更新処理が動いている」は 1、配信元の差し替えの誤りは 2、中断は 130）。両者は `<packages>\.edictora-update.lock` で排他する。
- **実行中の edictora との排他**: Update.exe の apply はインストール先配下のプロセスを止める可能性があるので、Chrome を開くコマンド（login / pull / diff / push / submit）はブラウザ操作の間だけ `<データの根>\edictora-running.lock` を共有で持つ（CLI 同士は同時に動ける）。更新側はダウンロードと検証の後、Update.exe を起動する直前にこれを排他で取り、取れなければ適用を後に回す（`AppInUse`。`update` は 1、更新用 exe は試し直す。次回はダウンロード済みのものを検証して使う）。取れたらプロセスの終了まで持つ。適用中に Chrome を開くコマンドを実行すると、Chrome を開かずに 1 で止まる。ロックファイルを開けないだけなら CLI は続行する（インストールされていない実行でも無害）。`pull` / `diff` / `push` / `submit` の中では更新を確かめない。インストールされていなければ、更新用 exe は何もせず、`edictora update` は失敗（1）にする。
- **配信元の差し替え**: 検証用に環境変数 `EDICTORA_UPDATE_URL`（読むのは各 `main` だけ）。平文の http は 127.0.0.1 だけ。CLI のテストは偽の更新処理を差し込み、`CliApp` の既定（`refuse_update`）では本物の配信元・Update.exe に届かない。
- **データの置き場を根と分ける**: 根はアンインストールでフォルダーごと消えるので、Chrome プロファイル・ログ・診断情報は `%LOCALAPPDATA%\Edictora.Data` に置く（旧 `%LOCALAPPDATA%\Edictora` からの自動移行はしない）。

### MCP サーバー（`edictora mcp`）

- 背景: 2026-11-15、別の作業の AI が掲載情報を直そうとして Edictora に気付かず、Toolport で見つかった期限切れの CWS メタデータ API（`cws_mcp__update_metadata`、v1.1、2026-10-15 で終了）を探していた。CLI を教えると今度は `--show-browser` を付けた。AI が見つけられ、画面を出さない形でしか呼べない入口が要る。
- 形: 同じ exe のサブコマンドにし、別の exe・crate は足さない。プロトコルは 2025-06-18（2025-03-26・2024-11-05 を要求されればその版で応える）。`initialize` / `ping` / `tools/list` / `tools/call` に応え、通知には応えない。一括送信（配列）は -32600。
- ツール: `edictora_list_items`（設定の対象を表示、ブラウザなし）、`edictora_diff` / `edictora_pull`（読み取り専用）、`edictora_push`（`confirm: true` で `--yes`）、`edictora_submit`（`item` と `confirm: true` が必須）。中身は CLI の引数の配列を作って `CliApp::run` を呼ぶ。CLI と同じ設定の検証・終了コード・伏せ字・書き込みの安全策・実行中ロック・失敗時の画面保存を通すため。
- 選ばせないもの: 画面表示（`--show-browser`）・`login`・`update`・`--profile` / `--chrome` / `--config`。スキーマは `additionalProperties: false` で、無い引数は黙って無視せずエラーにする。ログイン切れ（終了コード 5）は「利用者が端末で `edictora login`」と案内する。
- 結果: 先頭に終了コードとその意味、続けて標準出力（結果）と標準エラー（進捗・エラー）。0 と 3（差分あり）は `isError: false`、ほかは `true`。
- 並行: 作業スレッド 1 本で 1 件ずつ（同じプロファイルの Chrome を同時に開かない）。受信は続け、`notifications/cancelled` で実行中の呼び出しを中断する（CLI の Ctrl+C と同じ経路で Chrome を正常終了させる）。入力が閉じても、受け付け済みの呼び出しは最後まで実行してから終わる（書き込みを半端にしない・1 件送ってすぐ閉じる使い方でも結果を返す）。
- 更新との関係: サーバーはインストール先の exe で常駐する。Velopack の `Update.exe apply` はインストール先から動いている他のプロセスを強制終了してから差し替える（`force_stop_package`、Velopack のソースで確認済み。共通仕様 `APP-DISTRIBUTION.md`）。待機中でも止められるとクライアントのセッションから Edictora が消えるので、**`main` がサーバーの生存期間中ずっと実行中ロックを共有で持つ**（複数セッションの MCP 同士、呼び出しの中の `CliApp::run` とも共有で両立する）。サーバーが動いている間に更新が来たら、ダウンロードと検証だけ済ませて適用を見送る（`AppInUse`）。ログオン時の更新用 exe は 30 秒・90 秒の間隔で試し直し、それでも動いていれば次のログオンに回す（ダウンロード済みのものを検証して使う）。手動の `edictora update` は 1 で終わり、クライアントを閉じてから実行し直すよう案内する。更新の適用中（排他で持たれている）に起動されたサーバーは、標準出力に何も書かず終了コード 1 で終わる（起動しても Update.exe に止められるため）。残る隙: 更新プロセスが終わってから Update.exe が差し替え終えるまでの数秒に起動したサーバーは守れない（CLI と同じ）。ログオン直後にクライアントが自動で起動する環境では、適用が延び続けることがある。
- 登録: Setup 版はインストール・更新・アンインストールのフックで、Claude Code（`claude mcp add --scope user edictora -- <根>\current\edictora.exe mcp`）と Codex（`codex mcp add edictora -- …`）へ自動で登録・解除する（共通仕様に揃える。手動の登録はアンインストール後に壊れたパスの登録が残るため）。公式 CLI（PATH の `claude.exe` / `codex.exe`。`.cmd` のラッパーは使わない）を呼び、設定ファイルは直接書かない。CLI 1 回は 3 秒、フック全体にも締め切りを置く（install / uninstall は 30 秒、updated は 15 秒で Velopack に止められるため）。Run・PATH の登録を先に済ませる。既に自分を指す登録は触らない。登録済みの CLI は `Edictora.Data\setup-registration.json` に覚え、二度と呼ばない（利用者が外した登録を復活させない）。時間切れなどで終わらなかった CLI は、ログオン時の更新用 exe が更新の確認より前にやり直す（1 回 30 秒）。アンインストールでは、自分の exe を指す登録だけを外し、状態ファイルを消す。Portable 版は登録しない。Claude Desktop・Toolport などのゲートウェイは README の手順で手動登録する。
- 見つけやすさ: ツールの説明に英語と日本語の検索語（Chrome Web Store・store listing・description・掲載情報・説明文・審査）を入れ、テストで固定する。

## 実画面での確認状況

- 確認済み: ログインとプロファイル、`pull` / `diff`（ヘッドレス、Chrome が前面に出ないこと）、言語切替、説明欄の書き換えと保存・読み込み直し後の照合（フェーズ 0 の検証）、審査送信ダイアログの文面。
- 2026-11-15 に Rust 版で確認: 9 言語（ja / en と pt-BR など 7 言語）を持つアイテムで、`push`（説明欄だけ）による全言語の書き換え・保存・読み直しての照合と、`submit` での審査送信（API でも PENDING_REVIEW を確認）。ステータス「拒否」（審査で却下）のアイテムは説明欄が編集でき「下書きとして保存する」も押せたので、許可リストに足した。言語が多い一覧では、開くアニメーションの途中で下の方の選択肢（ja・pt-BR）の中心に別の要素が重なることがあり、クリックは覆われている間だけ測り直す（最大 8 回、合計約 2 秒。上限を過ぎて覆われていれば従来どおり止める）。
- 2026-11-16 にストア アイコンを実画面（Clean Reload、審査待ち。8 アイテムの `diff` は読み取りのみ）で確認: アイコン欄は `input[type=file]`（accept `.png,.jpg,.jpeg`）で、枠の文字列は「ショップ アイコン / 128x128 ピクセル / アイコンが画像に関するガイドラインに沿っているか確認してください」、画像の alt は「ショップ アイコン」（`=s128-rw` の URL を `=s0` にして取得でき、RGBA の PNG）、ボタンは aria-label「画像を削除 ショップ アイコン」と「画像アップロードのステータスを閉じる」（ホバーしても 0x0 のまま。審査待ちでは小タイルの削除ボタンも同じ）。`diff` は「差分あり」（現在のアイコンは旧デザイン。アルファが 14,007 画素違う）と、現在のアイコン自体を正本にしたときの「一致」を確認した。**`push`（削除 → アップロード → 保存 → 読み直し）は実行していない**: 確認した 8 アイテムがすべて「審査待ち」で、書き込み可否が（設計どおり）終了コード 4 で止めた。サーバーの再エンコード後に、アルファ込みの画素（半透明の色・完全に透明な画素の扱い）が一致するかは未確認。
- 2026-11-15 に掲載画像を実画面で確認（読み出しは SearchClock と、審査待ちの Vuora の読み取りのみ）:
  - 読み出し: 単一言語のアイテム（SearchClock）で 3 つのスロット（小タイル・マーキー・全言語向けスクリーンショット）を特定し、`=s0` の原本を Cookie なしの WinHTTP で取得できた（200）。複数言語のアイテム（Vuora、9 言語）で、全言語向けと言語ごとのローカライズ版（9 言語、言語切替つき）を特定して読めた。取得した原本はバックアップの原本とバイト列まで一致した。
  - 書き込み（SearchClock の 1 件、全言語向けのみ。審査送信はしていない）。**最終のコードで「置換 → 保存 → 読み直し → 照合 OK」を一度に通した実行は無い**（記録は事実どおりに残す）:
    - `--yes` なしの差分確認（ドライラン）の手順は省略し、差分の確認は `diff` で行った。
    - 1 回目の `push --yes`: 削除後の確認ダイアログを知らず、押さずに止まった（何も削除されていない）。
    - 2 回目: 確認ダイアログの「削除」を押して小タイルが削除されたが、削除後のスロットに src が空の `<img>` が残り、枚数が減ったと判定できず止まった。**この時点で公開済みアイテムの下書きから小タイルが欠けた**（削除は即時反映）。
    - 3 回目: 小タイル・マーキー・スクリーンショット 3 枚の削除 → アップロード → 下書き保存 → 読み直しまで通ったが、その時点の比較がバイト列の SHA-256 だったため保存後の照合が不一致で終了コード 1（5 枚とも再エンコードで別のバイト列）。画素の比較へ変えた後は `diff` が 3 スロットとも一致で、続けて実行した `push --yes` は「変更なし」（終了コード 0）で終わった。置換を伴う実行を、画素の比較のコードでやり直してはいない。
  - **削除は保存を待たず即座にサーバーへ反映される**（削除の確認だけ押して止めた後に読み直すと、スロットが空だった）。
  - **アップロードした PNG はサーバーが再エンコードする**（バイト列・サイズは変わり、画素は同じ）。そのため画素で比べる。
  - 規格外の画像をアップロードしたときのエラー表示（`selectors::image_error_messages` の語「失敗・エラー・無効・できません」は推測値）、アップロード中の画面の状態（`blob:` の仮表示があるか）、beforeunload の有無（画像の操作が即時反映のため、未保存の変更として扱われるかも未確認）は確かめていない。
  - **ローカライズ版スクリーンショットの書き込みは実画面で一度も実行していない**（読み出しだけ確認）。ロケールの切替完了は、説明欄の値が 2 回続けて同じになるまで待って判定する（`switch_locale`）が、Vuora のように 7 言語が同じ英語の説明を持つアイテムでは、説明欄が切替前後で変わらないため、画像の切替完了の合図にならない（置換や照合を、切替前の言語の画像に対して行う恐れがある。ローカライズ版の書き込みを実画面で試す前に、スロットの画像の変化など別の合図を足すか確かめる）。
  - 画素の比較で扱えない形式（JPEG・パレット・透過あり・16 ビット・インターレース）は「比較不能」で、そのスロットは置換されない。2026-11-15 のバックアップ（15 件・icon を除く 188 枚）を調べたところ、すべて PNG で、181 枚が 8 ビット RGB、7 枚（DarkBlueThemeX の 6 枚と ScreenGuard の 1 枚）は 8 ビット RGBA だが全画素が不透明で、RGB に落として比べられる。
- 未確認（`write_gate.rs` の許可リストはこれらを確かめて更新する）:
  - Ctrl+C での中断、beforeunload の扱い。
  - 審査待ちのアイテムの編集画面（編集がロックされるか）。
  - **「拒否」のアイテムを再提出した後のステータス表記。** 2026-11-15 の再提出では、送信直後に読んだ表記が「拒否」のままだった（別のアイテムは送信直後に「審査待ち」になった）。審査中でも「拒否」と出続けるなら、`LOCKED_STATUS_KEYWORDS` に当たらず許可リストを通ってしまう（残る歯止めは説明欄・保存ボタンの判定だけ）。再提出したアイテムを読み直して表記を確かめ、必要なら「拒否」の許可に条件を足す。
  - MCP（`edictora mcp`）経由の実画面操作、`notifications/cancelled` での中断、サーバーの待機中に自動更新が来たときの動き。
  - 言語が 1 つだけのアイテム。
  - URL 欄（公式・ホームページ・サポート）の書き換えと保存。
  - 15 件の全アイテムでの読み出し。
- 単体テストは 127.0.0.1 の偽配信元・専用ハイブ・偽の Update.exe 起動で行う。自動更新の結合は、検証用 packId `EdictoraTest` で実機確認済み: Install フックで Run（`EdictoraTest Updater`）と PATH（REG_EXPAND_SZ のまま）を登録、`edictora update` で 1.0.5 → 1.0.6（Obsolete / Updated フック）、`edictora-updater.exe` で 1.0.6 → 1.0.7、`Update.exe uninstall` で Uninstall フックが Run と PATH から自分の分だけを消し `Edictora.Data` は残る。未確認: 実署名での一連、R2 への実配信、本番の HTTPS（edictora.kagayoi.com）、本番 packId `Edictora` での Setup、Portable 版の一連、Setup 直後の初回起動の見え方（`--silent` の Setup は主 exe を起動しないらしい）、実行中ロックで適用を後に回す動作の実機確認。
- 英語表示のダッシュボード: 文言セット（`text.rs` の EN）・成功メッセージの判定・ステータスの語はすべて実画面で未確認の暫定値。確かめるまで英語の画面での `push` / `submit` は利用者の確認のうえで行う。
