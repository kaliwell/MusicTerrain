param([switch]$SkipTests)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
Set-Location $repo
$cargo = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
if (-not (Test-Path $cargo)) { throw 'Install Rust stable (MSVC) with rustup first.' }
if (-not $SkipTests) {
    & $cargo test --locked -j 4 -p musicterrain
    if ($LASTEXITCODE -ne 0) { throw 'MusicTerrain tests failed.' }
}
& $cargo build --locked --profile fast -j 4 -p musicterrain
if ($LASTEXITCODE -ne 0) { throw 'MusicTerrain build failed.' }
$destination = Join-Path $repo 'dist\MusicTerrain'
New-Item -ItemType Directory -Force $destination | Out-Null
Copy-Item -LiteralPath (Join-Path $repo 'target\fast\musicterrain.exe') -Destination $destination -Force
Copy-Item -LiteralPath (Join-Path $repo 'LICENSE') -Destination $destination -Force
Write-Host "Deployed to $destination. Run Start-MusicTerrain.cmd from the repository."
