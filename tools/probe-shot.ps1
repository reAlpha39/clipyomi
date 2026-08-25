# THROWAWAY PROBE (spike, revert me): screenshots the app window so its
# appearance can be looked at rather than inferred.
#
# Written after a long detour: every earlier instrument here reported numbers
# from sampled regions, and numbers kept agreeing that nothing was wrong while a
# native Windows title bar was sitting in plain view above the content. A picture
# settled in one look what four passing checks had missed.
#
# Usage: powershell -File tools/probe-shot.ps1 [-Out path] [-Pad 30] [-Height 160]
#   -Height 0 captures the whole window instead of just the top band.

param(
  [string]$Out = "$env:TEMP\clipyomi-shot.png",
  [int]$Pad = 30,
  [int]$Height = 160
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct RC { public int L, T, R, B; }
public class Shot {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RC r);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
}
"@

# Without this the coordinates come back virtualized — 813x313 for a window that
# is 1240x490 physical — and the capture lands somewhere else entirely.
[Shot]::SetProcessDPIAware() | Out-Null

$proc = Get-Process clipyomi -ErrorAction SilentlyContinue
if (-not $proc) { Write-Error 'ClipYomi is not running'; exit 1 }

$hwnd = $proc.MainWindowHandle
$rect = New-Object RC
if (-not [Shot]::GetWindowRect($hwnd, [ref]$rect)) { Write-Error 'GetWindowRect failed'; exit 1 }

$focused = ([Shot]::GetForegroundWindow() -eq $hwnd)
$w = ($rect.R - $rect.L) + 2 * $Pad
$h = if ($Height -gt 0) { $Height } else { ($rect.B - $rect.T) + 2 * $Pad }

$bmp = New-Object System.Drawing.Bitmap($w, $h)
$gfx = [System.Drawing.Graphics]::FromImage($bmp)
$gfx.CopyFromScreen($rect.L - $Pad, $rect.T - $Pad, 0, 0, (New-Object System.Drawing.Size($w, $h)))
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$gfx.Dispose()
$bmp.Dispose()

"rect x=$($rect.L) y=$($rect.T) w=$($rect.R - $rect.L) h=$($rect.B - $rect.T) focused=$focused"
"saved $Out"
