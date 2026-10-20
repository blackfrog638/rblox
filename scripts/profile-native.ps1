[CmdletBinding()]
param(
    [ValidateSet('arithmetic', 'globals', 'zoo', 'closures', 'strings', 'inheritance')]
    [string]$Benchmark = 'zoo',
    [ValidateRange(1, 10000000)]
    [int]$Iterations = 3000000,
    [string]$OutputDirectory,
    [switch]$PrepareOnly
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$session = 'rblox-' + [Guid]::NewGuid().ToString('N')
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $repo "target\profiles\$session"
}
$destination = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $destination) {
    throw "Output directory already exists; choose a new directory to preserve previous profiles: $destination"
}

Push-Location $repo
try {
    $target = Join-Path $repo 'target'
    & cargo build -p rblox-vm --bins --profile profiling --no-default-features --target-dir $target
    if ($LASTEXITCODE -ne 0) { throw 'Profiling build failed.' }

    New-Item -ItemType Directory -Path $destination | Out-Null
    $built = Join-Path $target 'profiling'
    # Keep the exact EXE and PDB together so later rebuilds cannot invalidate symbols.
    foreach ($name in @('rblox-vm.exe', 'rblox_vm.pdb')) {
        Copy-Item -LiteralPath (Join-Path $built $name) -Destination $destination
    }
    $workload = Join-Path $destination "$Benchmark.lox"
    & (Join-Path $built 'rblox-bench.exe') --filter $Benchmark --iterations $Iterations --emit $workload
    if ($LASTEXITCODE -ne 0) { throw 'Workload export failed.' }

    @(
        "Benchmark: $Benchmark"
        "Iterations: $Iterations"
        "Date: $([DateTime]::UtcNow.ToString('o'))"
        "Rust: $(& rustc --version)"
        "Git HEAD: $(& git rev-parse HEAD)"
        'Profile: release optimization + debug symbols; no instrumentation features'
        'Working tree:'
        (& git status --short)
    ) | Set-Content -LiteralPath (Join-Path $destination 'metadata.txt') -Encoding UTF8

    Write-Host "Prepared executable, symbols and workload: $destination"
    if ($PrepareOnly) { return }

    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'WPR CPU recording requires an elevated PowerShell. Run this script there, or use -PrepareOnly to build without recording.'
    }
    $wpr = (Get-Command wpr.exe -ErrorAction Stop).Source
    $trace = Join-Path $destination 'cpu.etl'
    $started = $false
    try {
        # A unique WPR instance avoids stopping or cancelling another recording.
        & $wpr -start CPU -filemode -recordtempto $destination -instancename $session
        if ($LASTEXITCODE -ne 0) { throw 'WPR could not start the CPU recording.' }
        $started = $true
        & (Join-Path $destination 'rblox-vm.exe') $workload
        if ($LASTEXITCODE -ne 0) { throw "Workload failed with exit code $LASTEXITCODE." }
    }
    finally {
        if ($started) {
            & $wpr -stop $trace -instancename $session
            if ($LASTEXITCODE -ne 0) {
                & $wpr -cancel -instancename $session
                throw "WPR could not save the trace: $trace"
            }
        }
    }
    Write-Host "CPU trace: $trace"
    Write-Host 'Open it in WPA, load symbols, and filter CPU Usage (Sampled) to rblox-vm.exe.'
}
finally {
    Pop-Location
}
