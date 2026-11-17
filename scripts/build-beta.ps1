# Membuat installer Zandor BETA (NSIS). Dipanggil oleh Buat-Installer-Beta.cmd.
# Langkah: cek alat -> bun install -> unduh+verifikasi opencode (sha256, engine.json) -> tauri build (rilis, NSIS)
# -> salin installer ke rilis-beta\.
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

function Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Need($cmd, $hint) {
  if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) { throw "Perintah '$cmd' tidak ditemukan. $hint" }
}

Step 'Memeriksa alat build'
Need 'bun' 'Pasang Bun: https://bun.sh'
Need 'cargo' 'Pasang Rust: https://rustup.rs'
Need 'npm' 'Pasang Node.js (npm dipakai untuk mengunduh opencode).'

Step 'Memasang dependensi (bun install)'
bun install
if ($LASTEXITCODE -ne 0) { throw 'bun install gagal' }

Step 'Menyiapkan engine opencode (unduh + verifikasi sha256)'
bun scripts/prepare-engine.ts --fetch
if ($LASTEXITCODE -ne 0) { throw 'Menyiapkan opencode gagal' }

Step 'Membangun Zandor rilis + installer NSIS (bisa beberapa menit)'
Push-Location (Join-Path $repo 'apps\desktop')
try {
  bun x tauri build --bundles nsis
  if ($LASTEXITCODE -ne 0) { throw 'tauri build gagal' }
} finally { Pop-Location }

Step 'Menyalin installer'
$nsis = Join-Path $repo 'target\release\bundle\nsis'
$exe = Get-ChildItem $nsis -Filter '*-setup.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $exe) { throw "Installer tidak ditemukan di $nsis" }
$outDir = Join-Path $repo 'rilis-beta'
New-Item -ItemType Directory -Force $outDir | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmm'
$name = $exe.Name -replace '-setup\.exe$', "-BETA-$stamp-setup.exe"
$dest = Join-Path $outDir $name
Copy-Item $exe.FullName $dest -Force
$mb = [math]::Round((Get-Item $dest).Length / 1MB)
Write-Host "`nInstaller siap: $dest ($mb MB)" -ForegroundColor Green
