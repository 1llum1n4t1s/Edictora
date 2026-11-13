# AGENTS.md — Edictora 開発ガイド（LLM / 開発者向け）

Chrome Web Store（CWS）の掲載情報（説明・公式 URL・ホームページ URL・サポート URL）を、各拡張機能リポジトリの正本ファイルから一括で読み出し・比較・更新する Windows アプリ。CLI（`edictora.exe`）と Avalonia の画面（`Edictora.UI.exe`）を持ち、ほかの人にも配る前提。
CWS API v2 に掲載情報の読み書きがなく、拡張機能からの CWS 操作は Chrome が拒否するため、専用プロファイルの Chrome を CDP で操作してデベロッパー ダッシュボードの画面を書き換える。
全体設計・2026-11-13 の実画面検証で確かめた画面構成と操作方法は [DESIGN.md](DESIGN.md) を正本とする。

## アーキテクチャ

```
src/
├─ Edictora.Core/     … UI 非依存の本体（CLI と UI の両方が使う）
│  ├─ Cdp/            … CdpClient（ClientWebSocket の CDP JSON-RPC）/ CdpSession（flatten sessionId）/ CdpPage（評価・マウス・キー・遷移）
│  ├─ Chrome/         … ChromeLocator（chrome.exe 検出）/ ChromeLauncher（専用プロファイルで起動 or 既存へ接続、Browser.close で終了）
│  ├─ Config/         … ConfigLoader（config.json の読込・検証）/ SectionExtractor（正本の区間切り出し）
│  ├─ Listing/        … ListingField / 正本と現在値のモデル / ListingComparer（完全一致の比較）/ TextDiff（行差分）
│  ├─ Dashboard/      … DashboardUrls / DashboardText（画面の文言）/ PageScripts（要素の収集）/ ElementSelectors（要素の選択）
│  │                    WriteGate（書き込み可否）/ DashboardItemPage（1 操作ずつの実装）/ DashboardBrowser（接続とログイン確認）
│  ├─ Services/       … ListingService（pull・diff・push・submit の手順と中止判断）/ DashboardSessionManager（UI 用の使い回し）
│  └─ Logging/        … LoggerService（SuperLightLogger ラッパー、%LOCALAPPDATA%\Edictora\logs。publisherId を伏せる）
├─ Edictora.Cli/      … edictora.exe（login / pull / diff / push / submit。外部の引数パーサなし、NativeAOT 可）
├─ Edictora.UI/       … Avalonia 12 + FluentTheme + CommunityToolkit.Mvvm + Microsoft.Extensions.DependencyInjection
└─ Edictora.Tests/    … MSTest（命名: Xxx.test.cs）。ブラウザ不要な部分だけを検証する
scripts/              … リリース用 PowerShell（release-local.ps1 / Sigilune.Common.ps1 / R2 配信）
tests/R2Transfer/     … R2 転送スクリプトの検証（CI の ci.yml が Run.ps1 を実行）
.github/workflows/    … ci.yml（ビルド・テスト）/ codeql.yml / release.yml（署名付きリリース）
```

## 設計上の重要な前提（このとおりに実装する）

- **手順は Core の `ListingService` に置き、CLI と UI は呼ぶだけにする。** 画面操作は `IListingPage` 越しに行い、手順はテスト用の偽ページでブラウザなしに検証する。
- **Chrome の起動引数は最小限**（`--user-data-dir`・`--remote-debugging-port=0`・`--no-first-run`・`--no-default-browser-check` と描画の間引き抑止 3 つ）。`--enable-automation` などの自動化フラグや画面外配置は付けない（Google ログインを弾かれる恐れ）。変える場合は実画面でログインを再検証する。
- **プロファイルは秘密情報で、ツールは絶対に削除しない。** 終了は `Browser.close`、応答しないときだけプロセスを止める。ログイン待ちの間はタブに接続しない。
- **publisherId はログイン後の URL から実行時に取り出し、コード・設定・ログに書かない。** ログと CLI のエラー出力は `LoggerService.Redact` を通す。
- **要素はラベル・役割・文言で探し、クラス名に頼らない。** 文言は `DashboardText` に集める。候補がちょうど 1 つに絞れなければ止める（最初の 1 つを選ばない）。ダッシュボードは日本語表示が前提。
- **操作の前に毎回 `Page.bringToFront` と `Emulation.setFocusEmulationEnabled`。** クリックは scrollIntoView 後に矩形中心へ実際のマウス入力（`clickCount: 1`）、覆われていたら押さない。書き換えはフォーカス → 全選択 → `Input.insertText`。DOM の `value` 直接代入はしない。待ちは固定 sleep でなく上限つきの条件確認。
- **ページ内スクリプトに利用者の入力値を埋め込まない。** 書き込む文字列は CDP の `Input.insertText` で渡す。
- **書き込みの安全策**: `WriteGate` は確認済みの状態（許可リスト）以外を止める。審査の取り消しはしない。言語ごとに「切替 → 書換 → 全欄照合 → 保存 → 再読込 → 照合」、最後に全言語を読み直す。途中で失敗したら、そのアイテムの残りは行わない。beforeunload が出たら変更を捨てずに中止する。
- **審査送信は 1 件ずつ明示**（CLI は `submit --item <id> --yes`）。「自動的に公開する」チェックは変えずに報告する。
- **区間切り出しは検証スクリプト（push.mjs）と完全に同じ結果にする。** 見出しが無ければ例外。`RegexOptions.ECMAScript` は使わない（全角空白が `\s` から外れる）。`Fixtures/ReplaceTranslator` の SHA-256 テストを壊さない。
- **設定に書いていない欄は、読み出し・比較・書き換えの対象外。** 未知のキーはエラー。URL 欄はキーなし＝対象外、`""`＝空欄が正本。
- 失敗時の画面保存は `%LOCALAPPDATA%\Edictora\diagnostics` に置き、リポジトリには置かない。

## 保存先

- 設定（正本の対応づけ）: `%APPDATA%\Edictora\config.json`（CLI `--config` で変更可）
- 画面の設定: `%APPDATA%\Edictora\settings.json`（SettingsService、変更即保存）
- Chrome プロファイル: `%LOCALAPPDATA%\Edictora\chrome-profile`（設定の `profileDir` / CLI `--profile` で変更可）
- ログ: `%LOCALAPPDATA%\Edictora\logs`、失敗時の画面: `%LOCALAPPDATA%\Edictora\diagnostics`

## コーディング規約

- UI 文言・コメント・コミットメッセージは **日本語**。コメントは目的・設計意図・制約を多めに書く
- 非同期メソッドは `Async` サフィックス + `CancellationToken` 伝播、ライブラリ層は `ConfigureAwait(false)`
- ViewModel は CommunityToolkit.Mvvm の **partial property 構文**（`[ObservableProperty] public partial T Prop { get; set; }`）+ `[RelayCommand]`
- ダイアログ表示など View 依存処理は **デリゲート注入**で VM から分離する（`ConfirmAsync` / `PickConfigFileAsync`）
- JSON はソース生成（`EdictoraJsonContext` / `EdictoraUiJsonContext`）で NativeAOT に対応する
- バージョンは `Directory.Build.props` の `<Version>` に一元化。**更新は /vava 指示時のみ**

## ビルド / テスト

```powershell
dotnet build Edictora.slnx -c Release
dotnet test Edictora.slnx -c Release
dotnet publish src/Edictora.Cli/Edictora.Cli.csproj -c Release -r win-x64   # NativeAOT の edictora.exe
```

- 実ダッシュボードに接続するテストは書かない（実画面の確認は手動で行う）
- lockfile 運用（`RestorePackagesWithLockFile`）: CI では `RestoreLockedMode` が自動で有効化される。`dotnet publish -r win-x64` の後に lockfile へ RID が入って `--locked-mode` が失敗したら、`dotnet restore Edictora.slnx --force-evaluate` で作り直す

## 既知の制約 / 今後の課題

- ダッシュボードの英語表示は未対応（文言で要素を探すため）
- 未確認の画面状態: 審査待ちアイテムの編集画面、URL 欄の書き換えと保存、言語が 1 つだけ / ja・en 以外のアイテム。`WriteGate` の許可リスト（公開済み・下書き）は実画面の表記で確かめて更新する
- 配布先（R2 bucket・公開ドメイン）は未確定。`scripts/release-local.ps1` の `$Bucket` / `$BaseUrl` が空のあいだは、配信を伴う実行は「配布先が未設定」で止まる
- Velopack の起動フックと Certum SimplySign のクラウド鍵（Sigilune）によるコード署名リリースを導入済み。`scripts/release-local.ps1` が `win-x64` の署名付き Setup.exe / Portable ZIP（UI と CLI を同梱）を生成し、通常は GitHub Actions の `.github/workflows/release.yml` から実行する。自動更新の配信先は未設定

## リリース

詳細は [docs/release.md](docs/release.md)。通常の公開は、`/vava` で版を上げて push した後に CI の `release.yml`（手動起動・Environment `release` の承認）で行い、手元から重ねて配信しない。署名は Sigilune（`scripts/Sigilune.Common.ps1`）で、CI は `SIGILUNE_EMAIL` / `SIGILUNE_TOTP_URI` / `CLOUDFLARE_API_TOKEN` を Environment の Secrets から受け取る。配布先が決まるまでは `skip_upload=true` での確認だけが通る。

```powershell
gh workflow run release.yml --repo 1llum1n4t1s/Edictora --ref main
```

ローカル実行は検証（`-SkipUpload`）に使う。事前に Sigilune のクラウドログインを確認し、失敗時は `sigilune status` で登録を確認するよう案内し、ログインを自動再試行しない。

```powershell
$sigilune = if ($env:SIGILUNE_EXE) { $env:SIGILUNE_EXE } else { Join-Path $env:USERPROFILE '.local\Sigilune\bin\sigilune.exe' }
& $sigilune login
if ($LASTEXITCODE -eq 0) { pwsh -NoProfile -File scripts/release-local.ps1 -SkipUpload } else { throw 'Sigilune でクラウドにログインできなかったよ。sigilune status で登録を確認してね' }
```
