$ErrorActionPreference = "Stop"

$workspace = Split-Path -Parent $PSScriptRoot
$metadata = cargo metadata --locked --offline --format-version 1 --no-deps | ConvertFrom-Json
$desktop = $metadata.packages | Where-Object { $_.name -eq "dither-desktop" }
$name = "dithering-$($desktop.version)-windows-x64"
$zip = Join-Path $workspace "dist\$name.zip"
$smokeRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("dithering-portable-smoke-" + [guid]::NewGuid().ToString("N"))
$process = $null

New-Item -ItemType Directory -Path $smokeRoot | Out-Null
try {
    Expand-Archive -LiteralPath $zip -DestinationPath $smokeRoot
    $packageDirectory = Join-Path $smokeRoot $name
    $exe = Join-Path $packageDirectory "dithering.exe"
    $before = @(Get-ChildItem -LiteralPath $packageDirectory -File -Recurse).Count
    Get-ChildItem -LiteralPath $packageDirectory -File -Recurse | ForEach-Object {
        $_.IsReadOnly = $true
    }

    $process = Start-Process -FilePath $exe -WorkingDirectory $packageDirectory -WindowStyle Hidden -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 100
        $process.Refresh()
    } while (-not $process.HasExited -and $process.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $deadline)

    if ($process.HasExited) {
        throw "El ejecutable portable terminó durante el inicio"
    }
    if ($process.MainWindowHandle -eq 0) {
        throw "El ejecutable portable no creó una ventana"
    }
    $after = @(Get-ChildItem -LiteralPath $packageDirectory -File -Recurse).Count
    if ($after -ne $before) {
        throw "El ejecutable creó archivos junto a sí mismo"
    }

    Write-Output "Ventana creada: sí"
    Write-Output "Archivos creados junto al ejecutable: 0"
}
finally {
    if ($null -ne $process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit()
    }
    if (Test-Path -LiteralPath $smokeRoot) {
        Get-ChildItem -LiteralPath $smokeRoot -File -Recurse | ForEach-Object {
            $_.IsReadOnly = $false
        }
        Remove-Item -LiteralPath $smokeRoot -Recurse -Force
    }
}
