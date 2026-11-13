# 公開手順（Velopack + Cloudflare R2）

EdictoraのWindows配布物（画面版 `Edictora.UI.exe` とCLI `edictora.exe` を同じパッケージに入れたもの）は、GitHub Actions（`.github/workflows/release.yml`）上でSigilune（Certum SimplySignのクラウド鍵）により署名し、Cloudflare R2から配信する予定です。

> [!IMPORTANT]
> **配布先は未確定です。** R2 bucket・公開ドメイン・ランディングページはまだ作っていません（外部リソースの作成は別途決定）。
> `scripts/release-local.ps1` の `$Bucket` / `$BaseUrl` が空のあいだは、配信を伴う実行は「配布先が未設定」で止まります。
> build・package・署名の確認（`-SkipUpload`）だけは実行できます。

## 配信構成

| 用途 | 値 |
|---|---|
| R2 bucket | 未定（`release-local.ps1` の `$Bucket`） |
| 公開URL | 未定（`release-local.ps1` の `$BaseUrl`） |
| Velopack channel | `win-x64` |
| package ID | `Edictora` |
| 起動する実行ファイル | `Edictora.UI.exe`（同じフォルダに `edictora.exe`） |

## 外部側の準備

配布先が決まったら、次を用意してから `release-local.ps1` の `$Bucket` / `$BaseUrl` を設定します。これらはリポジトリの設定だけでは作成されません。

1. R2 bucketを作成する。
2. bucketのCustom Domainに公開ドメインを接続する（DNS解決できること）。
3. リリース用の `cloudflare.api_token`（ローカル）と Environment `release` の `CLOUDFLARE_API_TOKEN`（CI）へR2編集、zone参照、キャッシュパージに必要な最小権限を付与する。
4. Environment `release` に署名用の `SIGILUNE_EMAIL` / `SIGILUNE_TOTP_URI` を登録する（承認者とブランチ制限を付ける）。

## アプリのリリース

バージョン更新は `/vava` でのみ行います。通常のリリースは、push 後に GitHub Actions の Release ワークフローを手動起動し、Environment `release` の承認後に `scripts/release-local.ps1` が実行されます。

```powershell
gh workflow run release.yml --repo 1llum1n4t1s/Edictora --ref main
# 公開せずに確認: -f skip_upload=true / 旧 nupkg の削除も行う: -f cleanup=true
```

配布先が未設定のあいだは、`skip_upload=true` 以外の実行はスクリプトの冒頭で止まります。

ローカルでも同じスクリプトを実行できます（Sigilune を `sigilune setup` で登録済みであること）。公開せず、build・package・署名だけを確認する場合:

```powershell
pwsh scripts/release-local.ps1 -SkipUpload
```

スクリプトは次の順序を守ります。

1. 配布先の設定を確認（未設定かつ配信を伴う実行なら、何もせずに止まる）
2. Sigilune のクラウドログイン、R2 bucket、zone、DNS、認証権限をプリフライト確認
3. `win-x64` で UI と CLI を locked restore・publish（同じフォルダへ）
4. Velopack packageを生成し、Setup、nupkg内・Portable内の `Edictora.UI.exe` と `edictora.exe` の署名を検証
5. packageと固定成果物をR2へupload
6. `releases.win-x64.json`を最後にupload
7. 固定URLのCloudflareキャッシュをpurge

manifest外の古いnupkgは通常リリースでは削除しません。全channelのmanifestを生成したうえで削除を明示する場合だけ、次を使います。

```powershell
pwsh scripts/release-local.ps1 -Cleanup
```

## バージョン付きSetup

リリーススクリプトは固定名Setupに加え、内容が変わらないバージョン付きの署名済みコピー（`Edictora-<VERSION>-win-x64-Setup.exe`）を生成・保持します。Microsoft Storeには未申請ですが、MSI/EXE申請で必要になる形を残しています。

## 自動更新

現行のEdictoraには、アプリ内の更新チェックの実行経路がありません。将来自動更新を実装する場合は、配布先が決まってから、配信元を固定した `SimpleWebSource` と更新経路のテストを別途追加します。
