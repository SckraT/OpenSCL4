param(
    [Parameter(Mandatory = $true)][ValidateSet('rect', 'focus', 'click', 'shot', 'wheel', 'key')][string]$Action,
    [string]$ProcessName = 'openscl4*',
    [int]$X = 0,
    [int]$Y = 0,
    [int]$Width = 0,
    [int]$Height = 0,
    [string]$Out,
    [int]$Delta = 0,
    [string]$Key,
    [int]$SettleMilliseconds = 300
)

# Drives a running desktop window from a script: click, scroll, press a key, capture.
#
# Three things make this work where a hand-written snippet usually does not:
#   - the process is made DPI aware before any coordinate is read, so window rectangles come back in physical pixels
#     and match what SetCursorPos expects; without it a 125% display reports every coordinate 1.25 times too small and
#     clicks land beside their target, silently;
#   - the window is focused through AttachThreadInput, because SetForegroundWindow alone is refused while another
#     process owns the foreground;
#   - a capture crops to the window (or to -Width/-Height) instead of the whole screen, which keeps what an agent reads
#     small.
#
# Example:
#   pwsh scripts/ui.ps1 -Action rect
#   pwsh scripts/ui.ps1 -Action click -X 1430 -Y 140
#   pwsh scripts/ui.ps1 -Action shot -Out .local/page.png
#   pwsh scripts/ui.ps1 -Action wheel -Delta -360
#   pwsh scripts/ui.ps1 -Action key -Key PAGE_DOWN

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;

public class UiWindow {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr window, out Rect rectangle);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, IntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte key, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr window);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr window, IntPtr processId);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint attach, uint attachTo, bool doAttach);

    public struct Rect { public int Left; public int Top; public int Right; public int Bottom; }

    public const uint LeftDown = 0x0002;
    public const uint LeftUp = 0x0004;
    public const uint Wheel = 0x0800;
    public const uint KeyUp = 0x0002;
}
'@

# Physical pixels from here on. -4 is PER_MONITOR_AWARE_V2; the older call covers Windows before 10 1703.
if (-not [UiWindow]::SetProcessDpiAwarenessContext([IntPtr](-4))) {
    [UiWindow]::SetProcessDPIAware() | Out-Null
}

function Get-TargetWindow {
    $candidate = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } |
        Select-Object -First 1
    if (-not $candidate) {
        throw "No window found for process '$ProcessName'. Start the application first, and give the process name without the .exe suffix."
    }
    $candidate.Refresh()
    return $candidate
}

function Get-WindowRectangle($window) {
    $rectangle = New-Object UiWindow+Rect
    if (-not [UiWindow]::GetWindowRect($window.MainWindowHandle, [ref]$rectangle)) {
        throw 'GetWindowRect failed for the window handle.'
    }
    return $rectangle
}

function Set-WindowForeground($window) {
    $handle = $window.MainWindowHandle
    [UiWindow]::ShowWindow($handle, 9) | Out-Null
    Start-Sleep -Milliseconds 200
    $foreground = [UiWindow]::GetForegroundWindow()
    $foregroundThread = [UiWindow]::GetWindowThreadProcessId($foreground, [IntPtr]::Zero)
    $currentThread = [UiWindow]::GetCurrentThreadId()
    [UiWindow]::AttachThreadInput($currentThread, $foregroundThread, $true) | Out-Null
    try {
        [UiWindow]::BringWindowToTop($handle) | Out-Null
        [UiWindow]::SetForegroundWindow($handle) | Out-Null
    } finally {
        [UiWindow]::AttachThreadInput($currentThread, $foregroundThread, $false) | Out-Null
    }
    Start-Sleep -Milliseconds $SettleMilliseconds
    return ([UiWindow]::GetForegroundWindow() -eq $handle)
}

function Invoke-Click($window, [int]$offsetX, [int]$offsetY) {
    $rectangle = Get-WindowRectangle $window
    $screenX = $rectangle.Left + $offsetX
    $screenY = $rectangle.Top + $offsetY
    [UiWindow]::SetCursorPos($screenX, $screenY) | Out-Null
    Start-Sleep -Milliseconds 250
    [UiWindow]::mouse_event([UiWindow]::LeftDown, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 120
    [UiWindow]::mouse_event([UiWindow]::LeftUp, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds $SettleMilliseconds
    return "clicked $screenX,$screenY"
}

function Invoke-Wheel($window, [int]$amount) {
    $rectangle = Get-WindowRectangle $window
    [UiWindow]::SetCursorPos(($rectangle.Left + [Math]::Max($X, 1)), ($rectangle.Top + [Math]::Max($Y, 1))) | Out-Null
    Start-Sleep -Milliseconds 200
    # The data field is unsigned; one notch is 120, and a negative delta travels as its two's complement.
    $data = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]$amount), 0)
    [UiWindow]::mouse_event([UiWindow]::Wheel, 0, 0, $data, [IntPtr]::Zero)
    Start-Sleep -Milliseconds $SettleMilliseconds
    return "wheeled $amount"
}

function Invoke-Key($virtualKey) {
    [UiWindow]::keybd_event($virtualKey, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [UiWindow]::keybd_event($virtualKey, 0, [UiWindow]::KeyUp, [IntPtr]::Zero)
    Start-Sleep -Milliseconds $SettleMilliseconds
    return "pressed key $virtualKey"
}

function Invoke-Shot($window, [string]$path) {
    if (-not $path) {
        $path = Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..')).Path ('target/ui-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.png')
    }
    $rectangle = Get-WindowRectangle $window
    $captureWidth = if ($Width -gt 0) { $Width } else { $rectangle.Right - $rectangle.Left }
    $captureHeight = if ($Height -gt 0) { $Height } else { $rectangle.Bottom - $rectangle.Top }
    $bitmap = New-Object System.Drawing.Bitmap $captureWidth, $captureHeight
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen(
            $rectangle.Left, $rectangle.Top, 0, 0,
            (New-Object System.Drawing.Size $captureWidth, $captureHeight)
        )
        $directory = Split-Path -Parent $path
        if ($directory) { New-Item -ItemType Directory -Path $directory -Force | Out-Null }
        $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
    return "saved $path (${captureWidth}x${captureHeight})"
}

$window = Get-TargetWindow
switch ($Action) {
    'rect' {
        $rectangle = Get-WindowRectangle $window
        $width = $rectangle.Right - $rectangle.Left
        $height = $rectangle.Bottom - $rectangle.Top
        "window $($window.ProcessName) at $($rectangle.Left),$($rectangle.Top) ${width}x${height}"
    }
    'focus' {
        $focused = Set-WindowForeground $window
        "focused=$focused"
    }
    'click' {
        Set-WindowForeground $window | Out-Null
        Invoke-Click $window $X $Y
    }
    'shot' {
        Invoke-Shot $window $Out
    }
    'wheel' {
        Set-WindowForeground $window | Out-Null
        Invoke-Wheel $window $Delta
    }
    'key' {
        if (-not $Key) { throw 'Pass -Key with a key name, for example PAGE_DOWN or ENTER.' }
        # PAGE_DOWN reads as PageDown, which is how the Keys enumeration spells it.
        $parts = ($Key -replace '^KEYCODE_', '') -split '_'
        $keyName = ($parts | Where-Object { $_.Length -gt 0 } | ForEach-Object {
                $_.Substring(0, 1).ToUpperInvariant() + $_.Substring(1).ToLowerInvariant()
            }) -join ''
        try {
            $virtualKey = [System.Windows.Forms.Keys]$keyName
        } catch {
            throw "Unknown key '$Key'."
        }
        Set-WindowForeground $window | Out-Null
        Invoke-Key ([byte]$virtualKey)
    }
}
