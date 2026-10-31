param([switch]$Preview,[string]$WallpaperEngine,[string]$Project)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
if(-not $WallpaperEngine){
 $steam=(Get-ItemProperty 'HKCU:\Software\Valve\Steam' -ErrorAction SilentlyContinue).SteamPath
 if($steam){$WallpaperEngine=Join-Path $steam 'steamapps\common\wallpaper_engine'}
}
$engine=if($WallpaperEngine){Join-Path $WallpaperEngine 'wallpaper64.exe'}else{''}
if(-not $engine -or -not(Test-Path -LiteralPath $engine)){throw 'Pass -WallpaperEngine with the installed Wallpaper Engine directory.'}
if(-not $Project){
 $installed=Join-Path $repo 'run\validation\web-installed-project.txt'
 if(Test-Path $installed){$Project=Join-Path (Get-Content $installed -Raw).Trim() 'project.json'}
 if(-not $Project -or -not(Test-Path -LiteralPath $Project)){$Project=Join-Path $repo 'dist\MusicTerrain-Web\project.json'}
}
if(-not(Test-Path -LiteralPath $Project)){throw 'Run tools\build-musicterrain-web.ps1 first.'}
$running=Get-Process wallpaper64 -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq $engine}
if(-not $running){
 Start-Process -FilePath $engine -WorkingDirectory $WallpaperEngine -WindowStyle Hidden
 for($attempt=0;$attempt -lt 20;$attempt++){
  Start-Sleep -Milliseconds 250
  if(Get-Process wallpaper64 -ErrorAction SilentlyContinue){break}
 }
}
$quote=[string][char]34
$arguments=@('-control','openWallpaper','-file',($quote+(Resolve-Path -LiteralPath $Project).Path+$quote))
if($Preview){$arguments+=@('-playInWindow','MusicTerrain-Web-Preview','-width','1280','-height','720')}
Start-Process -FilePath $engine -ArgumentList $arguments -WorkingDirectory $WallpaperEngine -WindowStyle Hidden
Write-Host 'MusicTerrain web wallpaper requested in Wallpaper Engine.'
