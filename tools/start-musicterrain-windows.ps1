param(
    [switch]$Silent,
    [switch]$SystemAudio,
    [ValidateSet('zh-CN', 'en')][string]$Language,
    [string]$Audio,
    [ValidateSet('square', 'cylinder', 'hexagon', 'diamond')][string]$Shape = 'square',
    [ValidateSet(64, 128, 256)][int]$Size = 64,
    [ValidateSet('full', 'adaptive')][string]$Quality = 'full',
    [switch]$Orbit,
    [switch]$Orthographic,
    [string]$Biome = 'auto'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$binary = Join-Path $repo 'dist\MusicTerrain\musicterrain.exe'
if (-not (Test-Path $binary)) { throw 'Run tools\deploy-musicterrain-windows.ps1 first.' }
$session = Join-Path $repo ('run\session-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $session | Out-Null
$env:HORIZON_HOME = Join-Path $session 'horizon'
$env:HORIZON_LAUNCHER_HOME = Join-Path $session 'launcher'
$env:XDG_DATA_HOME = Join-Path $session 'data'
$env:XDG_CONFIG_HOME = Join-Path $session 'config'
$env:HORIZON_LAUNCHER_NO_SELF_UPDATE = '1'
$arguments = @('demo', '--biome', $Biome, '--log-dir', (Join-Path $session 'logs'))
$arguments += @('--shape', $Shape, '--size', "$Size", '--quality', $Quality)
if ($Orbit) { $arguments += '--orbit' }
if ($SystemAudio) { $arguments += '--system-audio' }
if ($Orthographic) { $arguments += '--orthographic' }
if ($Language) { $arguments += @('--language', $Language) }
if ($Silent) {
    $env:HORIZON_NO_AUDIO = '1'
    $arguments += '--silent-preview'
}
if ($Audio) { $arguments += @('--audio', (Resolve-Path -LiteralPath $Audio).Path) }
Set-Location $session
& $binary @arguments
exit $LASTEXITCODE
