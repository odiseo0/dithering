$ErrorActionPreference = "Stop"

$workspace = Split-Path -Parent $PSScriptRoot
$metadata = cargo metadata --locked --offline --format-version 1 --no-deps | ConvertFrom-Json
$desktop = $metadata.packages | Where-Object { $_.name -eq "dither-desktop" }
$name = "dithering-$($desktop.version)-windows-x64"
$exe = Join-Path $workspace "target\release\dither-desktop.exe"
$zip = Join-Path $workspace "dist\$name.zip"
$checksumFile = "$zip.sha256"

foreach ($path in @($exe, $zip, $checksumFile)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Falta el archivo requerido: $path"
    }
}

$headers = objdump -f $exe | Out-String
if ($headers -notmatch "pei-x86-64") {
    throw "El ejecutable no es PE para Windows x64"
}
$portableHeaders = objdump -p $exe | Out-String
if ($portableHeaders -notmatch "Subsystem\s+00000002\s+\(Windows GUI\)") {
    throw "El ejecutable no usa el subsistema gráfico de Windows"
}
$forbiddenRuntime = $portableHeaders | Select-String -Pattern "(?i)(vcruntime|msvcp|ucrtbase|libgcc|libstdc\+\+)"
if ($null -ne $forbiddenRuntime) {
    throw "El ejecutable depende de un entorno de C externo: $forbiddenRuntime"
}
$sections = objdump -h $exe | Out-String
if ($sections -notmatch "\.rsrc") {
    throw "El ejecutable no contiene recursos de Windows"
}

$featureTree = cargo tree -p dither-desktop -e features --locked --offline | Out-String
if ($featureTree -match "(?i)(persistence|wgpu)") {
    throw "La aplicación incluye persistencia o un segundo motor gráfico"
}
$sourceFiles = Get-ChildItem -LiteralPath (Join-Path $workspace "crates\dither-desktop\src") -File -Recurse
$persistentCode = $sourceFiles | Select-String -Pattern "eframe::Storage|AppData|Microsoft\.Win32\.Registry"
if ($null -ne $persistentCode) {
    throw "El código contiene acceso a almacenamiento persistente"
}

Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Icon]::ExtractAssociatedIcon($exe)
if ($null -eq $icon) {
    throw "El ejecutable no contiene un icono accesible"
}
$icon.Dispose()

$binary = [System.IO.File]::ReadAllBytes($exe)
$utf8 = [System.Text.Encoding]::UTF8.GetString($binary)
$utf16 = [System.Text.Encoding]::Unicode.GetString($binary)
foreach ($marker in @("requestedExecutionLevel", "PerMonitorV2", "longPathAware")) {
    if (-not $utf8.Contains($marker) -and -not $utf16.Contains($marker)) {
        throw "El manifiesto incrustado no contiene $marker"
    }
}

$actualHash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash
$expectedHash = ((Get-Content -Raw -LiteralPath $checksumFile).Trim() -split "\s+")[0]
if ($actualHash -ne $expectedHash) {
    throw "El SHA-256 del ZIP no coincide"
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($zip)
try {
    $entries = @($archive.Entries | ForEach-Object { $_.FullName.Replace("\", "/") })
    $expectedEntries = @(
        "$name/DISTRIBUTION-README.txt",
        "$name/dithering.exe",
        "$name/LICENSE.txt",
        "$name/THIRD-PARTY-NOTICES.txt"
    )
    if ([string]::Join("`n", $entries) -ne [string]::Join("`n", $expectedEntries)) {
        throw "El contenido del ZIP no coincide con la lista esperada"
    }
}
finally {
    $archive.Dispose()
}

Write-Output "Arquitectura: Windows x64"
Write-Output "Subsistema: Windows GUI"
Write-Output "CRT: estático; no aparecen DLL de Visual C++"
Write-Output "Recursos: icono y manifiesto incrustados"
Write-Output "Persistencia: desactivada"
Write-Output "SHA-256: $actualHash"
