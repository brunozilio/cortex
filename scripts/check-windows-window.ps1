<# Run on an interactive Windows desktop. Checks actual native window bounds. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Report,
    [switch]$Keep
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public class WindowBounds {
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct MonitorInfo { public int Size; public Rect Monitor, Work; public int Flags; }
    public class Found { public IntPtr Handle; public string Title; public bool Visible; public Rect Bounds; }
    delegate bool EnumProc(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc proc, IntPtr data);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr window, StringBuilder text, int max);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rect);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr window, int flags);
    [DllImport("user32.dll")] public static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    public static List<Found> Of(uint process) {
        var found = new List<Found>();
        EnumWindows((window, data) => {
            uint pid;
            GetWindowThreadProcessId(window, out pid);
            if (pid != process) return true;
            var text = new StringBuilder(256);
            GetWindowText(window, text, text.Capacity);
            Rect bounds;
            GetWindowRect(window, out bounds);
            found.Add(new Found { Handle = window, Title = text.ToString(), Visible = IsWindowVisible(window), Bounds = bounds });
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
'@
$null = [WindowBounds]::SetProcessDPIAware()
$App = Start-Process -FilePath (Resolve-Path $Executable).Path -PassThru
try {
    # The app's own window is the visible one titled Superfik; helper
    # windows (IME, clipboard, event targets) are hidden or untitled.
    $Deadline = [DateTime]::UtcNow.AddSeconds(90)
    $Window = $null
    do {
        Start-Sleep -Milliseconds 250
        if ($App.HasExited) { throw "App exited: $($App.ExitCode)" }
        $Window = [WindowBounds]::Of($App.Id) | Where-Object { $_.Visible -and $_.Title -eq 'Superfik' } | Select-Object -First 1
    } while ($null -eq $Window -and [DateTime]::UtcNow -lt $Deadline)
    if ($null -eq $Window) { throw 'No visible Superfik window' }
    Start-Sleep -Seconds 2
    $Rect = New-Object WindowBounds+Rect
    if (-not [WindowBounds]::GetWindowRect($Window.Handle, [ref]$Rect)) { throw 'GetWindowRect failed' }
    $Info = New-Object WindowBounds+MonitorInfo
    $Info.Size = [Runtime.InteropServices.Marshal]::SizeOf($Info)
    $Monitor = [WindowBounds]::MonitorFromWindow($Window.Handle, 2)
    if (-not [WindowBounds]::GetMonitorInfo($Monitor, [ref]$Info)) { throw 'GetMonitorInfo failed' }
    $Inside = $Rect.Left -ge $Info.Work.Left -and $Rect.Top -ge $Info.Work.Top -and
        $Rect.Right -le $Info.Work.Right -and $Rect.Bottom -le $Info.Work.Bottom
    # A collapsed window would also be "inside"; require a usable size.
    $Usable = ($Rect.Right - $Rect.Left) -ge 400 -and ($Rect.Bottom - $Rect.Top) -ge 300
    $Fits = $Inside -and $Usable
    $All = [WindowBounds]::Of($App.Id) | ForEach-Object { @{ title = $_.Title; visible = $_.Visible; bounds = $_.Bounds } }
    @{ fits = $Fits; window = $Rect; work = $Info.Work; windows = @($All) } |
        ConvertTo-Json -Depth 4 | Set-Content $Report -Encoding UTF8
    if (-not $Fits) { throw 'Initial window is outside the monitor work area or collapsed' }
} finally {
    if (-not $Keep -and -not $App.HasExited) { $null = $App.CloseMainWindow() }
}
