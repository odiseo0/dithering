$ErrorActionPreference = "Stop"

cargo build --release --locked -p dither-desktop --example hardening_probe
$probe = Join-Path $PSScriptRoot "..\target\release\examples\hardening_probe.exe"

function Invoke-MeasuredProbe {
    param(
        [Parameter(Mandatory = $true)][string]$Mode
    )

    $output = [System.IO.Path]::GetTempFileName()
    try {
        $process = Start-Process -FilePath $probe -ArgumentList $Mode -PassThru -WindowStyle Hidden -RedirectStandardOutput $output
        $processHandle = $process.Handle
        $peakWorkingSet = 0L
        $minimumWorkingSet = [long]::MaxValue
        $minimumThreads = [int]::MaxValue
        $maximumThreads = 0
        $workingSets = [System.Collections.Generic.List[long]]::new()
        $threadCounts = [System.Collections.Generic.List[int]]::new()
        while (-not $process.HasExited) {
            $process.Refresh()
            $workingSets.Add($process.WorkingSet64)
            $threadCounts.Add($process.Threads.Count)
            $peakWorkingSet = [Math]::Max($peakWorkingSet, $process.WorkingSet64)
            $minimumWorkingSet = [Math]::Min($minimumWorkingSet, $process.WorkingSet64)
            $minimumThreads = [Math]::Min($minimumThreads, $process.Threads.Count)
            $maximumThreads = [Math]::Max($maximumThreads, $process.Threads.Count)
            Start-Sleep -Milliseconds 10
        }
        $process.WaitForExit()
        $process.Refresh()
        if ($process.ExitCode -ne 0) {
            throw "La sonda $Mode terminó con código $($process.ExitCode)"
        }
        $quarter = [Math]::Max(1, [Math]::Floor($workingSets.Count / 4))
        $earlyWorkingSet = ($workingSets | Select-Object -Skip $quarter -First $quarter | Measure-Object -Maximum).Maximum
        $lateWorkingSet = ($workingSets | Select-Object -Last $quarter | Measure-Object -Maximum).Maximum
        $earlyThreads = ($threadCounts | Select-Object -Skip $quarter -First $quarter | Measure-Object -Maximum).Maximum
        $lateThreads = ($threadCounts | Select-Object -Last $quarter | Measure-Object -Maximum).Maximum
        [pscustomobject]@{
            Mode = $Mode
            PeakWorkingSetMiB = [Math]::Round($peakWorkingSet / 1MB, 1)
            WorkingSetRangeMiB = [Math]::Round(($peakWorkingSet - $minimumWorkingSet) / 1MB, 1)
            MinimumThreads = $minimumThreads
            MaximumThreads = $maximumThreads
            EarlyThreads = $earlyThreads
            LateThreads = $lateThreads
            LateGrowthMiB = [Math]::Round(($lateWorkingSet - $earlyWorkingSet) / 1MB, 1)
            Output = (Get-Content -Raw $output).Trim()
        }
    }
    finally {
        Remove-Item -LiteralPath $output -Force -ErrorAction SilentlyContinue
    }
}

$memory = Invoke-MeasuredProbe -Mode "memory"
$cancellation = Invoke-MeasuredProbe -Mode "cancellation"
$soak = Invoke-MeasuredProbe -Mode "soak"
$response = [regex]::Match($cancellation.Output, "response_ms=(\d+)")
if (-not $response.Success -or [int]$response.Groups[1].Value -gt 100) {
    throw "La cancelación superó el objetivo de 100 ms: $($cancellation.Output)"
}
if ($soak.LateThreads -gt $soak.EarlyThreads) {
    throw "La cantidad de hilos creció durante la prueba larga"
}
if ($soak.LateGrowthMiB -gt 32) {
    throw "La memoria creció más de 32 MiB durante la prueba larga"
}

Write-Output ($memory | ConvertTo-Json -Compress)
Write-Output ($cancellation | ConvertTo-Json -Compress)
Write-Output ($soak | ConvertTo-Json -Compress)
