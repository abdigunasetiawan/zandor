# Deteksi shell/terminal yang terpasang di Windows untuk pemilih shell panel Terminal.
# Keluaran: JSON [{id, name, path, args[], icon (PNG base64 atau kosong)}]. Hanya MEMBACA berkas, PATH, dan registri;
# tidak menjalankan shell apa pun.
$ErrorActionPreference = 'SilentlyContinue'
Add-Type -AssemblyName System.Drawing

$out = New-Object System.Collections.ArrayList
$seen = @{}
$icons = @{}

function Get-IconB64([string]$exe) {
  if (-not $exe -or -not (Test-Path -LiteralPath $exe -PathType Leaf)) { return '' }
  $k = $exe.ToLower()
  if ($icons.ContainsKey($k)) { return $icons[$k] }
  $b64 = ''
  try {
    $ico = [System.Drawing.Icon]::ExtractAssociatedIcon($exe)
    $ms = New-Object IO.MemoryStream
    $ico.ToBitmap().Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $b64 = [Convert]::ToBase64String($ms.ToArray())
  } catch { }
  $icons[$k] = $b64
  return $b64
}

# $key membedakan entri dengan exe sama tetapi argumen berbeda (mis. distro WSL)
function Add-Shell([string]$id, [string]$name, [string]$path, [string[]]$shellArgs, [string]$iconFrom, [string]$key) {
  if (-not $path) { return }
  $path = [Environment]::ExpandEnvironmentVariables($path.Trim().Trim('"'))
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { return }
  $path = (Resolve-Path -LiteralPath $path).ProviderPath
  if (-not $key) { $key = $path.ToLower() }
  if ($seen.ContainsKey($key)) { return }
  $seen[$key] = $true
  if (-not $iconFrom) { $iconFrom = $path }
  [void]$out.Add([pscustomobject]@{
      id   = $id
      name = $name
      path = $path
      args = @($shellArgs | Where-Object { $_ })
      icon = (Get-IconB64 $iconFrom)
    })
}

$sys = Join-Path $env:SystemRoot 'System32'

# PowerShell 7+ (instalasi MSI, Store/winget, PATH)
$pwsh = @()
$pwsh += Get-ChildItem -Path (Join-Path $env:ProgramFiles 'PowerShell\*\pwsh.exe') | ForEach-Object FullName
$pwsh += Get-Command pwsh.exe -All -CommandType Application | ForEach-Object Source
$pwsh += Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps\pwsh.exe'
foreach ($p in $pwsh) {
  $label = 'PowerShell'
  if ($p -match 'preview') { $label = 'PowerShell Preview' }
  # satu entri saja per varian (Store + Program Files sering menunjuk produk yang sama)
  Add-Shell 'pwsh' $label $p @('-NoLogo') $null ('pwsh|' + $label)
}

Add-Shell 'powershell' 'Windows PowerShell' (Join-Path $sys 'WindowsPowerShell\v1.0\powershell.exe') @('-NoLogo') $null $null
Add-Shell 'cmd' 'Command Prompt' $env:ComSpec @() $null $null
Add-Shell 'cmd' 'Command Prompt' (Join-Path $sys 'cmd.exe') @() $null $null

# Git Bash: dari registri Git for Windows, folder bawaan, atau letak git.exe di PATH
$gitRoots = @()
foreach ($hive in 'HKLM', 'HKCU') {
  $gitRoots += (Get-ItemProperty -LiteralPath "Registry::$hive\SOFTWARE\GitForWindows").InstallPath
}
$gitRoots += Join-Path $env:ProgramFiles 'Git'
$gitRoots += Join-Path ${env:ProgramFiles(x86)} 'Git'
$gitRoots += Join-Path $env:LOCALAPPDATA 'Programs\Git'
$gitExe = (Get-Command git.exe -CommandType Application | Select-Object -First 1).Source
if ($gitExe) { $gitRoots += (Split-Path (Split-Path $gitExe)) }
foreach ($g in $gitRoots) {
  if (-not $g) { continue }
  Add-Shell 'git-bash' 'Git Bash' (Join-Path $g 'bin\bash.exe') @('--login', '-i') (Join-Path $g 'git-bash.exe') 'git-bash'
}

# WSL: satu entri per distro terpasang (dibaca dari registri, tanpa menjalankan wsl.exe)
$wsl = Join-Path $sys 'wsl.exe'
if (Test-Path -LiteralPath $wsl) {
  $distros = Get-ChildItem -LiteralPath 'Registry::HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Lxss' |
    ForEach-Object { (Get-ItemProperty -LiteralPath $_.PSPath).DistributionName } |
    Where-Object { $_ -and $_ -notmatch '^docker-desktop' }
  foreach ($d in $distros) {
    # tanpa --cd: WSL memetakan folder kerja Windows (folder project) ke /mnt/...
    Add-Shell 'wsl' "WSL: $d" $wsl @('-d', $d) $null ('wsl|' + $d.ToLower())
  }
}

# MSYS2 dan Cygwin di lokasi bawaannya
foreach ($m in 'C:\msys64', 'C:\msys32') {
  Add-Shell 'msys2' 'MSYS2' (Join-Path $m 'usr\bin\bash.exe') @('-i') (Join-Path $m 'msys2.exe') $null
}
foreach ($c in 'C:\cygwin64', 'C:\cygwin') {
  Add-Shell 'cygwin' 'Cygwin' (Join-Path $c 'bin\bash.exe') @('-i') (Join-Path $c 'Cygwin.ico') $null
}

# Shell lain yang ada di PATH
$extra = [ordered]@{ 'nu.exe' = 'Nushell'; 'fish.exe' = 'fish'; 'zsh.exe' = 'zsh'; 'elvish.exe' = 'Elvish'; 'xonsh.exe' = 'xonsh' }
foreach ($e in $extra.Keys) {
  $p = (Get-Command $e -CommandType Application | Select-Object -First 1).Source
  if ($p) { Add-Shell ($e -replace '\.exe$', '') $extra[$e] $p @() $null $null }
}

ConvertTo-Json -Compress -Depth 3 -InputObject @($out)
