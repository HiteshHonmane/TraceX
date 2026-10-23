[CmdletBinding()]
param([switch]$PublicDevelopmentPreview)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repositoryRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repositoryRoot
try {
    function Invoke-Text([string]$Program, [string[]]$Arguments) {
        $result = & $Program @Arguments
        if ($LASTEXITCODE -ne 0) { throw "$Program failed: $LASTEXITCODE" }
        return ($result -join "`n")
    }
    function Get-TextHash([string]$Value) {
        $algorithm = [Security.Cryptography.SHA256]::Create()
        try { return ([BitConverter]::ToString($algorithm.ComputeHash([Text.Encoding]::UTF8.GetBytes($Value)))).Replace('-', '').ToLowerInvariant() }
        finally { $algorithm.Dispose() }
    }
    function Get-InputHash {
        $paths = (Invoke-Text 'git' @('ls-files', '--cached', '--others', '--exclude-standard')) -split "`n"
        $records = foreach ($relative in ($paths | Sort-Object -Unique)) {
            if ($relative -match '(^src/|\.rs$|^Cargo\.(toml|lock)$|^\.cargo/|^rust-toolchain(\.toml)?$)') {
                if (Test-Path -LiteralPath $relative -PathType Leaf) { "$relative $((Get-FileHash -LiteralPath $relative -Algorithm SHA256).Hash)" }
                else { "$relative DELETED" }
            }
        }
        return Get-TextHash ($records -join "`n")
    }
    $commit = Invoke-Text 'git' @('rev-parse', 'HEAD')
    $dirty = -not [string]::IsNullOrWhiteSpace((Invoke-Text 'git' @('status', '--porcelain', '--untracked-files=all')))
    $diff = Invoke-Text 'git' @('diff', '--binary', 'HEAD', '--')
    $untracked = foreach ($relative in ((Invoke-Text 'git' @('ls-files', '--others', '--exclude-standard')) -split "`n" | Sort-Object)) {
        if ($relative -and (Test-Path -LiteralPath $relative -PathType Leaf)) { "$relative $((Get-FileHash -LiteralPath $relative -Algorithm SHA256).Hash)" }
    }
    $diffHash = Get-TextHash ($diff + "`nUNTRACKED`n" + ($untracked -join "`n"))
    $inputHash = Get-InputHash
    $rustInfo = Invoke-Text 'rustc' @('-vV')
    $cargoInfo = Invoke-Text 'cargo' @('--version')
    $hostTarget = [regex]::Match($rustInfo, '(?m)^host: (.+)$').Groups[1].Value.Trim()
    if ($hostTarget -notmatch 'windows') { throw 'Windows host required.' }
    if (!(Test-Path -LiteralPath 'Cargo.lock')) { throw 'Cargo.lock required.' }
    $distribution = 'private-unsigned-development-preview'
    $packageLabel = 'private'
    if ($PublicDevelopmentPreview) {
        foreach ($required in @('BINARY-LICENSE.txt', 'THIRD-PARTY-NOTICES.txt', 'THIRD-PARTY-INVENTORY.json', 'THIRD-PARTY-RUST-RUNTIME.json')) {
            if (!(Test-Path -LiteralPath (Join-Path 'distribution' $required) -PathType Leaf)) { throw "Public development preview requires distribution/$required." }
        }
        $inventory = Get-Content -Raw -LiteralPath 'distribution/THIRD-PARTY-INVENTORY.json' | ConvertFrom-Json
        $runtime = Get-Content -Raw -LiteralPath 'distribution/THIRD-PARTY-RUST-RUNTIME.json' | ConvertFrom-Json
        if ($runtime.rustc -ne $rustInfo) { throw 'Rust runtime notices are stale for the current compiler.' }
        foreach ($notice in $runtime.files.PSObject.Properties) {
            $runtimePath = Join-Path 'distribution/THIRD-PARTY-RUST-RUNTIME' $notice.Name
            if (!(Test-Path -LiteralPath $runtimePath) -or (Get-FileHash -LiteralPath $runtimePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $notice.Value) { throw 'Missing or altered Rust runtime notice.' }
        }
        if ($inventory.cargo_lock_sha256 -ne (Get-FileHash Cargo.lock -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Third-party inventory is stale for Cargo.lock.' }
        foreach ($dependency in $inventory.packages) {
            $noticeRoot = Join-Path 'distribution/THIRD-PARTY-LICENSES' ($dependency.name + '-' + $dependency.version)
            if (!(Test-Path -LiteralPath $noticeRoot -PathType Container)) { throw "Missing third-party notices: $($dependency.name)" }
            foreach ($notice in $dependency.notice_sha256.PSObject.Properties) {
                $noticePath = Join-Path $noticeRoot $notice.Name
                if (!(Test-Path -LiteralPath $noticePath) -or (Get-FileHash -LiteralPath $noticePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $notice.Value) { throw "Missing or altered notice: $($dependency.name)/$($notice.Name)" }
            }
            if ($dependency.source_archive_bundled) {
                $sourceArchive = Join-Path 'distribution/THIRD-PARTY-SOURCES' ($dependency.name + '-' + $dependency.version + '.crate')
                if (!(Test-Path -LiteralPath $sourceArchive) -or (Get-FileHash -LiteralPath $sourceArchive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $dependency.crate_sha256) { throw "Missing or altered source archive: $($dependency.name)" }
            }
        }
        $distribution = 'public-unsigned-development-preview'
        $packageLabel = 'development-preview'
    }
    $buildStart = [DateTime]::UtcNow.ToString('o')
    # Always ask Cargo to build: no option to package an unchecked stale binary.
    $messages = & cargo build --release --locked --message-format=json
    if ($LASTEXITCODE -ne 0) { throw 'Locked release build failed.' }
    $artifact = $null
    foreach ($line in $messages) {
        $message = $line | ConvertFrom-Json
        if ($message.reason -eq 'compiler-artifact' -and $message.target.name -eq 'tracex' -and $message.executable) { $artifact = $message }
    }
    if (!$artifact -or !(Test-Path -LiteralPath $artifact.executable)) { throw 'Cargo executable artifact missing.' }
    if ((Get-InputHash) -ne $inputHash -or (Invoke-Text 'git' @('rev-parse', 'HEAD')) -ne $commit) { throw 'Build inputs changed during build; retry after source edits finish.' }
    $metadata = (Invoke-Text 'cargo' @('metadata', '--no-deps', '--format-version', '1', '--locked')) | ConvertFrom-Json
    $package = $metadata.packages | Where-Object { $_.id -eq $artifact.package_id }
    if (!$package) { throw 'Package version unavailable.' }
    $version = $package.version
    $executable = $artifact.executable
    $binaryHash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant()
    $bytes = [IO.File]::ReadAllBytes($executable)
    $machine = [BitConverter]::ToUInt16($bytes, [BitConverter]::ToInt32($bytes, 0x3c) + 4)
    $architecture = switch ($machine) { 0x8664 { 'x86_64' }; 0xaa64 { 'aarch64' }; 0x014c { 'x86' }; default { throw 'Unsupported PE architecture.' } }
    if ($PublicDevelopmentPreview -and $architecture -ne 'x86_64') { throw 'Current dependency inventory qualifies only Windows x86_64.' }
    $runId = [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffZ') + '-' + [Guid]::NewGuid().ToString('N').Substring(0, 8)
    $runRoot = Join-Path $repositoryRoot "target/preview/$runId"
    $stage = Join-Path $runRoot 'package'
    $verification = Join-Path $runRoot 'verification-extract'
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    Copy-Item -LiteralPath $executable -Destination (Join-Path $stage 'tracex.exe')
    $readme = @'
TraceX PRIVATE DEVELOPMENT PREVIEW

Unsigned internal development artifact, not a public production release.
This private package supplies no redistribution grant or third-party bundle.
Use the explicit public development preview mode for the complete license
and source notice bundle. Keep this package private.

Extract before running tracex.exe. No installer, updater, release signing,
hardware qualification or compatibility certification is provided.
SHA-256 detects changes; it does not authenticate the publisher.
Runtime prerequisite: Microsoft Visual C++ v14 x64 runtime, version 14.44 or
newer for this build. The executable imports VCRUNTIME140.dll. Install only
from Microsoft's official source if the runtime is missing:
https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/
https://aka.ms/vc14/vc_redist.x64.exe
This ZIP does not include or install the runtime.

Known limits: monitor tabs share capture/connection. Pause can omit in-memory
rows. Retention and ingestion queues remain unbounded; memory can be exhausted.
Export and some I/O may block the interface. Auto-save is not a crash-recovery
or durable-storage guarantee. Abrupt termination, disk failure or overload can
lose logs. Keep independent copies of important evidence. This preview offers
no reliability, fitness or data-preservation guarantee.

manifest.json identifies the base commit, dirty state, source-diff fingerprint,
local build-input fingerprint, tool versions and binary hash. A dirty binary
is not an artifact of the base commit alone. No source diff is included.
SHA256SUMS.txt verifies payload files; the adjacent .zip.sha256 verifies the ZIP.
Packaging does not launch the application or upload anything.
'@
    [IO.File]::WriteAllText((Join-Path $stage 'README.txt'), $readme)
    if ($PublicDevelopmentPreview) {
        $readme = $readme.Replace('TraceX PRIVATE DEVELOPMENT PREVIEW', 'TraceX PUBLIC DEVELOPMENT PREVIEW').Replace('Unsigned internal development artifact, not a public production release.', 'Unsigned development preview, not a production release.')
        $readme = [regex]::Replace($readme, '(?s)This private package supplies no redistribution grant.*?Keep this package private\.', "See BINARY-LICENSE.txt for TraceX binary permissions and limits. Third-party`nrights remain under their own licenses; see THIRD-PARTY/THIRD-PARTY-NOTICES.txt.`nMPL source archives and license texts are included in THIRD-PARTY/.")
        [IO.File]::WriteAllText((Join-Path $stage 'README.txt'), $readme)
        Copy-Item -LiteralPath 'distribution/BINARY-LICENSE.txt' -Destination $stage
        $thirdParty = Join-Path $stage 'THIRD-PARTY'
        New-Item -ItemType Directory -Path $thirdParty | Out-Null
        Get-ChildItem -LiteralPath 'distribution' -Filter 'THIRD-PARTY*' | ForEach-Object { Copy-Item -LiteralPath $_.FullName -Destination $thirdParty -Recurse }
    }
    $manifest = [ordered]@{
        product = 'TraceX'; distribution = $distribution; version = $version
        architecture = $architecture; source_commit = $commit; source_dirty = $dirty
        source_diff_sha256 = $diffHash; source_build_inputs_sha256 = $inputHash
        provenance_note = 'Base commit plus local modifications. Fingerprints identify local inputs; no clean-commit or bit-for-bit reproducibility certification.'
        build_started_utc = $buildStart; packaged_utc = [DateTime]::UtcNow.ToString('o')
        command = 'cargo build --release --locked --message-format=json'; profile = 'release'
        rustc = $rustInfo; cargo = $cargoInfo; host_target = $hostTarget
        cargo_lock_sha256 = (Get-FileHash -LiteralPath 'Cargo.lock' -Algorithm SHA256).Hash.ToLowerInvariant()
        executable_sha256 = $binaryHash; signed = $false
        project_license = $(if ($PublicDevelopmentPreview) { 'See BINARY-LICENSE.txt; separate third-party terms prevail' } else { 'private evaluation; no redistribution grant in this package' })
    }
    [IO.File]::WriteAllText((Join-Path $stage 'manifest.json'), ($manifest | ConvertTo-Json -Depth 5))
    $payload = @(Get-ChildItem -LiteralPath $stage -Recurse -File | ForEach-Object { $_.FullName.Substring($stage.Length + 1).Replace('\', '/') } | Sort-Object)
    $checksums = foreach ($name in $payload) { "$((Get-FileHash -LiteralPath (Join-Path $stage $name) -Algorithm SHA256).Hash.ToLowerInvariant())  $name" }
    [IO.File]::WriteAllText((Join-Path $stage 'SHA256SUMS.txt'), ($checksums -join "`n"))
    $archive = Join-Path $runRoot "TraceX-$version-windows-$architecture-$packageLabel.zip"
    $archiveInputs = @(Get-ChildItem -LiteralPath $stage | ForEach-Object { $_.FullName })
    Compress-Archive -LiteralPath $archiveInputs -DestinationPath $archive
    Expand-Archive -LiteralPath $archive -DestinationPath $verification
    $expected = @($payload + 'SHA256SUMS.txt') | Sort-Object
    $actual = @(Get-ChildItem -LiteralPath $verification -Recurse -File | ForEach-Object { $_.FullName.Substring($verification.Length + 1).Replace('\', '/') }) | Sort-Object
    if (Compare-Object $expected $actual) { throw 'Unexpected archive contents.' }
    foreach ($checksum in (Get-Content -LiteralPath (Join-Path $verification 'SHA256SUMS.txt'))) {
        $parts = $checksum -split '  ', 2
        if ((Get-FileHash -LiteralPath (Join-Path $verification $parts[1]) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $parts[0]) { throw 'Extracted payload checksum mismatch.' }
    }
    if ((Get-FileHash -LiteralPath (Join-Path $verification 'tracex.exe') -Algorithm SHA256).Hash.ToLowerInvariant() -ne $binaryHash) { throw 'Extracted executable differs from Cargo artifact.' }
    $archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(($archive + '.sha256'), "$archiveHash  $([IO.Path]::GetFileName($archive))`n")
    Write-Output "Validated $distribution artifact: $archive"
    Write-Output "ZIP SHA256: $archiveHash"
    Write-Output "Source dirty: $dirty; architecture: $architecture; version: $version"
} finally { Pop-Location }
