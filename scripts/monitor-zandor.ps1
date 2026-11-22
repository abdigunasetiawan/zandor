# Pantau CPU & RAM Zandor GPUI + semua proses anaknya (sidecar opencode, shell, node, dll.) secara realtime.
# Pakai: powershell -ExecutionPolicy Bypass -File scripts\monitor-zandor.ps1
#   -IntervalSec 1      jeda penyegaran (detik)
#   -IncludeBuild       ikut tampilkan cargo/rustc (berguna saat dev-watch membangun ulang)
#   -Name zandor-gpui   nama proses akar (tanpa .exe); bisa lebih dari satu: -Name zandor-gpui,zandor
# Hentikan dengan Ctrl+C.
# CPU% = persen dari total kapasitas semua core (100% = seluruh CPU penuh), sama seperti Task Manager.
param(
  [double]$IntervalSec = 1,
  [switch]$IncludeBuild,
  [string[]]$Name = @('zandor-gpui', 'zandor')
)

$ErrorActionPreference = 'SilentlyContinue'
$cores = [Environment]::ProcessorCount
$prev = @{}   # pid -> TotalProcessorTime (ms) sampel sebelumnya
$prevAt = Get-Date

function Get-Tree {
  $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name
  $byParent = @{}
  foreach ($p in $all) {
    $k = [int]$p.ParentProcessId
    if (-not $byParent.ContainsKey($k)) { $byParent[$k] = New-Object System.Collections.ArrayList }
    [void]$byParent[$k].Add($p)
  }
  $rootNames = @($Name | ForEach-Object { "$_.exe".ToLower() })
  if ($IncludeBuild) { $rootNames += @('cargo.exe', 'rustc.exe') }
  $roots = $all | Where-Object { $rootNames -contains $_.Name.ToLower() }

  $result = New-Object System.Collections.ArrayList
  $seen = @{}
  $stack = New-Object System.Collections.Stack
  foreach ($r in $roots) { $stack.Push(@($r, 0)) }
  while ($stack.Count -gt 0) {
    $item = $stack.Pop(); $p = $item[0]; $depth = $item[1]
    $id = [int]$p.ProcessId
    if ($seen.ContainsKey($id)) { continue }
    $seen[$id] = $true
    [void]$result.Add([pscustomobject]@{ Id = $id; Name = $p.Name; Depth = $depth })
    if ($byParent.ContainsKey($id)) {
      foreach ($c in $byParent[$id]) { $stack.Push(@($c, ($depth + 1))) }
    }
  }
  $result
}

[Console]::CursorVisible = $false
try {
  while ($true) {
    $now = Get-Date
    $elapsedMs = [math]::Max(1, ($now - $prevAt).TotalMilliseconds)
    $tree = Get-Tree
    $rows = @(); $cur = @{}
    $sumCpu = 0.0; $sumWs = 0; $sumPriv = 0

    foreach ($t in $tree) {
      $gp = Get-Process -Id $t.Id
      if (-not $gp) { continue }
      $cpuMs = $gp.TotalProcessorTime.TotalMilliseconds
      $cur[$t.Id] = $cpuMs
      $cpu = if ($prev.ContainsKey($t.Id)) { ($cpuMs - $prev[$t.Id]) / $elapsedMs / $cores * 100 } else { 0 }
      if ($cpu -lt 0) { $cpu = 0 }
      $ws = $gp.WorkingSet64; $priv = $gp.PrivateMemorySize64
      $sumCpu += $cpu; $sumWs += $ws; $sumPriv += $priv
      $rows += [pscustomobject]@{
        PID       = $t.Id
        Proses    = ('. ' * $t.Depth) + $t.Name
        'CPU %'   = '{0,6:N1}' -f $cpu
        'RAM MB'  = '{0,8:N0}' -f ($ws / 1MB)
        'Privat MB' = '{0,8:N0}' -f ($priv / 1MB)
        Thread    = $gp.Threads.Count
      }
    }
    $prev = $cur; $prevAt = $now

    $os = Get-CimInstance Win32_OperatingSystem -Property TotalVisibleMemorySize, FreePhysicalMemory
    $totalMb = $os.TotalVisibleMemorySize / 1KB
    $usedMb = ($os.TotalVisibleMemorySize - $os.FreePhysicalMemory) / 1KB

    $out = New-Object System.Text.StringBuilder
    [void]$out.AppendLine(("Zandor monitor  {0}  (interval {1}s, {2} core)  Ctrl+C untuk berhenti" -f $now.ToString('HH:mm:ss'), $IntervalSec, $cores))
    [void]$out.AppendLine(("Sistem: RAM terpakai {0:N0} / {1:N0} MB" -f $usedMb, $totalMb))
    [void]$out.AppendLine('')
    if ($rows.Count -eq 0) {
      [void]$out.AppendLine("Zandor tidak berjalan (mencari: $($Name -join ', ')). Menunggu...")
    } else {
      [void]$out.Append(($rows | Format-Table -AutoSize | Out-String -Width 200))
      [void]$out.AppendLine(("TOTAL {0} proses   CPU {1:N1}%   RAM {2:N0} MB   Privat {3:N0} MB" -f $rows.Count, $sumCpu, ($sumWs / 1MB), ($sumPriv / 1MB)))
    }
    Clear-Host
    Write-Host $out.ToString()
    Start-Sleep -Milliseconds ([int]($IntervalSec * 1000))
  }
} finally {
  [Console]::CursorVisible = $true
}
