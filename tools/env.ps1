# Dot-source helper for local tools and Cargo on PATH.
$Root = Split-Path -Parent $PSScriptRoot
$T = $PSScriptRoot
$env:Path = "$T\bin;$T\cmake\bin;$T\ffmpeg\bin;$env:USERPROFILE\.cargo\bin;$env:Path"
Write-Host "[env] LWSMF_ROOT=$Root"
