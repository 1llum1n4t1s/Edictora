# 公開手順（Velopack + Cloudflare R2）

EdictoraのWindows配布物は、Rust版CLI `edictora.exe` と自動更新用の `edictora-updater.exe`（コンソールを出さない別bin）を、`LICENSE`・`README.md`・`PRIVACY_POLICY.md` と一緒に [Velopack](https://velopack.io/) でパッケージにしたものです。GitHub Actions（`.github/workflows/release.yml`）上で `cargo build --release --locked` でビルドし、`vpk pack` の中でSigilune（Certum SimplySignのクラウド鍵）が1ファイルずつ署名して、Cloudflare R2から配信します。自動更新はRust側（`edictora-updater.exe`）がこの配信先のフィード `releases.win.json` を読んで行います。

## 配信構成

| 用途 | 値 |
|---|---|
| R2 bucket | `edictora-updates`（`release-local.ps1` の `$Bucket`） |
| 公開URL | `https://edictora.kagayoi.com`（`release-local.ps1` の `$BaseUrl`。zone は `kagayoi.com`） |
| 対象 | `win-x64`（`x86_64-pc-windows-msvc`） |
| Velopack | vpk `1.2.161`、packId `Edictora`、channel `win`、mainExe `edictora.exe`、packAuthors `Kagayoi`、ショートカットなし（`--shortcuts None`） |
| 自動更新の本体 | `Edictora-<VERSION>-full.nupkg`（不変。`immutable` で配信） |
| インストーラー | `Edictora-win-Setup.exe`（固定名。毎回キャッシュをpurge）、`Edictora-<VERSION>-win-Setup.exe`（版付き、不変） |
| Portable | `Edictora-win-Portable.zip`（固定名。毎回キャッシュをpurge）、`Edictora-<VERSION>-win-Portable.zip`（版付き、不変） |
| フィード | `releases.win.json`（クライアントが読む）、`assets.win.json`、`RELEASES`（旧形式）。すべて固定名で毎回purge |
| パッケージの中身 | `edictora.exe`・`edictora-updater.exe`（署名済み）、`LICENSE`、`README.md`、`PRIVACY_POLICY.md`。Velopackが `Update.exe`（nupkgでは `Squirrel.exe`）と実行スタブを加える |

インストール先は `%LOCALAPPDATA%\Edictora`（packIdで決まる。一度配布したらpackIdは変えない）で、アプリのデータは `%LOCALAPPDATA%\Edictora.Data` に分けて置きます。

版は `Cargo.toml` の `[workspace.package] version` を `cargo metadata` で読みます。版の更新は `/vava` でのみ行います。

## 外部側の準備

作成済み:

1. R2 bucket `edictora-updates` と、そのCustom Domain `edictora.kagayoi.com`（作成時点でSSL証明書は初期化中。初回配信の前に `https://edictora.kagayoi.com/` へ接続できることを確かめる）。

初回配信の前に確認するもの:

2. リリース用の `cloudflare.api_token`（ローカル）と Environment `release` の `CLOUDFLARE_API_TOKEN`（CI）が、`edictora-updates` のR2編集、zone `kagayoi.com` の参照とキャッシュパージに必要な最小権限を持つこと。
3. Environment `release` に署名用の `SIGILUNE_EMAIL` / `SIGILUNE_TOTP_URI` があること（ブランチ制限だけを付け、承認者は付けない。`/vava` を離席中も最後まで進めるため）。

## アプリのリリース

通常のリリースは、`/vava` で版を上げて push した後に GitHub Actions の Release ワークフローを手動起動すると、承認待ちなしで `scripts/release-local.ps1` が実行されます（Secrets は Environment `release` から渡します。workflow が vpk 1.2.161 を導入します）。

```powershell
gh workflow run release.yml --repo 1llum1n4t1s/Edictora --ref main
# 公開せずに確認: -f skip_upload=true / 古い版の削除も行う: -f cleanup=true
```

ローカルでも同じスクリプトを実行できます。前提は、Sigiluneを `sigilune setup` で登録済みであることと、`rust-toolchain.toml` の toolchain と MSVC リンカー（Visual Studio C++ Build Tools）、`dotnet`（vpk を dotnet tool として使うため）が使えることです。ローカルの vpk の版が違えば、スクリプトが 1.2.161 に入れ替えます。公開せず、ビルド・パッケージ作成・署名だけを確認する場合:

```powershell
$sigilune = if ($env:SIGILUNE_EXE) { $env:SIGILUNE_EXE } else { Join-Path $env:USERPROFILE '.local\Sigilune\bin\sigilune.exe' }
& $sigilune login
if ($LASTEXITCODE -eq 0) { pwsh -NoProfile -File scripts/release-local.ps1 -SkipUpload } else { throw 'Sigilune でクラウドにログインできませんでした。sigilune status で登録を確認してください' }
```

ログインに失敗しても自動で再試行しません。`sigilune status` で登録を確認してください。

スクリプトは次の順序を守ります。

1. 配布先の設定を確認（空に戻した場合、配信を伴う実行は何も作らず・何も送らずに止まる）
2. `cargo metadata` で版と、bin `edictora` / `edictora-updater` を持つcrateを確認（どちらかが無ければ止まる）。`rustc` の host が `x86_64-pc-windows-msvc` であること、同梱文書の有無、vpk 1.2.161 を確認
3. Sigilune のクラウドログイン、R2 bucket、zone、DNS、認証権限をプリフライト確認
4. `local-release/` を作り直し、`cargo build --release --locked` で両方のexeをビルドして `local-release/payload-win-x64/` に文書と一緒に集める
5. `vpk pack` でパッケージを作る。署名は `--signTemplate`（`sigilune sign --replace {{file}}`）で1ファイルずつ行い、vpk の一時物は `local-release/temp/` に閉じる（`--signParallel 1` も付けるが、単一ファイル用テンプレートでは vpk が無視し、もともと1ファイルずつ呼ぶ）
6. 固定名の Setup.exe と Portable.zip の版付きコピーを作り、`releases.win.json` が今回の版の full nupkg を指すことを確認
7. Setup.exe（固定名・版付き）と、nupkg・Portable.zip の中の全exe（`edictora.exe`・`edictora-updater.exe`・`Update.exe`／`Squirrel.exe`・実行スタブ）のAuthenticode署名を、署名者CNとタイムスタンプまで検証（成果物は `local-release/artifacts/`）
8. R2へupload: nupkg → 版付き Setup / Portable → 固定名 Setup / Portable → `assets.win.json` → `RELEASES` → `releases.win.json`（フィードを最後に置き換え、未uploadのnupkgをクライアントが取りに行く時間を作らない）
9. 固定名ファイルとフィードのCloudflareキャッシュをpurge

版付きファイルとnupkgは `immutable` でuploadするため、R2にある同名のobjectは上書きしません。同じ版を作り直すと署名のタイムスタンプで中身が変わり、upload後の照合で止まります。途中で失敗した版を出し直すときは、版を上げるか、その版のobjectを手動で消してから実行してください（フィードは最後にuploadするので、途中で止まっても利用者には前の版が見えたままです）。

R2上の古い版付きファイルは通常リリースでは削除しません。削除を明示する場合だけ、次を使います。直近2版（今回の版を含む）と固定名ファイル、今回の `releases.win.json` が参照するファイルを残し、それより古い版付きファイル（nupkg・Setup・Portable）を消します。今回の版が保持対象に入らない場合は何も消さずに止まります。

```powershell
pwsh scripts/release-local.ps1 -Cleanup
```

## page-scripts の生成物

ダッシュボードのページ内で評価するJavaScriptは `page-scripts/`（Bun + TypeScript）で作り、生成物 `page-scripts/dist/*.js` をコミットして Rust が `include_str!` で埋め込みます。リリースのビルドにBunは要りません。生成物が `src/` と一致しているかはCI（`bun run check`）で確かめるため、リリース前にCIが通っていることを確認してください。

## 自動更新

インストール版（Setup.exe）とPortable版は、Velopackの `Update.exe` を同梱します。更新の確認先は `https://edictora.kagayoi.com/releases.win.json` です。いつ確認し、どう適用するか（ログオン時の `edictora-updater.exe` の動作など）はRust側の実装で決まるため、ここでは配信側の約束だけを書きます。フィードは配信のたびにキャッシュをpurgeし、`no-cache` で配るため、upload直後から新しい版が見えます。
