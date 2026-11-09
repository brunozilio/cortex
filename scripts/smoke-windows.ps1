<# Run from an interactive Windows desktop. Verifies that the GUI stays
   alive and owns a window, then closes it; functional QA is still manual. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [int]$TimeoutSeconds = 30
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$Executable = (Resolve-Path $Executable).Path
$App = Start-Process -FilePath $Executable -WorkingDirectory (Split-Path $Executable) -PassThru
try {
    $Deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
    do {
        Start-Sleep -Milliseconds 200
        $App.Refresh()
        if ($App.HasExited) { throw "Superfik exited before showing a window (code $($App.ExitCode)). Check LocalAppData\Superfik\logs." }
    } while ($App.MainWindowHandle -eq 0 -and [DateTime]::UtcNow -lt $Deadline)
    if ($App.MainWindowHandle -eq 0) { throw "Superfik did not show a desktop window within $TimeoutSeconds seconds." }
    Start-Sleep -Seconds 2
    $App.Refresh()
    if ($App.HasExited) { throw "Superfik exited after creating its window." }
    Write-Output "Superfik opened a window and stayed alive (PID $($App.Id))."
} finally {
    $App.Refresh()
    if (-not $App.HasExited) {
        $null = $App.CloseMainWindow()
        if (-not $App.WaitForExit(10000)) {
            & taskkill /PID $App.Id /T /F | Out-Null
        }
    }
    $App.Dispose()
}
