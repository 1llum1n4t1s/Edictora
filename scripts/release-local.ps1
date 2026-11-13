# release-local.ps1 — 署名付き Portable ZIP リリース（Rust 版 edictora.exe）
#
# 流れ:
#   1. 配布先 (下の $Bucket / $BaseUrl) を確認する。未設定かつ配信を伴う実行なら、何も作らず・何も送らずに止まる
#   2. cargo metadata から版を読み、Sigilune のクラウドログインと R2 / zone / DNS をプリフライト確認する
#   3. cargo build --release --locked で edictora.exe を作る (page-scripts/dist の JS は include_str! で埋め込み済み。Bun は要らない)
#   4. exe を作業フォルダーへ写し、Sigilune で署名して検証する (署名者 CN とタイムスタンプも照合)
#   5. 署名済み exe と LICENSE・README などを Portable ZIP にまとめ、ZIP 内の exe の署名を再検証する
#   6. 版付き ZIP を先に、固定名 ZIP を後に R2 へ upload し、固定 URL の Cloudflare キャッシュを purge する
#
# 署名は Sigilune (Certum SimplySign のクラウド鍵を signtool /dlib で使う CLI) で行う。
# 通常のリリースは GitHub Actions の .github/workflows/release.yml (手動起動・Environment "release") が
# 本スクリプトを実行する。ローカル実行も同じ手順で動き、検証や再実行に使う。
#
# 前提:
#   - Sigilune を使えること (scripts/Sigilune.Common.ps1 参照)。
#     ローカルは `sigilune setup` で Certum アカウントを登録済み、CI は SIGILUNE_EMAIL / SIGILUNE_TOTP_URI を渡す
#   - Cargo.toml の [workspace.package] version が配布したいバージョンになっていること (更新は /vava のときだけ)
#   - rust-toolchain.toml の toolchain (rustup が自動で使う) と MSVC リンカーが使えること
#   - 配布先 (下の $Bucket / $BaseUrl) が設定済みで、Cloudflare 側にその bucket と公開ドメインが準備済みであること。
#     Edictora の配布先は未確定のため、現在は空。公開を伴う実行は「配布先が未設定」で止まる (-SkipUpload は可)
#   - 環境変数 CLOUDFLARE_API_TOKEN (CI)、なければ D:\Dev\Secret\secrets.json の cloudflare.api_token (ローカル)
#
# 成果物 (local-release/artifacts/):
#   - Edictora-<version>-win-x64.zip … 版付き。中身が変わらないので R2 では immutable で配る
#   - Edictora-win-x64.zip           … 固定名の最新版コピー。ダウンロードページから常に同じ URL で指せるようにする
#
# 使い方:
#   pwsh scripts/release-local.ps1
#   pwsh scripts/release-local.ps1 -SkipUpload
#   pwsh scripts/release-local.ps1 -Cleanup

[CmdletBinding()]
param(
    [switch]$SkipUpload,
    [switch]$Cleanup
)




# 検証済みの作業対象だけを標準APIで削除する。
function Remove-LocalWorkItem {
    param([string[]]$LiteralPath, [string]$AllowedRoot, [switch]$IgnoreMissing, [switch]$AllowLeafJunction)
    if ([string]::IsNullOrWhiteSpace($AllowedRoot) -or -not [IO.Path]::IsPathRooted($AllowedRoot)) { throw 'Cleanup root must be absolute.' }
    $boundary = [IO.Path]::GetFullPath($AllowedRoot).TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar
    foreach ($target in $LiteralPath) {
        if ([string]::IsNullOrWhiteSpace($target) -or -not [IO.Path]::IsPathRooted($target)) { throw 'Cleanup target must be absolute.' }
        $resolved = [IO.Path]::GetFullPath($target)
        if (-not $resolved.StartsWith($boundary, [StringComparison]::OrdinalIgnoreCase)) { throw 'Cleanup target is outside the allowed root.' }
        $ancestor = $resolved
        while ($ancestor) {
            if (Test-Path -LiteralPath $ancestor) {
                $item = Get-Item -LiteralPath $ancestor -Force -ErrorAction Stop
                if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -and -not ($AllowLeafJunction -and $ancestor -eq $resolved -and $item.LinkType -eq 'Junction')) { throw 'Cleanup path contains a reparse point.' }
            }
            $ancestor = Split-Path $ancestor -Parent
        }
        if (-not (Test-Path -LiteralPath $resolved)) { if ($IgnoreMissing) { continue }; throw "Cleanup target is missing: $resolved" }
        $item = Get-Item -LiteralPath $resolved -Force -ErrorAction Stop
        $leafLink = [bool]($item.Attributes -band [IO.FileAttributes]::ReparsePoint)
        if ($item.PSIsContainer -and -not $leafLink) {
            if (@(Get-ChildItem -LiteralPath $resolved -Recurse -Force -ErrorAction Stop | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }).Count) { throw 'Cleanup directory contains a reparse point.' }
        }
        Remove-Item -LiteralPath $resolved -Recurse:($item.PSIsContainer -and -not $leafLink) -Force -ErrorAction Stop
        if (Test-Path -LiteralPath $resolved) { throw "Cleanup target remains: $resolved" }
    }
}

# R2 への書込みは同梱の PowerShell/.NET 実装で行う。
. (Join-Path $PSScriptRoot 'R2Distribution.Common.ps1')

function Send-ReleaseR2Object {
    param([Parameter(Mandatory)][System.IO.FileInfo]$File)
    # バージョン付き成果物は不変 (immutable)、固定名の最新版コピーは毎回再検証させる。
    $immutable = $File.Name -match '\d+\.\d+\.\d+'
    $cacheControl = if ($immutable) { 'public, max-age=31536000, immutable' } else { 'no-cache, must-revalidate' }
    $contentType = switch ($File.Extension.ToLowerInvariant()) {
        '.zip' { 'application/zip'; break }
        default { 'application/octet-stream' }
    }
    $null = Publish-R2Object -AccountId $AccountId -Bucket $Bucket -FilePath $File.FullName `
        -ApiToken $env:CLOUDFLARE_API_TOKEN -ContentType $contentType -CacheControl $cacheControl -Immutable:$immutable
}

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# 配布先 (R2 bucket と公開 URL)。Edictora の配布先はまだ決まっていない (外部リソースの作成は未決定)。
# 決まるまで空のままにし、公開を伴う実行は下の検査で「配布先が未設定」として止める (fail closed)。
# 作り替え前の製品の bucket・ドメインは削除済みなので流用しない。決まったらここに設定する。
# ⚠️ この検査は Sigilune のログイン・ビルド・local-release の削除・Cloudflare への接続より前に置くこと。
$Bucket = ''
$BaseUrl = ''
if (-not $SkipUpload -and ([string]::IsNullOrWhiteSpace($Bucket) -or [string]::IsNullOrWhiteSpace($BaseUrl))) {
    # 署名ログインやビルドより前に止める (何も作らず・何も送らない)。
    # -SkipUpload (build・ZIP 化・署名の確認だけ) は配布先を使わないので続行できる。
    throw '配布先が未設定です。scripts/release-local.ps1 の $Bucket と $BaseUrl を設定してから実行してください (確認だけなら -SkipUpload)。'
}
if ($Cleanup -and $SkipUpload) {
    # -Cleanup は R2 の一覧取得と削除を伴う。ビルドや署名の前に矛盾した指定を止める。
    throw '-Cleanup は R2 へ接続するため、-SkipUpload と同時には指定できません'
}
$AccountId = '10901bfadbf1005164774a7350082985'
# 秘密情報の正本 (ローカル実行時だけ読む。CI は環境変数で渡す)。
$SecretsPath = 'D:\Dev\Secret\secrets.json'
# 署名者の CN。証明書の年次更新で thumbprint が変わっても同じ値で照合できる。
$CertSubjectName = 'Open Source Developer Yuichiro Shinozaki'
# 配布する唯一の runtime。cargo build は既定で host 向けに作るため、host がこの triple であることを確かめる。
$Channel = 'win-x64'
$ExpectedHostTriple = 'x86_64-pc-windows-msvc'
# 配布する crate と実行ファイル名 (crates/edictora の [[bin]])。
$PackageName = 'edictora'
$ExeName = 'edictora.exe'
# ZIP に同梱する文書 (リポジトリ直下からの相対パス)。どれかが欠けていたら配布物が不完全なので止める。
$BundledDocuments = @('LICENSE', 'README.md', 'PRIVACY_POLICY.md')

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$WorkDir = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot 'local-release'))
$ArtifactsDir = Join-Path $WorkDir 'artifacts'
$StagingDir = Join-Path $WorkDir "publish-$Channel"
$SignatureVerificationDir = Join-Path $WorkDir 'signature-verification'
$RepoPrefix = $RepoRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar

if (-not $WorkDir.StartsWith($RepoPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "ローカルリリース出力先がリポジトリ外です: $WorkDir"
}

Set-Location $RepoRoot
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

function Invoke-Native {
    param(
        [Parameter(Mandatory)]
        [string]$Description,
        [Parameter(Mandatory)]
        [scriptblock]$Block
    )

    & $Block
    if ($LASTEXITCODE -ne 0) {
        throw "$Description が失敗しました (exit $LASTEXITCODE)"
    }
}

function Test-Signature {
    param([Parameter(Mandatory)][string]$Path)

    $signature = Get-AuthenticodeSignature -FilePath $Path
    if ($signature.Status -ne 'Valid' -or
        $null -eq $signature.SignerCertificate -or
        $signature.SignerCertificate.Subject -notlike "CN=$CertSubjectName*") {
        throw "署名検証失敗: $([System.IO.Path]::GetFileName($Path)) → $($signature.Status)"
    }

    Write-Host "  ✅ $([System.IO.Path]::GetFileName($Path)): Valid ($($signature.SignerCertificate.Subject -replace ',.*$'))"
}

# ZIP の中の実行ファイルを取り出して署名を確かめる (ZIP 化の途中で未署名の exe に入れ替わっていないことの確認)。
function Test-ArchiveEntrySignature {
    param(
        [Parameter(Mandatory)][string]$ArchivePath,
        [Parameter(Mandatory)][string]$EntryPath,
        [Parameter(Mandatory)][string]$Label
    )

    $archive = [System.IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        $entry = $archive.GetEntry($EntryPath)
        if ($null -eq $entry) {
            throw "$Label に署名検証対象がありません: $EntryPath"
        }

        $outputName = "$([System.IO.Path]::GetFileNameWithoutExtension($ArchivePath))-$([System.IO.Path]::GetFileName($EntryPath))"
        $outputPath = Join-Path $SignatureVerificationDir $outputName
        $input = $entry.Open()
        try {
            $output = [System.IO.File]::Create($outputPath)
            try {
                $input.CopyTo($output)
            }
            finally {
                $output.Dispose()
            }
        }
        finally {
            $input.Dispose()
        }

        Test-Signature -Path $outputPath
    }
    finally {
        $archive.Dispose()
    }
}

function Send-R2Object {
    param([Parameter(Mandatory)][System.IO.FileInfo]$File)

    Write-Host "  ↑ $($File.Name)"
    Send-ReleaseR2Object -File $File
}

Write-Host '== プリフライト ==' -ForegroundColor Cyan

# 版と出力先は cargo metadata から読む。crate 側は version.workspace = true でリテラルの版を持たないため、
# Cargo.toml を正規表現で読むより、cargo が解決した値を使うほうが確実。--locked で Cargo.lock の不整合も早く落とす。
$metadataJson = cargo metadata --no-deps --format-version 1 --locked
if ($LASTEXITCODE -ne 0) {
    throw "cargo metadata が失敗しました (exit $LASTEXITCODE)"
}
$metadata = ($metadataJson -join "`n") | ConvertFrom-Json
$package = @($metadata.packages | Where-Object { $_.name -eq $PackageName })
if ($package.Count -ne 1) {
    throw "cargo metadata に crate '$PackageName' が見つかりません"
}
$Version = [string]$package[0].version
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    # R2 の保持ポリシーと immutable 判定は x.y.z を前提にするため、プレリリース表記などは止める。
    throw "配布できない版の形式です: '$Version' (x.y.z のみ)"
}
$TargetDir = [string]$metadata.target_directory
Write-Host "バージョン: $Version"

# cargo build --release は host 向けに作る。配布 runtime (win-x64) と host が違えば別物を配ってしまうので止める。
$rustcInfo = rustc -vV
if ($LASTEXITCODE -ne 0) {
    throw "rustc -vV が失敗しました (exit $LASTEXITCODE)"
}
$hostLine = @($rustcInfo | Where-Object { $_ -like 'host:*' }) | Select-Object -First 1
$hostTriple = if ($hostLine) { ($hostLine -replace '^host:\s*', '').Trim() } else { '' }
if ($hostTriple -ne $ExpectedHostTriple) {
    throw "ビルド環境の host が $ExpectedHostTriple ではありません: '$hostTriple'"
}
Write-Host "rustc host: $hostTriple"

foreach ($document in $BundledDocuments) {
    if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot $document) -PathType Leaf)) {
        throw "ZIP に同梱する文書がありません: $document"
    }
}

# 署名の準備 (Sigilune の場所とクラウドへのログインを、ビルド前に確認して早く落とす)。
. (Join-Path $PSScriptRoot 'Sigilune.Common.ps1')
$Sigilune = Resolve-SigiluneExe
Connect-Sigilune -SigiluneExe $Sigilune

$cloudflareHeaders = $null
$zoneId = $null
if (-not $SkipUpload) {
    # CI では Environment "release" の Secret が環境変数で渡る。ローカルは secrets.json から必要なキーだけ読む。
    if (-not $env:CLOUDFLARE_API_TOKEN) {
        if (-not (Test-Path -LiteralPath $SecretsPath)) {
            throw "Cloudflare 認証情報が見つかりません: $SecretsPath"
        }

        $secrets = Get-Content -LiteralPath $SecretsPath -Raw | ConvertFrom-Json
        if (-not $secrets.cloudflare.api_token) {
            throw 'secrets.json に cloudflare.api_token が見つかりません'
        }

        $env:CLOUDFLARE_API_TOKEN = $secrets.cloudflare.api_token
    }
    $env:CLOUDFLARE_ACCOUNT_ID = $AccountId
    $cloudflareHeaders = @{ Authorization = "Bearer $($env:CLOUDFLARE_API_TOKEN)" }

    # upload 前に bucket と zone 権限を確認し、途中まで公開された状態を防ぐ。
    $bucketResponse = Invoke-RestMethod -Uri "https://api.cloudflare.com/client/v4/accounts/$AccountId/r2/buckets/$Bucket" `
        -Headers $cloudflareHeaders -TimeoutSec 30
    if (-not $bucketResponse.success) {
        throw "Cloudflare R2 bucket '$Bucket' を確認できませんでした"
    }

    $zoneName = ([uri]$BaseUrl).Host -replace '^[^.]+\.', ''
    $zoneResponse = Invoke-RestMethod -Uri "https://api.cloudflare.com/client/v4/zones?name=$zoneName" `
        -Headers $cloudflareHeaders -TimeoutSec 30
    if (-not $zoneResponse.success -or @($zoneResponse.result).Count -eq 0) {
        throw "Cloudflare zone '$zoneName' を確認できませんでした"
    }
    $zoneId = $zoneResponse.result[0].id

    try {
        [void][System.Net.Dns]::GetHostAddresses(([uri]$BaseUrl).Host)
    }
    catch {
        throw "配信ドメインが DNS 解決できません: $BaseUrl"
    }
}

if (Test-Path -LiteralPath $WorkDir) {
    Remove-LocalWorkItem -LiteralPath $WorkDir -AllowedRoot $RepoRoot
}
New-Item -ItemType Directory -Path $ArtifactsDir -Force | Out-Null
New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null
New-Item -ItemType Directory -Path $SignatureVerificationDir -Force | Out-Null

Write-Host "== cargo build: $Channel ==" -ForegroundColor Cyan
# --locked: Cargo.lock を書き換えない (CI と同じ依存で作る)。toolchain は rust-toolchain.toml に従う。
Invoke-Native 'cargo build --release' { cargo build --release --locked -p $PackageName }

$builtExe = Join-Path (Join-Path $TargetDir 'release') $ExeName
if (-not (Test-Path -LiteralPath $builtExe -PathType Leaf)) {
    throw "$ExeName がビルド出力にありません: $builtExe"
}

# target/ の exe には直接署名しない (次の cargo build で上書き・再利用されるため)。作業フォルダーへ写してから署名する。
$stagedExe = Join-Path $StagingDir $ExeName
Copy-Item -LiteralPath $builtExe -Destination $stagedExe
foreach ($document in $BundledDocuments) {
    Copy-Item -LiteralPath (Join-Path $RepoRoot $document) -Destination (Join-Path $StagingDir (Split-Path $document -Leaf))
}

Write-Host "== 署名: $Channel ==" -ForegroundColor Cyan
# Sigilune は署名後に有効性・署名者・タイムスタンプ (http://time.certum.pl) を検証して失敗なら非0で終わる。
# Invoke-SigiluneSign はさらに署名者 CN とタイムスタンプの有無を照合する。
Invoke-SigiluneSign -SigiluneExe $Sigilune -Path $stagedExe -CertificateSubjectName $CertSubjectName
Test-Signature -Path $stagedExe

Write-Host "== Portable ZIP: $Channel ==" -ForegroundColor Cyan
$versionedZip = Join-Path $ArtifactsDir "Edictora-$Version-$Channel.zip"
$latestZip = Join-Path $ArtifactsDir "Edictora-$Channel.zip"
# ZIP の直下に exe と文書を置く (展開してすぐ edictora.exe を実行できるように)。
[System.IO.Compression.ZipFile]::CreateFromDirectory($StagingDir, $versionedZip, [System.IO.Compression.CompressionLevel]::Optimal, $false)
Test-ArchiveEntrySignature -ArchivePath $versionedZip -EntryPath $ExeName -Label 'Portable ZIP'

# 固定名の最新版コピー。中身は版付き ZIP と同一。
Copy-Item -LiteralPath $versionedZip -Destination $latestZip
Test-ArchiveEntrySignature -ArchivePath $latestZip -EntryPath $ExeName -Label 'Portable ZIP (固定名)'

if ($SkipUpload) {
    Write-Host "`n✅ -SkipUpload 指定のため公開せず終了します: $ArtifactsDir" -ForegroundColor Green
    Get-ChildItem -LiteralPath $ArtifactsDir -File |
        Select-Object Name, @{ Name = 'Size(MB)'; Expression = { [math]::Round($_.Length / 1MB, 1) } } |
        Format-Table -AutoSize
    return
}

# 版付き ZIP を先に、固定名 ZIP を最後に公開する。
# 固定名を先に置くと、版付き ZIP の upload が失敗したときに「最新版」だけが先行して公開されてしまう。
$versionedFiles = @(Get-ChildItem -LiteralPath $ArtifactsDir -File | Where-Object { $_.Name -match '\d+\.\d+\.\d+' } | Sort-Object Name)
$fixedFiles = @(Get-ChildItem -LiteralPath $ArtifactsDir -File | Where-Object { $_.Name -notmatch '\d+\.\d+\.\d+' } | Sort-Object Name)
if ($versionedFiles.Count -eq 0 -or $fixedFiles.Count -eq 0) {
    throw 'R2 へアップロードする成果物が揃っていません'
}

Write-Host '== R2 アップロード: 版付き成果物 ==' -ForegroundColor Cyan
foreach ($file in $versionedFiles) {
    Send-R2Object -File $file
}

Write-Host '== R2 アップロード: 固定名成果物 ==' -ForegroundColor Cyan
foreach ($file in $fixedFiles) {
    Send-R2Object -File $file
}

# 固定 URL の ZIP が旧キャッシュを返さないよう purge する (版付きは immutable なので不要)。
Write-Host '== Cloudflare キャッシュパージ ==' -ForegroundColor Cyan
$purgeUrls = @($fixedFiles | ForEach-Object { "$BaseUrl/$([uri]::EscapeDataString($_.Name))" })
if ($purgeUrls.Count -gt 0) {
    try {
        for ($offset = 0; $offset -lt $purgeUrls.Count; $offset += 30) {
            $last = [math]::Min($offset + 29, $purgeUrls.Count - 1)
            $batch = @($purgeUrls[$offset..$last])
            $body = @{ files = $batch } | ConvertTo-Json -Depth 3 -Compress
            $purgeResponse = Invoke-RestMethod -Method Post `
                -Uri "https://api.cloudflare.com/client/v4/zones/$zoneId/purge_cache" `
                -Headers $cloudflareHeaders -ContentType 'application/json' -Body $body -TimeoutSec 30
            if (-not $purgeResponse.success) {
                throw ($purgeResponse.errors | ConvertTo-Json -Compress)
            }
        }
        Write-Host "  ✅ $($purgeUrls.Count) URL"
    }
    catch {
        Write-Warning "R2 アップロードは完了しましたが、キャッシュパージに失敗しました: $($_.Exception.Message)"
    }
}

if ($Cleanup) {
    Write-Host '== 古い版のクリーンアップ ==' -ForegroundColor Cyan
    # 今回 upload したファイルは必ず保持する。
    $keep = @{}
    foreach ($file in @($versionedFiles + $fixedFiles)) {
        $keep[$file.Name] = $true
    }
    if ($keep.Count -eq 0) {
        throw '保持対象が空のため cleanup を中止します'
    }

    $api = "https://api.cloudflare.com/client/v4/accounts/$AccountId/r2/buckets/$Bucket"
    $allKeys = [System.Collections.Generic.List[string]]::new()
    $cursor = ''
    while ($true) {
        $uri = "$api/objects?per_page=1000" + $(if ($cursor) { "&cursor=$cursor" })
        $response = Invoke-RestMethod -Uri $uri -Headers $cloudflareHeaders -TimeoutSec 30
        foreach ($object in @($response.result)) {
            $allKeys.Add($object.key)
        }

        $info = $response.PSObject.Properties['result_info']
        if (-not $info -or -not $info.Value) { break }
        $isTruncated = $info.Value.PSObject.Properties['is_truncated']
        if (-not $isTruncated -or -not $isTruncated.Value) { break }
        $cursorProperty = $info.Value.PSObject.Properties['cursor']
        $cursor = if ($cursorProperty) { $cursorProperty.Value } else { '' }
        if (-not $cursor) { break }
    }

    # 全プロジェクト共通の保持ポリシー: 直近 2 バージョン。
    # 固定ファイル名 (Edictora-win-x64.zip) はバージョン文字列を含まないので対象外 = 毎リリース上書きで保持。
    $KeepVersionCount = 2
    $versionPattern = '(\d+\.\d+\.\d+)'
    $allVersions = @(
        $allKeys | ForEach-Object {
            $m = [regex]::Match($_, $versionPattern)
            if ($m.Success) { $m.Groups[1].Value }
        } | Sort-Object -Property { [version]$_ } -Unique
    )
    $keepVersions = @($allVersions | Select-Object -Last $KeepVersionCount)
    # 今回の版が保持対象に入っていなければ (R2 に今回より新しい版が 2 つ以上ある等)、何かがおかしいので消さずに止める。
    if ($keepVersions -notcontains $Version) {
        throw "今回の版 $Version が保持対象 ($($keepVersions -join ', ')) に入らないため cleanup を中止します"
    }
    Write-Host "  保持バージョン: $($keepVersions -join ', ') (全 $($allVersions.Count) 世代)"

    $deleteCandidates = @($allKeys | Where-Object {
        # 今回 upload したファイルは絶対保持
        if ($keep.ContainsKey($_)) { return $false }
        # 固定ファイル名はバージョン文字列を含まない = 毎リリース上書きなので保持
        $m = [regex]::Match($_, $versionPattern)
        if (-not $m.Success) { return $false }
        return $keepVersions -notcontains $m.Groups[1].Value
    })
    if ($deleteCandidates.Count -eq 0) {
        Write-Host '  ✅ 削除対象なし'
    }
    else {
        Write-Host "  削除対象: $($deleteCandidates.Count) 件"
        foreach ($key in $deleteCandidates) {
            $encodedKey = [uri]::EscapeDataString($key)
            Invoke-RestMethod -Method Delete -Uri "$api/objects/$encodedKey" -Headers $cloudflareHeaders -TimeoutSec 30 | Out-Null
            Write-Host "  🗑️  $key"
        }
    }
}
else {
    Write-Host '== cleanup ==' -ForegroundColor Cyan
    Write-Host '  省略（古い版を消すリリースで -Cleanup を明示してください）'
}

Write-Host "`n🎉 リリース完了: v$Version → $BaseUrl" -ForegroundColor Green
