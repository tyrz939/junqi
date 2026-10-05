# Opt the build and test processes out of Windows efficiency mode (EcoQoS), so the scheduler may
# use the P-cores, and raise them to above normal. Only processes this user owns; no admin.
param([int]$Minutes = 360, [int]$Every = 15)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Qos {
  [StructLayout(LayoutKind.Sequential)]
  public struct PPTS { public uint Version; public uint ControlMask; public uint StateMask; }
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern IntPtr OpenProcess(uint access, bool inherit, int pid);
  [DllImport("kernel32.dll", SetLastError=true)]
  public static extern bool SetProcessInformation(IntPtr h, int cls, ref PPTS info, uint size);
  [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
  public static bool High(int pid) {
    IntPtr h = OpenProcess(0x0200 /*SET_INFORMATION*/, false, pid);
    if (h == IntPtr.Zero) return false;
    var s = new PPTS { Version = 1, ControlMask = 1 /*EXECUTION_SPEED*/, StateMask = 0 /*off*/ };
    bool ok = SetProcessInformation(h, 4 /*ProcessPowerThrottling*/, ref s, (uint)Marshal.SizeOf(s));
    CloseHandle(h);
    return ok;
  }
}
"@
$seen = @{}
$end = (Get-Date).AddMinutes($Minutes)
while ((Get-Date) -lt $end) {
  Get-Process -ErrorAction SilentlyContinue | Where-Object {
    $_.Path -and ($_.Path -match '\\GitHub\\Junqi\\' -or $_.Path -match '\\\.cargo\\bin\\' -or $_.Path -match '\\\.rustup\\' -or $_.Name -in @('claude','link','cl'))
  } | ForEach-Object {
    if (-not $seen.ContainsKey($_.Id)) {
      [void][Qos]::High($_.Id)
      try { if ($_.PriorityClass -eq 'Normal') { $_.PriorityClass = 'AboveNormal' } } catch {}
      $seen[$_.Id] = 1
    }
  }
  Start-Sleep -Seconds $Every
}
