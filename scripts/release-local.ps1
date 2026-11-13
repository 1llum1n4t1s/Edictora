# release-local.ps1 — 署名付き Velopack リリース（Rust 版 edictora.exe + edictora-updater.exe）
#
# 流れ:
#   1. 配布先 (下の $Bucket / $BaseUrl) を確認する。未設定かつ配信を伴う実行なら、何も作らず・何も送らずに止まる
#   2. cargo metadata から版と同梱する bin (edictora / edictora-updater) を確かめ、rustc の host・同梱文書・vpk の版を確認する
#      (updater の bin がまだ無ければ、Sigilune のログインやビルドより前に止まる)
#   3. Sigilune のクラウドログインと R2 bucket / zone / DNS をプリフライト確認する
#   4. cargo build --release --locked で両方の exe を作り、payload フォルダーへ文書と一緒に集める
#   5. vpk pack で Velopack のパッケージ (full nupkg・Setup.exe・Portable.zip・フィード) を作る。
#      署名は vpk の --signTemplate から Sigilune を 1 ファイルずつ呼ぶ (--signParallel 1)
#   6. Setup.exe と、nupkg / Portable.zip の中の全 exe (edictora.exe・edictora-updater.exe・Update.exe・実行スタブ) の
#      Authenticode 署名 (署名者 CN・タイムスタンプ) を検証する
#   7. パッケージ類を先に、フィード (assets.*.json → RELEASES → releases.*.json) を最後に R2 へ upload する
#   8. 固定 URL (フィード・固定名 Setup / Portable) の Cloudflare キャッシュを purge する
#   9. -Cleanup のときだけ、直近 2 版より古い版付きファイルを R2 から消す (今回のフィードが参照するファイルは必ず残す)
#
# 署名は Sigilune (Certum SimplySign のクラウド鍵を signtool /dlib で使う CLI) で行う。
# 通常のリリースは GitHub Actions の .github/workflows/release.yml (手動起動・Environment "release") が
# 本スクリプトを実行する。ローカル実行も同じ手順で動き、検証や再実行に使う。
#
# 前提:
#   - Sigilune を使えること (scripts/Sigilune.Common.ps1 参照)。
#     ローカルは `sigilune setup` で Certum アカウントを登録済み、CI は SIGILUNE_EMAIL / SIGILUNE_TOTP_URI を渡す
#   - Velopack CLI (vpk) 1.2.161 を dotnet tool として使えること。ローカルで版が違えば本スクリプトが入れ替える
#     (CI は workflow 側で同じ版を導入する)
#   - Cargo.toml の [workspace.package] version が配布したいバージョンになっていること (更新は /vava のときだけ)
#   - rust-toolchain.toml の toolchain (rustup が自動で使う) と MSVC リンカーが使えること
#   - 環境変数 CLOUDFLARE_API_TOKEN (CI)、なければ D:\Dev\Secret\secrets.json の cloudflare.api_token (ローカル)
#
# 成果物 (local-release/artifacts/):
#   - Edictora-<version>-full.nupkg           … 自動更新の本体。版付きで不変 (immutable)
#   - Edictora-win-Setup.exe                  … 固定名のインストーラー。ダウンロードページから常に同じ URL で指す
#   - Edictora-<version>-win-Setup.exe        … 版付きのインストーラー (固定名と同一内容)
#   - Edictora-win-Portable.zip               … 固定名の Portable 版 (インストール不要。旧 Portable ZIP の置き換え)
#   - Edictora-<version>-win-Portable.zip     … 版付きの Portable 版 (固定名と同一内容)
#   - releases.win.json / assets.win.json / RELEASES … Velopack のフィード。クライアントは releases.win.json を読む
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

# 版付きファイル・nupkg は不変 (immutable)、固定名ファイルとフィードは毎回再検証させる。
# immutable で置いた object は R2Transfer が上書きせず、既存と内容が違えば検証で止まる
# (同じ版を作り直して再配信しようとすると、署名のタイムスタンプが変わるため止まる。版を上げるか、手動で古い object を消す)。
function Test-ImmutableReleaseFile {
    param([Parameter(Mandatory)][System.IO.FileInfo]$File)
    return $File.Extension -eq '.nupkg' -or $File.Name -match '\d+\.\d+\.\d+'
}

function Send-ReleaseR2Object {
    param([Parameter(Mandatory)][System.IO.FileInfo]$File)
    $immutable = Test-ImmutableReleaseFile -File $File
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

# 配布先 (R2 bucket と公開 URL)。Cloudflare 側で bucket と公開ドメイン (Custom Domain) を作成済み。
# 空に戻した場合に備え、公開を伴う実行は下の検査で「配布先が未設定」として止める (fail closed)。
# ⚠️ この検査は Sigilune のログイン・ビルド・local-release の削除・Cloudflare への接続より前に置くこと。
$Bucket = 'edictora-updates'
$BaseUrl = 'https://edictora.kagayoi.com'
if (-not $SkipUpload -and ([string]::IsNullOrWhiteSpace($Bucket) -or [string]::IsNullOrWhiteSpace($BaseUrl))) {
    # 署名ログインやビルドより前に止める (何も作らず・何も送らない)。
    # -SkipUpload (build・パッケージ作成・署名の確認だけ) は配布先を使わないので続行できる。
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
# 他アプリ (Taskmetry・Skilvella) と揃えた Velopack CLI の版。変えるときは release.yml の導入版も合わせる。
$VpkVersion = '1.2.161'
# Velopack の識別子。packId はインストール先 (%LOCALAPPDATA%\Edictora) と更新フィードの名前を決めるため、
# 一度配布したら変えない (変えると既存の利用者が自動更新を受け取れなくなる)。
# アプリのデータはインストール先と分けて %LOCALAPPDATA%\Edictora.Data に置く (Rust 側の app_paths.rs)。
$PackId = 'Edictora'
$PackTitle = 'Edictora'
$PackAuthors = 'Kagayoi'
# Velopack の channel。フィード名 (releases.<channel>.json) と固定名ファイル (Edictora-<channel>-Setup.exe) に入る。
$VelopackChannel = 'win'
# 配布する唯一の runtime。cargo build は既定で host 向けに作るため、host がこの triple であることを確かめる。
$Runtime = 'win-x64'
$ExpectedHostTriple = 'x86_64-pc-windows-msvc'
# 同梱する bin (Cargo の [[bin]] 名)。どの crate にあるかは cargo metadata で探す (crate 名は決め打ちしない)。
# 先頭が Velopack の mainExe (CLI 本体)。edictora-updater はログオン時の自動更新用で、コンソールを出さない別 bin。
$MainBinName = 'edictora'
$BinNames = @($MainBinName, 'edictora-updater')
$MainExeName = "$MainBinName.exe"
$PayloadExeNames = @($BinNames | ForEach-Object { "$_.exe" })
# 同梱する文書 (リポジトリ直下からの相対パス)。どれかが欠けていたら配布物が不完全なので止める。
$BundledDocuments = @('LICENSE', 'README.md', 'PRIVACY_POLICY.md')

$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$WorkDir = [System.IO.Path]::GetFullPath((Join-Path $RepoRoot 'local-release'))
$ArtifactsDir = Join-Path $WorkDir 'artifacts'
$PayloadDir = Join-Path $WorkDir "payload-$Runtime"
# vpk は TEMP の下でファイルを展開・署名する。作業フォルダーの中に閉じて、ほかの一時物と混ぜない。
$VpkTempDir = Join-Path $WorkDir 'temp'
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

# 署名が有効で、署名者 CN が一致し、タイムスタンプが付いていることを確かめる。
# タイムスタンプが無いと証明書の期限切れ後に署名が無効になるため、必ず照合する。
function Test-Signature {
    param(
        [Parameter(Mandatory)][string]$Path,
        [string]$Label = [System.IO.Path]::GetFileName($Path)
    )

    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($signature.Status -ne 'Valid' -or
        $null -eq $signature.SignerCertificate -or
        $signature.SignerCertificate.Subject -notlike "CN=$CertSubjectName*" -or
        $null -eq $signature.TimeStamperCertificate) {
        throw "署名検証失敗: $Label → $($signature.Status)"
    }

    Write-Host "  ✅ ${Label}: Valid ($($signature.SignerCertificate.Subject -replace ',.*$'))"
}

# アーカイブ (nupkg / Portable.zip) の中の .exe をすべて取り出して署名を確かめる。
# vpk は payload の exe・Update.exe (nupkg では Squirrel.exe)・実行スタブを署名してから詰めるため、
# 未署名の exe が 1 つでも混ざっていたら止める。$RequiredEntries が揃っていることも確かめる
# (vpk の版や配置が変わって想定のファイルが入らなかった場合に気づけるように)。
function Test-ArchiveExecutableSignatures {
    param(
        [Parameter(Mandatory)][string]$ArchivePath,
        [Parameter(Mandatory)][string[]]$RequiredEntries
    )

    $archiveName = [System.IO.Path]::GetFileName($ArchivePath)
    $archive = [System.IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        $entryNames = @($archive.Entries | ForEach-Object { $_.FullName })
        foreach ($required in $RequiredEntries) {
            if ($entryNames -notcontains $required) {
                throw "$archiveName に必要なファイルがありません: $required"
            }
        }

        $executables = @($archive.Entries | Where-Object { $_.FullName -like '*.exe' })
        foreach ($entry in $executables) {
            # 取り出し先の名前はアーカイブ名とエントリのパスから作り、フォルダー違いの同名 exe と衝突させない。
            $safeEntry = $entry.FullName -replace '[\\/]', '_'
            $outputPath = Join-Path $SignatureVerificationDir "$([System.IO.Path]::GetFileNameWithoutExtension($ArchivePath))--$safeEntry"
            $entryStream = $entry.Open()
            try {
                $output = [System.IO.File]::Create($outputPath)
                try {
                    $entryStream.CopyTo($output)
                }
                finally {
                    $output.Dispose()
                }
            }
            finally {
                $entryStream.Dispose()
            }

            Test-Signature -Path $outputPath -Label "$archiveName!$($entry.FullName)"
        }
    }
    finally {
        $archive.Dispose()
    }
}

# 版付きコピーが固定名ファイルと同一内容であることを確かめる (コピー後に片方だけ差し替わっていないことの確認)。
function Assert-SameFile {
    param([Parameter(Mandatory)][string]$Expected, [Parameter(Mandatory)][string]$Actual)
    $expectedHash = (Get-FileHash -LiteralPath $Expected -Algorithm SHA256).Hash
    $actualHash = (Get-FileHash -LiteralPath $Actual -Algorithm SHA256).Hash
    if ($expectedHash -cne $actualHash) {
        throw "版付きコピーの内容が一致しません: $([System.IO.Path]::GetFileName($Actual))"
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

# bin 名ごとに、それを持つ workspace の crate を 1 つだけ探す。
# updater の bin がまだ無い (Rust 側の実装前) なら、ログイン・ビルドより前にここで止める。
$binPackages = [ordered]@{}
foreach ($binName in $BinNames) {
    $owners = @($metadata.packages | Where-Object {
        @($_.targets | Where-Object { $_.name -eq $binName -and @($_.kind) -contains 'bin' }).Count -gt 0
    })
    if ($owners.Count -ne 1) {
        throw "bin '$binName' を持つ crate が workspace に 1 つだけ見つかりません (見つかった数: $($owners.Count))。Cargo.toml の [[bin]] を確認してください"
    }
    $binPackages[$binName] = $owners[0]
}
$Version = [string]$binPackages[$MainBinName].version
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    # R2 の保持ポリシーと immutable 判定は x.y.z を前提にするため、プレリリース表記などは止める。
    throw "配布できない版の形式です: '$Version' (x.y.z のみ)"
}
foreach ($binName in $BinNames) {
    # 別 crate に分かれていても、同じ版で揃っていなければ配布物の版表記が食い違うので止める。
    if ([string]$binPackages[$binName].version -ne $Version) {
        throw "bin '$binName' の crate の版 ($($binPackages[$binName].version)) が本体の版 ($Version) と一致しません"
    }
}
$BuildPackageNames = @($binPackages.Values | ForEach-Object { [string]$_.name } | Sort-Object -Unique)
$TargetDir = [string]$metadata.target_directory
Write-Host "バージョン: $Version"
Write-Host "同梱する bin: $($BinNames -join ', ') (crate: $($BuildPackageNames -join ', '))"

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
        throw "パッケージに同梱する文書がありません: $document"
    }
}

# vpk を固定版で用意する。ローカルで版が違えば入れ替え、最後に起動できて版が合うことを --help の表示で確かめる。
# CI は workflow 側で同じ版を導入済みなので、ここでは入れ替えない (照合だけが通る)。
$installedVpk = dotnet tool list --global |
    Select-String -Pattern '^vpk\s+' |
    ForEach-Object { ($_.Line -split '\s+')[1] } |
    Select-Object -First 1
if ($installedVpk -ne $VpkVersion) {
    if ($installedVpk) {
        Invoke-Native 'vpk の更新' { dotnet tool update --global vpk --version $VpkVersion }
    }
    else {
        Invoke-Native 'vpk のインストール' { dotnet tool install --global vpk --version $VpkVersion }
    }
}
# GitHub Actions では vpk が色付け用のエスケープシーケンスを出し、`\b` の照合が外れるため取り除いてから比べる。
$vpkHelp = (vpk --help 2>&1 | Out-String) -replace "`e\[[0-9;]*m", ''
if ($vpkHelp -notmatch "\bVelopack CLI $([regex]::Escape($VpkVersion))\b") {
    throw "Velopack CLI $VpkVersion を起動できません。dotnet tool install --global vpk --version $VpkVersion を確認してください"
}
Write-Host "vpk: $VpkVersion"

# 署名の準備 (Sigilune の場所とクラウドへのログインを、ビルド前に確認して早く落とす)。
# セッションは TEMP ではなく SIGILUNE_HOME (CI) / %LOCALAPPDATA%\Sigilune (ローカル) に保存されるため、
# vpk pack の間だけ TEMP を作業フォルダーへ切り替えても、vpk から起動される署名プロセスと共有される。
. (Join-Path $PSScriptRoot 'Sigilune.Common.ps1')
$Sigilune = Resolve-SigiluneExe
Connect-Sigilune -SigiluneExe $Sigilune
# vpk はファイルごとに別プロセスで署名テンプレートを実行する ({{file}} は 1 ファイルのパスに置き換わる)。
# --replace は既存の署名を置き換える (Sigilune は既定で署名済みファイルを拒否するため。Skilvella と同じ)。
# Sigilune は署名後に有効性・署名者・タイムスタンプを検証して失敗なら非0で終わり、vpk も失敗として止まる。
$SignTemplate = "`"$Sigilune`" sign --replace {{file}}"

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
    $bucketResponse = Invoke-RestMethod -Uri "https://api.cloudflare.com/client/v4/accounts/$AccountId/r2/buckets/$([uri]::EscapeDataString($Bucket))" `
        -Headers $cloudflareHeaders -TimeoutSec 30 -MaximumRedirection 0
    if (-not $bucketResponse.success -or $bucketResponse.result.name -cne $Bucket) {
        throw "Cloudflare R2 bucket '$Bucket' を確認できませんでした"
    }

    # <sub>.kagayoi.com → kagayoi.com (apex)。キャッシュパージに zone ID が要る。
    $zoneName = ([uri]$BaseUrl).Host -replace '^[^.]+\.', ''
    $zoneResponse = Invoke-RestMethod -Uri "https://api.cloudflare.com/client/v4/zones?name=$zoneName" `
        -Headers $cloudflareHeaders -TimeoutSec 30 -MaximumRedirection 0
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
New-Item -ItemType Directory -Path $PayloadDir -Force | Out-Null
New-Item -ItemType Directory -Path $VpkTempDir -Force | Out-Null
New-Item -ItemType Directory -Path $SignatureVerificationDir -Force | Out-Null

Write-Host "== cargo build: $Runtime ==" -ForegroundColor Cyan
# --locked: Cargo.lock を書き換えない (CI と同じ依存で作る)。toolchain は rust-toolchain.toml に従う。
# 同梱する bin だけを明示して作る (テスト用・開発用の bin が増えても配布物に混ぜない)。
$cargoBuildArguments = @('build', '--release', '--locked')
foreach ($packageName in $BuildPackageNames) { $cargoBuildArguments += @('-p', $packageName) }
foreach ($binName in $BinNames) { $cargoBuildArguments += @('--bin', $binName) }
Invoke-Native 'cargo build --release' { cargo @cargoBuildArguments }

# target/ の exe には直接署名しない (次の cargo build で上書き・再利用されるため)。payload へ写し、vpk の中で署名する。
foreach ($exeName in $PayloadExeNames) {
    $builtExe = Join-Path (Join-Path $TargetDir 'release') $exeName
    if (-not (Test-Path -LiteralPath $builtExe -PathType Leaf)) {
        throw "$exeName がビルド出力にありません: $builtExe"
    }
    Copy-Item -LiteralPath $builtExe -Destination (Join-Path $PayloadDir $exeName)
}
foreach ($document in $BundledDocuments) {
    Copy-Item -LiteralPath (Join-Path $RepoRoot $document) -Destination (Join-Path $PayloadDir (Split-Path $document -Leaf))
}

Write-Host "== vpk pack + 署名: v$Version ($Runtime) ==" -ForegroundColor Cyan
# オプションは vpk 1.2.161 の `vpk pack -H` (拡張ヘルプ) で確認したもの。
#   --shortcuts None … CLI なのでスタートメニュー・デスクトップのショートカットを作らない
#                      (既定は Desktop,StartMenuRoot。None で nuspec の shortcutLocations が None になる)
#   --delta None     … 毎回まっさらな出力先で作るため差分の元になる旧版が無い。明示して意図を残す
#   --signParallel 1 … 1 回の署名コマンドに渡すファイル数。{{file}} だけの単一ファイル用テンプレートでは
#                      vpk 自身がこの指定を無視し、もともと 1 ファイルずつ順に呼ぶ (ログに "ignoring --signParallel")。
#                      テンプレートを複数ファイル用に変えた場合でも直列のままにするため、明示して残す
#   --icon           … exe のアイコンが未整備のため付けない (vpk 既定のアイコンになる)
# VelopackApp の存在確認は、Rust の native exe では止まらないことを 1.2.161 で確かめたため --skipVeloAppCheck は付けない。
$packArguments = @(
    'pack',
    '--packId', $PackId,
    '--packVersion', $Version,
    '--packTitle', $PackTitle,
    '--packAuthors', $PackAuthors,
    '--mainExe', $MainExeName,
    '--packDir', $PayloadDir,
    '--outputDir', $ArtifactsDir,
    '--channel', $VelopackChannel,
    '--runtime', $Runtime,
    '--shortcuts', 'None',
    '--delta', 'None',
    '--signParallel', '1',
    '--signTemplate', $SignTemplate
)
$previousTemp = $env:TEMP
$previousTmp = $env:TMP
try {
    $env:TEMP = $VpkTempDir
    $env:TMP = $VpkTempDir
    Invoke-Native 'Velopack パッケージ作成と署名' { vpk @packArguments }
}
finally {
    $env:TEMP = $previousTemp
    $env:TMP = $previousTmp
}

# vpk 1.2.161 が出すファイル名。どれかが欠けていたらフィードと配布物が食い違うので止める。
$nupkgPath = Join-Path $ArtifactsDir "$PackId-$Version-full.nupkg"
$setupPath = Join-Path $ArtifactsDir "$PackId-$VelopackChannel-Setup.exe"
$portablePath = Join-Path $ArtifactsDir "$PackId-$VelopackChannel-Portable.zip"
$feedPath = Join-Path $ArtifactsDir "releases.$VelopackChannel.json"
foreach ($expected in @($nupkgPath, $setupPath, $portablePath, $feedPath)) {
    if (-not (Test-Path -LiteralPath $expected -PathType Leaf)) {
        throw "vpk pack の出力がありません: $([System.IO.Path]::GetFileName($expected))"
    }
}

# フィードが今回の版の full nupkg だけを指していることを確かめる (upload 後にクライアントが読む唯一の入口)。
$feed = Get-Content -LiteralPath $feedPath -Raw | ConvertFrom-Json
$feedAssets = @($feed.Assets)
$currentAsset = @($feedAssets | Where-Object { $_.FileName -eq [System.IO.Path]::GetFileName($nupkgPath) -and $_.Version -eq $Version -and $_.Type -eq 'Full' })
if ($currentAsset.Count -ne 1) {
    throw "releases.$VelopackChannel.json に v$Version の full nupkg がありません"
}
foreach ($asset in $feedAssets) {
    if (-not (Test-Path -LiteralPath (Join-Path $ArtifactsDir $asset.FileName) -PathType Leaf)) {
        throw "フィードが参照するファイルが成果物にありません: $($asset.FileName)"
    }
}

# 版付きのコピーを作る (固定名は「最新版」を常に同じ URL で指すため、版付きは過去版を名指しで取れるようにするため)。
$versionedSetupPath = Join-Path $ArtifactsDir "$PackId-$Version-$VelopackChannel-Setup.exe"
$versionedPortablePath = Join-Path $ArtifactsDir "$PackId-$Version-$VelopackChannel-Portable.zip"
Copy-Item -LiteralPath $setupPath -Destination $versionedSetupPath
Copy-Item -LiteralPath $portablePath -Destination $versionedPortablePath

Write-Host '== Authenticode 署名検証 ==' -ForegroundColor Cyan
# Setup.exe (固定名・版付き)。vpk は nupkg を埋め込んだ後に Setup.exe 自体も署名する。
foreach ($executable in Get-ChildItem -LiteralPath $ArtifactsDir -Filter '*.exe' -File) {
    Test-Signature -Path $executable.FullName
}
# nupkg (自動更新で展開される中身)。lib/app/ に payload の exe と Squirrel.exe (= Update.exe)・実行スタブが入る。
Test-ArchiveExecutableSignatures -ArchivePath $nupkgPath `
    -RequiredEntries (@($PayloadExeNames | ForEach-Object { "lib/app/$_" }) + 'lib/app/Squirrel.exe')
# Portable.zip。直下に Update.exe と実行スタブ (Edictora.exe)、current/ に payload が入る。
Test-ArchiveExecutableSignatures -ArchivePath $portablePath `
    -RequiredEntries (@($PayloadExeNames | ForEach-Object { "current/$_" }) + 'Update.exe')
Assert-SameFile -Expected $setupPath -Actual $versionedSetupPath
Assert-SameFile -Expected $portablePath -Actual $versionedPortablePath

if ($SkipUpload) {
    Write-Host "`n✅ -SkipUpload 指定のため公開せず終了します: $ArtifactsDir" -ForegroundColor Green
    Get-ChildItem -LiteralPath $ArtifactsDir -File |
        Select-Object Name, @{ Name = 'Size(MB)'; Expression = { [math]::Round($_.Length / 1MB, 1) } } |
        Format-Table -AutoSize
    return
}

# アップロード順:
#   1. nupkg (フィードが指す本体)
#   2. 版付き Setup / Portable
#   3. 固定名 Setup / Portable
#   4. assets.*.json → RELEASES (旧形式のフィード) → releases.*.json (クライアントが読むフィード) を最後に置き換える
# フィードを先に置くと、まだ upload していない nupkg をクライアントが取りに行く時間ができてしまう。
$allFiles = @(Get-ChildItem -LiteralPath $ArtifactsDir -File)
$feedFiles = @($allFiles |
    Where-Object { $_.Name -like 'releases.*.json' -or $_.Name -like 'assets.*.json' -or $_.Name -like 'RELEASES*' } |
    Sort-Object @{ Expression = { if ($_.Name -like 'releases.*.json') { 3 } elseif ($_.Name -like 'RELEASES*') { 2 } else { 1 } } }, Name)
$packageFiles = @($allFiles |
    Where-Object { $feedFiles -notcontains $_ } |
    Sort-Object @{ Expression = { if ($_.Extension -eq '.nupkg') { 0 } elseif (Test-ImmutableReleaseFile -File $_) { 1 } else { 2 } } }, Name)
if ($packageFiles.Count -eq 0 -or @($feedFiles | Where-Object { $_.Name -like 'releases.*.json' }).Count -eq 0) {
    throw 'R2 へアップロードする成果物が揃っていません'
}

Write-Host '== R2 アップロード: パッケージ ==' -ForegroundColor Cyan
foreach ($file in $packageFiles) {
    Send-R2Object -File $file
}

Write-Host '== R2 アップロード: フィード (最後) ==' -ForegroundColor Cyan
foreach ($file in $feedFiles) {
    Send-R2Object -File $file
}

# 固定 URL (フィード・固定名 Setup / Portable) が旧キャッシュを返さないよう purge する。
# 版付きファイルと nupkg は URL が一意 (immutable) なので不要。
Write-Host '== Cloudflare キャッシュパージ ==' -ForegroundColor Cyan
$purgeUrls = @($allFiles | Where-Object { -not (Test-ImmutableReleaseFile -File $_) } | ForEach-Object { "$BaseUrl/$([uri]::EscapeDataString($_.Name))" })
if ($purgeUrls.Count -gt 0) {
    try {
        for ($offset = 0; $offset -lt $purgeUrls.Count; $offset += 30) {
            $last = [math]::Min($offset + 29, $purgeUrls.Count - 1)
            $batch = @($purgeUrls[$offset..$last])
            $body = @{ files = $batch } | ConvertTo-Json -Depth 3 -Compress
            $purgeResponse = Invoke-RestMethod -Method Post `
                -Uri "https://api.cloudflare.com/client/v4/zones/$zoneId/purge_cache" `
                -Headers $cloudflareHeaders -ContentType 'application/json' -Body $body -TimeoutSec 30 -MaximumRedirection 0
            if (-not $purgeResponse.success) {
                throw ($purgeResponse.errors | ConvertTo-Json -Compress)
            }
        }
        Write-Host "  ✅ $($purgeUrls.Count) URL"
    }
    catch {
        # 配信自体は完了している。固定名ファイルは no-cache で配っているため、パージ失敗は警告にとどめる。
        Write-Warning "R2 アップロードは完了しましたが、キャッシュパージに失敗しました: $($_.Exception.Message)"
    }
}

if ($Cleanup) {
    Write-Host '== 古い版のクリーンアップ ==' -ForegroundColor Cyan
    # 今回 upload したファイルと、今回のフィード (releases.*.json) が参照するファイルは必ず保持する
    # (フィードが指す nupkg を消すと自動更新が壊れる)。
    $keep = @{}
    foreach ($file in $allFiles) {
        $keep[$file.Name] = $true
    }
    foreach ($manifest in @($feedFiles | Where-Object { $_.Name -like 'releases.*.json' })) {
        foreach ($asset in @((Get-Content -LiteralPath $manifest.FullName -Raw | ConvertFrom-Json).Assets)) {
            if ($asset.FileName) { $keep[[string]$asset.FileName] = $true }
        }
    }
    if ($keep.Count -eq 0) {
        throw '保持対象が空のため cleanup を中止します'
    }

    $api = "https://api.cloudflare.com/client/v4/accounts/$AccountId/r2/buckets/$([uri]::EscapeDataString($Bucket))"
    $allKeys = [System.Collections.Generic.List[string]]::new()
    $cursor = ''
    while ($true) {
        $uri = "$api/objects?per_page=1000" + $(if ($cursor) { "&cursor=$([uri]::EscapeDataString($cursor))" })
        $response = Invoke-RestMethod -Uri $uri -Headers $cloudflareHeaders -TimeoutSec 30 -MaximumRedirection 0
        foreach ($object in @($response.result)) {
            $allKeys.Add($object.key)
        }

        # 全件 1 ページに収まると result_info が省略される (StrictMode 下では直接参照が throw)。
        $info = $response.PSObject.Properties['result_info']
        if (-not $info -or -not $info.Value) { break }
        $isTruncated = $info.Value.PSObject.Properties['is_truncated']
        if (-not $isTruncated -or -not $isTruncated.Value) { break }
        $cursorProperty = $info.Value.PSObject.Properties['cursor']
        $cursor = if ($cursorProperty) { $cursorProperty.Value } else { '' }
        if (-not $cursor) { break }
    }

    # 全プロジェクト共通の保持ポリシー: 直近 2 バージョン。
    # 固定ファイル名 (Edictora-win-Setup.exe・Edictora-win-Portable.zip・フィード) はバージョン文字列を含まないので
    # 対象外 = 毎リリース上書きで保持。
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
        # 今回 upload したファイルとフィードが参照するファイルは絶対保持
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
            Invoke-RestMethod -Method Delete -Uri "$api/objects/$encodedKey" -Headers $cloudflareHeaders -TimeoutSec 30 -MaximumRedirection 0 | Out-Null
            Write-Host "  🗑️  $key"
        }
    }
}
else {
    Write-Host '== cleanup ==' -ForegroundColor Cyan
    Write-Host '  省略（古い版を消すリリースで -Cleanup を明示してください）'
}

Write-Host "`n🎉 リリース完了: v$Version → $BaseUrl" -ForegroundColor Green
