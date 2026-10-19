param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [ValidateSet('OPENSCL4_CONFIG_DIR', 'OPENSCQ30_CONFIG_DIR')][string]$ConfigVariable = 'OPENSCL4_CONFIG_DIR',
    [ValidateRange(1, 30)][int]$SampleSeconds = 5,
    [string]$Label = 'desktop'
)

$ErrorActionPreference = 'Stop'
$workspaceDirectory = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$executablePath = (Resolve-Path -LiteralPath $Executable).Path
$sampleDirectory = Join-Path $workspaceDirectory ('target/benchmarks/' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $sampleDirectory -Force | Out-Null
$oldConfigValue = [Environment]::GetEnvironmentVariable($ConfigVariable, 'Process')
$sampleProcess = $null
try {
    [Environment]::SetEnvironmentVariable($ConfigVariable, (Join-Path $sampleDirectory 'config'), 'Process')
    $startupWatch = [Diagnostics.Stopwatch]::StartNew()
    $sampleProcess = Start-Process -FilePath $executablePath -WindowStyle Hidden -PassThru
    $inputIdle = $sampleProcess.WaitForInputIdle(10000)
    $startupWatch.Stop()
    if (-not $inputIdle -or $sampleProcess.HasExited) { throw 'Application did not reach input idle' }
    Start-Sleep -Seconds 3
    $sampleProcess.Refresh()
    $cpuBefore = $sampleProcess.TotalProcessorTime.TotalMilliseconds
    $sampleWatch = [Diagnostics.Stopwatch]::StartNew()
    Start-Sleep -Seconds $SampleSeconds
    $sampleProcess.Refresh()
    $sampleWatch.Stop()
    $cpuMilliseconds = $sampleProcess.TotalProcessorTime.TotalMilliseconds - $cpuBefore
    $report = [ordered]@{
        label = $Label
        executable = $executablePath
        sha256 = (Get-FileHash -LiteralPath $executablePath -Algorithm SHA256).Hash.ToLowerInvariant()
        scenario = 'isolated empty configuration; no paired devices'
        startupMarker = 'Win32 WaitForInputIdle; not a first-frame measurement'
        inputIdleMilliseconds = $startupWatch.Elapsed.TotalMilliseconds
        workingSetBytes = $sampleProcess.WorkingSet64
        privateBytes = $sampleProcess.PrivateMemorySize64
        idleSampleMilliseconds = $sampleWatch.Elapsed.TotalMilliseconds
        idleCpuMilliseconds = $cpuMilliseconds
        idleCpuPercentOfOneCore = 100 * $cpuMilliseconds / $sampleWatch.Elapsed.TotalMilliseconds
        stoppedBySampler = $true
        gracefulQuitTest = $false
    }
    $reportPath = Join-Path $sampleDirectory 'report.json'
    $report | ConvertTo-Json | Set-Content -LiteralPath $reportPath -Encoding UTF8
    Write-Output $reportPath
    $report | ConvertTo-Json
}
finally {
    [Environment]::SetEnvironmentVariable($ConfigVariable, $oldConfigValue, 'Process')
    if ($null -ne $sampleProcess -and -not $sampleProcess.HasExited) {
        $sampleProcess.Kill()
        $sampleProcess.WaitForExit(5000) | Out-Null
    }
}
