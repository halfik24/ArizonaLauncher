# Ждёт, пока у процесса появится ВИДИМОЕ окно, и печатает:
#   "<epochMs старта скрипта>|<мс от старта скрипта>|<заголовок>"
# epochMs нужен, чтобы вызывающий скрипт свёл время окна с временем запуска
# процесса, не полагаясь на момент старта самого PowerShell.
# Использование: powershell -File tools/win-visible.ps1 -ProcessId <pid> [-TimeoutMs 30000]
param(
  [Parameter(Mandatory = $true)][int]$ProcessId,
  [int]$TimeoutMs = 30000
)

# Секундомер — ДО Add-Type: компиляция C#-сниппета занимает сотни миллисекунд
# и сдвигала замер в плюс (окно «появлялось» за 8 мс, хотя это была погрешность).
$epochMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds()
$sw = [System.Diagnostics.Stopwatch]::StartNew()

$sig = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

public static class WinProbe {
  public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

  [DllImport("user32.dll")]
  private static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

  [DllImport("user32.dll")]
  private static extern bool IsWindowVisible(IntPtr hWnd);

  [DllImport("user32.dll")]
  private static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint lpdwProcessId);

  [DllImport("user32.dll", CharSet = CharSet.Unicode)]
  private static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);

  // Первое видимое окно процесса с непустым заголовком.
  public static string FindVisibleWindow(uint pid) {
    string found = null;
    EnumWindows((h, l) => {
      uint p;
      GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h)) {
        var sb = new StringBuilder(512);
        GetWindowText(h, sb, sb.Capacity);
        if (sb.Length > 0) { found = sb.ToString(); return false; }
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
'@

if (-not ("WinProbe" -as [type])) { Add-Type -TypeDefinition $sig }

while ($sw.ElapsedMilliseconds -lt $TimeoutMs) {
  $title = [WinProbe]::FindVisibleWindow([uint32]$ProcessId)
  if ($title) {
    Write-Output "$epochMs|$([math]::Round($sw.ElapsedMilliseconds))|$title"
    exit 0
  }
  Start-Sleep -Milliseconds 15
}
Write-Output "$epochMs|-1|"
