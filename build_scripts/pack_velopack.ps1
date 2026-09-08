[CmdletBinding()]
param(
    # Expose the very same allowlist used by staging, without building or packaging.
    [switch] $ListDataFiles
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$dataFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $repo 'data') -File |
        Where-Object { $_.Name -clike 'sweep*.wav' -or $_.Name -clike 'harman*.csv' } |
        Sort-Object Name
)
if ($dataFiles.Count -eq 0) { throw 'No bundled sweep or Harman data found.' }
if ($ListDataFiles) {
    ConvertTo-Json -InputObject @($dataFiles.Name) -Compress
    return
}
if (-not $IsWindows) { throw 'Velopack Windows packaging requires Windows.' }

$manifest = Get-Content -LiteralPath (Join-Path $repo 'Cargo.toml') -Raw
$workspace = [regex]::Match($manifest, '(?ms)^\[workspace\.package\]\s*\r?\n(?<body>.*?)(?=^\[|\z)')
$versionMatch = [regex]::Match($workspace.Groups['body'].Value, '(?m)^version\s*=\s*"(?<version>[^"]+)"')
if (-not $versionMatch.Success) { throw 'Missing workspace.package.version in Cargo.toml.' }
$version = $versionMatch.Groups['version'].Value

$vpkCommand = Get-Command vpk -ErrorAction SilentlyContinue
$vpk = if ($vpkCommand) { $vpkCommand.Source } else { Join-Path $HOME '.dotnet/tools/vpk.exe' }
if (-not (Test-Path -LiteralPath $vpk -PathType Leaf)) {
    & dotnet tool install -g vpk --add-source https://api.nuget.org/v3/index.json
    if ($LASTEXITCODE -ne 0) { throw "dotnet tool install failed: $LASTEXITCODE" }
    if (-not (Test-Path -LiteralPath $vpk -PathType Leaf)) { throw 'vpk executable not found after installation.' }
}

$target = Join-Path $repo 'target'
Push-Location $repo
try {
    & cargo build -p impulcifer-app --release --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed: $LASTEXITCODE" }
} finally {
    Pop-Location
}

# Never reuse or clear a caller-owned directory. Keep each run for inspection.
$run = Join-Path $target ('p21/pack-' + [guid]::NewGuid().ToString('N'))
$stage = Join-Path $run 'stage'
$out = Join-Path $run 'releases'
$null = New-Item -ItemType Directory -Path (Join-Path $stage 'data'), $out
Copy-Item -LiteralPath (Join-Path $target 'release/impulcifer-app.exe') -Destination $stage
foreach ($file in $dataFiles) {
    Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $stage 'data')
}
Write-Output "Workspace version: $version"
Write-Output "Stage: $stage"
Write-Output "Output: $out"
Write-Output 'No application install, apply or uninstall will run.'

& $vpk pack --packId Impulcifer --packTitle 'Modern Impulcifer' `
    --packVersion $version --packDir $stage --mainExe impulcifer-app.exe `
    --icon (Join-Path $repo 'logo/pulse.ico') --shortcuts Desktop,StartMenuRoot `
    --outputDir $out --channel win
if ($LASTEXITCODE -ne 0) { throw "vpk pack failed: $LASTEXITCODE" }

foreach ($name in @('Impulcifer-win-Setup.exe', 'releases.win.json', 'RELEASES')) {
    if (-not (Test-Path -LiteralPath (Join-Path $out $name) -PathType Leaf)) {
        throw "Missing packaging output: $name"
    }
}
$fullPackages = @(Get-ChildItem -LiteralPath $out -File -Filter '*-full.nupkg')
if ($fullPackages.Count -ne 1) { throw 'Expected exactly one full nupkg in the unique output directory.' }
Get-ChildItem -LiteralPath $out -File | Sort-Object Name | ForEach-Object {
    Write-Output ('Artifact: {0} ({1} bytes)' -f $_.FullName, $_.Length)
}
$result = @{ version = $version; stage = $stage; output = $out; data_files = @($dataFiles.Name) }
Write-Output ('P21_RESULT=' + (ConvertTo-Json -InputObject $result -Compress))
