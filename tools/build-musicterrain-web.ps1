param([switch]$Install,[string]$WallpaperEngine)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
Set-Location $repo
$cargo=Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
& $cargo build --locked --release --target wasm32-unknown-unknown -j 4 -p musicterrain-web
if($LASTEXITCODE -ne 0){throw 'WASM build failed. Install the target with rustup target add wasm32-unknown-unknown.'}
$destination=Join-Path $repo 'dist\MusicTerrain-Web'
New-Item -ItemType Directory -Force $destination | Out-Null
Get-ChildItem (Join-Path $repo 'web\musicterrain') -File | Copy-Item -Destination $destination -Force
Copy-Item (Join-Path $repo 'LICENSE') $destination -Force
$wasm=Join-Path $repo 'target\wasm32-unknown-unknown\release\musicterrain_web.wasm'
$encoded=[Convert]::ToBase64String([IO.File]::ReadAllBytes($wasm))
[IO.File]::WriteAllText((Join-Path $destination 'kernel-base64.js'),('self.MusicTerrainKernel="'+$encoded+'";'),[Text.UTF8Encoding]::new($false))
$manifest=@{generator='MusicTerrain web packager';schema=1;wasmSha256=(Get-FileHash $wasm -Algorithm SHA256).Hash;created=(Get-Date).ToString('o')}
$manifest | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $destination 'musicterrain-build.json')
$archive=Join-Path $repo 'dist\MusicTerrain-Web.zip'
Compress-Archive -Path (Join-Path $destination '*') -DestinationPath $archive -Force
Write-Host "Web wallpaper: $destination"
Write-Host "Import archive: $archive"
if($Install){
 if(-not $WallpaperEngine){
  $steam=(Get-ItemProperty 'HKCU:\Software\Valve\Steam' -ErrorAction SilentlyContinue).SteamPath
  if($steam){$WallpaperEngine=Join-Path $steam 'steamapps\common\wallpaper_engine'}
 }
 if(-not $WallpaperEngine -or -not(Test-Path (Join-Path $WallpaperEngine 'wallpaper64.exe'))){throw 'Specify the installed Wallpaper Engine folder with -WallpaperEngine.'}
 $project=Join-Path $WallpaperEngine 'projects\myprojects\musicterrain-web'
 if(Test-Path $project){$project+='-'+[guid]::NewGuid().ToString('N').Substring(0,8)}
 New-Item -ItemType Directory -Force $project | Out-Null
 Get-ChildItem $destination -File | Copy-Item -Destination $project
 [IO.File]::WriteAllText((Join-Path $repo 'run\validation\web-installed-project.txt'),$project,[Text.UTF8Encoding]::new($false))
 Write-Host "Installed local project: $project"
}
