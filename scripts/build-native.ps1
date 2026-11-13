[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT' -or [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
    throw 'This native preview currently builds for Windows x64 only.'
}
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repositoryRoot 'native\desktop\Cargo.toml'
function Invoke-CheckedCargo([string[]] $CargoArguments) {
    & cargo @CargoArguments
    if ($LASTEXITCODE -ne 0) { throw "Cargo failed ($LASTEXITCODE): $($CargoArguments -join ' ')" }
}
Invoke-CheckedCargo @('fmt', '--manifest-path', $manifestPath, '--', '--check')
Invoke-CheckedCargo @('test', '--locked', '--manifest-path', $manifestPath)
Invoke-CheckedCargo @('clippy', '--locked', '--manifest-path', $manifestPath, '--all-targets', '--', '-D', 'warnings')
& python (Join-Path $repositoryRoot 'tests\native\test_runtime_probe.py')
if ($LASTEXITCODE -ne 0) { throw 'Native runtime preflight tests failed.' }
& python (Join-Path $repositoryRoot 'tests\native\test_model_download.py')
if ($LASTEXITCODE -ne 0) { throw 'Native model fetcher tests failed.' }
Invoke-CheckedCargo @('build', '--release', '--locked', '--manifest-path', $manifestPath)
$buildStamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$packageName = "voice-studio-native-preview-$buildStamp-windows-x64"
$packageRoot = Join-Path $repositoryRoot "dist\$packageName"
New-Item -ItemType Directory -Path $packageRoot -ErrorAction Stop | Out-Null
$executablePath = Join-Path $repositoryRoot 'native\desktop\target\release\voice-studio-native.exe'
Copy-Item -LiteralPath $executablePath -Destination $packageRoot
Copy-Item -LiteralPath (Join-Path $repositoryRoot 'native\desktop\README.md') -Destination (Join-Path $packageRoot 'README.md')
foreach ($licenseName in @('LICENSE', 'LICENSE-NOTICE.md')) {
    Copy-Item -LiteralPath (Join-Path $repositoryRoot $licenseName) -Destination $packageRoot
}
$runtimeSource = Join-Path $repositoryRoot 'native\desktop\target\runtime\omnivoice-cpu-040c8b344d8c'
$runtimeIncluded = Test-Path -LiteralPath (Join-Path $runtimeSource 'engine.json')
if ($runtimeIncluded) {
    & $executablePath --engine-probe $runtimeSource | Out-String | Write-Output
    if ($LASTEXITCODE -ne 0) { throw 'Native engine failed preflight; refusing to package it.' }
    $runtimeDestination = Join-Path $packageRoot 'runtime\omnivoice-cpu'
    New-Item -ItemType Directory -Path $runtimeDestination | Out-Null
    foreach ($runtimeFile in @('engine.json','omnivoice-tts.exe','LICENSE-omnivoice.txt','LICENSE-ggml.txt')) {
        Copy-Item -LiteralPath (Join-Path $runtimeSource $runtimeFile) -Destination $runtimeDestination
    }
}
$gitExecutable = (Get-Command git -ErrorAction Stop).Source
$directGit = Join-Path (Split-Path (Split-Path $gitExecutable -Parent) -Parent) 'mingw64\bin\git.exe'
if (Test-Path -LiteralPath $directGit) { $gitExecutable = $directGit }
$sourceCommit = & $gitExecutable -C $repositoryRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0) { throw 'Cannot record source revision.' }
$sourceStatus = & $gitExecutable -C $repositoryRoot status --porcelain
if ($LASTEXITCODE -ne 0) { throw 'Cannot record source working-tree state.' }
$sourceDirty = [bool]$sourceStatus
$fileManifest = @(Get-ChildItem -LiteralPath $packageRoot -File -Recurse | ForEach-Object {
    [ordered]@{ name = $_.FullName.Substring($packageRoot.Length + 1).Replace('\','/'); bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
})
[ordered]@{
    product = 'Voice Studio Native development preview'
    source_commit = $sourceCommit.Trim()
    source_dirty = $sourceDirty
    built_utc = (Get-Date).ToUniversalTime().ToString('o')
    renderer = 'Windows standard controls / GDI'
    inference_ready = $false
    native_cpu_runtime_included = $runtimeIncluded
    files = $fileManifest
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $packageRoot 'build-manifest.json') -Encoding utf8
$archivePath = "$packageRoot.zip"
Compress-Archive -LiteralPath $packageRoot -DestinationPath $archivePath -CompressionLevel Optimal
Write-Output "Portable preview: $archivePath"
Get-FileHash -LiteralPath $archivePath -Algorithm SHA256
