# Auto rebuild + restart Zandor GPUI setiap file di apps/desktop-gpui/src (atau assets) disimpan.
# Pakai: powershell -ExecutionPolicy Bypass -File scripts\dev-watch.ps1   (atau klik dev-watch.cmd)
#   -Auto   : perilaku lama, aplikasi langsung dimatikan & dijalankan ulang tiap build sukses.
#   bawaan  : mode jeda. Build tetap jalan di latar saat file berubah, tapi aplikasi yang sedang dipakai
#             TIDAK ditutup. Restart baru terjadi setelah Anda menekan Enter di jendela ini.
# Hentikan dengan Ctrl+C. Catatan: state aplikasi hilang tiap restart (tidak ada hot reload sungguhan).
#
# Aplikasi dijalankan dari salinan di target\run\, bukan dari target\debug\, karena Windows mengunci exe
# yang sedang berjalan sehingga cargo tidak bisa menimpanya. Dengan salinan, build tidak perlu menutup aplikasi.
param([int]$DebounceMs = 600, [switch]$Auto)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$app = Join-Path $root 'apps\desktop-gpui'
$built = Join-Path $app 'target\debug\zandor-gpui.exe'
$runDir = Join-Path $app 'target\run'
$exe = Join-Path $runDir 'zandor-gpui.exe'
$procName = 'zandor-gpui'

function Write-Stamp($msg, $color) { Write-Host "[$(Get-Date -Format HH:mm:ss)] $msg" -ForegroundColor $color }

function Stop-App {
  # Hanya matikan Zandor Dev di dalam repo ini; JANGAN pernah menyentuh Zandor produksi yang terpasang.
  $running = @(Get-Process -Name $procName -ErrorAction SilentlyContinue | Where-Object {
    try { $_.Path -like "$root*" } catch { $false }
  })
  if ($running.Count -eq 0) { return }
  foreach ($p in $running) {
    $where = try { $p.Path } catch { '?' }
    Write-Stamp "Zandor Dev masih berjalan (PID $($p.Id), $where). Mematikan..." Yellow
  }
  $running | Stop-Process -Force -ErrorAction SilentlyContinue
  $deadline = (Get-Date).AddSeconds(10)
  while ((Get-Process -Name $procName -ErrorAction SilentlyContinue | Where-Object {
    try { $_.Path -like "$root*" } catch { $false }
  }) -and (Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 200
  }
  if (Get-Process -Name $procName -ErrorAction SilentlyContinue | Where-Object {
    try { $_.Path -like "$root*" } catch { $false }
  }) {
    throw "Zandor Dev tidak bisa dimatikan. Tutup manual lalu jalankan ulang skrip ini."
  }
  Write-Stamp 'Zandor Dev lama sudah mati.' DarkGray
}

function Invoke-Build {
  Write-Stamp 'Membangun (incremental + lld)...' Cyan
  Push-Location $app
  $env:CARGO_INCREMENTAL = '1'
  $sysroot = (rustc --print sysroot 2>$null | Out-String).Trim()
  if ($sysroot) {
    $lld = Join-Path $sysroot 'lib\rustlib\x86_64-pc-windows-msvc\bin\rust-lld.exe'
    if (Test-Path $lld) { $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $lld }
  }
  try {
    cargo build --offline
    if ($LASTEXITCODE -ne 0) {
      Write-Stamp 'Build inkremental gagal; membersihkan cache crate utama lalu mencoba ulang...' Yellow
      cargo clean -p zandor-gpui
      cargo build --offline
    }
  } finally { Pop-Location }
  if ($LASTEXITCODE -ne 0) {
    Write-Host 'Build gagal. Perbaiki lalu simpan lagi.' -ForegroundColor Red
    return $false
  }
  return $true
}

function Start-App {
  Stop-App
  New-Item -ItemType Directory -Force $runDir | Out-Null
  Copy-Item $built $exe -Force
  $pdb = Join-Path $app 'target\debug\zandor_gpui.pdb'
  if (Test-Path $pdb) { Copy-Item $pdb $runDir -Force }
  Write-Stamp 'Menjalankan Zandor.' Green
  Start-Process -FilePath $exe -WorkingDirectory $app | Out-Null
}

function Get-Stamp {
  # Tanda perubahan: jumlah file + waktu tulis terbaru di src/, assets/, dan dev-build.txt.
  $files = @(foreach ($dir in @('src', 'assets')) {
    $path = Join-Path $app $dir
    if (Test-Path $path) { Get-ChildItem -Path $path -Recurse -File -ErrorAction SilentlyContinue }
  })
  $devBuild = Join-Path $app 'dev-build.txt'
  if (Test-Path $devBuild) { $files += Get-Item $devBuild }
  $max = ($files | Measure-Object -Property LastWriteTimeUtc -Maximum).Maximum
  "$($files.Count)|$($max.Ticks)"
}

function Test-EnterPressed {
  # Cek tombol tanpa memblokir loop; abaikan tombol selain Enter.
  try {
    while ([Console]::KeyAvailable) {
      if ([Console]::ReadKey($true).Key -eq 'Enter') { return $true }
    }
  } catch {}
  return $false
}

try {
  Stop-App   # pastikan tidak ada instance lama (mis. cargo run) yang mengunci exe sebelum build pertama
  $stamp = Get-Stamp
  if (Invoke-Build) { Start-App }
  $pending = $false   # build baru siap tapi belum dijalankan (mode jeda)
  if ($Auto) {
    Write-Host 'Memantau perubahan di src/ dan assets/ (mode otomatis) ... (Ctrl+C untuk berhenti)'
  } else {
    Write-Host 'Memantau perubahan di src/ dan assets/ (mode jeda: restart hanya setelah Enter) ... (Ctrl+C untuk berhenti)'
  }
  while ($true) {
    Start-Sleep -Milliseconds 500
    if ($pending -and (Test-EnterPressed)) {
      $pending = $false
      Start-App
    }
    $now = Get-Stamp
    if ($now -ne $stamp) {
      Start-Sleep -Milliseconds $DebounceMs   # tunggu simpanan beruntun selesai
      $stamp = Get-Stamp
      $ok = Invoke-Build
      $stamp = Get-Stamp
      if (-not $ok) { continue }
      $running = [bool](Get-Process -Name $procName -ErrorAction SilentlyContinue | Where-Object {
        try { $_.Path -like "$root*" } catch { $false }
      })
      if ($Auto -or -not $running) {
        $pending = $false
        Start-App
      } else {
        $pending = $true
        while (Test-EnterPressed) {}   # buang Enter yang ditekan selama build
        Write-Stamp 'Build baru siap. Zandor tetap berjalan; tekan Enter untuk restart.' Yellow
      }
    }
  }
} finally {
  Stop-App
}
