[CmdletBinding()]
param([switch] $PackageExisting)
$ErrorActionPreference = 'Stop'
$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$targetRoot = Join-Path $repositoryRoot 'native\desktop\target'
$sourceRoot = Join-Path $targetRoot 'engine-source\omnivoice-cpp'
$sourceCommit = '040c8b344d8c670ce1475194751d119b5ef82c78'
$ggmlCommit = 'd5152bf4348453068aa8de0d479caef40a613b9d'
$git = (Get-Command git -ErrorAction Stop).Source
$directGit = Join-Path (Split-Path (Split-Path $git -Parent) -Parent) 'mingw64\bin\git.exe'
if (Test-Path -LiteralPath $directGit) { $git = $directGit }
$previousPath = $env:PATH
function Invoke-Git([string[]] $GitArguments) {
    $result = & $git @GitArguments
    if ($LASTEXITCODE -ne 0) { throw 'Pinned inference source operation failed.' }
    return $result
}
try {
    $env:PATH = (Split-Path $git -Parent) + ';' + $env:PATH
    if (-not (Test-Path -LiteralPath $sourceRoot)) {
        New-Item -ItemType Directory -Path $sourceRoot | Out-Null
        Invoke-Git @('init', '-q', $sourceRoot)
        Invoke-Git @('-C', $sourceRoot, 'remote', 'add', 'origin', 'https://github.com/ServeurpersoCom/omnivoice.cpp.git')
        Invoke-Git @('-c', 'http.https://github.com/.sslBackend=schannel', '-C', $sourceRoot, 'fetch', '--depth', '1', 'origin', $sourceCommit)
        Invoke-Git @('-C', $sourceRoot, 'checkout', '-q', '--detach', 'FETCH_HEAD')
        Invoke-Git @('-c', 'http.https://github.com/.sslBackend=schannel', '-C', $sourceRoot, 'submodule', 'update', '--init', '--recursive', '--depth', '1')
    }
    if ((Invoke-Git @('-C', $sourceRoot, 'rev-parse', 'HEAD')).Trim() -ne $sourceCommit -or
        (Invoke-Git @('-C', (Join-Path $sourceRoot 'ggml'), 'rev-parse', 'HEAD')).Trim() -ne $ggmlCommit) { throw 'Inference source does not match its pinned commits.' }
    if (Invoke-Git @('-C', $sourceRoot, 'status', '--porcelain')) { throw 'Inference source has local changes; preserve and review them before building.' }
    $locator = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $locator)) { throw 'Visual Studio C++ build tools are required.' }
    $installation = @(& $locator -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -format json | ConvertFrom-Json)[0]
    if (-not $installation) { throw 'An installed MSVC C++ compiler was not found.' }
    $major = [int]($installation.installationVersion.Split('.')[0])
    $year = switch ($major) { 16 {2019} 17 {2022} 18 {2026} default {throw 'Unsupported Visual Studio generator.'} }
    $buildRoot = Join-Path $targetRoot "engine-build\cpu-vs$year"
    $cmake = (Get-Command cmake -ErrorAction Stop).Source
    $helper = Join-Path $targetRoot 'release\voice-studio-native.exe'
    if (-not (Test-Path $helper)) { throw 'Build the native Rust helper before building the engine.' }
    $flags = [ordered]@{
        BUILD_SHARED_LIBS=$false; GGML_BACKEND_DL=$false; GGML_NATIVE=$false; GGML_CPU=$true
        GGML_CUDA=$false; GGML_VULKAN=$false; GGML_METAL=$false; GGML_SYCL=$false
        GGML_BLAS=$false; GGML_OPENMP=$false; GGML_AVX=$false; GGML_AVX2=$false
        GGML_AVX512=$false; GGML_FMA=$false; GGML_F16C=$false; GGML_SSE42=$false
    }
    if (-not $PackageExisting) {
        New-Item -ItemType Directory -Path $buildRoot -Force | Out-Null
        $configure = @('-S',$sourceRoot,'-B',$buildRoot,'-G',"Visual Studio $major $year",'-A','x64','-DCMAKE_POLICY_DEFAULT_CMP0091=NEW','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded','-DCMAKE_CUDA_ARCHITECTURES=75')
        foreach ($entry in $flags.GetEnumerator()) { $configure += "-D$($entry.Key)=$(if ($entry.Value) {'ON'} else {'OFF'})" }
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        & $helper --run-tool-log 768 180 (Join-Path $buildRoot "configure-$stamp.log") $cmake @configure | Out-String | Write-Output
        if ($LASTEXITCODE -ne 0) { throw 'CPU engine configuration failed or was refused by its resource policy.' }
        & $helper --run-tool-log 768 900 (Join-Path $buildRoot "compile-$stamp.log") $cmake --build $buildRoot --config Release --target omnivoice-tts --parallel 1 -- /m:1 /nr:false /p:CL_MPCount=1 | Out-String | Write-Output
        if ($LASTEXITCODE -ne 0) { throw 'CPU engine compilation failed or was stopped by its resource limits.' }
    }
    $cache = Get-Content -LiteralPath (Join-Path $buildRoot 'CMakeCache.txt') -Raw
    foreach ($entry in $flags.GetEnumerator()) {
        $expected = if ($entry.Value) {'ON'} else {'OFF'}
        if ($cache -notmatch "(?m)^$($entry.Key):(?:BOOL|UNINITIALIZED)=$expected\r?$" ) { throw "CPU build flag mismatch: $($entry.Key)" }
    }
    if ($cache -notmatch '(?m)^CMAKE_MSVC_RUNTIME_LIBRARY:.*=MultiThreaded\r?$') { throw 'Static MSVC runtime was not selected.' }
    $binary = Join-Path $buildRoot 'Release\omnivoice-tts.exe'
    $binaryInfo = Get-Item -LiteralPath $binary
    if ($binaryInfo.Length -lt 1024) { throw 'Inference executable is missing or is a placeholder.' }
    $digest = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
    $runtimeRoot = Join-Path $targetRoot "runtime\omnivoice-cpu-$($sourceCommit.Substring(0,12))"
    New-Item -ItemType Directory -Path $runtimeRoot -Force | Out-Null
    $stagedBinary = Join-Path $runtimeRoot 'omnivoice-tts.exe'
    if ((Test-Path $stagedBinary) -and (Get-FileHash -LiteralPath $stagedBinary -Algorithm SHA256).Hash.ToLowerInvariant() -ne $digest) { throw 'An existing runtime differs; preserve it rather than overwrite it.' }
    if (-not (Test-Path $stagedBinary)) { Copy-Item -LiteralPath $binary -Destination $stagedBinary }
    Copy-Item -LiteralPath (Join-Path $sourceRoot 'LICENSE') -Destination (Join-Path $runtimeRoot 'LICENSE-omnivoice.txt')
    Copy-Item -LiteralPath (Join-Path $sourceRoot 'ggml\LICENSE') -Destination (Join-Path $runtimeRoot 'LICENSE-ggml.txt')
    [ordered]@{schema_version=1;engine='omnivoice-gguf-cpu';executable='omnivoice-tts.exe';sha256=$digest;bytes=$binaryInfo.Length;source_commit=$sourceCommit;ggml_commit=$ggmlCommit;build_options=$flags} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $runtimeRoot 'engine.json') -Encoding utf8NoBOM
    & $helper --engine-probe $runtimeRoot | Out-String | Write-Output
    if ($LASTEXITCODE -ne 0) { throw 'Staged inference runtime failed its checksum/CLI preflight.' }
    Write-Output "Verified CPU runtime: $runtimeRoot"
} finally { $env:PATH = $previousPath }
