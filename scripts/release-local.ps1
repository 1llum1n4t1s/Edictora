# release-local.ps1 — 署名付き Velopack リリース
#
# 署名は Sigilune (Certum SimplySign のクラウド鍵を signtool /dlib で使う CLI) で行う。
# 通常のリリースは GitHub Actions の .github/workflows/release.yml (手動起動・Environment "release") が
# 本スクリプトを実行する。ローカル実行も同じ手順で動き、検証や再実行に使う。
# 生成した成果物は Cloudflare R2 にアップロードし、更新 manifest は最後に公開する。
#
# 前提:
#   - Sigilune を使えること (scripts/Sigilune.Common.ps1 参照)。
#     ローカルは `sigilune setup` で Certum アカウントを登録済み、CI は SIGILUNE_EMAIL / SIGILUNE_TOTP_URI を渡す
#   - Directory.Build.props の <Version> が配布したいバージョンになっていること
#   - 配布先 (下の $Bucket / $BaseUrl) が設定済みで、Cloudflare 側にその bucket と公開ドメインが準備済みであること。
#     Edictora の配布先は未確定のため、現在は空。公開を伴う実行は「配布先が未設定」で止まる (-SkipUpload は可)
#   - 環境変数 CLOUDFLARE_API_TOKEN (CI)、なければ D:\Dev\Secret\secrets.json の cloudflare.api_token (ローカル)
#
# 使い方:
#   pwsh scripts/release-local.ps1
#   pwsh scripts/release-local.ps1 -SkipUpload
#   pwsh scripts/release-local.ps1 -Cleanup
#   pwsh scripts/release-local.ps1 -Runtimes win-x64

[CmdletBinding()]
param(
    [switch]$SkipUpload,
    [switch]$Cleanup,
    [string[]]$Runtimes = @('win-x64')
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
    # バージョン付き成果物は不変、固定 URL とメタデータは毎回再検証する。
    $immutable = $File.Extension -eq '.nupkg' -or $File.Name -match '\d+\.\d+\.\d+'
    $cacheControl = if ($immutable) { 'public, max-age=31536000, immutable' } else { 'no-cache, must-revalidate' }
    $contentType = switch ($File.Extension.ToLowerInvariant()) {
        '.json' { 'application/json'; break }
        '.exe' { 'application/vnd.microsoft.portable-executable'; break }
        '.zip' { 'application/zip'; break }
        default { if ($File.Name -like 'RELEASES*') { 'text/plain; charset=utf-8' } else { 'application/octet-stream' } }
    }
    $null = Publish-R2Object -AccountId $AccountId -Bucket $Bucket -FilePath $File.FullName `
        -ApiToken $env:CLOUDFLARE_API_TOKEN -ContentType $contentType -CacheControl $cacheControl -Immutable:$immutable
}

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# 配布先 (R2 bucket と公開 URL)。Edictora の配布先はまだ決まっていない (外部リソースの作成は未決定)。
# 決まるまで空のままにし、公開を伴う実行は下の検査で「配布先が未設定」として止める。
# 作り替え前の製品の bucket・ドメインは削除済みなので流用しない。決まったらここに設定する。
$Bucket = ''
$BaseUrl = ''
if (-not $SkipUpload -and ([string]::IsNullOrWhiteSpace($Bucket) -or [string]::IsNullOrWhiteSpace($BaseUrl))) {
    # 署名ログインやビルドより前に止める (何も作らず・何も送らない)。
    # -SkipUpload (build・package・署名の確認だけ) は配布先を使わないので続行できる。
    throw '配布先が未設定です。scripts/release-local.ps1 の $Bucket と $BaseUrl を設定してから実行してください (確認だけなら -SkipUpload)。'
}
$AccountId = '10901bfadbf1005164774a7350082985'
# 秘密情報の正本 (ローカル実行時だけ読む。CI は環境変数で渡す)。
$SecretsPath = 'D:\Dev\Secret\secrets.json'
# 署名者の CN。証明書の年次更新で thumbprint が変わっても同じ値で照合できる。
$CertSubjectName = 'Open Source Developer Yuichiro Shinozaki'
$RuntimeMatrix = @{
    'win-x64' = @{ Channel = 'win-x64' }
}

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$WorkDir = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot 'local-release'))
$ArtifactsDir = Join-Path $WorkDir 'artifacts'
$SignatureVerificationDir = Join-Path $WorkDir 'signature-verification'
$RepoPrefix = $RepoRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar

if (-not $WorkDir.StartsWith($RepoPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "ローカルリリース出力先がリポジトリ外です: $WorkDir"
}

Set-Location $RepoRoot
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

if (-not ${env:ProgramFiles(x86)}) {
    ${env:ProgramFiles(x86)} = 'C:\Program Files (x86)'
}

$vsInstallerDir = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer'
if ($env:PATH -notlike "*$vsInstallerDir*") {
    $env:PATH = "$env:PATH;$vsInstallerDir"
}

$env:DOTNET_ROLL_FORWARD = 'Major'

$versionNode = ([xml](Get-Content -LiteralPath 'Directory.Build.props' -Raw)).SelectSingleNode('/Project/PropertyGroup/Version')
$Version = if ($versionNode) { $versionNode.InnerText.Trim() } else { $null }
if (-not $Version) {
    throw 'Directory.Build.props から <Version> を取得できませんでした'
}
Write-Host "バージョン: $Version"

# 署名の準備 (Sigilune の場所とクラウドへのログインを、ビルド前に確認して早く落とす)。
. (Join-Path $PSScriptRoot 'Sigilune.Common.ps1')
$Sigilune = Resolve-SigiluneExe
Connect-Sigilune -SigiluneExe $Sigilune
# Velopack はファイルごとに別プロセスで署名テンプレートを実行する。Sigilune はログインを共有し、
# 署名後に有効性・署名者・タイムスタンプ (http://time.certum.pl) を検証して失敗なら非0で終わる。
$SignTemplate = "`"$Sigilune`" sign {{file}}"

$vpkVersion = (Invoke-RestMethod 'https://api.nuget.org/v3-flatcontainer/vpk/index.json' -TimeoutSec 30).versions |
    Where-Object { $_ -notmatch '-' } |
    Select-Object -Last 1
if (-not $vpkVersion) {
    throw 'vpk の最新安定版バージョンの取得に失敗しました (NuGet API)'
}
Write-Host "vpk 最新安定版: $vpkVersion"

$vpkToolLine = dotnet tool list --global |
    Where-Object { $_ -match '^\s*vpk\s+' } |
    Select-Object -First 1
$installedVpkVersion = if ($vpkToolLine) {
    @($vpkToolLine -split '\s+' | Where-Object { $_ })[1]
} else {
    $null
}

if (-not $installedVpkVersion) {
    Invoke-Native 'vpk のインストール' { dotnet tool install --global vpk --version $vpkVersion }
}
elseif ($installedVpkVersion -ne $vpkVersion) {
    Invoke-Native 'vpk の更新' { dotnet tool update --global vpk --version $vpkVersion }
}

foreach ($runtime in $Runtimes) {
    if (-not $RuntimeMatrix.ContainsKey($runtime)) {
        throw "未知の runtime: $runtime"
    }
}

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

if ($Cleanup -and $SkipUpload) {
    throw '-Cleanup は R2 へ接続するため、-SkipUpload と同時には指定できません'
}

if (Test-Path -LiteralPath $WorkDir) {
    Remove-LocalWorkItem -LiteralPath $WorkDir -AllowedRoot $RepoRoot
}
New-Item -ItemType Directory -Path $ArtifactsDir -Force | Out-Null
New-Item -ItemType Directory -Path $SignatureVerificationDir -Force | Out-Null

foreach ($runtime in $Runtimes) {
    $config = $RuntimeMatrix[$runtime]
    $publishDir = Join-Path $WorkDir "publish-$runtime"
    $mainExe = Join-Path $publishDir 'Edictora.UI.exe'

    # 画面版 (Edictora.UI.exe) と CLI (edictora.exe) を同じフォルダに入れて 1 つのパッケージで配る。
    $cliExe = Join-Path $publishDir 'edictora.exe'

    Write-Host "== restore: $runtime ==" -ForegroundColor Cyan
    Invoke-Native "dotnet restore ($runtime)" {
        dotnet restore src/Edictora.UI/Edictora.UI.csproj -p:Configuration=Release --locked-mode
    }
    Invoke-Native "dotnet restore CLI ($runtime)" {
        dotnet restore src/Edictora.Cli/Edictora.Cli.csproj -p:Configuration=Release --locked-mode
    }

    Write-Host "== publish: $runtime ==" -ForegroundColor Cyan
    Invoke-Native "dotnet publish ($runtime)" {
        dotnet publish src/Edictora.UI/Edictora.UI.csproj -c Release -r $runtime --self-contained true --no-restore -o $publishDir
    }
    Invoke-Native "dotnet publish CLI ($runtime)" {
        dotnet publish src/Edictora.Cli/Edictora.Cli.csproj -c Release -r $runtime --self-contained true --no-restore -o $publishDir
    }

    if (-not (Test-Path -LiteralPath $mainExe)) {
        throw "Edictora.UI.exe が publish 出力にありません ($runtime)"
    }
    if (-not (Test-Path -LiteralPath $cliExe)) {
        throw "edictora.exe (CLI) が publish 出力にありません ($runtime)"
    }

    Write-Host "== vpk pack + 署名: $runtime ==" -ForegroundColor Cyan
    Invoke-Native "vpk pack ($runtime)" {
        vpk pack `
            --packId Edictora `
            --packVersion $Version `
            --packTitle 'Edictora' `
            --packAuthors 'Kagayoi' `
            --mainExe 'Edictora.UI.exe' `
            --icon (Join-Path 'icon' 'app.ico') `
            --packDir $publishDir `
            --outputDir $ArtifactsDir `
            --channel $config.Channel `
            --shortcuts 'StartMenuRoot,Desktop' `
            --signParallel 1 `
            --signTemplate $SignTemplate
    }

    Write-Host "== 署名検証: $runtime ==" -ForegroundColor Cyan
    $fullPackage = Join-Path $ArtifactsDir "Edictora-$Version-$($config.Channel)-full.nupkg"
    $portablePackage = Join-Path $ArtifactsDir "Edictora-$($config.Channel)-Portable.zip"
    $setupPackage = Join-Path $ArtifactsDir "Edictora-$($config.Channel)-Setup.exe"
    if (-not (Test-Path -LiteralPath $fullPackage) -or
        -not (Test-Path -LiteralPath $portablePackage) -or
        -not (Test-Path -LiteralPath $setupPackage)) {
        throw "配布パッケージが生成されませんでした ($runtime)"
    }

    Test-ArchiveEntrySignature -ArchivePath $fullPackage -EntryPath 'lib/app/Edictora.UI.exe' -Label 'フルパッケージ'
    Test-ArchiveEntrySignature -ArchivePath $portablePackage -EntryPath 'current/Edictora.UI.exe' -Label 'Portable パッケージ'
    Test-ArchiveEntrySignature -ArchivePath $fullPackage -EntryPath 'lib/app/edictora.exe' -Label 'フルパッケージ (CLI)'
    Test-ArchiveEntrySignature -ArchivePath $portablePackage -EntryPath 'current/edictora.exe' -Label 'Portable パッケージ (CLI)'
    Test-Signature -Path $setupPackage

    # 内容が変わらないバージョン付き URL のインストーラーも残す (Microsoft Store は未申請だが、申請時に必要になる)。
    $storeInstaller = Join-Path $ArtifactsDir "Edictora-$Version-$($config.Channel)-Setup.exe"
    Copy-Item -LiteralPath $setupPackage -Destination $storeInstaller
    Test-Signature -Path $storeInstaller
}

$manifestFiles = @(Get-ChildItem -LiteralPath $ArtifactsDir -Filter 'releases.*.json' -File)
if ($manifestFiles.Count -ne $Runtimes.Count) {
    throw "manifest 数が runtime 数と一致しません (manifest=$($manifestFiles.Count), runtime=$($Runtimes.Count))"
}

foreach ($manifest in $manifestFiles) {
    $manifestData = Get-Content -LiteralPath $manifest.FullName -Raw | ConvertFrom-Json
    foreach ($asset in @($manifestData.Assets)) {
        if (-not $asset.FileName) {
            throw "manifest に FileName のない asset があります: $($manifest.Name)"
        }
        if (-not (Test-Path -LiteralPath (Join-Path $ArtifactsDir $asset.FileName))) {
            throw "manifest 参照パッケージが artifacts にありません: $($asset.FileName)"
        }
    }
}

if ($SkipUpload) {
    Write-Host "`n✅ -SkipUpload 指定のため公開せず終了します: $ArtifactsDir" -ForegroundColor Green
    Get-ChildItem -LiteralPath $ArtifactsDir -File |
        Select-Object Name, @{ Name = 'Size(MB)'; Expression = { [math]::Round($_.Length / 1MB, 1) } } |
        Format-Table -AutoSize
    return
}

# manifest を先に公開すると、クライアントが未到達の package を参照し得る。
# package と固定成果物を先に upload・検証し、manifest は最後に公開する。
$metadataFiles = @(Get-ChildItem -LiteralPath $ArtifactsDir -File | Where-Object { ($_.Name -like 'assets.*.json' -or $_.Name -like 'RELEASES*') -and $_.Name -notlike 'releases.*.json' } | Sort-Object Name)
$payloadFiles = @(Get-ChildItem -LiteralPath $ArtifactsDir -File |
    Where-Object { $_.Name -notlike 'releases.*.json' -and $_.Name -notlike 'assets.*.json' -and $_.Name -notlike 'RELEASES*' } |
    Sort-Object Name)
if ($payloadFiles.Count -eq 0) {
    throw 'R2 へアップロードする成果物がありません'
}

Write-Host '== R2 アップロード: package / 固定成果物 ==' -ForegroundColor Cyan
foreach ($file in $payloadFiles) {
    Send-R2Object -File $file
}

Write-Host '== R2 アップロード: manifest ==' -ForegroundColor Cyan
foreach ($manifest in @($metadataFiles + @($manifestFiles | Sort-Object Name))) {
    Send-R2Object -File $manifest
}

# 固定 URL の Setup / Portable / manifest が旧キャッシュを返さないよう purge する。
Write-Host '== Cloudflare キャッシュパージ ==' -ForegroundColor Cyan
$purgeFiles = @(Get-ChildItem -LiteralPath $ArtifactsDir -File | Where-Object {
    $_.Name -notlike '*.nupkg' -and $_.Name -notmatch "^Edictora-$([regex]::Escape($Version))-.*-Setup\.exe$"
})
$purgeUrls = @($purgeFiles | ForEach-Object { "$BaseUrl/$([uri]::EscapeDataString($_.Name))" })
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
    if ($Runtimes.Count -ne $RuntimeMatrix.Count) {
        throw '-Cleanup は全 runtime の manifest を生成した実行でのみ使用できます'
    }

    Write-Host '== manifest 外 nupkg クリーンアップ ==' -ForegroundColor Cyan
    $keep = @{}
    foreach ($manifest in $manifestFiles) {
        $manifestData = Get-Content -LiteralPath $manifest.FullName -Raw | ConvertFrom-Json
        foreach ($asset in @($manifestData.Assets)) {
            # manifest が参照するファイルはすべて保持対象 (拡張子で絞らない)
            if ($asset.FileName) {
                $keep[$asset.FileName] = $true
            }
        }
    }
    if ($keep.Count -eq 0) {
        throw '保持対象 nupkg が空のため cleanup を中止します'
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
    # ⚠️ 旧実装は '*.nupkg' だけを削除対象にしていたため、バージョン付きの配布物
    #    (zip / deb / rpm / AppImage 等) が R2 に永久に溜まる構造だった
    #    (Ferry では同原因で 351 個 7.2GB = 含有枠 10GB の 72% を滞留させていた)。
    #    固定ファイル名 (Setup.exe / Portable.zip / RELEASES* / releases.*.json) は
    #    バージョン文字列を含まないので対象外 = 安全。
    $KeepVersionCount = 2
    $versionPattern = '(\d+\.\d+\.\d+)'
    $allVersions = @(
        $allKeys | ForEach-Object {
            $m = [regex]::Match($_, $versionPattern)
            if ($m.Success) { $m.Groups[1].Value }
        } | Sort-Object -Property { [version]$_ } -Unique
    )
    $keepVersions = @($allVersions | Select-Object -Last $KeepVersionCount)
    Write-Host "  保持バージョン: $($keepVersions -join ', ') (全 $($allVersions.Count) 世代)"

    $deleteCandidates = @($allKeys | Where-Object {
        # manifest が参照するファイルは絶対保持 (消すと自動更新が壊れる)
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
    Write-Host '  省略（必要なリリースで -Cleanup を明示してください）'
}

Write-Host "`n🎉 リリース完了: v$Version → $BaseUrl" -ForegroundColor Green
