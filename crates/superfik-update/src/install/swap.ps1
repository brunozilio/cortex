param([int]$Id, [string]$Msi, [string]$App)
$ErrorActionPreference = 'SilentlyContinue'

$running = Get-Process -Id $Id
if ($running -and -not $running.WaitForExit(60000)) { exit 1 }

try {
    $arguments = @('/i', ('"{0}"' -f $Msi), '/passive', '/norestart')
    Start-Process -FilePath "$env:SystemRoot\System32\msiexec.exe" -ArgumentList $arguments -Verb RunAs -Wait -ErrorAction Stop
} catch {
}

Remove-Item -LiteralPath (Split-Path -Parent $Msi) -Recurse -Force
Start-Process -FilePath $App
Remove-Item -LiteralPath $PSCommandPath -Force
Remove-Item -LiteralPath (Split-Path -Parent $PSCommandPath) -Force
exit 0
