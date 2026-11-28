# リリース

詳細は [docs/release.md](../release.md)。

通常の公開は、`/vava` が版を上げた main へ軽量タグ `v<version>` を付けて push すると、そのタグの push で CI の `release.yml`（Environment `release`、承認待ちなし）が自動で起動して行い、手元から重ねて配信しない。`workflow_dispatch` は同じタグを指定した再実行にだけ使う（`guard` ジョブと Environment の tag ポリシーが不正な指定を拒否する。GitHub Release は作らない）。`vpk pack`（Velopack 1.2.161、packId `Edictora`、チャンネル `win`）の中で Sigilune が 1 ファイルずつ署名し、nupkg・Setup・Portable とフィード `releases.win.json` を R2（`edictora-updates`、`https://edictora.kagayoi.com`）へ配信する。CI は `SIGILUNE_EMAIL` / `SIGILUNE_TOTP_URI` / `CLOUDFLARE_API_TOKEN` を Environment の Secrets から受け取る。

```powershell
gh workflow run release.yml --repo 1llum1n4t1s/Edictora --ref v<version>
```

ローカル実行は検証（`-SkipUpload`）に使う。事前に Sigilune のクラウドログインを確認し、失敗時は `sigilune status` で登録を確認するよう案内し、ログインを自動再試行しない。

```powershell
$sigilune = if ($env:SIGILUNE_EXE) { $env:SIGILUNE_EXE } else { Join-Path $env:USERPROFILE '.local\Sigilune\bin\sigilune.exe' }
& $sigilune login
if ($LASTEXITCODE -eq 0) { pwsh -NoProfile -File scripts/release-local.ps1 -SkipUpload } else { throw 'Sigilune でクラウドにログインできなかったよ。sigilune status で登録を確認してね' }
```
