# Daftar aplikasi terdaftar Windows untuk satu ekstensi (dipakai Zandor: "Buka dengan"). Keluaran: JSON array
# [{name, path, icon (PNG base64 atau kosong), isDefault}]. Hanya BACA registri dan metadata exe; tidak menjalankan apa pun.
param([string]$Ext, [switch]$TextLike)
$ErrorActionPreference = 'SilentlyContinue'
Add-Type -AssemblyName System.Drawing
$ext = $Ext.ToLower()
if (-not $ext.StartsWith('.')) { $ext = ".$ext" }

$found = [ordered]@{}   # path(lower) -> @{ path; isDefault }
function Add-App([string]$path, [bool]$isDef) {
  if (-not $path) { return }
  $path = [Environment]::ExpandEnvironmentVariables($path.Trim())
  if ($path -notmatch '\.exe$') { return }
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return }
  $k = $path.ToLower()
  if ($found.Contains($k)) { if ($isDef) { $found[$k].isDefault = $true }; return }
  $found[$k] = @{ path = $path; isDefault = $isDef }
}
function Exe-From-Command([string]$cmd) {
  if (-not $cmd) { return $null }
  if ($cmd -match '^\s*"([^"]+)"') { return $Matches[1] }
  if ($cmd -match '^\s*(\S+\.exe)') { return $Matches[1] }
  return $null
}
function Progid-Exe([string]$progid) {
  if (-not $progid) { return $null }
  $c = (Get-ItemProperty -LiteralPath "Registry::HKEY_CLASSES_ROOT\$progid\shell\open\command").'(default)'
  return Exe-From-Command $c
}
function Name-Exe([string]$name) {
  foreach ($root in 'HKLM', 'HKCU') {
    $p = (Get-ItemProperty -LiteralPath "Registry::${root}\Software\Microsoft\Windows\CurrentVersion\App Paths\$name").'(default)'
    if ($p) { return $p.Trim('"') }
  }
  $c = (Get-ItemProperty -LiteralPath "Registry::HKEY_CLASSES_ROOT\Applications\$name\shell\open\command").'(default)'
  return Exe-From-Command $c
}

$fe = "Registry::HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\$ext"
# 1) aplikasi bawaan (pilihan pengguna, atau ProgID ekstensi)
$choice = (Get-ItemProperty -LiteralPath "$fe\UserChoice").ProgId
Add-App (Progid-Exe $choice) $true
if (-not $found.Count) { Add-App (Progid-Exe ((Get-ItemProperty -LiteralPath "Registry::HKEY_CLASSES_ROOT\$ext").'(default)')) $true }
# 2) daftar "Buka dengan" milik pengguna dan sistem
$list = Get-Item -LiteralPath "$fe\OpenWithList"
if ($list) { foreach ($n in $list.GetValueNames()) { if ($n -ne 'MRUList') { Add-App (Name-Exe ($list.GetValue($n))) $false } } }
foreach ($sub in (Get-ChildItem -LiteralPath "Registry::HKEY_CLASSES_ROOT\$ext\OpenWithList")) { Add-App (Name-Exe $sub.PSChildName) $false }
$progids = @()
$progids += (Get-Item -LiteralPath "$fe\OpenWithProgids").GetValueNames()
$progids += (Get-Item -LiteralPath "Registry::HKEY_CLASSES_ROOT\$ext\OpenWithProgids").GetValueNames()
foreach ($p in ($progids | Where-Object { $_ } | Select-Object -Unique)) { Add-App (Progid-Exe $p) $false }
# 3) editor teks untuk berkas teks/kode
if ($TextLike) {
  foreach ($c in "$env:LOCALAPPDATA\Programs\Microsoft VS Code\Code.exe", "$env:ProgramFiles\Microsoft VS Code\Code.exe", "$env:windir\System32\notepad.exe") { Add-App $c $false }
}

$out = @()
foreach ($a in $found.Values) {
  $name = (Get-Item -LiteralPath $a.path).VersionInfo.FileDescription
  if (-not $name) { $name = [IO.Path]::GetFileNameWithoutExtension($a.path) }
  $icon = ''
  try {
    $ico = [System.Drawing.Icon]::ExtractAssociatedIcon($a.path)
    $ms = New-Object IO.MemoryStream
    $ico.ToBitmap().Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $icon = [Convert]::ToBase64String($ms.ToArray())
  } catch { }
  $out += [pscustomobject]@{ name = $name; path = $a.path; icon = $icon; isDefault = [bool]$a.isDefault }
}
ConvertTo-Json -Compress -InputObject @($out)
