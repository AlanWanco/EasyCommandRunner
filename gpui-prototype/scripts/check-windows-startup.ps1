param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [Parameter(Mandatory = $true)]
    [ValidateSet('debug', 'release')]
    [string]$Profile
)

$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
$root = Join-Path $env:RUNNER_TEMP "gpui-startup-$Profile"
New-Item -ItemType Directory -Path $root -Force | Out-Null
$stderr = Join-Path $root 'stderr.txt'
$stdout = Join-Path $root 'stdout.txt'
$previousDataDir = [Environment]::GetEnvironmentVariable('ECR_DATA_DIR', 'Process')
$env:ECR_DATA_DIR = Join-Path $root 'fresh-profile'
New-Item -ItemType Directory -Path $env:ECR_DATA_DIR -Force | Out-Null
$process = $null
$passed = $false
try {
    $process = Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) `
        -RedirectStandardError $stderr -RedirectStandardOutput $stdout -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while ([DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 500
        $process.Refresh()
        if ($process.HasExited) {
            throw "$Profile GPUI exited before displaying a window (exit code $($process.ExitCode))"
        }
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) {
            $passed = $true
            Write-Host "$Profile GPUI opened its main window (PID $($process.Id))"
            break
        }
    }
    if (-not $passed) { throw "$Profile GPUI is running but no main window appeared after 20 seconds" }
} finally {
    if ($null -ne $process) {
        $process.Refresh()
        if (-not $process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
            $process.WaitForExit(10000) | Out-Null
        }
    }
    if (-not $passed) {
        foreach ($log in @($stderr, $stdout)) {
            if (Test-Path -LiteralPath $log) {
                Write-Host "--- $log ---"
                Get-Content -LiteralPath $log -ErrorAction Continue
            }
        }
    }
    [Environment]::SetEnvironmentVariable('ECR_DATA_DIR', $previousDataDir, 'Process')
}
