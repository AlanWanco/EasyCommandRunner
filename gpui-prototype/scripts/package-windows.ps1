param(
    [Parameter(Mandatory = $true)][ValidateSet('x86_64', 'arm64')][string]$Arch,
    [string]$Binary = 'target/release/easy-command-runner-gpui-prototype.exe',
    [string]$Dist = 'dist'
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'Native Windows packaging is required' }
$nativeArch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
$expectedArch = if ($Arch -eq 'arm64') { 'Arm64' } else { 'X64' }
if ($nativeArch -ne $expectedArch) { throw "Native $Arch Windows runner is required, got $nativeArch" }
Set-Location (Join-Path $PSScriptRoot '..')
if (-not $env:RELEASE_VERSION -or -not $env:SOURCE_SHA) { throw 'RELEASE_VERSION and SOURCE_SHA are required' }
$exe = (Resolve-Path -LiteralPath $Binary).Path
& pwsh -NoProfile -File scripts/check-windows-pe.ps1 -Path $exe -Arch $Arch
if ($LASTEXITCODE -ne 0) { throw 'Release PE verification failed' }
$name = & python scripts/release_support.py name --version $env:RELEASE_VERSION --platform windows --arch $Arch
if ($LASTEXITCODE -ne 0) { throw 'Invalid package name/version' }
New-Item -ItemType Directory -Path $Dist -Force | Out-Null
$distPath = (Resolve-Path $Dist).Path
$work = Join-Path ([System.IO.Path]::GetTempPath()) ('ecr-gpui-zip-' + [guid]::NewGuid())
$stage = Join-Path $work 'EasyCommandRunner'
try {
    & python scripts/release_support.py prepare --version $env:RELEASE_VERSION --platform windows --arch $Arch `
        --source-sha $env:SOURCE_SHA --binary $exe --output $stage
    if ($LASTEXITCODE -ne 0) { throw 'Staging failed' }
    Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'EasyCommandRunner.exe')
    # Copy any imported MSVC redistributable DLLs, never random DLLs from the host.
    # The built-in UCRT / Windows system DLLs remain OS requirements.
    if ($IsWindows) {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if (-not (Test-Path $vswhere)) { throw 'Cannot locate Visual Studio tools' }
        $vs = & $vswhere -latest -products '*' -property installationPath
        if ($LASTEXITCODE -ne 0 -or -not $vs) { throw 'Visual Studio lookup failed' }
        $folderArch = if ($Arch -eq 'arm64') { 'arm64' } else { 'x64' }
        $dumpbin = Get-ChildItem -Path (Join-Path $vs 'VC/Tools/MSVC') -Filter dumpbin.exe -Recurse |
            Where-Object { $_.FullName -match "[/\\]$folderArch[/\\]dumpbin.exe$" } |
            Sort-Object FullName -Descending | Select-Object -First 1
        if (-not $dumpbin) { throw 'Matching architecture dumpbin is missing' }
        $imports = & $dumpbin.FullName /DEPENDENTS $exe
        if ($LASTEXITCODE -ne 0) { throw 'Cannot audit executable imports' }
        $runtime = @($imports | Select-String -Pattern '^\s*((?:vcruntime|msvcp|concrt|vcomp)\d+[^\s]*\.dll)\s*$' |
            ForEach-Object { $_.Matches[0].Groups[1].Value })
        foreach ($dll in $runtime) {
            $candidate = Get-ChildItem -Path (Join-Path $vs 'VC/Redist/MSVC') -Filter $dll -Recurse |
                Where-Object { $_.FullName -match "[/\\]$folderArch[/\\]" } |
                Sort-Object FullName -Descending | Select-Object -First 1
            if (-not $candidate) { throw "Required $Arch runtime is missing: $dll" }
            Copy-Item -LiteralPath $candidate.FullName -Destination $stage
        }
    }
    $zip = Join-Path $distPath $name
    Compress-Archive -Path $stage -DestinationPath $zip -Force
    $check = Join-Path $work 'extracted'
    Expand-Archive -LiteralPath $zip -DestinationPath $check
    $packagedExe = Join-Path $check 'EasyCommandRunner/EasyCommandRunner.exe'
    if (-not (Test-Path $packagedExe)) { throw 'Portable ZIP is missing the main executable' }
    if ((Get-FileHash $packagedExe -Algorithm SHA256).Hash -ne (Get-FileHash $exe -Algorithm SHA256).Hash) {
        throw 'Portable executable differs from the verified release binary'
    }
    foreach ($file in @('LICENSE', 'README-runtime.md', 'build-info.json', 'dependency-notices.json',
                         'licenses/LICENSE-LUCIDE', 'licenses/LICENSE-DEVICON') ) {
        if (-not (Test-Path (Join-Path $check "EasyCommandRunner/$file"))) { throw "Missing ZIP entry: $file" }
    }
    foreach ($dll in $runtime) {
        $packedDll = Join-Path $check "EasyCommandRunner/$dll"
        if (-not (Test-Path $packedDll)) { throw "Missing ZIP runtime: $dll" }
        if ((Get-FileHash $packedDll -Algorithm SHA256).Hash -ne (Get-FileHash (Join-Path $stage $dll) -Algorithm SHA256).Hash) {
            throw "Packaged runtime differs from staging: $dll"
        }
    }
    & python scripts/release_support.py stamp --version $env:RELEASE_VERSION --package $zip `
        --info (Join-Path $stage 'build-info.json')
    if ($LASTEXITCODE -ne 0) { throw 'Cannot record package provenance' }
    Write-Host "Verified Portable ZIP: $zip"
} finally {
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
}
