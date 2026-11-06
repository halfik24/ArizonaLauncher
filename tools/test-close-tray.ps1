# Test: window_frame=system, WM_CLOSE must hide launcher to tray
# (process alive, "Tauri Window" not visible), not kill the app.
# Frame in config is temporarily set to "system" and restored after.
# NOTE: checks window by class "Tauri Window" via EnumWindows:
# Process.MainWindowHandle picks the single-instance plugin window
# (com.arizona.launcher-sic) after the main one is hidden - false FAIL.
param(
  [Parameter(Mandatory = $true)][string]$Exe
)
$ErrorActionPreference = 'Continue'
if (-not (Test-Path $Exe)) { Write-Output "RESULT: FAIL - exe not found: $Exe"; exit 1 }
$cfgPath = Join-Path $env:USERPROFILE 'Documents\ArizonaLauncher\config.json'
# Targeted replace, no JSON round-trip: PS 5.1 writes BOM, serde_json rejects it.
$raw = [System.IO.File]::ReadAllText($cfgPath)
$hadFrame = $raw -match '"windowFrame"\s*:\s*"([^"]*)"'
$frameBackup = if ($hadFrame) { $Matches[1] } else { $null }
if ($hadFrame) {
  $raw2 = $raw -replace '"windowFrame"\s*:\s*"[^"]*"', '"windowFrame":"system"'
} else {
  $raw2 = $raw
}
[System.IO.File]::WriteAllText($cfgPath, $raw2, (New-Object System.Text.UTF8Encoding($false)))
Write-Output "frame in config: system (was: $frameBackup)"

Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class TauriWin {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int c);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  // Returns the Tauri main window hwnd of the process, or zero.
  public static IntPtr Find(uint wantPid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint pid; GetWindowThreadProcessId(h, out pid);
      if (pid == wantPid) {
        var cls = new StringBuilder(16); GetClassName(h, cls, 16);
        if (cls.ToString() == "Tauri Window") { found = h; return false; }
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
  public static bool MainVisible(uint wantPid) {
    IntPtr found = Find(wantPid);
    return found != IntPtr.Zero && IsWindowVisible(found);
  }
  public static bool SendWmClose(IntPtr h) {
    return PostMessage(h, 0x0010, IntPtr.Zero, IntPtr.Zero);
  }
}
'@

$proc = Start-Process $Exe -PassThru
if ($null -eq $proc) { Write-Output 'RESULT: FAIL - Start-Process returned null'; exit 1 }
$myPid = [uint32]$proc.Id
$win = [IntPtr]::Zero
for ($i = 0; $i -lt 60; $i++) {
  Start-Sleep -Milliseconds 500
  $win = [TauriWin]::Find($myPid)
  if ($win -ne [IntPtr]::Zero) { break }
}
if ($win -eq [IntPtr]::Zero) {
  Write-Output 'RESULT: FAIL - Tauri window did not appear'
  taskkill /PID $proc.Id /T /F | Out-Null
  exit 1
}
Write-Output "Tauri window found, hwnd=$win"

[TauriWin]::SendWmClose($win) | Out-Null
Start-Sleep -Seconds 2
$proc.Refresh()
$alive = -not $proc.HasExited
$mainVisible = [TauriWin]::MainVisible($myPid)
Write-Output "process alive after WM_CLOSE: $alive"
Write-Output "Tauri window visible:         $mainVisible"
if ($alive -and -not $mainVisible) {
  Write-Output 'RESULT: PASS - window hidden to tray, process alive, tray icon kept'
} else {
  Write-Output 'RESULT: FAIL - app exited (or main window still visible)'
}

taskkill /IM arizona-launcher.exe /T /F 2>&1 | Out-Null
Start-Sleep -Milliseconds 800
Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
  Where-Object { $_.CommandLine -like '*arizona-launcher*' } |
  ForEach-Object { taskkill /PID $_.ProcessId /T /F 2>&1 | Out-Null }

$rawNow = [System.IO.File]::ReadAllText($cfgPath)
if ($null -ne $frameBackup) {
  $rawBack = $rawNow -replace '"windowFrame"\s*:\s*"[^"]*"', ('"windowFrame":"' + $frameBackup + '"')
} else {
  $rawBack = $rawNow -replace '"windowFrame"\s*:\s*"[^"]*",\s*', ''
}
[System.IO.File]::WriteAllText($cfgPath, $rawBack, (New-Object System.Text.UTF8Encoding($false)))
Write-Output "frame in config restored: $frameBackup"
