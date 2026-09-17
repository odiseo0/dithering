$ErrorActionPreference = "Stop"

$metadata = cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc | ConvertFrom-Json
$packages = @{}
foreach ($package in $metadata.packages) {
    $packages[$package.id] = $package
}
$nodes = @{}
foreach ($node in $metadata.resolve.nodes) {
    $nodes[$node.id] = $node
}

$pending = [System.Collections.Generic.Queue[string]]::new()
foreach ($package in $metadata.packages | Where-Object { $null -eq $_.source }) {
    $pending.Enqueue($package.id)
}
$visited = [System.Collections.Generic.HashSet[string]]::new()
while ($pending.Count -gt 0) {
    $id = $pending.Dequeue()
    if (-not $visited.Add($id)) {
        continue
    }
    foreach ($dependency in $nodes[$id].deps) {
        $pending.Enqueue($dependency.pkg)
    }
}

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add("THIRD-PARTY SOFTWARE NOTICES")
$lines.Add("")
$lines.Add("Generated from Cargo.lock for the Windows x64 production dependency graph.")
$lines.Add("Each section contains the license files shipped by that crate.")
$lines.Add("")
$external = $visited | ForEach-Object { $packages[$_] } | Where-Object { $null -ne $_.source } | Sort-Object name, version
foreach ($package in $external) {
    if ([string]::IsNullOrWhiteSpace($package.license)) {
        throw "La dependencia $($package.name) $($package.version) no declara licencia"
    }
    $crateRoot = Split-Path -Parent $package.manifest_path
    $licenseFiles = Get-ChildItem -LiteralPath $crateRoot -File -Recurse | Where-Object {
        ($_.Name -match '(?i)(LICENSE|LICENCE|COPYING|NOTICE|OFL|UFL)') -or
            ($_.Directory.Name -eq "fonts" -and $_.Extension -eq ".txt")
    } | Sort-Object Name

    $lines.Add("================================================================================")
    $lines.Add("$($package.name) $($package.version) - $($package.license)")
    if (-not [string]::IsNullOrWhiteSpace($package.repository)) {
        $lines.Add($package.repository)
    }
    if ($licenseFiles.Count -eq 0) {
        $lines.Add("The crate archive does not contain a separate license file. See the SPDX expression above.")
        $lines.Add("")
    }
    foreach ($licenseFile in $licenseFiles) {
        $lines.Add("--------------------------------------------------------------------------------")
        $lines.Add($licenseFile.Name)
        $lines.Add("")
        $content = (Get-Content -Raw -LiteralPath $licenseFile.FullName).Trim()
        $content = ([regex]::Split($content, "\r?\n") | ForEach-Object { $_.TrimEnd() }) -join "`n"
        $lines.Add($content)
        $lines.Add("")
    }
}

$destination = Join-Path $PSScriptRoot "..\THIRD-PARTY-NOTICES.txt"
[System.IO.File]::WriteAllLines($destination, $lines, [System.Text.UTF8Encoding]::new($false))
Write-Output "Dependencias de producción: $($external.Count)"
Write-Output "Avisos: $destination"
