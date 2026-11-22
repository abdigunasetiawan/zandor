# Skrip tunggal development: monitor CPU/RAM di jendela samping + dev-watch (rebuild/restart) di jendela ini.
# Pakai lewat dev.cmd di root. Parameter: -Auto (restart otomatis), -NoMonitor, -DebounceMs.
param([switch]$Auto, [switch]$NoMonitor, [int]$DebounceMs = 600)

$ErrorActionPreference = 'Stop'
$monitor = $null
try {
  if (-not $NoMonitor) {
    $monitorScript = Join-Path $PSScriptRoot 'monitor-zandor.ps1'
    $monitor = Start-Process powershell -PassThru -ArgumentList @(
      '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$monitorScript`"", '-IncludeBuild'
    )
  }
  $watchArgs = @{ DebounceMs = $DebounceMs }
  if ($Auto) { $watchArgs['Auto'] = $true }
  & (Join-Path $PSScriptRoot 'dev-watch.ps1') @watchArgs
} finally {
  if ($monitor -and -not $monitor.HasExited) { Stop-Process -Id $monitor.Id -Force -ErrorAction SilentlyContinue }
}
