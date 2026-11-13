# 公開手順（Portable ZIP + Cloudflare R2）

EdictoraのWindows配布物は、Rust版CLI `edictora.exe` を `LICENSE`・`README.md`・`PRIVACY_POLICY.md` と一緒にまとめたPortable ZIPです。GitHub Actions（`.github/workflows/release.yml`）上で `cargo build --release --locked` でビルドし、Sigilune（Certum SimplySignのクラウド鍵）で署名して、Cloudflare R2から配信する予定です。インストーラーと自動更新はありません。

> [!IMPORTANT]
> **配布先は未確定です。** R2 bucket・公開ドメイン・ランディングページはまだ作っていません（外部リソースの作成は別途決定）。
> `scripts/release-local.ps1` の `$Bucket` / `$BaseUrl` が空のあいだは、配信を伴う実行は「配布先が未設定」で止まります。
> ビルド・署名・ZIP化の確認（`-SkipUpload`）だけは実行できます。

## 配信構成

| 用途 | 値 |
|---|---|
| R2 bucket | 未定（`release-local.ps1` の `$Bucket`） |
| 公開URL | 未定（`release-local.ps1` の `$BaseUrl`） |
| 対象 | `win-x64`（`x86_64-pc-windows-msvc`） |
| 版付きZIP | `Edictora-<VERSION>-win-x64.zip`（不変。`immutable` で配信） |
| 固定名ZIP | `Edictora-win-x64.zip`（最新版のコピー。毎回キャッシュをpurge） |
| ZIPの中身 | `edictora.exe`（署名済み）、`LICENSE`、`README.md`、`PRIVACY_POLICY.md` |

版は `Cargo.toml` の `[workspace.package] version` を `cargo metadata` で読みます。版の更新は `/vava` でのみ行います。

## 外部側の準備

配布先が決まったら、次を用意してから `release-local.ps1` の `$Bucket` / `$BaseUrl` を設定します。これらはリポジトリの設定だけでは作成されません。

1. R2 bucketを作成する。
2. bucketのCustom Domainに公開ドメインを接続する（DNS解決できること）。
3. リリース用の `cloudflare.api_token`（ローカル）と Environment `release` の `CLOUDFLARE_API_TOKEN`（CI）へR2編集、zone参照、キャッシュパージに必要な最小権限を付与する。
4. Environment `release` に署名用の `SIGILUNE_EMAIL` / `SIGILUNE_TOTP_URI` を登録する（承認者とブランチ制限を付ける）。

## アプリのリリース

通常のリリースは、`/vava` で版を上げて push した後に GitHub Actions の Release ワークフローを手動起動し、Environment `release` の承認後に `scripts/release-local.ps1` が実行されます。

```powershell
gh workflow run release.yml --repo 1llum1n4t1s/Edictora --ref main
# 公開せずに確認: -f skip_upload=true / 古い版の削除も行う: -f cleanup=true
```

配布先が未設定のあいだは、`skip_upload=true` 以外の実行はスクリプトの冒頭（Sigiluneのログインやビルドより前）で止まります。

ローカルでも同じスクリプトを実行できます。前提は、Sigiluneを `sigilune setup` で登録済みであることと、`rust-toolchain.toml` の toolchain と MSVC リンカー（Visual Studio C++ Build Tools）が使えることです。公開せず、ビルド・署名・ZIP化だけを確認する場合:

```powershell
$sigilune = if ($env:SIGILUNE_EXE) { $env:SIGILUNE_EXE } else { Join-Path $env:USERPROFILE '.local\Sigilune\bin\sigilune.exe' }
& $sigilune login
if ($LASTEXITCODE -eq 0) { pwsh -NoProfile -File scripts/release-local.ps1 -SkipUpload } else { throw 'Sigilune でクラウドにログインできませんでした。sigilune status で登録を確認してください' }
```

ログインに失敗しても自動で再試行しません。`sigilune status` で登録を確認してください。

スクリプトは次の順序を守ります。

1. 配布先の設定を確認（未設定かつ配信を伴う実行なら、何も作らず・何も送らずに止まる）
2. `cargo metadata` で版を読み、`rustc` の host が `x86_64-pc-windows-msvc` であることと同梱文書の有無を確認
3. Sigilune のクラウドログイン、R2 bucket、zone、DNS、認証権限をプリフライト確認
4. `local-release/` を作り直し、`cargo build --release --locked` で `edictora.exe` をビルド
5. `edictora.exe` を `local-release/publish-win-x64/` に写して Sigilune で署名し、署名者CNとタイムスタンプを検証
6. 署名済み exe と文書をZIPにまとめ、ZIP内の `edictora.exe` の署名を再検証（成果物は `local-release/artifacts/`）
7. 版付きZIPを先に、固定名ZIPを最後にR2へupload
8. 固定名ZIPのCloudflareキャッシュをpurge

R2上の古い版付きZIPは通常リリースでは削除しません。削除を明示する場合だけ、次を使います。直近2版（今回の版を含む）と固定名ファイルを残し、それより古い版付きファイルを消します。今回の版が保持対象に入らない場合は何も消さずに止まります。

```powershell
pwsh scripts/release-local.ps1 -Cleanup
```

## page-scripts の生成物

ダッシュボードのページ内で評価するJavaScriptは `page-scripts/`（Bun + TypeScript）で作り、生成物 `page-scripts/dist/*.js` をコミットして Rust が `include_str!` で埋め込みます。リリースのビルドにBunは要りません。生成物が `src/` と一致しているかはCI（`bun run check`）で確かめるため、リリース前にCIが通っていることを確認してください。

## 自動更新

Edictoraには、アプリ内の更新チェックの実行経路がありません。新しい版は配布ページから版付きZIPまたは固定名ZIPを取得して置き換えます。
