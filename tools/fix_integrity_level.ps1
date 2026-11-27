param(
    [string]$Path = (Split-Path -Parent $PSScriptRoot),
    [switch]$GrantCurrentUser
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $Path)) {
    throw "Path not found: $Path"
}
$resolved = (Resolve-Path -LiteralPath $Path).Path

if ($GrantCurrentUser) {
    $user = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    & icacls.exe $resolved /grant "${user}:(OI)(CI)F" /T | Out-Null
}

for ($attempt = 1; $attempt -le 3; $attempt++) {
    & icacls.exe $resolved /setintegritylevel M /T | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Write-Host "INTEGRITY_OK $resolved"
        exit 0
    }
    Start-Sleep -Milliseconds 500
}

throw "Unable to set Medium integrity level. Run from an elevated PowerShell or pass -GrantCurrentUser."
