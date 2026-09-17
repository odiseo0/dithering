$ErrorActionPreference = "Stop"

$workspace = Split-Path -Parent $PSScriptRoot
$metadata = cargo metadata --locked --offline --format-version 1 --no-deps | ConvertFrom-Json
$desktop = $metadata.packages | Where-Object { $_.name -eq "dither-desktop" }
if ($null -eq $desktop) {
    throw "No se encontró el paquete dither-desktop"
}

$name = "dithering-$($desktop.version)-windows-x64"
$dist = Join-Path $workspace "dist"
$stageRoot = Join-Path $dist (".stage-" + [guid]::NewGuid().ToString("N"))
$packageDirectory = Join-Path $stageRoot $name
$zip = Join-Path $dist "$name.zip"
$checksumFile = "$zip.sha256"

cargo build --release --locked --offline -p dither-desktop
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

New-Item -ItemType Directory -Path $packageDirectory -Force | Out-Null
try {
    Copy-Item -LiteralPath (Join-Path $workspace "target\release\dither-desktop.exe") -Destination (Join-Path $packageDirectory "dithering.exe")
    Copy-Item -LiteralPath (Join-Path $workspace "DISTRIBUTION-README.txt") -Destination $packageDirectory
    Copy-Item -LiteralPath (Join-Path $workspace "LICENSE.txt") -Destination $packageDirectory
    Copy-Item -LiteralPath (Join-Path $workspace "THIRD-PARTY-NOTICES.txt") -Destination $packageDirectory
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    Compress-Archive -LiteralPath $packageDirectory -DestinationPath $zip -CompressionLevel Optimal -Force
    $hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
    [System.IO.File]::WriteAllText(
        $checksumFile,
        "$hash  $name.zip`r`n",
        [System.Text.UTF8Encoding]::new($false)
    )
}
finally {
    if (Test-Path -LiteralPath $stageRoot) {
        Remove-Item -LiteralPath $stageRoot -Recurse -Force
    }
}

Write-Output "ZIP: $zip"
Write-Output "SHA-256: $hash"

& (Join-Path $PSScriptRoot "verify-release.ps1")
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
