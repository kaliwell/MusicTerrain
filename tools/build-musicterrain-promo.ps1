param([switch]$SkipCapture)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
Set-Location $repo
$ffmpeg=(Get-Command ffmpeg -ErrorAction Stop).Source
if(-not $SkipCapture){
 & node tools/render-musicterrain-promo.cjs
 if($LASTEXITCODE -ne 0){throw 'Promo capture failed'}
}
if(-not(Test-Path run/validation/promo-frames/capture.json)){throw 'Capture manifest is missing'}
$out=Join-Path $repo 'dist/MusicTerrain-Publish'
New-Item -ItemType Directory -Force $out | Out-Null
$frames='run/validation/promo-frames/%04d.png'
& $ffmpeg -hide_banner -loglevel error -y -framerate 15 -i $frames -filter_complex 'fps=7,crop=640:640:160:0,scale=128:128:flags=lanczos,split[a][b];[a]palettegen=max_colors=48:stats_mode=full[p];[b][p]paletteuse=dither=none:diff_mode=rectangle' -loop 0 "$out/preview.gif"
if($LASTEXITCODE -ne 0){throw 'Preview GIF encoding failed'}
if((Get-Item "$out/preview.gif").Length -ge 500000){throw 'Preview exceeds the recommended 500 KB limit'}
& $ffmpeg -hide_banner -loglevel error -y -framerate 15 -i $frames -filter_complex 'fps=12,scale=640:426:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=full[p];[b][p]paletteuse=dither=none:diff_mode=rectangle' -loop 0 "$out/demo.gif"
if($LASTEXITCODE -ne 0){throw 'Demo GIF encoding failed'}
& $ffmpeg -hide_banner -loglevel error -y -framerate 15 -i $frames -c:v libx264 -preset medium -crf 18 -pix_fmt yuv420p -movflags +faststart "$out/demo.mp4"
if($LASTEXITCODE -ne 0){throw 'Video encoding failed'}
Copy-Item run/validation/promo-frames/0000.png "$out/poster.png" -Force
Copy-Item docs/musicterrain/PUBLISHING.md "$out/README.md" -Force
Write-Host "Publishing assets: $out"
